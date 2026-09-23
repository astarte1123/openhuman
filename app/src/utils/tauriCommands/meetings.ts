/**
 * Meetings domain commands (core RPC).
 *
 * Mirrors `cron.ts` exactly — `src/aura/meetings/schemas.rs` says its
 * controller schemas "follow cron::schemas exactly". Field/type shapes below
 * are taken from `src/aura/meetings/types.rs` (Meeting / TranscriptChunk
 * / MeetingStatus) and the *actual* `RpcOutcome<T>` value each
 * `src/aura/meetings/ops.rs` function wraps (not the `schemas.rs`
 * `outputs` metadata, which only documents field names for the schema
 * catalog and does not necessarily match the wire shape):
 *   - `meetings_list`            -> RpcOutcome<Vec<Meeting>>          (bare array)
 *   - `meetings_get`             -> RpcOutcome<MeetingWithChunks>     ({ meeting, chunks, screenshots })
 *   - `meetings_update_title`    -> RpcOutcome<Meeting>               (bare object)
 *   - `meetings_delete`          -> RpcOutcome<json!({ deleted }) >   ({ deleted })
 *   - `meetings_summarize`       -> RpcOutcome<json!({ summary }) >   ({ summary })
 *   - `meetings_summarize_structured` -> RpcOutcome<MeetingSummary>  (bare object; see
 *     `src/aura/meetings/types.rs`'s `MeetingSummary` / `ActionItem` — Block 4)
 *   - `meetings_search`          -> RpcOutcome<Vec<SearchResult>>     (bare array)
 *   - `meetings_append_screenshot` -> RpcOutcome<MeetingScreenshot>   (bare object)
 *
 * Field names below are kept snake_case to match the wire format exactly —
 * this file does no snake_case -> camelCase mapping anywhere (see
 * `Meeting.created_at` / `TranscriptChunk.start_ms` above), so
 * `TranscriptSearchResult` follows the same convention rather than the
 * camelCase shape one might otherwise assume.
 */
import { callCoreRpc } from '../../services/coreRpcClient';
import { CommandResponse, isTauri } from './common';

export type MeetingStatus = 'recording' | 'completed' | 'error';

export interface Meeting {
  id: string;
  title: string;
  source: string;
  status: MeetingStatus;
  /** RFC3339 timestamp. */
  created_at: string;
  /** RFC3339 timestamp. */
  updated_at: string;
  duration_ms: number | null;
  summary: string | null;
  /**
   * Structured decisions + action items (Block 4), alongside `summary`'s
   * flat prose. `null` for a meeting never (re-)summarized via
   * `auraMeetingsSummarizeStructured`, or predating this field entirely —
   * `summary` always stays populated and readable regardless.
   */
  summary_structured: MeetingSummary | null;
}

/** One action item extracted from a meeting's structured summary. */
export interface MeetingActionItem {
  text: string;
  /** Owner named in the transcript, if the model could tell. */
  owner: string | null;
}

/** Structured meeting summary: prose overview + decisions + action items. */
export interface MeetingSummary {
  overview: string;
  decisions: string[];
  action_items: MeetingActionItem[];
}

export interface TranscriptChunk {
  id: number;
  meeting_id: string;
  start_ms: number;
  end_ms: number;
  speaker: string | null;
  text: string;
  /** RFC3339 timestamp. */
  created_at: string;
}

/** One full-text search hit against a meeting transcript. */
export interface TranscriptSearchResult {
  meeting_id: string;
  meeting_title: string;
  chunk_id: number;
  start_ms: number;
  /** Excerpt with `<mark>`/`</mark>` wrapped around the matched terms. */
  snippet: string;
}

/** One periodically-captured screenshot of a meeting's shared slides. */
export interface MeetingScreenshot {
  id: number;
  meeting_id: string;
  /** Milliseconds since the meeting started, matching `TranscriptChunk.start_ms`. */
  captured_at_ms: number;
  /** Absolute path of the saved PNG on disk. */
  image_path: string;
  ocr_text: string;
  /** RFC3339 timestamp. */
  created_at: string;
}

export async function auraMeetingsList(): Promise<CommandResponse<Meeting[]>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<Meeting[]>>({ method: 'aura.meetings_list' });
}

export async function auraMeetingsGet(
  meetingId: string
): Promise<
  CommandResponse<{ meeting: Meeting; chunks: TranscriptChunk[]; screenshots: MeetingScreenshot[] }>
> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<
    CommandResponse<{
      meeting: Meeting;
      chunks: TranscriptChunk[];
      screenshots: MeetingScreenshot[];
    }>
  >({ method: 'aura.meetings_get', params: { meeting_id: meetingId } });
}

export async function auraMeetingsUpdateTitle(
  meetingId: string,
  title: string
): Promise<CommandResponse<Meeting>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<Meeting>>({
    method: 'aura.meetings_update_title',
    params: { meeting_id: meetingId, title },
  });
}

export async function auraMeetingsDelete(
  meetingId: string
): Promise<CommandResponse<{ deleted: boolean }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ deleted: boolean }>>({
    method: 'aura.meetings_delete',
    params: { meeting_id: meetingId },
  });
}

export async function auraMeetingsSummarize(
  meetingId: string
): Promise<CommandResponse<{ summary: string }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ summary: string }>>({
    method: 'aura.meetings_summarize',
    params: { meeting_id: meetingId },
  });
}

/**
 * Summarizes a meeting's transcript into a structured decisions + action
 * items payload (Block 4), persisting it alongside a rendered flat-prose
 * `summary` for backward compatibility. See `auraMeetingsSummarize` for the
 * plain-prose-only variant.
 */
export async function auraMeetingsSummarizeStructured(
  meetingId: string
): Promise<CommandResponse<MeetingSummary>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<MeetingSummary>>({
    method: 'aura.meetings_summarize_structured',
    params: { meeting_id: meetingId },
  });
}

export async function auraMeetingsSearch(
  query: string
): Promise<CommandResponse<TranscriptSearchResult[]>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<TranscriptSearchResult[]>>({
    method: 'aura.meetings_search',
    params: { query },
  });
}

export type MeetingExportFormat = 'pdf' | 'docx' | 'pptx';

/** Renders a meeting's transcript + summary to disk and returns its path. */
export async function auraMeetingsExport(
  meetingId: string,
  format: MeetingExportFormat
): Promise<CommandResponse<{ path: string }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ path: string }>>({
    method: 'aura.meetings_export',
    params: { meeting_id: meetingId, format },
  });
}

/**
 * Appends one OCR'd slide screenshot to a recording meeting. Primarily
 * called from the Tauri shell's `meeting_recorder::screenshot` capture
 * loop (Rust, over its own HTTP JSON-RPC helper) — exposed here too so any
 * future in-app "add a screenshot manually" affordance can reuse the exact
 * same wire contract.
 */
export async function auraMeetingsAppendScreenshot(
  meetingId: string,
  capturedAtMs: number,
  imageBase64: string
): Promise<CommandResponse<MeetingScreenshot>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<MeetingScreenshot>>({
    method: 'aura.meetings_append_screenshot',
    params: { meeting_id: meetingId, captured_at_ms: capturedAtMs, image_base64: imageBase64 },
  });
}
