// meetings/watcher.rs — Polling watcher for auto-import of audio voice memos.
//
// Every 60 seconds, checks settings for a configured watch folder.
// Scans for new audio files (mp3, m4a, wav, ogg, webm, flac).
// Transcribes via Whisper, saves as a meeting, moves file to importate/ subfolder.

use std::path::{Path, PathBuf};

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "ogg", "webm", "flac", "aac", "opus"];

/// Background task — call once from main.rs after DB init.
/// Polls every 60 seconds and processes any new audio files in the configured folder.
pub async fn run_watcher() {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    interval.tick().await; // skip immediate first tick
    eprintln!("[meetings/watcher] started");
    loop {
        interval.tick().await;
        if let Err(e) = tick().await {
            eprintln!("[meetings/watcher] tick error: {e}");
        }
    }
}

/// Extract the string value from a `db::settings_get` response.
/// Returns `None` if the key is unset or on error.
fn db_setting(key: &str) -> Option<String> {
    crate::db::settings_get(key)
        .ok()
        .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
}

async fn tick() -> Result<(), String> {
    let folder = match db_setting("meetings.watch_folder") {
        Some(f) if !f.trim().is_empty() => f,
        _ => return Ok(()),
    };

    if db_setting("meetings.auto_import").as_deref() != Some("1") {
        return Ok(());
    }

    let do_summarize = db_setting("meetings.auto_summarize").as_deref() != Some("0");
    let default_template = db_setting("meetings.default_template").unwrap_or_else(|| "general".into());
    let default_lang = db_setting("meetings.default_lang").unwrap_or_else(|| "it-IT".into());

    let dir = Path::new(&folder);
    if !dir.is_dir() {
        eprintln!("[meetings/watcher] folder does not exist: {}", dir.display());
        return Ok(());
    }

    // Refuse to watch filesystem roots and home directories to prevent
    // accidental mass upload of all audio files on the machine to Whisper.
    if is_dangerous_root(dir) {
        eprintln!("[meetings/watcher] refusing to watch dangerous root path: {}", dir.display());
        return Ok(());
    }

    // Ensure importate/ subfolder exists.
    let done_dir = dir.join("importate");
    if let Err(e) = std::fs::create_dir_all(&done_dir) {
        eprintln!("[meetings/watcher] cannot create importate/: {e}");
        return Ok(());
    }

    // Collect audio files in the watch folder (non-recursive, non-importate).
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[meetings/watcher] read_dir {}: {e}", dir.display());
            return Ok(());
        }
    };

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                files.push(path);
            }
        }
    }

    if files.is_empty() { return Ok(()); }

    eprintln!("[meetings/watcher] found {} audio file(s) to import", files.len());

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        eprintln!("[meetings/watcher] no OPENAI_API_KEY — cannot transcribe");
        return Ok(());
    }

    for path in &files {
        if let Err(e) = import_file(path, &done_dir, &api_key, do_summarize, &default_template, &default_lang).await {
            eprintln!("[meetings/watcher] import {}: {e}", path.display());
        }
    }

    Ok(())
}

async fn import_file(
    path: &Path,
    done_dir: &Path,
    api_key: &str,
    do_summarize: bool,
    template: &str,
    lang: &str,
) -> Result<(), String> {
    eprintln!("[meetings/watcher] importing {}", path.display());

    // Read audio bytes.
    let audio_bytes = std::fs::read(path)
        .map_err(|e| format!("read audio: {e}"))?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_lowercase();

    // Transcribe via Whisper.
    let transcript = crate::ai::transcribe_audio_whisper(audio_bytes, &ext, api_key).await
        .map_err(|e| format!("transcribe: {e}"))?;

    if transcript.trim().is_empty() {
        // Leave file in place — empty audio should not silently disappear.
        eprintln!("[meetings/watcher] empty transcript for {} — skipping, file left in folder", path.display());
        return Ok(());
    }

    // Derive title from filename stem.
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Memo vocale")
        .to_string();

    // AI summary (best-effort).
    let summary = if do_summarize {
        match crate::meetings::summarize::summarize_with_template(&transcript, template, lang).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[meetings/watcher] summarize error: {e}");
                String::new()
            }
        }
    } else {
        String::new()
    };

    // Language-aware section headers.
    let (sec_summary, sec_transcript) = match lang.get(..2).unwrap_or("it") {
        "en" => ("## AI Summary",              "## Transcript"),
        "de" => ("## KI-Zusammenfassung",       "## Transkript"),
        "fr" => ("## Résumé IA",                "## Transcription"),
        "es" => ("## Resumen IA",               "## Transcripción"),
        "pt" => ("## Resumo IA",                "## Transcrição"),
        _    => ("## Riassunto AI",             "## Trascrizione"),
    };

    let body = if summary.is_empty() {
        format!("{sec_transcript}\n\n{transcript}")
    } else {
        format!("{sec_summary}\n\n{summary}\n\n---\n\n{sec_transcript}\n\n{transcript}")
    };

    // Build meeting metadata.
    let id = uuid::Uuid::new_v4().to_string();
    let date = chrono::Utc::now().to_rfc3339();
    let file_name = format!("{}-{}.md", &date[..10], &id[..8]);

    let meta = crate::meetings::store::MeetingMeta {
        id: id.clone(),
        title: stem,
        date,
        duration_secs: 0,
        attendees: Vec::new(),
        tags: vec!["auto-import".into()],
        file_name,
        template: template.to_string(),
        lang: lang.to_string(),
    };

    let dir = crate::meetings::store::meetings_dir();
    crate::meetings::markdown::write_meeting(&dir, &meta, &body)?;

    // Index in KB (best-effort).
    match crate::knowledge::ingest_meeting(&id, &meta.title, &body, &meta.tags, &meta.date) {
        Ok(n) => eprintln!("[meetings/watcher] KB indexed {n} chunks for id={id}"),
        Err(e) => eprintln!("[meetings/watcher] KB index failed: {e}"),
    }

    eprintln!("[meetings/watcher] saved meeting id={id} from {}", path.display());

    // Move source file to importate/ to avoid re-processing.
    move_to_done(path, done_dir)?;

    Ok(())
}

/// Returns true if the path is a filesystem root or user home directory.
/// Prevents the watcher from batch-uploading an entire drive to Whisper.
fn is_dangerous_root(path: &Path) -> bool {
    // Check against known dangerous patterns
    let s = path.to_string_lossy();
    let s = s.trim_end_matches(['/', '\\']);

    // Unix/macOS roots and home
    if matches!(s, "" | "/" | "/home" | "/Users" | "/root") {
        return true;
    }
    // User home directory
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let home = home.trim_end_matches(['/', '\\']).to_string();
        if s == home {
            return true;
        }
    }
    // Windows drive roots: C:, C:\, D:\, etc.
    if s.len() == 2 && s.ends_with(':') {
        return true; // bare drive letter
    }
    if s.len() == 3 && s.chars().nth(1) == Some(':') && matches!(s.chars().nth(2), Some('/') | Some('\\')) {
        return true; // C:/ or C:\
    }
    // Windows common root folders
    if matches!(
        s.to_lowercase().as_str(),
        "c:\\windows" | "c:\\users" | "c:\\program files" | "c:\\program files (x86)"
    ) {
        return true;
    }
    false
}

fn move_to_done(path: &Path, done_dir: &Path) -> Result<(), String> {
    let file_name = path.file_name()
        .ok_or("invalid filename")?;
    let dest = done_dir.join(file_name);
    std::fs::rename(path, &dest)
        .map_err(|e| format!("move to importate/: {e}"))?;
    eprintln!("[meetings/watcher] moved {} → {}", path.display(), dest.display());
    Ok(())
}
