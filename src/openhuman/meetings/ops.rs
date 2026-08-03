//! RPC-facing operations for the meetings domain.
//!
//! Each `meetings_*` function is a thin async wrapper around [`store`]: it
//! validates input, applies the domain's business rules (e.g. transcript
//! chunks can only be appended while a meeting is `recording`), and wraps
//! the result in [`RpcOutcome`] — mirroring `cron::ops::cron_list` /
//! `cron_update` / etc.

use crate::openhuman::config::Config;
use crate::openhuman::meetings::export;
use crate::openhuman::meetings::ocr;
use crate::openhuman::meetings::store;
use crate::openhuman::meetings::store::SearchResult;
use crate::openhuman::meetings::types::{Meeting, MeetingScreenshot, MeetingStatus, TranscriptChunk};
use crate::rpc::RpcOutcome;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;
use serde_json::json;

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
    log::debug!("[meetings] meetings_create id={} source={source}", meeting.id);
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

    let deleted = store::delete_meeting(config, meeting_id).map_err(|e| e.to_string())?;
    Ok(RpcOutcome::new(
        json!({ "deleted": deleted }),
        vec![format!("meeting delete requested: {meeting_id} (deleted={deleted})")],
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
        vec![format!("transcript chunk appended to meeting: {meeting_id}")],
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
        .map(|c| format!("[{}]: {}", c.speaker.as_deref().unwrap_or("unknown"), c.text))
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
        vec![format!("transcript search: {count} result(s) for {query:?}")],
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
        return Err(format!("unsupported export format '{format}': expected 'pdf' or 'docx'"));
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
        vec![format!("meeting exported: {meeting_id} -> {}", output_path.display())],
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
        crate::openhuman::inference::provider::create_chat_provider("summarization", config)
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

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
