//! RPC-facing operations for the meetings domain.
//!
//! Each `meetings_*` function is a thin async wrapper around [`store`]: it
//! validates input, applies the domain's business rules (e.g. transcript
//! chunks can only be appended while a meeting is `recording`), and wraps
//! the result in [`RpcOutcome`] — mirroring `cron::ops::cron_list` /
//! `cron_update` / etc.

use crate::aura::config::Config;
use crate::aura::inference::local as local_ai;
use crate::aura::meetings::capture;
use crate::aura::meetings::export;
use crate::aura::meetings::ocr;
use crate::aura::meetings::store;
use crate::aura::meetings::store::SearchResult;
use crate::aura::meetings::types::{
    Meeting, MeetingScreenshot, MeetingStatus, TranscriptChunk,
};
use crate::rpc::RpcOutcome;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const LOG_PREFIX: &str = "[meetings-capture]";

/// One slot in [`ACTIVE_CAPTURES`]: either a reservation for a session
/// that's still starting, or a fully started, running session.
///
/// [`meetings_start_local_capture`] inserts `Starting` *before* doing the
/// slow async start work (device/permission setup, which cannot happen
/// while holding this registry's lock — see that function). Reserving the
/// slot up front closes the check-then-insert race where two concurrent
/// calls for the same `meeting_id` could both pass the "already capturing"
/// check and race to insert, silently overwriting (and thus leaking, per
/// [`capture::CaptureHandle`]'s doc comment) the loser's capture threads.
enum CaptureSlot {
    Starting,
    Active(capture::CaptureHandle),
}

/// Process-wide registry of in-flight local capture sessions, keyed by
/// `meeting_id`. This is genuinely new shared mutable state for this domain
/// — every other function here is stateless per-call SQLite I/O — needed so
/// [`meetings_stop_local_capture`] (and [`meetings_finish`] /
/// [`meetings_delete`] force-stopping an active session) can find and tear
/// down the right capture session. Lives at process scope (not per-`Config`)
/// because there's exactly one in-process core per launch (`CoreProcessHandle`,
/// no sidecar) — see `CLAUDE.md`'s "Core is in-process" note.
static ACTIVE_CAPTURES: Lazy<Mutex<HashMap<String, CaptureSlot>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Guards one-time registration of the graceful-shutdown hook that stops
/// every in-flight local capture on process exit — see
/// [`register_shutdown_hook_once`].
static SHUTDOWN_HOOK_REGISTERED: OnceLock<()> = OnceLock::new();

/// Removes and stops any *fully started* in-flight local capture session for
/// `meeting_id`, if one exists. Shared by [`meetings_stop_local_capture`]
/// (explicit stop) and [`meetings_finish`] / [`meetings_delete`] (implicit
/// force-stop) so a capture thread is never left running past its meeting's
/// lifecycle.
///
/// A `Starting` reservation (a concurrent [`meetings_start_local_capture`]
/// call still mid-flight) is left untouched — there's no capture handle to
/// stop yet — and this returns `Ok(false)` for it, same as "nothing active".
fn force_stop_capture(meeting_id: &str) -> Result<bool, String> {
    let mut registry = ACTIVE_CAPTURES
        .lock()
        .map_err(|e| format!("capture registry lock poisoned: {e}"))?;
    match registry.get(meeting_id) {
        Some(CaptureSlot::Starting) => {
            log::warn!(
                "{LOG_PREFIX} force_stop_capture: meeting_id={meeting_id} capture is still \
                 starting, nothing to stop yet"
            );
            Ok(false)
        }
        Some(CaptureSlot::Active(_)) => {
            let handle = match registry.remove(meeting_id) {
                Some(CaptureSlot::Active(h)) => h,
                _ => unreachable!("checked Active above under the same lock guard"),
            };
            drop(registry);
            log::debug!("{LOG_PREFIX} force_stop_capture: stopping active session meeting_id={meeting_id}");
            handle.stop()?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Removes this meeting's `Starting` reservation after a failed start
/// attempt (whisper load failure or backend start failure), so a retried
/// [`meetings_start_local_capture`] call isn't permanently blocked by a
/// stale reservation. Only ever removes a `Starting` entry — an `Active`
/// one belongs to a session that actually started and must be torn down via
/// [`force_stop_capture`] instead.
fn release_capture_reservation(meeting_id: &str) {
    match ACTIVE_CAPTURES.lock() {
        Ok(mut registry) => {
            if matches!(registry.get(meeting_id), Some(CaptureSlot::Starting)) {
                registry.remove(meeting_id);
            }
        }
        Err(e) => {
            log::warn!(
                "{LOG_PREFIX} release_capture_reservation: capture registry lock poisoned \
                 meeting_id={meeting_id}: {e}"
            );
        }
    }
}

/// Registers a `core::shutdown` hook, once per process, that force-stops
/// every in-flight local capture session on graceful shutdown (SIGINT /
/// SIGTERM). Without this, the mic/loopback OS capture threads behind an
/// active [`ACTIVE_CAPTURES`] entry are never signalled to stop during
/// shutdown — only the explicit [`meetings_stop_local_capture`] /
/// [`meetings_finish`] / [`meetings_delete`] paths call
/// [`force_stop_capture`]. Mirrors the idempotent-registration pattern used
/// by `http_host::ops::register_shutdown_hook_once` and
/// `channels::providers::whatsapp_web`. Called from
/// [`meetings_start_local_capture`] so the hook only exists once a capture
/// has actually been attempted.
fn register_shutdown_hook_once() {
    SHUTDOWN_HOOK_REGISTERED.get_or_init(|| {
        crate::core::shutdown::register(|| async {
            stop_all_active_captures().await;
        });
    });
}

/// Drains [`ACTIVE_CAPTURES`] and stops every session found — used by the
/// shutdown hook registered in [`register_shutdown_hook_once`]. Unlike
/// [`force_stop_capture`], this also drops any `Starting` reservation found
/// (there's nothing more to wait for at shutdown).
async fn stop_all_active_captures() {
    let slots: Vec<(String, CaptureSlot)> = match ACTIVE_CAPTURES.lock() {
        Ok(mut registry) => registry.drain().collect(),
        Err(e) => {
            log::warn!(
                "{LOG_PREFIX} shutdown hook: capture registry lock poisoned, cannot stop \
                 in-flight captures: {e}"
            );
            return;
        }
    };
    for (meeting_id, slot) in slots {
        match slot {
            CaptureSlot::Active(handle) => {
                log::debug!(
                    "{LOG_PREFIX} shutdown hook: stopping active local capture meeting_id={meeting_id}"
                );
                if let Err(e) = handle.stop() {
                    log::warn!(
                        "{LOG_PREFIX} shutdown hook: failed to stop capture meeting_id={meeting_id} error={e}"
                    );
                }
            }
            CaptureSlot::Starting => {
                log::warn!(
                    "{LOG_PREFIX} shutdown hook: meeting_id={meeting_id} capture was still \
                     starting at shutdown, nothing to stop"
                );
            }
        }
    }
}

/// Combined payload for `meetings_get`: the meeting row plus its ordered
/// transcript chunks and captured slide screenshots.
#[derive(Debug, Clone, Serialize)]
pub struct MeetingWithChunks {
    pub meeting: Meeting,
    pub chunks: Vec<TranscriptChunk>,
    pub screenshots: Vec<MeetingScreenshot>,
}

pub async fn meetings_create(
    config: &Config,
    title: Option<String>,
    source: Option<String>,
) -> Result<RpcOutcome<Meeting>, String> {
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled meeting".to_string());
    let source = source
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "manual".to_string());

    let meeting = store::create_meeting(config, &title, &source).map_err(|e| e.to_string())?;
    log::debug!(
        "[meetings] meetings_create id={} source={source}",
        meeting.id
    );
    Ok(RpcOutcome::new(
        meeting.clone(),
        vec![format!("meeting created: {}", meeting.id)],
    ))
}

pub async fn meetings_list(config: &Config) -> Result<RpcOutcome<Vec<Meeting>>, String> {
    let meetings = store::list_meetings(config).map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(meetings, "meetings listed"))
}

pub async fn meetings_get(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<MeetingWithChunks>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;
    let screenshots = store::list_screenshots(config, meeting_id).map_err(|e| e.to_string())?;

    Ok(RpcOutcome::new(
        MeetingWithChunks {
            meeting,
            chunks,
            screenshots,
        },
        vec![format!("meeting loaded: {meeting_id}")],
    ))
}

pub async fn meetings_update_title(
    config: &Config,
    meeting_id: &str,
    title: &str,
) -> Result<RpcOutcome<Meeting>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    if title.trim().is_empty() {
        return Err("'title' must not be empty".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting =
        store::update_title(config, meeting_id, title.trim()).map_err(|e| e.to_string())?;
    Ok(RpcOutcome::new(
        meeting,
        vec![format!("meeting title updated: {meeting_id}")],
    ))
}

pub async fn meetings_delete(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    match force_stop_capture(meeting_id) {
        Ok(true) => log::debug!(
            "{LOG_PREFIX} meetings_delete: force-stopped active local capture meeting_id={meeting_id}"
        ),
        Ok(false) => {}
        Err(e) => log::warn!(
            "{LOG_PREFIX} meetings_delete: failed to force-stop capture meeting_id={meeting_id} error={e}"
        ),
    }

    let deleted = store::delete_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    Ok(RpcOutcome::new(
        json!({ "deleted": deleted }),
        vec![format!(
            "meeting delete requested: {meeting_id} (deleted={deleted})"
        )],
    ))
}

pub async fn meetings_append_transcript_chunk(
    config: &Config,
    meeting_id: &str,
    start_ms: i64,
    end_ms: i64,
    text: &str,
    speaker: Option<String>,
) -> Result<RpcOutcome<TranscriptChunk>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    if meeting.status != MeetingStatus::Recording {
        return Err("meeting is not recording".to_string());
    }

    let chunk = store::append_transcript_chunk(
        config,
        meeting_id,
        start_ms,
        end_ms,
        speaker.as_deref(),
        text,
    )
    .map_err(|e| e.to_string())?;

    Ok(RpcOutcome::new(
        chunk,
        vec![format!(
            "transcript chunk appended to meeting: {meeting_id}"
        )],
    ))
}

/// Appends one OCR'd slide screenshot to a recording meeting: decodes the
/// base64 PNG, writes it to
/// `{workspace_dir}/meetings/screenshots/{meeting_id}/{captured_at_ms}.png`,
/// runs OCR over it (downloading the OCR models on first use into
/// `{workspace_dir}/meetings/models/` — the same directory the Tauri
/// shell's whisper model already lives in), and persists the result.
///
/// Mirrors [`meetings_append_transcript_chunk`]'s validation: the meeting
/// must exist and be `recording`.
pub async fn meetings_append_screenshot(
    config: &Config,
    meeting_id: &str,
    captured_at_ms: i64,
    image_base64: &str,
) -> Result<RpcOutcome<MeetingScreenshot>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    if meeting.status != MeetingStatus::Recording {
        return Err("meeting is not recording".to_string());
    }

    let png_bytes = BASE64
        .decode(image_base64.trim())
        .map_err(|e| format!("invalid 'image_base64': {e}"))?;

    log::debug!(
        "[meetings] meetings_append_screenshot: start meeting_id={meeting_id} \
         captured_at_ms={captured_at_ms} png_bytes={}",
        png_bytes.len()
    );

    let screenshots_dir = config
        .workspace_dir
        .join("meetings")
        .join("screenshots")
        .join(meeting_id);
    tokio::fs::create_dir_all(&screenshots_dir)
        .await
        .map_err(|e| format!("failed to create {}: {e}", screenshots_dir.display()))?;

    let image_path = screenshots_dir.join(format!("{captured_at_ms}.png"));
    tokio::fs::write(&image_path, &png_bytes)
        .await
        .map_err(|e| format!("failed to write {}: {e}", image_path.display()))?;

    let models_dir = config.workspace_dir.join("meetings").join("models");
    ocr::ensure_models_downloaded(&models_dir)
        .await
        .map_err(|e| format!("failed to prepare OCR models: {e}"))?;

    // OCR inference is CPU-bound and synchronous — run it off the async
    // runtime thread so this doesn't stall other in-flight RPC calls.
    let ocr_text = tokio::task::spawn_blocking(move || ocr::extract_text(&png_bytes, &models_dir))
        .await
        .map_err(|e| format!("OCR task panicked: {e}"))?
        .map_err(|e| format!("OCR extraction failed: {e}"))?;

    log::debug!(
        "[meetings] meetings_append_screenshot: ocr done meeting_id={meeting_id} \
         captured_at_ms={captured_at_ms} ocr_text_chars={}",
        ocr_text.len()
    );

    let image_path_str = image_path.display().to_string();
    let screenshot = store::insert_screenshot(
        config,
        meeting_id,
        captured_at_ms,
        &image_path_str,
        &ocr_text,
    )
    .map_err(|e| e.to_string())?;

    Ok(RpcOutcome::new(
        screenshot,
        vec![format!("screenshot appended to meeting: {meeting_id}")],
    ))
}

pub async fn meetings_finish(
    config: &Config,
    meeting_id: &str,
    duration_ms: i64,
) -> Result<RpcOutcome<Meeting>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    if meeting.status != MeetingStatus::Recording {
        return Err("meeting is not recording".to_string());
    }

    match force_stop_capture(meeting_id) {
        Ok(true) => log::debug!(
            "{LOG_PREFIX} meetings_finish: force-stopped active local capture meeting_id={meeting_id}"
        ),
        Ok(false) => {}
        Err(e) => log::warn!(
            "{LOG_PREFIX} meetings_finish: failed to force-stop capture meeting_id={meeting_id} error={e}"
        ),
    }

    let finished =
        store::finish_meeting(config, meeting_id, duration_ms).map_err(|e| e.to_string())?;

    Ok(RpcOutcome::new(
        finished,
        vec![format!("meeting finished: {meeting_id}")],
    ))
}

pub async fn meetings_summarize(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    // Surface a clear "not found" error before doing any transcript work.
    store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;

    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;
    if chunks.is_empty() {
        return Err("no transcript to summarize".to_string());
    }

    let transcript = chunks
        .iter()
        .map(|c| {
            format!(
                "[{}]: {}",
                c.speaker.as_deref().unwrap_or("unknown"),
                c.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    log::debug!(
        "[meetings] summarizing meeting_id={meeting_id} chunk_count={} transcript_chars={}",
        chunks.len(),
        transcript.len()
    );

    let summary = summarize_transcript(config, &transcript).await?;

    store::update_summary(config, meeting_id, &summary).map_err(|e| e.to_string())?;

    Ok(RpcOutcome::new(
        json!({ "summary": summary }),
        vec![format!("meeting summarized: {meeting_id}")],
    ))
}

/// Fixed result cap for [`meetings_search`]. Full-text search is meant for a
/// human skimming results, not bulk export, so there is no caller-supplied
/// `limit` param yet — raise this (and expose pagination) if that changes.
const SEARCH_RESULT_LIMIT: i64 = 50;

pub async fn meetings_search(
    config: &Config,
    query: String,
) -> Result<RpcOutcome<Vec<SearchResult>>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("'query' must not be empty".to_string());
    }

    let results =
        store::search_transcripts(config, query, SEARCH_RESULT_LIMIT).map_err(|e| e.to_string())?;
    let count = results.len();
    log::debug!("[meetings] meetings_search query={query:?} result_count={count}");

    Ok(RpcOutcome::new(
        results,
        vec![format!(
            "transcript search: {count} result(s) for {query:?}"
        )],
    ))
}

/// Renders a meeting's transcript (+ summary, if any) to PDF or DOCX and
/// writes it under `<workspace_dir>/meetings/exports/<meeting_id>.<format>`,
/// returning that absolute path. `format` must be `"pdf"` or `"docx"` —
/// anything else is rejected before any rendering work happens.
pub async fn meetings_export(
    config: &Config,
    meeting_id: &str,
    format: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();
    let format = format.trim().to_lowercase();

    log::debug!("[meetings] meetings_export: start meeting_id={meeting_id} format={format:?}");

    if format != "pdf" && format != "docx" {
        return Err(format!(
            "unsupported export format '{format}': expected 'pdf' or 'docx'"
        ));
    }

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;

    let bytes = if format == "pdf" {
        export::render_pdf(&meeting, &chunks).map_err(|e| e.to_string())?
    } else {
        export::render_docx(&meeting, &chunks).map_err(|e| e.to_string())?
    };

    let exports_dir = config.workspace_dir.join("meetings").join("exports");
    tokio::fs::create_dir_all(&exports_dir)
        .await
        .map_err(|e| format!("failed to create {}: {e}", exports_dir.display()))?;

    let output_path = exports_dir.join(format!("{meeting_id}.{format}"));
    tokio::fs::write(&output_path, &bytes)
        .await
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    log::debug!(
        "[meetings] meetings_export: done meeting_id={meeting_id} format={format:?} \
         bytes={} output_path={}",
        bytes.len(),
        output_path.display()
    );

    Ok(RpcOutcome::new(
        json!({ "path": output_path.display().to_string() }),
        vec![format!(
            "meeting exported: {meeting_id} -> {}",
            output_path.display()
        )],
    ))
}

/// Calls the existing LLM chat-completion infrastructure to summarize a
/// meeting transcript into prose.
///
/// Reuses `inference::provider::create_chat_provider` (role
/// `"summarization"`, which resolves to `config.memory_provider` — see
/// `inference::provider::factory::provider_for_role`) together with
/// `Provider::chat_with_system`. This is the exact same provider-resolution
/// + one-shot-completion pattern already used by
/// `tree_summarizer::engine::summarize_to_limit`, so no new HTTP/provider
/// plumbing is introduced here.
async fn summarize_transcript(config: &Config, transcript: &str) -> Result<String, String> {
    let (provider, model) =
        crate::aura::inference::provider::create_chat_provider("summarization", config)
            .map_err(|e| format!("failed to resolve summarization provider: {e}"))?;

    let system_prompt = "You are an assistant that writes concise meeting summaries. \
         Summarize the following meeting transcript for someone who did not attend. \
         Cover the key discussion points, decisions made, and action items with owners \
         when mentioned. Use clear, dense prose or short bullet points; no filler.";

    provider
        .chat_with_system(Some(system_prompt), transcript, &model, 0.3)
        .await
        .map_err(|e| format!("LLM summarization failed: {e}"))
}

/// Starts local live audio capture (microphone + best-effort system-audio
/// loopback, see [`capture::windows`](crate::aura::meetings::capture::windows)
/// on Windows) for a `recording` meeting, streaming transcript chunks in as
/// they're transcribed via [`capture::pipeline::run_capture_session`].
///
/// Mirrors [`meetings_append_transcript_chunk`]'s validation: the meeting
/// must exist and be `recording`. Rejects a second concurrent capture for
/// the same meeting rather than silently replacing it — call
/// [`meetings_stop_local_capture`] first.
pub async fn meetings_start_local_capture(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    register_shutdown_hook_once();

    log::debug!("{LOG_PREFIX} meetings_start_local_capture: start meeting_id={meeting_id}");

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    if meeting.status != MeetingStatus::Recording {
        let msg = "meeting is not recording".to_string();
        log::warn!(
            "{LOG_PREFIX} meetings_start_local_capture: rejected meeting_id={meeting_id} reason={msg}"
        );
        return Err(msg);
    }

    // Reserve this meeting_id's slot *before* the slow async work below
    // (whisper load + backend start) runs, so a second concurrent call for
    // the same meeting_id sees the reservation and is rejected instead of
    // racing to insert its own session — see `CaptureSlot`'s doc comment.
    {
        let mut registry = ACTIVE_CAPTURES
            .lock()
            .map_err(|e| format!("capture registry lock poisoned: {e}"))?;
        if registry.contains_key(meeting_id) {
            let msg = format!("meeting {meeting_id} already has an active local capture session");
            log::warn!("{LOG_PREFIX} meetings_start_local_capture: {msg}");
            return Err(msg);
        }
        registry.insert(meeting_id.to_string(), CaptureSlot::Starting);
    }

    let local_ai_service = local_ai::global(config);
    // Meeting capture calls `whisper_engine::transcribe_pcm_f32` directly
    // from the pipeline's consumer loop, with no CLI-subprocess fallback —
    // unlike dictation, it needs the in-process engine loaded *before* any
    // audio starts flowing, or every chunk for the whole meeting fails
    // transcription silently (see `ensure_whisper_loaded`'s doc comment).
    if let Err(e) = local_ai_service.ensure_whisper_loaded(config).await {
        log::warn!(
            "{LOG_PREFIX} meetings_start_local_capture: whisper engine not ready meeting_id={meeting_id} error={e}"
        );
        release_capture_reservation(meeting_id);
        return Err(format!("failed to prepare local transcription engine: {e}"));
    }
    let whisper = local_ai_service.whisper.clone();
    let backend = capture::default_backend();

    let start_result = capture::pipeline::run_capture_session(
        meeting_id.to_string(),
        config.clone(),
        whisper,
        backend,
    )
    .await;

    let capture_handle = match start_result {
        Ok(h) => h,
        Err(e) => {
            log::warn!(
                "{LOG_PREFIX} meetings_start_local_capture: backend failed meeting_id={meeting_id} error={e}"
            );
            release_capture_reservation(meeting_id);
            return Err(format!("failed to start local capture: {e}"));
        }
    };

    match ACTIVE_CAPTURES.lock() {
        Ok(mut registry) => {
            registry.insert(meeting_id.to_string(), CaptureSlot::Active(capture_handle));
        }
        Err(e) => {
            // Registry lock poisoned — can't safely record this session in
            // the map. Explicitly stop the capture threads we just started
            // instead of letting `capture_handle` drop unstopped, which
            // would leak them (see `capture::CaptureHandle`'s doc comment).
            log::warn!(
                "{LOG_PREFIX} meetings_start_local_capture: capture registry lock poisoned \
                 while finalizing meeting_id={meeting_id}, stopping the session we just started: {e}"
            );
            if let Err(stop_err) = capture_handle.stop() {
                log::warn!(
                    "{LOG_PREFIX} meetings_start_local_capture: failed to stop capture after \
                     poisoned-lock finalize meeting_id={meeting_id} error={stop_err}"
                );
            }
            return Err(format!("capture registry lock poisoned: {e}"));
        }
    }

    log::debug!("{LOG_PREFIX} meetings_start_local_capture: capturing meeting_id={meeting_id}");

    Ok(RpcOutcome::new(
        json!({ "capturing": true }),
        vec![format!("local capture started for meeting: {meeting_id}")],
    ))
}

/// Stops an in-flight local capture session for a meeting, if one is
/// running. Returns `stopped: false` (not an error) when no capture is
/// active for `meeting_id` — calling stop when nothing is capturing is not
/// exceptional (e.g. a UI that always calls stop on unmount).
pub async fn meetings_stop_local_capture(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    log::debug!("{LOG_PREFIX} meetings_stop_local_capture: start meeting_id={meeting_id}");

    // Validate the meeting exists so a typo'd meeting_id surfaces a clear
    // "not found" error instead of a silent stopped=false.
    store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;

    let stopped = force_stop_capture(meeting_id)?;

    log::debug!(
        "{LOG_PREFIX} meetings_stop_local_capture: done meeting_id={meeting_id} stopped={stopped}"
    );

    Ok(RpcOutcome::new(
        json!({ "stopped": stopped }),
        vec![format!(
            "local capture stop requested for meeting: {meeting_id} (stopped={stopped})"
        )],
    ))
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
