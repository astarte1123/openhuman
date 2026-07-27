/**
 * Typed wrappers for Tauri IPC commands.
 * All communication with the Rust backend goes through these functions.
 */

import { invoke as _tauriInvoke } from '@tauri-apps/api/core';

async function invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  try {
    return await _tauriInvoke(cmd, args);
  } catch (e) {
    // Graceful fallback in browser dev (no Tauri runtime available).
    if (!('__TAURI_INTERNALS__' in window)) {
      console.debug('[api stub]', cmd, args);
      return null;
    }
    throw e;
  }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Thread {
  id: string;
  title: string;
  labels: string[];
  created_at: string;
}

export interface Message {
  id?: string;
  role: 'user' | 'assistant';
  content: string;
  created_at?: string;
}

export interface AppendResult {
  user_message: Message;
  assistant_message: Message;
}

export interface HealthSnapshot {
  status: string;
  version: string;
}

export interface Config {
  provider: string;
  model: string;
}

// ---------------------------------------------------------------------------
// Core invoke helper
// ---------------------------------------------------------------------------

async function coreInvoke<T>(method: string, params: unknown = {}): Promise<T> {
  const result = await invoke('core_invoke', { method, params });
  // Unwrap standard API envelope { data: T, api_version: "..." } transparently.
  // Handlers not yet migrated return raw payloads — both formats are supported.
  const r = result as Record<string, unknown> | null;
  return (r !== null && typeof r === 'object' && 'data' in r ? r.data : r) as T;
}

// ---------------------------------------------------------------------------
// Threads
// ---------------------------------------------------------------------------

export async function listThreads(): Promise<Thread[]> {
  const res = await coreInvoke<{ result: Thread[] }>('openhuman.threads_list');
  return res.result ?? [];
}

export async function createThread(): Promise<Thread> {
  return coreInvoke('openhuman.threads_create_new');
}

export async function renameThread(threadId: string, title: string): Promise<void> {
  await coreInvoke('openhuman.threads_rename', { thread_id: threadId, title });
}

export async function deleteThread(threadId: string): Promise<void> {
  await coreInvoke('openhuman.threads_delete', { thread_id: threadId });
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

export async function listMessages(threadId: string): Promise<Message[]> {
  const res = await coreInvoke<{ result: Message[] }>('openhuman.threads_messages_list', {
    thread_id: threadId,
  });
  return res.result ?? [];
}

export async function appendMessage(
  threadId: string,
  message: Pick<Message, 'role' | 'content'>,
): Promise<AppendResult> {
  return coreInvoke('openhuman.threads_message_append', {
    thread_id: threadId,
    message,
  });
}

// ---------------------------------------------------------------------------
// Web tools (callable from UI directly)
// ---------------------------------------------------------------------------

export async function webSearch(query: string): Promise<string> {
  const res = await coreInvoke<{ result: string }>('openhuman.web_search', { query });
  return res.result ?? '';
}

export async function webFetch(url: string): Promise<string> {
  const res = await coreInvoke<{ result: string }>('openhuman.web_fetch', { url });
  return res.result ?? '';
}

// ---------------------------------------------------------------------------
// Health + Config
// ---------------------------------------------------------------------------

export async function healthSnapshot(): Promise<HealthSnapshot> {
  return coreInvoke('openhuman.health_snapshot');
}

export async function configGet(): Promise<Config> {
  return coreInvoke('openhuman.config_get');
}

/**
 * Persist an env var via the set_env_var Tauri command.
 * Used for API keys, model overrides, Ollama URL, etc.
 */
export async function setEnvVar(key: string, value: string): Promise<void> {
  await invoke('set_env_var', { key, value });
}

// ---------------------------------------------------------------------------
// Memories
// ---------------------------------------------------------------------------

export interface Memory {
  id: string;
  content: string;
  tags: string;
  created_at: string;
}

export async function listMemories(): Promise<Memory[]> {
  const res = await coreInvoke<{ result: Memory[] }>('openhuman.memories_list');
  return res.result ?? [];
}

export async function createMemory(content: string, tags = ''): Promise<Memory> {
  return coreInvoke('openhuman.memories_create', { content, tags });
}

export async function deleteMemory(id: string): Promise<void> {
  await coreInvoke('openhuman.memories_delete', { id });
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

export interface Task {
  id: string;
  title: string;
  status: 'todo' | 'done';
  created_at: string;
}

export async function listTasks(): Promise<Task[]> {
  const res = await coreInvoke<{ result: Task[] }>('openhuman.tasks_list');
  return res.result ?? [];
}

export async function createTask(title: string): Promise<Task> {
  return coreInvoke('openhuman.tasks_create', { title });
}

export async function updateTask(id: string, status: 'todo' | 'done'): Promise<Task> {
  return coreInvoke('openhuman.tasks_update', { id, status });
}

export async function deleteTask(id: string): Promise<void> {
  await coreInvoke('openhuman.tasks_delete', { id });
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

export interface Skill {
  id: string;
  name: string;
  description: string;
  category: string;
  enabled: boolean;
}

export async function listSkills(): Promise<Skill[]> {
  const res = await coreInvoke<{ result: Skill[] }>('openhuman.skills_list');
  return res.result ?? [];
}

export async function toggleSkill(id: string, enabled: boolean): Promise<Skill> {
  return coreInvoke('openhuman.skills_toggle', { id, enabled });
}

// ---------------------------------------------------------------------------
// Env var helpers — read directly from process env (source of truth: youma.env)
// ---------------------------------------------------------------------------

// Returns current string value of a non-sensitive env var.
export async function envGet(key: string): Promise<string> {
  const res = await coreInvoke<{ value: string }>('openhuman.env_get', { key });
  return res?.value ?? '';
}

// Returns whether a (possibly sensitive) env var is non-empty without exposing its value.
export async function envIsSet(key: string): Promise<boolean> {
  const res = await coreInvoke<{ set: boolean }>('openhuman.env_is_set', { key });
  return res?.set ?? false;
}

// ---------------------------------------------------------------------------
// App settings
// ---------------------------------------------------------------------------

export async function settingsGet(key: string): Promise<string | null> {
  const res = await coreInvoke<{ value: string | null }>('openhuman.settings_get', { key });
  return res.value ?? null;
}

export async function settingsSet(key: string, value: string): Promise<void> {
  await coreInvoke('openhuman.settings_set', { key, value });
}

// ---------------------------------------------------------------------------
// Invites
// ---------------------------------------------------------------------------

export interface InviteCode {
  code: string;
  created_at: string;
  used: boolean;
}

export async function listInvites(): Promise<InviteCode[]> {
  const res = await coreInvoke<{ result: InviteCode[] }>('openhuman.invites_list');
  return res.result ?? [];
}

export async function generateInvite(): Promise<InviteCode> {
  return coreInvoke('openhuman.invites_generate');
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

export interface RewardsInfo {
  referral_code: string;
  points: number;
  referrals: number;
  tier: string;
}

export async function getRewards(): Promise<RewardsInfo> {
  return coreInvoke('openhuman.rewards_get');
}

// ---------------------------------------------------------------------------
// Composio integrations
// ---------------------------------------------------------------------------

export interface ComposioApp {
  name: string;
  display_name: string;
  description: string;
  auth_mode: string;
  logo_url: string;
}

export interface ComposioConnection {
  id: string;
  app_name: string;
  app_display_name: string;
  entity_id: string;
  status: 'INITIATED' | 'ACTIVE' | 'FAILED' | 'UNKNOWN';
  redirect_url: string | null;
  last_fetched_at: string | null;
  created_at: string;
}

/** List apps from cache (falls back to live Composio API on first load). */
export async function composioListApps(): Promise<ComposioApp[]> {
  const res = await coreInvoke<{ result: ComposioApp[]; error?: string }>(
    'openhuman.composio_apps_list',
  );
  return res.result ?? [];
}

/** List locally-stored connections (fast, no network call). */
export async function composioListConnections(): Promise<ComposioConnection[]> {
  const res = await coreInvoke<{ result: ComposioConnection[] }>(
    'openhuman.composio_connections_list',
  );
  return res.result ?? [];
}

/** Initiate OAuth/API-key connection for an app. Opens the browser automatically. */
export async function composioConnect(
  appName: string,
  entityId = 'default',
): Promise<{ connectedAccountId?: string; redirectUrl?: string }> {
  return coreInvoke('openhuman.composio_connect', {
    app_name: appName,
    entity_id: entityId,
  });
}

/** Poll Composio for the current status of a connection. */
export async function composioPoll(
  connectionId: string,
): Promise<{ connection_id: string; status: string }> {
  return coreInvoke('openhuman.composio_poll', { connection_id: connectionId });
}

/** Disconnect a service — deletes the connection from Composio and locally. */
export async function composioDisconnect(connectionId: string): Promise<void> {
  await coreInvoke('openhuman.composio_disconnect', { connection_id: connectionId });
}

/** Execute an arbitrary Composio action. */
export async function composioExecute(
  actionId: string,
  entityId = 'default',
  input: Record<string, unknown> = {},
): Promise<unknown> {
  return coreInvoke('openhuman.composio_execute', {
    action_id: actionId,
    entity_id: entityId,
    input,
  });
}

// ---------------------------------------------------------------------------
// Knowledge base
// ---------------------------------------------------------------------------

export interface KnowledgeChunk {
  id: string;
  source_type: string;
  source_id: string;
  title: string;
  content_md: string;
  score: number;
  tokens: number;
  topic_group: string;
  tags: string;
  created_at: string;
}

export interface KnowledgeTreeNode {
  id: string;
  level: number;
  label: string;
  summary_md: string;
  child_ids: string[];
  chunk_ids: string[];
  topic_group: string;
}

/** List knowledge chunks, optionally filtered by topic (paginated). */
export async function knowledgeChunksList(opts: {
  topic?: string;
  limit?: number;
  offset?: number;
} = {}): Promise<KnowledgeChunk[]> {
  const res = await coreInvoke<{ result: KnowledgeChunk[] }>(
    'openhuman.knowledge_chunks_list',
    opts,
  );
  return res.result ?? [];
}

/** Lightweight chunk metadata for the Wiki graph (no content_md, top N per topic). */
export async function knowledgeChunksGraph(perTopic = 40): Promise<KnowledgeChunk[]> {
  const res = await coreInvoke<{ result: KnowledgeChunk[] }>(
    'openhuman.knowledge_chunks_graph',
    { per_topic: perTopic },
  );
  return res.result ?? [];
}

/** Fetch a single chunk with full content (for the node detail panel). */
export async function knowledgeChunkGet(id: string): Promise<KnowledgeChunk | null> {
  const res = await coreInvoke<{ result: KnowledgeChunk }>(
    'openhuman.knowledge_chunk_get',
    { id },
  );
  return res.result ?? null;
}

/** Return all summary-tree nodes (branches + root). */
export async function knowledgeTreeNodes(): Promise<KnowledgeTreeNode[]> {
  const res = await coreInvoke<{ result: KnowledgeTreeNode[] }>(
    'openhuman.knowledge_tree_nodes',
  );
  return res.result ?? [];
}

/** Trigger a full re-ingest + tree rebuild in the background. Returns immediately. */
export async function knowledgeRebuild(): Promise<void> {
  await coreInvoke('openhuman.knowledge_rebuild');
}

export interface KnowledgeSource {
  source_id: string;
  title: string;
  chunk_count: number;
  created_at: string;
  topic_group: string;
  tags: string;
}

/** Ingest plain text or Markdown as a manual document. */
export async function knowledgeIngestText(opts: {
  title: string;
  content: string;
  tags?: string;
  source_id?: string;
}): Promise<{ chunks: number; source_id: string }> {
  return coreInvoke('openhuman.knowledge_ingest_text', opts);
}

/** Ingest a file by absolute path on disk (.txt, .md, etc.). */
export async function knowledgeIngestFile(opts: {
  path: string;
  tags?: string;
}): Promise<{ chunks: number; source_id: string }> {
  return coreInvoke('openhuman.knowledge_ingest_file', opts);
}

/** List all manually uploaded document sources. */
export async function knowledgeSourcesList(): Promise<KnowledgeSource[]> {
  const res = await coreInvoke<{ result: KnowledgeSource[] }>('openhuman.knowledge_sources_list');
  return res.result ?? [];
}

/** Delete all chunks belonging to a manually uploaded document. */
export async function knowledgeSourceDelete(sourceId: string): Promise<void> {
  await coreInvoke('openhuman.knowledge_source_delete', { source_id: sourceId });
}

/** Sync knowledge base to Obsidian-compatible .md files. Returns file count written. */
export async function knowledgeVaultSync(): Promise<number> {
  const res = await coreInvoke<{ files_written: number }>(
    'openhuman.knowledge_vault_sync',
  );
  return res.files_written ?? 0;
}

/** Get the configured Obsidian vault path (returns default if not set). */
export async function knowledgeVaultPathGet(): Promise<string> {
  const res = await coreInvoke<{ path: string }>('openhuman.knowledge_vault_path_get');
  return res.path ?? '';
}

/** Persist a custom Obsidian vault path. */
export async function knowledgeVaultPathSet(path: string): Promise<void> {
  await coreInvoke('openhuman.knowledge_vault_path_set', { path });
}

/** Delete a single knowledge chunk by its ID. */
export async function knowledgeChunkDelete(id: string): Promise<void> {
  await coreInvoke('openhuman.knowledge_chunk_delete', { id });
}

/**
 * Send base64-encoded audio to Rust for Whisper transcription.
 * Requires OPENAI_API_KEY to be configured. Returns the transcript string.
 */
export async function transcribeAudio(audioB64: string, format = 'webm'): Promise<string> {
  const result = await invoke('transcribe_audio', { audioB64, format });
  return (result as string) ?? '';
}

/**
 * Synthesize text via OpenAI TTS (nova voice, MP3).
 * Returns base64 MP3 string, or throws "no_openai_key" if key missing.
 */
export async function speakText(text: string): Promise<string> {
  const result = await invoke('speak_text', { text });
  return (result as string) ?? '';
}

/** Native WinRT TTS — open source, no API key, uses best installed voice. */
export async function speakNative(text: string): Promise<void> {
  await invoke('speak_native', { text });
}

/**
 * Synthesize text via ElevenLabs TTS (high quality, low latency turbo model).
 * Returns base64 MP3. Throws "no_elevenlabs_key" if ELEVENLABS_API_KEY not set.
 */
export async function speakElevenLabs(text: string): Promise<string> {
  const result = await invoke('speak_elevenlabs', { text });
  return (result as string) ?? '';
}

// ---------------------------------------------------------------------------
// Coder toolset — filesystem, shell, git
// ---------------------------------------------------------------------------

/** Read a file. Returns its text content. */
export async function toolsFsRead(path: string, maxBytes?: number): Promise<{ content: string; path: string }> {
  return coreInvoke('openhuman.tools_fs_read', { path, ...(maxBytes !== undefined ? { max_bytes: maxBytes } : {}) });
}

/** Write content to a file (creates parent dirs as needed). */
export async function toolsFsWrite(path: string, content: string): Promise<void> {
  await coreInvoke('openhuman.tools_fs_write', { path, content });
}

/** List files and directories at a path. */
export async function toolsFsList(path: string, recursive = false): Promise<{ result: string[] }> {
  return coreInvoke('openhuman.tools_fs_list', { path, recursive });
}

/** Search for a literal string in files under a directory. */
export async function toolsFsGrep(
  pattern: string,
  path: string,
  glob?: string,
): Promise<{ result: string[]; count: number }> {
  return coreInvoke('openhuman.tools_fs_grep', { pattern, path, ...(glob ? { glob } : {}) });
}

// ---------------------------------------------------------------------------
// Inbox — unified message feed
// ---------------------------------------------------------------------------

export interface InboxMessage {
  id: string;
  service: string;
  sender: string;
  subject: string;
  body: string;
  received_at: string;
  read: boolean;
  metadata: string;
}

export interface InboxListResult {
  result: InboxMessage[];
  unread: number;
}

/** List inbox messages, optionally filtered by service slug (e.g. "gmail"). */
export async function inboxList(opts: { service?: string; limit?: number; offset?: number } = {}): Promise<InboxListResult> {
  return coreInvoke('openhuman.inbox_list', opts);
}

/** Send a message via a connected Composio service. */
export async function inboxSend(opts: {
  service: string;
  entity_id?: string;
  to: string;
  subject?: string;
  body: string;
}): Promise<{ ok: boolean }> {
  return coreInvoke('openhuman.inbox_send', opts);
}

/** Mark a message as read. */
export async function inboxMarkRead(id: string): Promise<void> {
  await coreInvoke('openhuman.inbox_mark_read', { id });
}

/** Trigger an immediate inbox refresh from all connected services. */
export async function inboxRefresh(): Promise<{ ok: boolean; unread: number }> {
  return coreInvoke('openhuman.inbox_refresh');
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// File drag-drop  (Tauri only — provides full OS paths)
// ---------------------------------------------------------------------------

/**
 * Subscribe to OS file/folder drag-drop events anywhere in the window.
 * Uses Tauri v2 Window.onDragDropEvent() API.
 * Returns an unlisten function to call on cleanup.
 * Falls back to a no-op outside Tauri (browser dev mode).
 */
export async function listenFileDrop(
  handler: (paths: string[]) => void,
  onEnter?: () => void,
  onLeave?: () => void,
): Promise<() => void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    const unlisten = await win.onDragDropEvent((event) => {
      const type = event.payload.type;
      if (type === 'enter') {
        onEnter?.();
      } else if (type === 'leave') {
        onLeave?.();
      } else if (type === 'drop') {
        onLeave?.();
        // Tauri v2: payload.paths on drop events
        const paths: string[] = (event.payload as { type: string; paths?: string[] }).paths ?? [];
        if (paths.length > 0) handler(paths);
      }
    });
    return unlisten;
  } catch {
    return () => {};
  }
}

// ---------------------------------------------------------------------------
// Privacy
// ---------------------------------------------------------------------------

/** Erase all user data (conversations, memories, tasks, knowledge, inbox).
 *  API keys and app settings are preserved. */
export async function resetAllData(): Promise<{ ok: boolean }> {
  return coreInvoke('openhuman.data_reset');
}

// ---------------------------------------------------------------------------

/** Execute a shell command. Returns stdout + stderr combined. */
export async function toolsShellExec(
  cmd: string,
  cwd?: string,
  timeoutMs?: number,
): Promise<{ output: string }> {
  return coreInvoke('openhuman.tools_shell_exec', {
    cmd,
    ...(cwd ? { cwd } : {}),
    ...(timeoutMs !== undefined ? { timeout_ms: timeoutMs } : {}),
  });
}

// ---------------------------------------------------------------------------
// Embedding configuration
// ---------------------------------------------------------------------------

export interface EmbeddingConfig {
  provider: string;
  model: string;
  base_url: string;
}

export interface EmbeddingTestResult {
  ok: boolean;
  dims?: number;
  provider?: string;
  model?: string;
  error?: string;
}

/** Get the current embedding provider config. */
export async function embeddingConfigGet(): Promise<EmbeddingConfig> {
  return coreInvoke('openhuman.embedding_config_get');
}

/** Persist embedding provider config and trigger background re-embedding. */
export async function embeddingConfigSet(config: {
  provider: string;
  model: string;
  base_url?: string;
  api_key?: string;
}): Promise<void> {
  await coreInvoke('openhuman.embedding_config_set', config);
}

/** Test the current embedding config. Returns dimension count on success. */
export async function embeddingTest(): Promise<EmbeddingTestResult> {
  return coreInvoke('openhuman.embedding_test');
}

// ---------------------------------------------------------------------------
// Meetings
// ---------------------------------------------------------------------------

export interface MeetingMeta {
  id: string;
  title: string;
  date: string;
  duration_secs: number;
  attendees: string[];
  tags: string[];
  file_name: string;
  template: string;
  lang: string;
}

export interface MeetingDetail {
  meta: MeetingMeta;
  body: string;
}

export async function meetingsList(): Promise<MeetingMeta[]> {
  const res = await coreInvoke<MeetingMeta[]>('meetings.list');
  return Array.isArray(res) ? res : [];
}

export async function meetingsGet(id: string): Promise<MeetingDetail> {
  return coreInvoke('meetings.get', { id });
}

export async function meetingsDelete(id: string): Promise<void> {
  await coreInvoke('meetings.delete', { id });
}

export async function meetingsSearch(query: string): Promise<MeetingMeta[]> {
  const res = await coreInvoke<MeetingMeta[]>('meetings.search', { query });
  return Array.isArray(res) ? res : [];
}

export interface MeetingSaveParams {
  transcript: string;
  title?: string;
  attendees?: string[];
  tags?: string[];
  duration_secs?: number;
  template?: string;
  lang?: string;
}

export interface CalendarHint {
  title: string | null;
  attendees: string[];
  meet_link?: string | null;
}

export async function meetingsCalendarHint(): Promise<CalendarHint> {
  return coreInvoke('meetings.calendar_hint');
}

export async function meetingsShare(params: {
  id: string;
  target: 'slack' | 'notion' | 'github' | 'jira';
  channel?: string;
  repo?: string;
  project?: string;
}): Promise<{ ok: boolean }> {
  return coreInvoke('meetings.share', params);
}

export async function meetingsSave(params: MeetingSaveParams): Promise<{ id: string; meta: MeetingMeta }> {
  return coreInvoke('meetings.save', params);
}

export async function meetingsUpdate(
  id: string,
  updates: { title?: string; tags?: string[]; attendees?: string[] }
): Promise<MeetingMeta> {
  return coreInvoke('meetings.update', { id, ...updates });
}

export async function meetingsExport(id: string): Promise<{ filename: string; content: string }> {
  return coreInvoke('meetings.export', { id });
}

export async function meetingsUpdateBody(id: string, body: string): Promise<void> {
  await coreInvoke('meetings.update_body', { id, body });
}

// ---------------------------------------------------------------------------
// License
// ---------------------------------------------------------------------------

export interface LicenseStatus {
  valid: boolean;
  status: 'active' | 'trial' | 'grace' | 'expired' | 'inactive';
  key: string | null;
  plan: string | null;
  expires_at: string | null;
  org: string | null;
  grace_hours_left: number | null;
  trial_days_left: number | null;
  machine_id: string;
}

/** Returns current license state (local check — no network call). */
export async function licenseStatus(): Promise<LicenseStatus> {
  return coreInvoke('license.status');
}

/** Activate with a license key. Optionally pass a custom server URL. */
export async function licenseActivate(
  key: string,
  serverUrl?: string,
): Promise<{ ok: boolean; error?: string }> {
  return coreInvoke('license.activate', {
    key,
    ...(serverUrl ? { server_url: serverUrl } : {}),
  });
}

/** Start a 30-day free trial. Returns error if trial already used. */
export async function licenseStartTrial(): Promise<{ ok: boolean; expires_at?: string; error?: string }> {
  return coreInvoke('license.start_trial');
}

/** Deactivate — removes license from this machine (notifies server). */
export async function licenseDeactivate(): Promise<{ ok: boolean }> {
  return coreInvoke('license.deactivate');
}

/** Persist the licensing server URL. */
export async function licenseServerUrlSet(url: string): Promise<void> {
  await coreInvoke('license.server_url_set', { url });
}

/** Get the raw (hashed) machine ID for support purposes. */
export async function licenseMachineId(): Promise<string> {
  const res = await coreInvoke<{ machine_id: string }>('license.machine_id');
  return res.machine_id ?? '';
}

export async function meetingsDiarize(params: {
  audio_b64: string;
  format: string;
  attendees?: string[];
  lang?: string;
}): Promise<{ transcript: string; speakers: number }> {
  return coreInvoke('meetings.transcribe_diarize', params);
}
