// meetings/mod.rs — Meeting recording, transcription and summarization module.
//
// Architecture: No binary dependencies (no whisper.cpp, no pyannote-rs).
// Transcription uses the existing `transcribe_audio` Whisper pipeline (OpenAI API).
// Summarization uses the existing `ai::chat_completion` pipeline.
// Storage: one Markdown file per meeting in {data_dir}/meetings/, YAML frontmatter.

pub mod diarize;
pub mod markdown;
pub mod store;
pub mod summarize;
pub mod watcher;

use serde_json::{json, Value};
use store::MeetingMeta;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// RPC handlers — called from rpc.rs dispatch
// ---------------------------------------------------------------------------

/// List all meetings, most recent first.
pub fn handle_list() -> Result<Value, String> {
    eprintln!("[meetings] list");
    let metas = store::list_meetings()?;
    Ok(json!(metas))
}

/// Get a single meeting (metadata + full body).
pub fn handle_get(params: &Value) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] get: missing id".to_string())?;
    eprintln!("[meetings] get id={id}");
    let (meta, body) = store::get_meeting(id)?;
    Ok(json!({ "meta": meta, "body": body }))
}

/// Delete a meeting by id.
pub fn handle_delete(params: &Value) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] delete: missing id".to_string())?;
    eprintln!("[meetings] delete id={id}");
    store::delete_meeting(id)?;
    if let Err(e) = crate::knowledge::delete_meeting_chunks(id) {
        eprintln!("[meetings] KB delete best-effort failed: {e}");
    }
    Ok(json!({ "ok": true }))
}

/// Full-text search across all meeting files.
pub fn handle_search(params: &Value) -> Result<Value, String> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    eprintln!("[meetings] search query={query}");
    let results = store::search_meetings(query)?;
    Ok(json!(results))
}

/// Update title, tags, and/or attendees of an existing meeting.
pub fn handle_update(params: &Value) -> Result<Value, String> {
    let id = params.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] update: missing id".to_string())?;
    let title = params.get("title").and_then(|v| v.as_str());
    let tags: Option<Vec<String>> = params.get("tags").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect());
    let attendees: Option<Vec<String>> = params.get("attendees").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect());
    eprintln!("[meetings] update id={id}");
    let meta = store::update_meeting(id, title, tags, attendees)?;
    Ok(json!(meta))
}

/// Update the full body (content) of a meeting, keeping all metadata intact.
pub fn handle_update_body(params: &Value) -> Result<Value, String> {
    let id = params.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] update_body: missing id".to_string())?;
    let body = params.get("body").and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] update_body: missing body".to_string())?;
    eprintln!("[meetings] update_body id={id} len={}", body.len());
    let meta = store::update_body(id, body)?;
    Ok(json!({ "ok": true, "meta": meta }))
}

/// Export a meeting as raw Markdown content (frontmatter + body).
pub fn handle_export(params: &Value) -> Result<Value, String> {
    let id = params.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings] export: missing id".to_string())?;
    eprintln!("[meetings] export id={id}");
    let (meta, body) = store::get_meeting(id)?;
    let content = markdown::format_meeting(&meta, &body);
    Ok(json!({ "filename": meta.file_name, "content": content }))
}

/// Save a completed meeting: transcript + AI summary.
/// Params: { transcript, title?, attendees?, tags?, duration_secs? }
pub async fn handle_save(params: &Value) -> Result<Value, String> {
    let transcript = params
        .get("transcript")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if transcript.trim().is_empty() {
        return Err("[meetings] save: transcript is empty".to_string());
    }

    let duration_secs: u64 = params
        .get("duration_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let attendees: Vec<String> = params
        .get("attendees")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let tags: Vec<String> = params
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| summarize::extract_title_from_transcript(&transcript));

    let id = Uuid::new_v4().to_string();
    let date = chrono::Utc::now().to_rfc3339();
    let file_name = format!(
        "{}-{}.md",
        &date[..10],
        &id[..8]
    );

    let template = params
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("general")
        .to_string();

    let lang = params
        .get("lang")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    eprintln!("[meetings] save id={id} title={title} template={template} lang={lang}");

    // AI summarization (best-effort — don't fail if AI is unavailable)
    let summary = match summarize::summarize_with_template(&transcript, &template, &lang).await {
        Ok(s) => {
            eprintln!("[meetings] summary generated len={}", s.len());
            s
        }
        Err(e) => {
            eprintln!("[meetings] summary failed: {e}");
            String::new()
        }
    };

    // Language-aware section titles so non-Italian meetings get correct headers.
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

    let meta = MeetingMeta {
        id: id.clone(),
        title,
        date,
        duration_secs,
        attendees,
        tags,
        file_name,
        template,
        lang,
    };

    let dir = store::meetings_dir();
    markdown::write_meeting(&dir, &meta, &body)?;

    // Auto-index into KB (best-effort — don't fail the save if KB is unavailable)
    match crate::knowledge::ingest_meeting(&id, &meta.title, &body, &meta.tags, &meta.date) {
        Ok(n) => eprintln!("[meetings] KB indexed {n} chunks for id={id}"),
        Err(e) => eprintln!("[meetings] KB index best-effort failed: {e}"),
    }

    Ok(json!({ "id": id, "meta": meta }))
}

// ---------------------------------------------------------------------------
// Calendar helpers
// ---------------------------------------------------------------------------

/// Parse the events array from a Composio calendar action response.
/// Handles nested response formats Composio returns across providers.
fn parse_calendar_events(result: &Value) -> Option<Vec<Value>> {
    result
        .pointer("/data/response_data/items")
        .or_else(|| result.pointer("/data/items"))
        .or_else(|| result.pointer("/items"))
        // Outlook: value[] at top-level or data.value[]
        .or_else(|| result.pointer("/data/response_data/value"))
        .or_else(|| result.pointer("/data/value"))
        .or_else(|| result.pointer("/value"))
        .and_then(|v| v.as_array())
        .cloned()
}

/// Extract a Google Meet or Teams video-conference link from a calendar event.
fn extract_meet_link(event: &Value) -> Option<String> {
    // Google Calendar: hangoutLink field
    if let Some(link) = event.get("hangoutLink").and_then(|v| v.as_str()) {
        return Some(link.to_string());
    }
    // Google Calendar: conferenceData.entryPoints[].uri where entryPointType == "video"
    if let Some(link) = event
        .pointer("/conferenceData/entryPoints")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find(|ep| {
                ep.get("entryPointType")
                    .and_then(|t| t.as_str())
                    .map(|t| t == "video")
                    .unwrap_or(false)
            })
        })
        .and_then(|ep| ep.get("uri").and_then(|v| v.as_str()))
    {
        return Some(link.to_string());
    }
    // Outlook: onlineMeeting.joinUrl
    if let Some(link) = event
        .pointer("/onlineMeeting/joinUrl")
        .and_then(|v| v.as_str())
    {
        return Some(link.to_string());
    }
    None
}

/// Extract attendee names from a calendar event (handles Google and Outlook schemas).
fn extract_attendees(event: &Value) -> Vec<String> {
    // Google: attendees[].displayName | .email
    if let Some(arr) = event.pointer("/attendees").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|a| {
                a.get("displayName")
                    .or_else(|| a.get("email"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .collect();
    }
    // Outlook: attendees[].emailAddress.name | .address
    if let Some(arr) = event.pointer("/attendees").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|a| {
                a.pointer("/emailAddress/name")
                    .or_else(|| a.pointer("/emailAddress/address"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .collect();
    }
    vec![]
}

/// Query Google Calendar (with Outlook fallback) for the current/imminent meeting.
/// Returns { title?, attendees?, meet_link? } pre-fill hints.
pub async fn handle_calendar_hint() -> Result<Value, String> {
    eprintln!("[meetings] calendar_hint");

    let client = match crate::composio::load_client() {
        Some(c) => c,
        None => {
            eprintln!("[meetings] calendar_hint: no Composio key");
            return Ok(json!({ "title": null, "attendees": [], "meet_link": null }));
        }
    };

    let now = chrono::Utc::now();
    let window_start = (now - chrono::Duration::minutes(30)).to_rfc3339();
    let window_end   = (now + chrono::Duration::minutes(60)).to_rfc3339();

    // --- A: Google Calendar ---
    let gcal_input = json!({
        "timeMin": window_start,
        "timeMax": window_end,
        "maxResults": 3,
        "singleEvents": true,
        "orderBy": "startTime",
    });

    let gcal_events = match client
        .execute_action("GOOGLECALENDAR_LIST_EVENTS", "default", gcal_input)
        .await
    {
        Ok(r) => {
            let ev = parse_calendar_events(&r);
            if ev.is_none() {
                eprintln!(
                    "[meetings] calendar_hint: Google Calendar — unexpected shape: {}",
                    serde_json::to_string(&r).unwrap_or_default()
                );
            }
            ev
        }
        Err(e) => {
            eprintln!("[meetings] calendar_hint: Google Calendar error: {e}");
            None
        }
    };

    // Use Google result if it has events; otherwise fall back to Outlook.
    let events = if gcal_events.as_ref().map(|v| !v.is_empty()).unwrap_or(false) {
        gcal_events
    } else {
        // --- A fallback: Outlook / Microsoft Calendar ---
        let outlook_input = json!({
            "startDateTime": window_start,
            "endDateTime": window_end,
            "top": 3,
            "orderby": "start/dateTime",
        });
        match client
            .execute_action("MICROSOFT_CALENDAR_LIST_EVENTS", "default", outlook_input)
            .await
        {
            Ok(r) => {
                let ev = parse_calendar_events(&r);
                if ev.is_none() {
                    eprintln!(
                        "[meetings] calendar_hint: Outlook — unexpected shape: {}",
                        serde_json::to_string(&r).unwrap_or_default()
                    );
                }
                ev
            }
            Err(e) => {
                eprintln!("[meetings] calendar_hint: Outlook error: {e}");
                gcal_events // return whatever Google gave (empty or None)
            }
        }
    };

    let Some(events) = events else {
        return Ok(json!({ "title": null, "attendees": [], "meet_link": null }));
    };

    if let Some(event) = events.first() {
        // Google uses "summary"; Outlook uses "subject"
        let title = event
            .get("summary")
            .or_else(|| event.get("subject"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let attendees = extract_attendees(event);
        let meet_link = extract_meet_link(event);

        eprintln!(
            "[meetings] calendar_hint: title={title} attendees={} meet_link={:?}",
            attendees.len(),
            meet_link
        );
        return Ok(json!({ "title": title, "attendees": attendees, "meet_link": meet_link }));
    }

    Ok(json!({ "title": null, "attendees": [], "meet_link": null }))
}

// ---------------------------------------------------------------------------
// Share handler — post meeting summary to connected services (B)
// ---------------------------------------------------------------------------

/// Share a meeting summary to a connected service via Composio.
///
/// Params:
/// - `id`      — meeting ID (required)
/// - `target`  — "slack" | "notion" | "github" | "jira" (required)
/// - `channel` — Slack channel name (slack only, default "general")
/// - `repo`    — "owner/repo" string (github only)
/// - `project` — Jira project key (jira only)
pub async fn handle_share(params: &Value) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "meetings.share: missing id".to_string())?;
    let target = params
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "meetings.share: missing target".to_string())?;

    eprintln!("[meetings] share id={id} target={target}");

    let (meta, body) = store::get_meeting(id)?;
    let client = crate::composio::load_client()
        .ok_or_else(|| "Composio key not configured".to_string())?;

    // Sanitize: strip control characters from title (could come from calendar events).
    let clean_title: String = meta
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect();
    let display_title = if clean_title.is_empty() { "Untitled Meeting".to_string() } else { clean_title };

    // Truncate body defensively; body is AI-generated + transcript (trusted but bounded).
    let body_safe = &body[..body.len().min(8_000)];

    match target {
        "slack" => {
            let channel = params
                .get("channel")
                .and_then(|v| v.as_str())
                .unwrap_or("general");
            let text = format!("*Meeting: {display_title}*\n\n{body_safe}");
            client
                .execute_action(
                    "SLACK_SENDS_A_MESSAGE_TO_A_SLACK_CHANNEL",
                    "default",
                    json!({ "channel": channel, "text": text }),
                )
                .await
                .map_err(|e| format!("Slack: {e}"))?;
        }
        "notion" => {
            client
                .execute_action(
                    "NOTION_CREATE_PAGE",
                    "default",
                    json!({ "title": &display_title, "content": body_safe }),
                )
                .await
                .map_err(|e| format!("Notion: {e}"))?;
        }
        "github" => {
            let repo = params
                .get("repo")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "meetings.share: github requires repo (owner/name)".to_string())?;
            // Validate repo format (owner/repo) — no shell injection possible since
            // this goes as a JSON field, but reject obviously wrong values early.
            let mut parts = repo.splitn(2, '/');
            let owner = parts.next().ok_or("Invalid repo — expected owner/repo")?;
            let repo_name = parts.next().ok_or("Invalid repo — expected owner/repo")?;
            if owner.is_empty() || repo_name.is_empty() {
                return Err("Invalid repo — expected owner/repo".to_string());
            }
            client
                .execute_action(
                    "GITHUB_CREATE_ISSUE",
                    "default",
                    json!({
                        "owner": owner,
                        "repo": repo_name,
                        "title": format!("Meeting: {display_title}"),
                        "body": body_safe,
                    }),
                )
                .await
                .map_err(|e| format!("GitHub: {e}"))?;
        }
        "jira" => {
            let project = params
                .get("project")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "meetings.share: jira requires project key".to_string())?;
            // Project key: alphanumeric + underscore only (Jira convention).
            if !project.chars().all(|c| c.is_alphanumeric() || c == '_') || project.len() > 20 {
                return Err(format!("Invalid Jira project key: '{project}'"));
            }
            client
                .execute_action(
                    "JIRA_CREATE_ISSUE",
                    "default",
                    json!({
                        "projectKey": project,
                        "summary": format!("Meeting: {display_title}"),
                        "description": body_safe,
                        "issueType": "Task",
                    }),
                )
                .await
                .map_err(|e| format!("Jira: {e}"))?;
        }
        other => return Err(format!("meetings.share: unknown target '{other}'")),
    }

    eprintln!("[meetings] share: done target={target}");
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// Diarization handler
// ---------------------------------------------------------------------------

/// Transcribe audio + diarize speakers in one call.
///
/// Params:
/// - `audio_b64`  — base64-encoded audio bytes (required)
/// - `format`     — audio container, e.g. "webm", "wav", "mp4" (default "webm")
/// - `attendees`  — optional list of attendee names for speaker labelling
/// - `lang`       — optional language hint (unused for diarization, forwarded if needed)
///
/// Returns `{ transcript: string, speakers: usize }`.
/// Falls back to plain Whisper text if diarization fails (model download error, ort error, …).
pub async fn handle_transcribe_diarize(params: &Value) -> Result<Value, String> {
    use base64::Engine as _;

    // --- Extract parameters ---
    let audio_b64 = params
        .get("audio_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "[meetings/diarize] missing audio_b64".to_string())?;

    // Guard against absurdly large payloads (base64 of 200 MB audio ≈ 267 MB string).
    // Whisper API itself rejects files > 25 MB, so 35 MB decoded is a safe ceiling.
    const MAX_B64_LEN: usize = 47_000_000; // ~35 MB decoded
    if audio_b64.len() > MAX_B64_LEN {
        return Err(format!(
            "[meetings/diarize] payload too large ({} bytes b64, max {})",
            audio_b64.len(),
            MAX_B64_LEN
        ));
    }

    let format = params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("webm")
        .to_string();

    let attendees: Vec<String> = params
        .get("attendees")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    eprintln!(
        "[meetings/diarize] handle_transcribe_diarize format={format} attendees={}",
        attendees.len()
    );

    // --- Decode base64 → raw audio bytes ---
    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(audio_b64)
        .map_err(|e| format!("[meetings/diarize] base64 decode: {e}"))?;

    eprintln!("[meetings/diarize] audio_bytes={}", audio_bytes.len());

    // --- Require OpenAI key for Whisper ---
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "[meetings/diarize] OPENAI_API_KEY not set".to_string())?;
    if api_key.is_empty() {
        return Err("[meetings/diarize] OPENAI_API_KEY is empty".to_string());
    }

    let data_dir = crate::data_dir();

    // --- Attempt diarization (best-effort; fall back on any error) ---
    let diarize_result: Option<(Vec<diarize::SpeakerSegment>, usize)> = {
        // Model download is async.
        match diarize::ensure_models(&data_dir).await {
            Err(e) => {
                eprintln!("[meetings/diarize] model download failed: {e} — skipping diarization");
                None
            }
            Ok((seg_path, emb_path)) => {
                // Audio decoding + ONNX inference are CPU-bound; run on blocking thread.
                let audio_bytes_clone = audio_bytes.clone();
                let seg_path_owned = seg_path.clone();
                let emb_path_owned = emb_path.clone();

                let blocking_result = tokio::task::spawn_blocking(move || {
                    let (f32_s, i16_s, sr) = diarize::decode_audio(&audio_bytes_clone)?;
                    diarize::diarize(&f32_s, &i16_s, sr, &seg_path_owned, &emb_path_owned)
                })
                .await;

                match blocking_result {
                    Ok(Ok(segs)) => {
                        let n = segs
                            .iter()
                            .map(|s| s.speaker.clone())
                            .collect::<std::collections::HashSet<_>>()
                            .len();
                        eprintln!(
                            "[meetings/diarize] diarization ok: {} segments, {} speakers",
                            segs.len(),
                            n
                        );
                        Some((segs, n))
                    }
                    Ok(Err(e)) => {
                        eprintln!("[meetings/diarize] diarization error: {e} — falling back to plain Whisper");
                        None
                    }
                    Err(e) => {
                        eprintln!("[meetings/diarize] spawn_blocking panicked: {e} — falling back");
                        None
                    }
                }
            }
        }
    };

    // --- Whisper transcription ---
    // Use verbose_json when diarization succeeded (we need timed segments).
    let transcript = if let Some((ref speaker_segs, speaker_count)) = diarize_result {
        match crate::ai::transcribe_audio_whisper_timed(
            audio_bytes.clone(),
            &format,
            &api_key,
        )
        .await
        {
            Ok(timed_segs) => {
                eprintln!(
                    "[meetings/diarize] whisper-timed ok: {} segments",
                    timed_segs.len()
                );
                let merged =
                    diarize::merge_transcript(&timed_segs, speaker_segs, &attendees);
                return Ok(json!({
                    "transcript": merged,
                    "speakers": speaker_count,
                }));
            }
            Err(e) => {
                eprintln!("[meetings/diarize] whisper-timed failed: {e}");
                // Fall through to plain Whisper.
                crate::ai::transcribe_audio_whisper(audio_bytes, &format, &api_key)
                    .await
                    .map(|t| (t, 0usize))
            }
        }
    } else {
        crate::ai::transcribe_audio_whisper(audio_bytes, &format, &api_key)
            .await
            .map(|t| (t, 0usize))
    };

    match transcript {
        Ok((text, speakers)) => {
            eprintln!("[meetings/diarize] fallback transcript len={}", text.len());
            Ok(json!({
                "transcript": text,
                "speakers": speakers,
            }))
        }
        Err(e) => Err(e),
    }
}
