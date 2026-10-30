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
use crate::aura::meetings::export_pptx;
use crate::aura::meetings::ocr;
use crate::aura::meetings::store;
use crate::aura::meetings::store::SearchResult;
use crate::aura::meetings::types::{
    Meeting, MeetingScreenshot, MeetingStatus, MeetingSummary, TranscriptChunk,
};
use crate::rpc::RpcOutcome;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const LOG_PREFIX: &str = "[meetings-capture]";
/// Log prefix for structured-summary (Block 4) work specifically, per this
/// domain's `[meetings-*]` grep-prefix convention — distinct from the plain
/// `"[meetings]"` prefix `meetings_summarize` already uses, so decisions /
/// action-item extraction is easy to isolate in logs.
const SUMMARY_LOG_PREFIX: &str = "[meetings-summary]";

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
            log::debug!(
                "{LOG_PREFIX} force_stop_capture: stopping active session meeting_id={meeting_id}"
            );
            handle.stop()?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// How long [`meetings_finish`] waits for an active local capture session's
/// consumer loop to fully drain its buffers and finalize the raw
/// `system_audio` WAV (see [`capture::CaptureHandle::stop_and_wait_for_drain`])
/// before marking the meeting `Completed`. Generous — the final flush
/// includes a synchronous whisper transcription call over whatever was still
/// buffered, which can itself take a few seconds — but bounded, so a stuck
/// consumer task can never hang the RPC caller indefinitely; see
/// [`force_stop_capture_and_wait_for_drain`] for what happens if it's
/// exceeded (the wait is best-effort, never a hard failure).
const CAPTURE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Same as [`force_stop_capture`], but for a capture session that was
/// actually running (`CaptureSlot::Active`), also waits up to
/// [`CAPTURE_DRAIN_TIMEOUT`] for its consumer loop to finish draining
/// buffered audio and finalizing the raw `system_audio` WAV before returning
/// — see [`capture::CaptureHandle::stop_and_wait_for_drain`]'s doc comment.
///
/// [`meetings_finish`] uses this instead of [`force_stop_capture`]
/// specifically because a client's natural next call is
/// `aura.meetings_diarize` against that same WAV file (see that RPC's doc
/// comment): without waiting here, `meetings_diarize` could open the WAV
/// before `RawAudioWriter::finish` had rewritten its header, read zero or
/// truncated samples, and silently report zero speakers for a meeting that
/// actually has a full recording on disk.
async fn force_stop_capture_and_wait_for_drain(meeting_id: &str) -> Result<bool, String> {
    let handle = {
        let mut registry = ACTIVE_CAPTURES
            .lock()
            .map_err(|e| format!("capture registry lock poisoned: {e}"))?;
        match registry.get(meeting_id) {
            Some(CaptureSlot::Starting) => {
                log::warn!(
                    "{LOG_PREFIX} force_stop_capture_and_wait_for_drain: meeting_id={meeting_id} \
                     capture is still starting, nothing to stop yet"
                );
                return Ok(false);
            }
            Some(CaptureSlot::Active(_)) => match registry.remove(meeting_id) {
                Some(CaptureSlot::Active(h)) => h,
                _ => unreachable!("checked Active above under the same lock guard"),
            },
            None => return Ok(false),
        }
    };

    log::debug!(
        "{LOG_PREFIX} force_stop_capture_and_wait_for_drain: stopping and draining active \
         session meeting_id={meeting_id}"
    );
    handle
        .stop_and_wait_for_drain(CAPTURE_DRAIN_TIMEOUT)
        .await?;
    Ok(true)
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
    participants: Option<Vec<String>>,
) -> Result<RpcOutcome<Meeting>, String> {
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled meeting".to_string());
    let source = source
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "manual".to_string());
    let participants: Vec<String> = participants
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();

    let meeting =
        store::create_meeting(config, &title, &source, &participants).map_err(|e| e.to_string())?;
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

    // Best-effort cleanup of the raw `system_audio` WAV, if the `diarize`
    // feature ever captured one for this meeting — `store::delete_meeting`
    // below only removes the DB row (and cascades transcript_chunks /
    // screenshots via SQLite FK); nothing else on disk knows this meeting_id
    // once the row is gone, so this is the only chance to reclaim a
    // potentially multi-hundred-MB uncompressed PCM file. A missing file (the
    // common case — most meetings never captured system_audio, or already
    // had it removed by a prior `meetings_diarize` call) and any other
    // removal failure are both logged and swallowed rather than failing the
    // delete: the DB row is the source of truth for "does this meeting
    // exist", and an orphaned WAV is a leak, not a correctness bug, for a
    // meeting that's about to have its row deleted anyway.
    #[cfg(feature = "diarize")]
    {
        let wav_path = system_audio_wav_path(config, meeting_id);
        match tokio::fs::remove_file(&wav_path).await {
            Ok(()) => log::debug!(
                "{LOG_PREFIX} meetings_delete: removed raw system_audio wav meeting_id={meeting_id} \
                 path={}",
                wav_path.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => log::debug!(
                "{LOG_PREFIX} meetings_delete: no raw system_audio wav to remove meeting_id={meeting_id} \
                 path={}",
                wav_path.display()
            ),
            Err(e) => log::warn!(
                "{LOG_PREFIX} meetings_delete: failed to remove raw system_audio wav \
                 meeting_id={meeting_id} path={} error={e}",
                wav_path.display()
            ),
        }
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
///
/// The PNG is written to disk before the (fallible) OCR + store steps run —
/// if any of those fail, the just-written file is removed (best-effort) so a
/// persistent failure (e.g. no network for a first-time OCR model download)
/// doesn't leak one orphaned full-resolution screenshot per capture tick for
/// the rest of the meeting.
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

    // Everything from here on (OCR model download, OCR itself, the store
    // insert) can fail, and none of it should leave the PNG just written
    // above behind as an orphan — a persistent failure (e.g. no network for
    // a first-time OCR model download) would otherwise leak one full-res
    // screenshot per capture tick for the rest of the meeting. `result`
    // captures the outcome so the cleanup-on-failure block below can run
    // unconditionally before this function returns, whichever way it went.
    let result =
        append_screenshot_after_write(config, meeting_id, captured_at_ms, &image_path, png_bytes)
            .await;
    if let Err(ref e) = result {
        match tokio::fs::remove_file(&image_path).await {
            Ok(()) => log::debug!(
                "[meetings] meetings_append_screenshot: cleaned up orphaned screenshot \
                 {} after failure: {e}",
                image_path.display()
            ),
            Err(remove_err) => log::warn!(
                "[meetings] meetings_append_screenshot: failed to clean up orphaned \
                 screenshot {} after failure (original error: {e}): {remove_err}",
                image_path.display()
            ),
        }
    }
    result
}

/// OCR + persist steps of [`meetings_append_screenshot`], split out so the
/// caller can run one cleanup path (remove the just-written PNG) over every
/// failure mode below without duplicating it at each `?`.
async fn append_screenshot_after_write(
    config: &Config,
    meeting_id: &str,
    captured_at_ms: i64,
    image_path: &std::path::Path,
    png_bytes: Vec<u8>,
) -> Result<RpcOutcome<MeetingScreenshot>, String> {
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

    match force_stop_capture_and_wait_for_drain(meeting_id).await {
        Ok(true) => log::debug!(
            "{LOG_PREFIX} meetings_finish: force-stopped and drained active local capture \
             meeting_id={meeting_id}"
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

/// Summarizes a meeting's transcript into flat prose only (pre-Block-4
/// behavior, unchanged). Kept alongside [`meetings_summarize_structured`]
/// for backward compatibility with existing callers that just want
/// [`Meeting::summary`] text — note it only ever writes that column via
/// [`store::update_summary`], never touching `summary_structured`, so
/// calling this *after* [`meetings_summarize_structured`] leaves a
/// previously-generated structured summary in place even though the flat
/// `summary` text has now moved on from it (the two go stale together only
/// in that direction; re-running `meetings_summarize_structured` always
/// refreshes both in lockstep — see `store::update_summary_structured`).
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

    let transcript = build_transcript_text(&chunks);

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

/// Summarizes a meeting's transcript into a structured [`MeetingSummary`]
/// (prose overview + explicit decisions + action items, Block 4) via the
/// configured LLM, and persists it alongside a rendered flat-prose summary
/// for backward compatibility with anything that only reads
/// [`Meeting::summary`] (older frontend builds, `export::render_pdf` /
/// `render_docx`, the plain `aura.meetings_summarize` RPC's own callers).
///
/// Shares [`meetings_summarize`]'s validation (meeting must exist, must have
/// a non-empty transcript) and transcript-building step exactly, so both
/// summarization flows see the identical input.
///
/// The LLM is asked to return JSON only; [`parse_structured_summary_response`]
/// tolerates a markdown code fence around it or stray prose around the JSON
/// object (a real, non-deterministic LLM does not always follow "JSON only"
/// literally). If parsing still fails after both fallbacks, this does not
/// error the whole RPC — it degrades to a [`MeetingSummary`] with the raw
/// model response as `overview` and empty `decisions`/`action_items`, logs a
/// warning, and persists that. A meeting always ends up with *some* readable
/// summary from a successful LLM call, even one that returned malformed
/// JSON; only a failed LLM call (network/provider error) errors this RPC.
pub async fn meetings_summarize_structured(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<MeetingSummary>, String> {
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

    let transcript = build_transcript_text(&chunks);

    log::debug!(
        "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: start meeting_id={meeting_id} \
         chunk_count={} transcript_chars={}",
        chunks.len(),
        transcript.len()
    );

    let raw_response = summarize_transcript_structured(config, &transcript).await?;

    let summary = match parse_structured_summary_response(&raw_response) {
        Ok(summary) => {
            log::debug!(
                "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: meeting_id={meeting_id} \
                 parsed structured response: decisions={} action_items={}",
                summary.decisions.len(),
                summary.action_items.len()
            );
            summary
        }
        Err(parse_err) => {
            log::warn!(
                "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: meeting_id={meeting_id} \
                 failed to parse LLM response as structured JSON ({parse_err}); falling back to \
                 an overview-only summary built from the raw response. raw_response_chars={}",
                raw_response.len()
            );
            MeetingSummary {
                overview: raw_response.trim().to_string(),
                decisions: Vec::new(),
                action_items: Vec::new(),
            }
        }
    };

    let plain_summary = render_structured_summary_prose(&summary);
    store::update_summary_structured(config, meeting_id, &plain_summary, &summary)
        .map_err(|e| e.to_string())?;

    log::debug!(
        "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: done meeting_id={meeting_id} \
         decisions={} action_items={}",
        summary.decisions.len(),
        summary.action_items.len()
    );

    // Best-effort: index this meeting into the memory_tree knowledge base now
    // that it has real content worth surfacing in cross-source search/recall.
    // Non-fatal — a knowledge-base indexing hiccup must never fail the
    // summarize RPC the user is waiting on.
    match meetings_index_to_knowledge_base(config, meeting_id).await {
        Ok(outcome) => log::debug!(
            "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: meeting_id={meeting_id} \
             indexed into knowledge base ({})",
            outcome.value
        ),
        Err(e) => log::warn!(
            "{SUMMARY_LOG_PREFIX} meetings_summarize_structured: meeting_id={meeting_id} \
             knowledge-base indexing failed (non-fatal): {e}"
        ),
    }

    Ok(RpcOutcome::new(
        summary,
        vec![format!("meeting summarized (structured): {meeting_id}")],
    ))
}

/// Indexes a meeting's transcript + summary into the memory_tree knowledge
/// base as a `meeting_notes` document — the same cross-source search/recall/
/// filter surface (`aura.memory_tree_list_chunks`, `..._list_sources`) that
/// already covers chat/email/other documents, per
/// `DocumentInput`'s own doc comment ("one meeting-note file").
///
/// Idempotent by design: `source_id` is `"meeting:{meeting_id}"`, and
/// `ingest_document` treats a document `source_id` as append-only/immutable
/// (see `memory::tree::ingest::ingest_document`'s doc comment) — a second
/// call for the same meeting is a harmless no-op (`already_ingested: true`),
/// it does NOT refresh the indexed content. Re-summarizing a meeting after
/// it was already indexed will not update the knowledge-base copy in this
/// MVP; re-indexing on content change is a follow-up, not silently promised
/// here.
///
/// Callable directly via `aura.meetings_index_to_knowledge_base` (e.g. to
/// index a meeting that only ever got the plain-text `meetings_summarize`,
/// never the structured one) and called automatically, best-effort, at the
/// end of [`meetings_summarize_structured`].
pub async fn meetings_index_to_knowledge_base(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;

    // Guard against permanently locking in a partial snapshot: ingest_document
    // is append-only, keyed purely on source_id ("meeting:{id}") — it has no
    // notion of "content changed, please refresh." Indexing a still-Recording
    // meeting would claim that source_id with a partial transcript, and every
    // later call (including the auto-trigger at the end of
    // meetings_summarize_structured, once the meeting actually finishes and
    // has a real summary) would then silently no-op as "already indexed,"
    // never capturing the final content. Only Completed meetings may be
    // indexed.
    if meeting.status != MeetingStatus::Completed {
        return Err(format!(
            "meeting is not completed (status: {}) — index it after calling meetings_finish, \
             not while still recording",
            meeting.status.as_str()
        ));
    }

    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;

    let body = render_meeting_knowledge_markdown(&meeting, &chunks);
    let modified_at = chrono::DateTime::parse_from_rfc3339(&meeting.updated_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let doc = crate::aura::memory::tree::canonicalize::document::DocumentInput {
        provider: "meeting_notes".to_string(),
        title: meeting.title.clone(),
        body,
        modified_at,
        source_ref: Some(format!("meeting:{}", meeting.id)),
    };

    // "local" — this is a single-user desktop app; meetings have no
    // multi-tenant owner concept (see Meeting's doc comment), so every
    // meeting-notes document ingests under a fixed owner tag, matching how
    // other single-user-only ingest call sites in this codebase behave.
    let tags = vec![
        "meeting".to_string(),
        format!("platform:{}", meeting.source),
    ];

    let result = crate::aura::memory::tree::ingest::ingest_document(
        config,
        &format!("meeting:{meeting_id}"),
        "local",
        tags,
        doc,
    )
    .await
    .map_err(|e| format!("knowledge-base ingest failed: {e}"))?;

    log::debug!(
        "{SUMMARY_LOG_PREFIX} meetings_index_to_knowledge_base: meeting_id={meeting_id} \
         chunks_written={} already_ingested={}",
        result.chunks_written,
        result.already_ingested
    );

    Ok(RpcOutcome::new(
        json!({
            "chunks_written": result.chunks_written,
            "already_ingested": result.already_ingested,
        }),
        vec![format!("meeting indexed into knowledge base: {meeting_id}")],
    ))
}

/// Renders a meeting's title, platform, summary (structured if present,
/// else the flat-prose fallback), and full transcript as a single markdown
/// document for knowledge-base ingestion. Pure/no I/O — unit-testable.
fn render_meeting_knowledge_markdown(meeting: &Meeting, chunks: &[TranscriptChunk]) -> String {
    let mut md = format!("# {}\n\n", meeting.title);
    md.push_str(&format!("Platform: {}\n\n", meeting.source));

    if let Some(structured) = &meeting.summary_structured {
        if !structured.overview.trim().is_empty() {
            md.push_str("## Summary\n\n");
            md.push_str(structured.overview.trim());
            md.push_str("\n\n");
        }
        if !structured.decisions.is_empty() {
            md.push_str("## Decisions\n\n");
            for decision in &structured.decisions {
                md.push_str(&format!("- {decision}\n"));
            }
            md.push('\n');
        }
        if !structured.action_items.is_empty() {
            md.push_str("## Action Items\n\n");
            for item in &structured.action_items {
                match &item.owner {
                    Some(owner) => md.push_str(&format!("- {} ({owner})\n", item.text)),
                    None => md.push_str(&format!("- {}\n", item.text)),
                }
            }
            md.push('\n');
        }
    } else if let Some(summary) = &meeting.summary {
        if !summary.trim().is_empty() {
            md.push_str("## Summary\n\n");
            md.push_str(summary.trim());
            md.push_str("\n\n");
        }
    }

    if !chunks.is_empty() {
        md.push_str("## Transcript\n\n");
        md.push_str(&build_transcript_text(chunks));
        md.push('\n');
    }

    md
}

/// Builds the `"[speaker]: text"` transcript blob fed to the summarization
/// LLM — shared verbatim by [`meetings_summarize`] and
/// [`meetings_summarize_structured`] so both flows see identical input, and
/// by `export::transcript_line` for the exported-document rendering (see
/// that function's doc comment).
fn build_transcript_text(chunks: &[TranscriptChunk]) -> String {
    chunks
        .iter()
        .map(|c| {
            format!(
                "[{}]: {}",
                c.speaker.as_deref().unwrap_or("unknown"),
                c.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders a [`MeetingSummary`] to flat prose for [`Meeting::summary`] —
/// the same field, and same rough shape (prose, then labeled sections), that
/// [`meetings_summarize`]'s LLM prompt already asks for directly. Used so a
/// meeting summarized via the structured flow still has a sensible
/// `Meeting::summary` for anything that only reads that field (see
/// `store::update_summary_structured`'s doc comment).
///
/// Never empty: falls back to a placeholder string if the structured summary
/// itself is completely empty (e.g. the LLM found no decisions, no action
/// items, and returned a blank overview) rather than persisting an empty
/// `summary` that would render as nothing in the UI.
fn render_structured_summary_prose(summary: &MeetingSummary) -> String {
    let mut sections: Vec<String> = Vec::new();

    let overview = summary.overview.trim();
    if !overview.is_empty() {
        sections.push(overview.to_string());
    }

    if !summary.decisions.is_empty() {
        let bullets = summary
            .decisions
            .iter()
            .map(|d| format!("- {}", d.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Decisions:\n{bullets}"));
    }

    if !summary.action_items.is_empty() {
        let bullets = summary
            .action_items
            .iter()
            .map(|item| match item.owner.as_deref().map(str::trim) {
                Some(owner) if !owner.is_empty() => format!("- {} ({owner})", item.text.trim()),
                _ => format!("- {}", item.text.trim()),
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Action items:\n{bullets}"));
    }

    if sections.is_empty() {
        "(no summary content)".to_string()
    } else {
        sections.join("\n\n")
    }
}

/// Parses the LLM's structured-summary response into a [`MeetingSummary`].
///
/// Tries, in order: (1) the trimmed response as-is; (2) with a surrounding
/// markdown code fence (`` ```json ... ``` `` or `` ``` ... ``` ``) stripped,
/// since models frequently wrap JSON in one despite a "JSON only" system
/// prompt; (3) the first balanced `{...}` object found anywhere in the
/// response, in case the model added stray prose before/after the JSON.
/// Returns the last, most specific error if every attempt fails.
fn parse_structured_summary_response(raw: &str) -> Result<MeetingSummary, String> {
    let trimmed = raw.trim();

    if let Ok(summary) = serde_json::from_str::<MeetingSummary>(trimmed) {
        return Ok(summary);
    }

    let unfenced = strip_markdown_json_fence(trimmed);
    if unfenced != trimmed {
        if let Ok(summary) = serde_json::from_str::<MeetingSummary>(unfenced) {
            return Ok(summary);
        }
    }

    match extract_first_json_object(unfenced) {
        Some(candidate) => serde_json::from_str::<MeetingSummary>(&candidate).map_err(|e| {
            format!("embedded JSON object did not match the expected MeetingSummary shape: {e}")
        }),
        None => Err("response contained no JSON object".to_string()),
    }
}

/// Strips a single surrounding markdown code fence (with an optional
/// language tag on the opening line, e.g. `` ```json ``) if present;
/// otherwise returns the input unchanged.
fn strip_markdown_json_fence(s: &str) -> &str {
    let Some(stripped) = s.strip_prefix("```") else {
        return s;
    };
    let after_opening_line = match stripped.find('\n') {
        Some(i) => &stripped[i + 1..],
        None => stripped,
    };
    after_opening_line.trim().trim_end_matches("```").trim()
}

/// Extracts the substring from the first `{` to the last `}` in `s`, as a
/// best-effort way to recover a JSON object embedded in surrounding prose.
/// Not a real JSON parser — a mismatched/unbalanced result is simply left for
/// `serde_json::from_str` to reject on the following parse attempt.
fn extract_first_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end < start {
        return None;
    }
    Some(s[start..=end].to_string())
}

/// Calls the existing LLM chat-completion infrastructure to extract a
/// structured summary (decisions + action items) from a meeting transcript,
/// returning the model's raw text response for [`parse_structured_summary_response`]
/// to parse. Mirrors [`summarize_transcript`]'s provider-resolution pattern
/// exactly; only the system prompt (asking for JSON) differs.
async fn summarize_transcript_structured(
    config: &Config,
    transcript: &str,
) -> Result<String, String> {
    let (provider, model) =
        crate::aura::inference::provider::create_chat_provider("summarization", config)
            .map_err(|e| format!("failed to resolve summarization provider: {e}"))?;

    let system_prompt = "You are an assistant that extracts structured information from meeting \
        transcripts. Given the transcript, respond with ONLY a single JSON object (no markdown \
        code fences, no commentary before or after it) matching exactly this shape:\n\
        {\"overview\": \"<2-4 sentence prose summary of the discussion>\", \"decisions\": \
        [\"<concrete decision that was made>\", ...], \"action_items\": [{\"text\": \"<what needs \
        doing>\", \"owner\": \"<name mentioned in the transcript, or null if unclear>\"}, ...]}\n\
        Use an empty array for \"decisions\" or \"action_items\" when the transcript has none. Do \
        not invent decisions, action items, or owners that are not clearly supported by the \
        transcript text.";

    // Lower temperature than `summarize_transcript`'s prose summary (0.3):
    // structured extraction benefits more from determinism than the free-form
    // prose case does, and a lower temperature also reduces (does not
    // eliminate — see `parse_structured_summary_response`) the chance of the
    // model wandering outside the requested JSON shape.
    provider
        .chat_with_system(Some(system_prompt), transcript, &model, 0.2)
        .await
        .map_err(|e| format!("LLM structured summarization failed: {e}"))
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

/// Renders a meeting's transcript (+ summary, if any) to PDF, DOCX, or
/// PPTX and writes it under
/// `<workspace_dir>/meetings/exports/<meeting_id>.<format>`, returning
/// that absolute path. `format` must be `"pdf"`, `"docx"`, or `"pptx"` —
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

    if format != "pdf" && format != "docx" && format != "pptx" {
        return Err(format!(
            "unsupported export format '{format}': expected 'pdf', 'docx', or 'pptx'"
        ));
    }

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;

    let bytes = match format.as_str() {
        "pdf" => export::render_pdf(&meeting, &chunks).map_err(|e| e.to_string())?,
        "docx" => export::render_docx(&meeting, &chunks).map_err(|e| e.to_string())?,
        _ => export_pptx::render_pptx(&meeting, &chunks).map_err(|e| e.to_string())?,
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
/// `capture_screenshots` (default `true` when `None`) additionally starts
/// [`capture::pipeline`]'s sibling periodic slide-screenshot-capture task
/// (Block 3 — see `capture::screenshot`'s doc comment) for the same session,
/// persisting new slides via the existing
/// [`meetings_append_screenshot`]. Pass `Some(false)` to opt a session out
/// (e.g. a privacy-sensitive meeting, or a headless/test invocation with no
/// real display to capture from) without disabling audio transcription.
///
/// Mirrors [`meetings_append_transcript_chunk`]'s validation: the meeting
/// must exist and be `recording`. Rejects a second concurrent capture for
/// the same meeting rather than silently replacing it — call
/// [`meetings_stop_local_capture`] first. Also rejects a meeting whose
/// `source` is `"meeting_recorder"` — that means the Tauri shell's own
/// independent `meeting_recorder` capture pipeline already owns this
/// meeting's audio + screenshot capture; running this core-native path for
/// it too would double both.
pub async fn meetings_start_local_capture(
    config: &Config,
    meeting_id: &str,
    capture_screenshots: Option<bool>,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();
    let capture_screenshots = capture_screenshots.unwrap_or(true);
    log::debug!(
        "{LOG_PREFIX} [meetings-slides] meetings_start_local_capture: meeting_id={meeting_id} \
         capture_screenshots={capture_screenshots}"
    );

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

    // The Tauri shell's own `meeting_recorder` (app/src-tauri/src/
    // meeting_recorder/) runs a fully independent mic/system-audio +
    // screenshot capture pipeline for meetings it creates itself, tagging
    // them `source: "meeting_recorder"` (see `meetings_create`). It has no
    // visibility into `ACTIVE_CAPTURES` below (that registry is
    // process-local to this core-native path), so without this check a
    // second, core-native capture session could be started for the same
    // meeting_id — doubling screenshot writes and OCR compute, with the two
    // loops' dedup baselines never seeing each other's frames. Reject
    // up front instead: this path is for meetings recorded via the
    // core-native flow only.
    if meeting.source == "meeting_recorder" {
        let msg = format!(
            "meeting {meeting_id} is already being captured by the desktop app's own meeting \
             recorder (source=meeting_recorder); core-native local capture is not applicable to it"
        );
        log::warn!("{LOG_PREFIX} meetings_start_local_capture: rejected meeting_id={meeting_id} reason={msg}");
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
        capture_screenshots,
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

/// Fixed on-disk path for a meeting's raw `system_audio` WAV recording:
/// `{workspace_dir}/meetings/audio/{meeting_id}.system.wav`. Deterministic
/// from `meeting_id` alone — no DB column needed, since
/// [`capture::pipeline`](crate::aura::meetings::capture::pipeline)'s writer
/// and [`meetings_diarize`] both compute the same path independently. Only
/// compiled under the `diarize` feature, the only thing that ever creates or
/// reads this file — see `capture::raw_audio_sink` and
/// [`crate::aura::meetings::diarize`].
#[cfg(feature = "diarize")]
pub(crate) fn system_audio_wav_path(config: &Config, meeting_id: &str) -> std::path::PathBuf {
    config
        .workspace_dir
        .join("meetings")
        .join("audio")
        .join(format!("{meeting_id}.system.wav"))
}

/// Result summary for [`meetings_diarize`].
#[cfg(feature = "diarize")]
#[derive(Debug, Clone, Serialize)]
pub struct DiarizeSummary {
    pub speakers_found: usize,
    pub chunks_updated: usize,
}

/// Runs real per-speaker diarization over a *completed* meeting's recorded
/// `system_audio` track and persists the resulting `"system_audio:speaker_N"`
/// labels onto its transcript chunks.
///
/// Only reachable via the explicit `aura.meetings_diarize` RPC — never
/// auto-triggered from capture or [`meetings_finish`]. Requires
/// `meeting.status == Completed` (diarization is an offline batch pass over
/// a finished recording, mirroring Minutes' own contract) and that the raw
/// `system_audio` WAV was actually captured — only true if the `diarize`
/// feature was compiled in *during* that meeting's recording (see
/// [`capture::pipeline`](crate::aura::meetings::capture::pipeline)'s doc
/// comment). A meeting recorded before this feature was enabled, or with the
/// feature off, has no WAV to diarize and this returns a clear error rather
/// than silently doing nothing.
#[cfg(feature = "diarize")]
pub async fn meetings_diarize(
    config: &Config,
    meeting_id: &str,
) -> Result<RpcOutcome<DiarizeSummary>, String> {
    if meeting_id.trim().is_empty() {
        return Err("Missing 'meeting_id' parameter".to_string());
    }
    let meeting_id = meeting_id.trim();

    log::debug!("{LOG_PREFIX} meetings_diarize: start meeting_id={meeting_id}");

    let meeting = store::get_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    if meeting.status != MeetingStatus::Completed {
        let msg = "meeting must be completed before it can be diarized".to_string();
        log::warn!("{LOG_PREFIX} meetings_diarize: rejected meeting_id={meeting_id} reason={msg}");
        return Err(msg);
    }

    let wav_path = system_audio_wav_path(config, meeting_id);
    if !wav_path.is_file() {
        let msg = format!(
            "no recorded system_audio to diarize for meeting {meeting_id} (expected {}); this \
             meeting may predate the diarize feature being enabled, or captured no system audio",
            wav_path.display()
        );
        log::warn!("{LOG_PREFIX} meetings_diarize: {msg}");
        return Err(msg);
    }

    let segments =
        crate::aura::meetings::diarize::diarize_meeting_system_audio(config, meeting_id, &wav_path)
            .await?;
    // Excludes `engine::UNKNOWN_SPEAKER_LABEL` — a segment gets that
    // placeholder when every speech segment in the meeting was too short to
    // embed reliably (see `engine::cluster_segments`'s doc comment), meaning
    // diarization identified zero real speaker templates. Counting the
    // repeated placeholder string as a distinct speaker would fabricate a
    // single-speaker identity for audio where none was ever detected — see
    // `engine::count_real_speakers`'s doc comment.
    let speakers_found = crate::aura::meetings::diarize::engine::count_real_speakers(
        segments.iter().map(|s| s.speaker.as_str()),
    );

    let chunks = store::list_transcript_chunks(config, meeting_id).map_err(|e| e.to_string())?;
    let updates =
        crate::aura::meetings::diarize::mapping::map_segments_to_chunks(&chunks, &segments);
    let chunks_updated =
        store::update_transcript_chunk_speakers(config, &updates).map_err(|e| e.to_string())?;

    // Diarization is a one-shot offline batch pass — see this module's doc
    // comment ("no cross-meeting speaker enrollment... every meeting is
    // diarized independently") — so nothing ever reads the raw WAV again
    // after this point. Remove it now rather than leaving a permanent
    // multi-hundred-MB uncompressed-PCM file behind for every diarized
    // meeting; best-effort, since a removal failure here must not undo the
    // speaker labels that were just successfully persisted above.
    match tokio::fs::remove_file(&wav_path).await {
        Ok(()) => log::debug!(
            "{LOG_PREFIX} meetings_diarize: removed raw system_audio wav after successful \
             diarization meeting_id={meeting_id} path={}",
            wav_path.display()
        ),
        Err(e) => log::warn!(
            "{LOG_PREFIX} meetings_diarize: failed to remove raw system_audio wav after \
             diarization meeting_id={meeting_id} path={} error={e}",
            wav_path.display()
        ),
    }

    log::debug!(
        "{LOG_PREFIX} meetings_diarize: done meeting_id={meeting_id} segments={} \
         speakers_found={speakers_found} chunks_updated={chunks_updated}",
        segments.len()
    );

    Ok(RpcOutcome::new(
        DiarizeSummary {
            speakers_found,
            chunks_updated,
        },
        vec![format!(
            "meeting diarized: {meeting_id} ({speakers_found} speaker(s), {chunks_updated} chunk(s) updated)"
        )],
    ))
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
