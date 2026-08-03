/**
 * Meetings domain commands (core RPC).
 *
 * Mirrors `cron.ts` exactly — `src/openhuman/meetings/schemas.rs` says its
 * controller schemas "follow cron::schemas exactly". Field/type shapes below
 * are taken from `src/openhuman/meetings/types.rs` (Meeting / TranscriptChunk
 * / MeetingStatus) and the *actual* `RpcOutcome<T>` value each
 * `src/openhuman/meetings/ops.rs` function wraps (not the `schemas.rs`
 * `outputs` metadata, which only documents field names for the schema
 * catalog and does not necessarily match the wire shape):
 *   - `meetings_list`            -> RpcOutcome<Vec<Meeting>>          (bare array)
 *   - `meetings_get`             -> RpcOutcome<MeetingWithChunks>     ({ meeting, chunks, screenshots })
 *   - `meetings_update_title`    -> RpcOutcome<Meeting>               (bare object)
 *   - `meetings_delete`          -> RpcOutcome<json!({ deleted }) >   ({ deleted })
 *   - `meetings_summarize`       -> RpcOutcome<json!({ summary }) >   ({ summary })
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

export async function openhumanMeetingsList(): Promise<CommandResponse<Meeting[]>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<Meeting[]>>({ method: 'openhuman.meetings_list' });
}

export async function openhumanMeetingsGet(
  meetingId: string
): Promise<
  CommandResponse<{ meeting: Meeting; chunks: TranscriptChunk[]; screenshots: MeetingScreenshot[] }>
> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<
    CommandResponse<{ meeting: Meeting; chunks: TranscriptChunk[]; screenshots: MeetingScreenshot[] }>
  >({
    method: 'openhuman.meetings_get',
    params: { meeting_id: meetingId },
  });
}

export async function openhumanMeetingsUpdateTitle(
  meetingId: string,
  title: string
): Promise<CommandResponse<Meeting>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<Meeting>>({
    method: 'openhuman.meetings_update_title',
    params: { meeting_id: meetingId, title },
  });
}

export async function openhumanMeetingsDelete(
  meetingId: string
): Promise<CommandResponse<{ deleted: boolean }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ deleted: boolean }>>({
    method: 'openhuman.meetings_delete',
    params: { meeting_id: meetingId },
  });
}

export async function openhumanMeetingsSummarize(
  meetingId: string
): Promise<CommandResponse<{ summary: string }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ summary: string }>>({
    method: 'openhuman.meetings_summarize',
    params: { meeting_id: meetingId },
  });
}

export async function openhumanMeetingsSearch(
  query: string
): Promise<CommandResponse<TranscriptSearchResult[]>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<TranscriptSearchResult[]>>({
    method: 'openhuman.meetings_search',
    params: { query },
  });
}

export type MeetingExportFormat = 'pdf' | 'docx';

/** Renders a meeting's transcript + summary to disk and returns its path. */
export async function openhumanMeetingsExport(
  meetingId: string,
  format: MeetingExportFormat
): Promise<CommandResponse<{ path: string }>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<{ path: string }>>({
    method: 'openhuman.meetings_export',
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
export async function openhumanMeetingsAppendScreenshot(
  meetingId: string,
  capturedAtMs: number,
  imageBase64: string
): Promise<CommandResponse<MeetingScreenshot>> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await callCoreRpc<CommandResponse<MeetingScreenshot>>({
    method: 'openhuman.meetings_append_screenshot',
    params: { meeting_id: meetingId, captured_at_ms: capturedAtMs, image_base64: imageBase64 },
  });
}
