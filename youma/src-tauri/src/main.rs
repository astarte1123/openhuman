#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod composio;
mod db;
mod embeddings;
mod encryption;
mod error;
mod file_extract;
mod inbox;
mod knowledge;
mod license;
mod meetings;
mod office;
mod provider;
mod rpc;
mod tools;
mod vector_index;

pub use error::{YoumaError, YoumaResult};

use std::sync::OnceLock;
use tauri::Manager;

static DATA_DIR: OnceLock<String> = OnceLock::new();

pub fn data_dir() -> &'static str {
    DATA_DIR.get().map(|s| s.as_str()).unwrap_or(".")
}

/// Route every frontend RPC call to the in-process dispatcher.
#[tauri::command]
async fn core_invoke(
    method: String,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tracing::debug!("[main] core_invoke method={method}");
    rpc::dispatch(&method, params).await
}

/// Synthesize text via OpenAI TTS (high quality). Returns base64 MP3.
/// Errors with "no_openai_key" when key absent so caller can fall back.
#[tauri::command]
async fn speak_text(text: String) -> Result<String, String> {
    tracing::debug!("[main] speak_text len={}", text.len());
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err("no_openai_key".to_string());
    }
    use base64::Engine as _;
    let mp3 = ai::text_to_speech(&text, &api_key).await?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&mp3))
}

/// Synthesize text via ElevenLabs TTS API. Returns base64 MP3.
/// Errors with "no_elevenlabs_key" when key absent so caller can fall back.
/// Reads ELEVENLABS_VOICE_ID for voice selection (default: Rachel / 21m00Tcm4TlvDq8ikWAM).
#[tauri::command]
async fn speak_elevenlabs(text: String) -> Result<String, String> {
    tracing::debug!("[main] speak_elevenlabs len={}", text.len());
    let api_key = std::env::var("ELEVENLABS_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err("no_elevenlabs_key".to_string());
    }
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string());
    use base64::Engine as _;
    let mp3 = ai::text_to_speech_elevenlabs(&text, &voice_id, &api_key).await?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&mp3))
}

/// Synthesize text via the best available native TTS engine.
/// Windows/macOS: WinRT / AVSpeechSynthesizer via the `tts` crate.
/// Linux: espeak-ng subprocess (install with `sudo apt install espeak-ng`).
#[tauri::command]
async fn speak_native(text: String) -> Result<(), String> {
    tracing::debug!("[main] speak_native len={}", text.len());
    speak_native_impl(text).await
}

#[cfg(not(target_os = "linux"))]
async fn speak_native_impl(text: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut tts = tts::Tts::default().map_err(|e| format!("TTS init: {e}"))?;

        if let Ok(voices) = tts.voices() {
            // Prefer female voices; never pick "guy" (male)
            let best = voices.iter().find(|v| {
                let n = v.name().to_lowercase();
                (n.contains("aria") || n.contains("jenny") || n.contains("zira")
                    || n.contains("elsa") || n.contains("hazel") || n.contains("susan")
                    || n.contains("female") || n.contains("natural") || n.contains("neural"))
                    && !n.contains("guy") && !n.contains("david") && !n.contains("mark")
            }).or_else(|| voices.iter().find(|v| {
                let n = v.name().to_lowercase();
                !n.contains("guy") && !n.contains("david") && !n.contains("mark")
            })).or_else(|| voices.first()).cloned();
            if let Some(v) = best {
                tracing::debug!("[main] speak_native voice={}", v.name());
                let _ = tts.set_voice(&v);
            }
        }

        tts.speak(&text, true).map_err(|e| format!("TTS speak: {e}"))?;

        loop {
            match tts.is_speaking() {
                Ok(true) => std::thread::sleep(std::time::Duration::from_millis(80)),
                _ => break,
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("TTS task: {e}"))?
}

#[cfg(target_os = "linux")]
async fn speak_native_impl(text: String) -> Result<(), String> {
    // Try espeak-ng first (modern, higher quality)
    let status = tokio::process::Command::new("espeak-ng")
        .args(["-s", "145", "-p", "50", "--", &text])
        .status()
        .await;
    match status {
        Ok(s) if s.success() => {
            tracing::debug!("[main] speak_native linux: espeak-ng ok");
            return Ok(());
        }
        Ok(s) => tracing::warn!("[main] speak_native linux: espeak-ng exit={}", s),
        Err(e) => tracing::warn!("[main] speak_native linux: espeak-ng not found: {e}"),
    }

    // Fallback: espeak (legacy)
    let status2 = tokio::process::Command::new("espeak")
        .args(["-s", "145", "--", &text])
        .status()
        .await;
    match status2 {
        Ok(s) if s.success() => {
            tracing::debug!("[main] speak_native linux: espeak ok");
            Ok(())
        }
        _ => Err(
            "No TTS engine found. Install one: sudo apt install espeak-ng".to_string(),
        ),
    }
}

/// Receive base64-encoded audio from the frontend, decode it, and transcribe
/// it via OpenAI Whisper.  Requires OPENAI_API_KEY to be set.
#[tauri::command]
async fn transcribe_audio(audio_b64: String, format: String) -> Result<String, String> {
    tracing::debug!("[main] transcribe_audio format={format} b64_len={}", audio_b64.len());

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err(
            "Whisper requires an OpenAI API key. Please set OPENAI_API_KEY in Settings."
                .to_string(),
        );
    }

    use base64::Engine as _;
    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(&audio_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;

    ai::transcribe_audio_whisper(audio_bytes, &format, &api_key).await
}

/// Persist a key=value pair in the SQLite app_settings table (encrypted)
/// and apply it to the current process environment immediately.
#[tauri::command]
fn set_env_var(key: String, value: String) -> Result<(), String> {
    tracing::debug!("[main] set_env_var key={key} empty={}", value.is_empty());
    db::settings_set_env(&key, &value)?;
    if value.is_empty() {
        std::env::remove_var(&key);
    } else {
        std::env::set_var(&key, &value);
    }
    Ok(())
}

/// Load env settings: try DB first, fall back to youma.env file for one-time migration.
fn load_env_settings(data_dir: &str) {
    // Try DB first (normal path after first run)
    match db::load_env_settings_from_db() {
        Ok(n) if n > 0 => {
            tracing::info!("[main] loaded {n} env settings from DB");
            return;
        }
        Ok(_) => tracing::debug!("[main] no env settings in DB, checking legacy file"),
        Err(e) => tracing::warn!("[main] load_env_settings_from_db error: {e}"),
    }
    // One-time migration from youma.env file
    let env_path = std::path::Path::new(data_dir).join("youma.env");
    if !env_path.exists() {
        tracing::info!("[main] no youma.env file, starting fresh");
        return;
    }
    tracing::info!("[main] migrating youma.env → DB");
    let contents = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(e) => { tracing::error!("[main] cannot read youma.env: {e}"); return; }
    };
    let enc_key = encryption::key();
    let mut migrated = 0usize;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() { continue; }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if k.is_empty() { continue; }
            let plain = if let Some(enc) = v.strip_prefix("ENC:") {
                match encryption::decrypt_str(enc_key, enc) {
                    Ok(p) => p,
                    Err(e) => { tracing::warn!("[main] migration decrypt failed for {k}: {e}"); continue; }
                }
            } else {
                v.to_string()
            };
            if plain.is_empty() { continue; }
            if let Err(e) = db::settings_set_env(k, &plain) {
                tracing::warn!("[main] migration DB write failed for {k}: {e}");
            } else {
                std::env::set_var(k, &plain);
                migrated += 1;
            }
        }
    }
    tracing::info!("[main] migrated {migrated} settings from youma.env → DB");
    // Rename the old file so it's not loaded again
    let old_path = env_path.with_extension("env.migrated");
    if let Err(e) = std::fs::rename(&env_path, &old_path) {
        tracing::warn!("[main] could not rename youma.env: {e}");
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,youma=debug"))
        )
        .with_target(true)
        .compact()
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|e| {
                    tracing::error!("[main] fatal: cannot resolve app data directory: {e}");
                    panic!("cannot resolve app data directory: {e}");
                });

            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("create app data dir: {e}"))?;

            let dir_str = data_dir.to_string_lossy().to_string();
            tracing::info!("[main] data dir: {dir_str}");

            // Init encryption key first — both DB env helpers and migration use it.
            encryption::init(&dir_str);

            // Init DB before loading env settings (migration writes to DB).
            let db_path = data_dir.join("youma.db").to_string_lossy().to_string();
            db::init(&db_path).unwrap_or_else(|e| {
                tracing::error!("[main] fatal: database initialisation failed: {e}");
                panic!("database initialisation failed: {e}");
            });
            tracing::info!("[main] db ready: {db_path}");

            // Load persisted env vars (API keys etc.) from DB; migrate from youma.env on first run.
            load_env_settings(&dir_str);

            DATA_DIR.set(dir_str).ok();

            // Transparent mascot window setup — must run from Rust before JS loads.
            if let Some(main_win) = app.get_webview_window("main") {
                if let Err(e) = main_win.set_ignore_cursor_events(false) {
                    tracing::warn!("[setup] set_ignore_cursor_events: {e}");
                }
                if let Err(e) = main_win.set_always_on_top(true) {
                    tracing::warn!("[setup] set_always_on_top: {e}");
                }
                // Force WebView2 background to fully transparent (alpha=0).
                // Without this, WebView2 on Windows renders a white background
                // even when the window has transparent:true in tauri.conf.json.
                if let Err(e) = main_win.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0))) {
                    tracing::warn!("[setup] set_background_color: {e}");
                }

                const WIN_SIZE: f64 = 32.0;
                const TASKBAR_H: f64 = 48.0;
                const INITIAL_X: f64 = 100.0;
                if let Ok(Some(monitor)) = app.primary_monitor() {
                    let sf = monitor.scale_factor();
                    let height_logical = monitor.size().height as f64 / sf;
                    let avail_logical = height_logical - TASKBAR_H;
                    // Center the 40px window in the 48px taskbar band
                    let x = INITIAL_X;
                    let y = avail_logical + (TASKBAR_H - WIN_SIZE) / 2.0;
                    if let Err(e) = main_win.set_position(tauri::LogicalPosition::new(x, y)) {
                        tracing::warn!("[setup] set_position: {e}");
                    }
                    tracing::debug!("[setup] mascot pos=({x},{y}) avail_h={avail_logical}");
                }
            }

            // Background license validation — runs once at startup + every 6 hours.
            tauri::async_runtime::spawn(async {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                license::validate_remote().await;
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(6 * 60 * 60));
                interval.tick().await; // skip immediate tick
                loop {
                    interval.tick().await;
                    license::validate_remote().await;
                }
            });

            // Background reflection task — runs every 30 minutes while app is open.
            // Reads recent messages, extracts facts, saves as memory notes.
            tauri::async_runtime::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30 * 60));
                interval.tick().await; // skip the immediate first tick
                loop {
                    interval.tick().await;
                    rpc::background_reflection().await;
                }
            });

            // Background Composio auto-fetch — runs every 20 minutes.
            // Iterates all ACTIVE connections, executes the default list action,
            // and saves a truncated summary as a memory note for AI context.
            tauri::async_runtime::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(20 * 60));
                interval.tick().await; // skip the immediate first tick
                loop {
                    interval.tick().await;
                    composio::auto_fetch_all().await;
                }
            });

            // Background inbox fetch — runs every 5 minutes while app is open.
            // Pulls inbound messages from Gmail, Slack, Discord, etc. via Composio.
            tauri::async_runtime::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5 * 60));
                interval.tick().await; // skip immediate first tick
                loop {
                    interval.tick().await;
                    inbox::fetch_all().await;
                }
            });

            // Meeting audio auto-import — polls configured watch folder every 60 s.
            tauri::async_runtime::spawn(async {
                meetings::watcher::run_watcher().await;
            });

            // Background knowledge rebuild — runs every 6 hours.
            // Re-ingests all sources (memories, threads, Composio) and rebuilds
            // the AI-driven 2-level summary tree for the Wiki knowledge base.
            tauri::async_runtime::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(6 * 60 * 60));
                interval.tick().await; // skip the immediate first tick
                loop {
                    interval.tick().await;
                    knowledge::ingest_all().await;
                    knowledge::build_tree().await;
                }
            });

            // On startup: detect embedding model changes, embed pending chunks,
            // then build the in-memory HNSW index for semantic search.
            tauri::async_runtime::spawn(async {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                // Purge stale embeddings if the model changed since last run.
                // Must run before rebuild_from_db so the HNSW is never seeded with
                // wrong-dimension vectors.
                knowledge::check_model_change().await;
                // Build index from stored embeddings (fast — skipped if just purged).
                match vector_index::rebuild_from_db() {
                    Ok(n) => tracing::info!("[main] HNSW index: {n} vectors loaded"),
                    Err(e) => tracing::error!("[main] HNSW index build error: {e}"),
                }
                // Generate embeddings for any chunks that don't have one yet.
                knowledge::embed_all_pending().await;
                // Rebuild once more to include newly generated embeddings.
                match vector_index::rebuild_from_db() {
                    Ok(n) => tracing::info!("[main] HNSW index post-embed: {n} vectors"),
                    Err(e) => tracing::error!("[main] HNSW post-embed error: {e}"),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_invoke, set_env_var, transcribe_audio, speak_text, speak_native, speak_elevenlabs])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("[main] fatal: tauri application error: {e}");
            panic!("tauri application error: {e}");
        });
}
