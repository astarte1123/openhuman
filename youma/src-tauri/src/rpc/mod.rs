//! RPC dispatcher — routes frontend calls to domain handlers.
//!
//! # API Contract v1.0
//! All responses include `api_version: "1.0"`.
//! Success: `{ "data": <payload>, "api_version": "1.0" }`
//! Error:   `{ "error": { "code": "CODE", "message": "..." }, "api_version": "1.0" }`
//!
//! # Breaking change policy
//! - Non-breaking additions (new response fields) allowed without version bump.
//! - Method removal requires deprecation: add `"deprecated": true` for one release first.
//! - Breaking changes require api_version increment to "2.0".

mod composio_handlers;
mod inbox_handlers;
mod knowledge_handlers;
mod license_handlers;
mod meetings_handlers;
mod messages;
mod system;
mod threads;
mod tools_handlers;
mod web_tools;

use crate::{ai, db};
use serde_json::{json, Value};

// ── Standard response helpers ──────────────────────────────────────────────
// All RPC responses include api_version for client-side compatibility checks.
// Success: { "data": <payload>, "api_version": "1.0" }
// Error:   { "error": { "code": "...", "message": "..." }, "api_version": "1.0" }

const API_VERSION: &str = "1.0";

pub(crate) fn ok(data: serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "data": data, "api_version": API_VERSION }))
}

#[allow(dead_code)]
pub(crate) fn err_not_found(msg: impl std::fmt::Display) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "error": { "code": "NOT_FOUND", "message": msg.to_string() },
        "api_version": API_VERSION
    }))
}

pub(crate) fn err_invalid(msg: impl std::fmt::Display) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "error": { "code": "INVALID_INPUT", "message": msg.to_string() },
        "api_version": API_VERSION
    }))
}

#[allow(dead_code)]
pub(crate) fn err_internal(msg: impl std::fmt::Display) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "error": { "code": "INTERNAL_ERROR", "message": msg.to_string() },
        "api_version": API_VERSION
    }))
}

// TODO: migrate all handlers to use ok()/err_*() helpers (API standardization)
// Pattern: ok(payload) for success, err_invalid/err_not_found/err_internal for errors

// Context management constants.
// Keep last N message turns verbatim; older turns are compressed into a summary.
const HISTORY_KEEP: usize = 30;
// Regenerate the thread summary when older-message count grew by this many.
const SUMMARY_REGEN_THRESHOLD: i64 = 8;
// Trigger per-thread knowledge extraction every N new messages.
const REFLECTION_TRIGGER: i64 = 10;
// Top K most relevant memories to inject (keyword-overlap scored).
const MEMORY_TOP_K: usize = 8;
// Max chars budget for memory section in system prompt.
const MEMORY_BUDGET: usize = 2_000;
// Max chars budget for RAG knowledge section.
const RAG_BUDGET: usize = 6_000;

/// Dispatch a JSON-RPC-style method call and return the result value.
///
/// On success the value is forwarded directly as the Tauri response body.
/// On error the string is surfaced as a Tauri command error.
pub async fn dispatch(method: &str, params: Value) -> Result<Value, String> {
    eprintln!("[rpc] dispatch method={method}");

    match method {
        // ------------------------------------------------------------------
        // Core diagnostics
        // ------------------------------------------------------------------
        "core.ping" => ok(json!({ "ok": true })),

        // ------------------------------------------------------------------
        // App state snapshot
        // ------------------------------------------------------------------
        "openhuman.app_state_snapshot" => ok(json!({
            "version": "0.1.0-youma",
            "core_ready": true,
            "user": null,
        })),

        // ------------------------------------------------------------------
        // Threads domain
        // ------------------------------------------------------------------
        "openhuman.threads_list"
        | "openhuman.threads_create_new"
        | "openhuman.threads_delete"
        | "openhuman.threads_rename" => threads::handle(method, &params),

        // ------------------------------------------------------------------
        // Messages domain
        // ------------------------------------------------------------------
        "openhuman.threads_messages_list"
        | "openhuman.threads_message_append"
        | "openhuman.agent_chat" => messages::handle(method, &params).await,

        // ------------------------------------------------------------------
        // Web tools domain
        // ------------------------------------------------------------------
        "openhuman.web_search" | "openhuman.web_fetch" => {
            web_tools::handle(method, &params).await
        }

        // ------------------------------------------------------------------
        // Filesystem + shell tools domain
        // ------------------------------------------------------------------
        "openhuman.tools_fs_read"
        | "openhuman.tools_fs_write"
        | "openhuman.tools_fs_list"
        | "openhuman.tools_fs_grep"
        | "openhuman.tools_shell_exec" => tools_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // Composio + channels domain
        // ------------------------------------------------------------------
        "openhuman.composio_apps_list"
        | "openhuman.composio_connections_list"
        | "openhuman.composio_connect"
        | "openhuman.composio_poll"
        | "openhuman.composio_disconnect"
        | "openhuman.composio_execute"
        | "openhuman.channels_list" => composio_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // Knowledge base domain
        // ------------------------------------------------------------------
        "openhuman.knowledge_chunks_graph"
        | "openhuman.knowledge_chunk_get"
        | "openhuman.knowledge_chunks_list"
        | "openhuman.knowledge_tree_nodes"
        | "openhuman.knowledge_rebuild"
        | "openhuman.knowledge_vault_sync"
        | "openhuman.knowledge_vault_path_get"
        | "openhuman.knowledge_vault_path_set"
        | "openhuman.knowledge_ingest_text"
        | "openhuman.knowledge_ingest_file"
        | "openhuman.knowledge_sources_list"
        | "openhuman.knowledge_source_delete"
        | "openhuman.knowledge_chunk_delete" => knowledge_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // Inbox domain
        // ------------------------------------------------------------------
        "openhuman.inbox_list"
        | "openhuman.inbox_send"
        | "openhuman.inbox_mark_read"
        | "openhuman.inbox_refresh" => inbox_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // Meetings domain
        // ------------------------------------------------------------------
        "meetings.list"
        | "meetings.get"
        | "meetings.delete"
        | "meetings.search"
        | "meetings.save"
        | "meetings.update"
        | "meetings.export"
        | "meetings.update_body"
        | "meetings.calendar_hint"
        | "meetings.share"
        | "meetings.transcribe_diarize" => meetings_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // License domain
        // ------------------------------------------------------------------
        "license.status"
        | "license.activate"
        | "license.start_trial"
        | "license.deactivate"
        | "license.server_url_set"
        | "license.machine_id" => license_handlers::handle(method, &params).await,

        // ------------------------------------------------------------------
        // System / config / auth / memories / tasks / skills / settings /
        // invites / rewards / privacy / embeddings — and unknown fallback
        // ------------------------------------------------------------------
        _ => system::handle(method, &params).await,
    }
}

// ---------------------------------------------------------------------------
// Shared helpers — used by messages.rs and called from background tasks
// ---------------------------------------------------------------------------

/// Score how relevant a memory string is to the query using keyword overlap.
/// Returns count of shared lowercase words (≥4 chars) to avoid noise words.
fn memory_relevance_score(memory: &str, query: &str) -> usize {
    let query_words: std::collections::HashSet<&str> = query
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 4)
        .collect();
    if query_words.is_empty() { return 0; }
    memory
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 4 && query_words.contains(w))
        .count()
}

/// Build a rich RAG query by combining the latest user message with recent context.
/// Uses last 3 user turns so referential queries ("dimmi di più", "e quello?") resolve correctly.
pub(super) fn build_rag_query(ctx: &[(String, String)], new_msg: &str) -> String {
    let mut parts: Vec<&str> = ctx.iter()
        .rev()
        .filter(|(role, _)| role == "user")
        .take(3)
        .map(|(_, c)| c.as_str())
        .collect();
    parts.reverse();
    parts.push(new_msg);
    let combined = parts.join(" ");
    // Truncate to 800 chars — enough context without flooding the embedding model.
    combined.chars().take(800).collect()
}

/// Return a windowed history for a thread:
/// - Last HISTORY_KEEP messages as JSON Value vec (ready for LLM)
/// - Last HISTORY_KEEP messages as raw (role, content) pairs (for RAG query building)
///
/// If the thread has more than HISTORY_KEEP messages, older turns are compressed into
/// a summary which is cached in the thread row and prepended as a context message.
pub(super) async fn get_history_window(thread_id: &str) -> (Vec<Value>, Vec<(String, String)>) {
    let all = match db::messages_list_raw(thread_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[ctx] get_history_window db error: {e}");
            return (Vec::new(), Vec::new());
        }
    };

    let total = all.len();
    eprintln!("[ctx] thread={thread_id} total_msgs={total} keep={HISTORY_KEEP}");

    if total <= HISTORY_KEEP {
        // Short thread: use everything verbatim.
        let msgs: Vec<Value> = all.iter()
            .map(|(role, content)| json!({ "role": role, "content": content }))
            .collect();
        return (msgs, all);
    }

    // Long thread: last HISTORY_KEEP verbatim + summary of older turns.
    let older_count = (total - HISTORY_KEEP) as i64;
    let recent = &all[total - HISTORY_KEEP..];

    // Try cached summary; regenerate if older message count grew significantly.
    let summary = match db::thread_get_summary(thread_id) {
        Some((cached, made_at_count)) if older_count - made_at_count < SUMMARY_REGEN_THRESHOLD => {
            eprintln!("[ctx] using cached summary (older={older_count} made_at={made_at_count})");
            cached
        }
        _ => {
            eprintln!("[ctx] generating new summary older_count={older_count}");
            let older = &all[..total - HISTORY_KEEP];
            let input: String = older.iter()
                .map(|(role, content)| format!("{role}: {content}"))
                .collect::<Vec<_>>()
                .join("\n");

            let sys = "You are a conversation summarizer. \
Produce a dense, factual summary of the conversation history below. \
Preserve: key decisions, facts the user stated, topics covered, any unresolved questions. \
Maximum 400 words. No preamble.";

            let prompt = vec![json!({ "role": "user", "content": format!("Summarize:\n{input}") })];
            let config = ai::load_config();
            match ai::chat_completion(prompt, sys, &config).await {
                Ok(s) => {
                    let _ = db::thread_set_summary(thread_id, &s, older_count);
                    s
                }
                Err(e) => {
                    eprintln!("[ctx] summary generation failed: {e}");
                    // Fallback: use truncated raw older messages.
                    let raw: String = older.iter().rev().take(5)
                        .map(|(r, c)| format!("{r}: {}", &c[..c.len().min(200)]))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("[Earlier conversation (compressed):\n{raw}]")
                }
            }
        }
    };

    // Prepend summary as a synthetic user/assistant exchange so all providers handle it.
    let mut msgs: Vec<Value> = vec![
        json!({ "role": "user", "content": format!("[Earlier conversation summary]\n{summary}") }),
        json!({ "role": "assistant", "content": "Understood. I have context from the earlier conversation." }),
    ];
    for (role, content) in recent {
        msgs.push(json!({ "role": role, "content": content }));
    }

    (msgs, recent.to_vec())
}

/// Build the system prompt with four structured sections:
///   1. Identity + date
///   2. Relevant memories (relevance-ranked, budgeted)
///   3. Knowledge base chunks (RAG — hybrid semantic + FTS5)
///   4. Connected services + operative instructions
pub(super) async fn build_system_prompt(rag_query: &str, ctx: &[(String, String)]) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    // --- 1. Memories — ranked by relevance to rag_query, budgeted ---
    let memory_section = {
        let raw = db::memories_list_raw().unwrap_or_default();
        if raw.is_empty() {
            String::new()
        } else {
            let mut scored: Vec<(usize, &str)> = raw.iter()
                .map(|(_, content)| (memory_relevance_score(content, rag_query), content.as_str()))
                .collect();
            // Sort: relevant first, then preserve recency (already DESC from DB).
            scored.sort_by(|a, b| b.0.cmp(&a.0));

            let mut lines: Vec<String> = Vec::new();
            let mut budget = MEMORY_BUDGET;
            for (_, content) in scored.iter().take(MEMORY_TOP_K) {
                if budget == 0 { break; }
                let snippet: String = content.chars().take(budget.min(300)).collect();
                budget = budget.saturating_sub(snippet.len() + 3);
                lines.push(format!("- {snippet}"));
            }
            if lines.is_empty() {
                String::new()
            } else {
                format!("\n\n## About the user (memories)\n{}", lines.join("\n"))
            }
        }
    };

    // --- Recent meetings context (last 3) ---
    let meetings_section = {
        let recent = crate::meetings::store::get_recent_summaries(3);
        if recent.is_empty() {
            String::new()
        } else {
            let lines: Vec<String> = recent.iter().map(|(title, date, snippet)| {
                let day = date.get(..10).unwrap_or(date.as_str());
                let preview: String = snippet.chars().take(200).collect();
                format!("- **{}** ({}): {}", title, day, preview)
            }).collect();
            format!("\n\n## Recent meetings\n{}", lines.join("\n"))
        }
    };

    // --- 2. RAG — hybrid semantic + FTS5 search ---
    let rag_section = {
        let embed_config = crate::embeddings::load_embedding_config();
        let chunks: Vec<serde_json::Value> = if !rag_query.is_empty() {
            if embed_config.is_configured() {
                match crate::embeddings::generate_embedding(rag_query, &embed_config).await {
                    Ok(q_emb) => {
                        let sem = db::knowledge_chunks_search_semantic(&q_emb, 8).unwrap_or_default();
                        if sem.is_empty() {
                            eprintln!("[rpc] RAG: semantic→0, fallback FTS5");
                            db::knowledge_chunks_search(rag_query, 8).unwrap_or_default()
                        } else {
                            eprintln!("[rpc] RAG: semantic→{} chunks", sem.len());
                            sem
                        }
                    }
                    Err(e) => {
                        eprintln!("[rpc] RAG: embedding failed ({e}), FTS5");
                        db::knowledge_chunks_search(rag_query, 8).unwrap_or_default()
                    }
                }
            } else {
                db::knowledge_chunks_search(rag_query, 8).unwrap_or_default()
            }
        } else {
            Vec::new()
        };

        let mut parts: Vec<String> = Vec::new();
        let mut budget = RAG_BUDGET;

        for chunk in &chunks {
            if budget == 0 { break; }
            let title   = chunk.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let content = chunk.get("content_md").and_then(|v| v.as_str()).unwrap_or("");
            let topic   = chunk.get("topic_group").and_then(|v| v.as_str()).unwrap_or("");
            if content.is_empty() { continue; }

            let cap = budget.min(1_500);
            let snippet = if content.len() <= cap {
                content.to_string()
            } else {
                let slice = &content[..cap];
                let cut = slice.rfind(|c| matches!(c, '.' | '!' | '?' | '\n')).unwrap_or(cap - 1);
                content[..=cut].to_string()
            };
            let heading = if title.is_empty() { topic.to_string() } else { title.to_string() };
            budget = budget.saturating_sub(snippet.len() + heading.len() + 10);
            parts.push(format!("### {heading}\n{snippet}"));
        }

        if parts.is_empty() {
            String::new()
        } else {
            eprintln!("[rpc] RAG: injecting {} chunks", parts.len());
            format!(
                "\n\n## Knowledge base (authoritative — answer from this first)\n{}",
                parts.join("\n\n")
            )
        }
    };

    // --- 3. Connected services ---
    let services_section = {
        let active: Vec<String> = db::composio_active_connections()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                c.get("app_display_name").or_else(|| c.get("app_name"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .collect();
        if active.is_empty() {
            String::new()
        } else {
            format!("\n\n## Connected services\n{} (data auto-fetched every 20 min).", active.join(", "))
        }
    };

    // Detect conversation language hint from recent context.
    let lang_hint = ctx.iter().rev()
        .find(|(role, _)| role == "user")
        .map(|(_, c)| {
            // Simple heuristic: if message contains common Italian words, hint Italian.
            let lower = c.to_lowercase();
            if lower.split_whitespace().any(|w| matches!(w, "che"|"non"|"per"|"una"|"del"|"con"|"sono"|"hai"|"come"|"cosa"|"questo"|"quello")) {
                " Rispondi nella stessa lingua dell'utente (probabilmente italiano)."
            } else {
                " Respond in the same language the user writes in."
            }
        })
        .unwrap_or(" Respond in the same language the user writes in.");

    format!(
        "You are Youma, a sharp and helpful personal AI assistant. Today is {now}.{lang_hint}\n\
\n\
INSTRUCTIONS:\n\
- When a Knowledge base section appears below, treat it as authoritative ground truth.\n\
  Answer from it first, quoting or paraphrasing directly. Only fall back to general knowledge\n\
  when the knowledge base does not cover the question.\n\
- When a Memories section appears, use it to personalize your answers to this specific user.\n\
- Use tools proactively when they would improve the answer:\n\
  web_search (DuckDuckGo), web_fetch (scrape any URL),\n\
  fs_read / fs_write / fs_list / fs_grep (filesystem),\n\
  shell_exec (git, cargo, npm, python, etc.),\n\
  meetings_search / meetings_get (search and read past meeting records),\n\
  create_word_doc (generate a .docx and save to Downloads — use when user asks to create/write/export a Word document),\n\
  create_excel_doc (generate a .xlsx and save to Downloads — use when user asks to create/write/export an Excel or spreadsheet),\n\
  open_file (open a saved file with the default system app — offer this after creating a document).\
- When the user asks to create a Word or Excel document, ALWAYS call the appropriate tool immediately.\n\
  Do NOT ask for confirmation — just create the file and report the saved path.\n\
  If specific content is not provided, generate reasonable placeholder content based on the title or context.\
{memory_section}{meetings_section}{rag_section}{services_section}"
    )
}

pub(super) async fn get_ai_reply_for_thread(thread_id: &str, task_type: &str) -> Result<String, String> {
    let (messages, ctx_raw) = get_history_window(thread_id).await;

    let last_user_msg = ctx_raw.iter().rev()
        .find(|(role, _)| role == "user")
        .map(|(_, c)| c.as_str())
        .unwrap_or("");
    let rag_query = build_rag_query(&ctx_raw, last_user_msg);
    let system = build_system_prompt(&rag_query, &ctx_raw).await;

    eprintln!(
        "[rpc] get_ai_reply_for_thread thread_id={thread_id} task={task_type} \
         history_len={} system_len={}",
        messages.len(), system.len()
    );

    let config = ai::load_config_for_task(task_type);
    ai::chat_completion(messages, &system, &config).await
}

/// Per-thread knowledge extraction.
/// Triggered after each AI reply; runs if the thread has grown by REFLECTION_TRIGGER
/// messages since the last extraction. Distils the conversation into standalone facts
/// stored as `auto,reflection` memories — higher-quality than raw message chunks.
pub(super) async fn maybe_reflect_thread(thread_id: &str) {
    let msg_count = db::messages_count_for_thread(thread_id);
    let last_reflected = db::thread_get_reflection_count(thread_id);
    let delta = msg_count - last_reflected;

    if delta < REFLECTION_TRIGGER {
        eprintln!("[reflect] thread={thread_id} delta={delta}/{REFLECTION_TRIGGER} — skip");
        return;
    }

    eprintln!("[reflect] thread={thread_id} msg_count={msg_count} — extracting knowledge");

    let msgs = match db::messages_last_n_raw(thread_id, 40) {
        Ok(m) if !m.is_empty() => m,
        _ => { eprintln!("[reflect] thread={thread_id}: no messages"); return; }
    };

    let conversation: String = msgs.iter()
        .map(|(role, content)| {
            let speaker = if role == "assistant" { "Youma" } else { "User" };
            format!("{speaker}: {content}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You are a knowledge extraction agent. \
Read a conversation and extract 2-5 concrete, self-contained facts, decisions, or \
preferences that emerged. Each item must stand alone — no pronouns like 'they' or \
phrases like 'in this conversation'. Write facts in third person about the user \
(e.g. 'User is building a Rust app called Youma', 'User prefers terse responses'). \
Include decisions reached, topics explored in depth, and follow-up items mentioned. \
If nothing is worth retaining, output exactly: NOTHING.";

    let messages = vec![json!({
        "role": "user",
        "content": format!("Conversation:\n{conversation}\n\nExtract persistent facts:")
    })];

    let config = ai::load_config();
    match ai::chat_completion(messages, system, &config).await {
        Ok(reply) if reply.trim() != "NOTHING" && !reply.trim().is_empty() => {
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            let content = format!("[Thread reflection {now}]\n{}", reply.trim());
            let _ = db::memories_create(&content, "auto,reflection");
            db::thread_set_reflection_count(thread_id, msg_count);
            eprintln!("[reflect] thread={thread_id} saved ({} chars)", content.len());
        }
        Ok(_) => {
            // Nothing to extract but advance the count so we don't retry immediately.
            db::thread_set_reflection_count(thread_id, msg_count);
            eprintln!("[reflect] thread={thread_id} — nothing to extract");
        }
        Err(e) => eprintln!("[reflect] thread={thread_id} AI error: {e}"),
    }
}

/// Background reflection: summarise recent activity and save as a memory note.
/// Called periodically by the background tokio task in main.rs.
pub async fn background_reflection() {
    eprintln!("[bg] background_reflection started");

    // Collect the last 20 messages across all threads.
    let recent = db::recent_messages(20).unwrap_or_default();
    if recent.is_empty() {
        eprintln!("[bg] no recent messages — skipping reflection");
        return;
    }

    let summary_input: String = recent.iter()
        .filter_map(|m| {
            let role = m.get("role").and_then(|r| r.as_str())?;
            let content = m.get("content").and_then(|c| c.as_str())?;
            Some(format!("{role}: {content}"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You are a background reflection agent for Youma. \
Your job is to read recent conversation snippets and extract 1-3 concise facts \
or preferences the user revealed about themselves. Output only a short bullet list, \
no preamble. Each bullet is a standalone fact starting with the user's implicit subject \
(e.g. \"Prefers dark mode\", \"Working on a Rust project called Youma\"). \
If there is nothing useful to extract, output exactly: NOTHING.";

    let messages = vec![json!({
        "role": "user",
        "content": format!("Recent conversation:\n{summary_input}\n\nExtract facts:")
    })];

    let config = ai::load_config();
    match ai::chat_completion(messages, system, &config).await {
        Ok(reply) if reply.trim() != "NOTHING" && !reply.trim().is_empty() => {
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            let content = format!("[Auto-reflection {now}] {}", reply.trim());
            let _ = db::memories_create(&content, "auto,reflection");
            eprintln!("[bg] saved reflection memory ({} chars)", content.len());
        }
        Ok(_) => eprintln!("[bg] reflection returned NOTHING — no memory saved"),
        Err(e) => eprintln!("[bg] reflection error: {e}"),
    }
}
