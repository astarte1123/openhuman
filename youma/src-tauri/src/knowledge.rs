// knowledge.rs — Local-first knowledge base for Youma.
//
// Pipeline:
//   ingest_all()   — canonicalize every source (memories, message threads, Composio data)
//                    into ≤3 000-token Markdown chunks, score by information density, and
//                    assign topic groups.  Stored in knowledge_chunks (SQLite).
//
//   build_tree()   — AI-driven 2-level summary tree:
//                    leaf chunks → branch node per topic → root node.
//                    Stored in knowledge_tree_nodes (SQLite).
//
//   sync_vault()   — Write the entire knowledge base as Obsidian-compatible .md files
//                    with YAML frontmatter + [[wikilinks]].  Zero external deps.
//
// Background rebuild: called every 6 h from main.rs.  Vault sync is on-demand (UI).

use crate::{ai, db};
use rayon::prelude::*;
use serde_json::json;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_CHUNK_CHARS: usize   = 1_000; // ≈ 250 tokens — precision sweet spot for retrieval
const CHUNK_OVERLAP_CHARS: usize = 150; // carry last paragraph forward for boundary continuity

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

pub fn estimate_tokens(s: &str) -> usize {
    s.len() / 4
}

// ---------------------------------------------------------------------------
// Internal helpers — chunking
// ---------------------------------------------------------------------------

fn flush_chunk(buf: &str, part: usize, base_title: &str, out: &mut Vec<(String, String)>) {
    let trimmed = buf.trim();
    if trimmed.is_empty() { return; }
    let title = if part == 0 {
        base_title.to_string()
    } else {
        format!("{base_title} (part {})", part + 1)
    };
    out.push((title, trimmed.to_string()));
}

/// Returns the last paragraph of `buf`, capped at CHUNK_OVERLAP_CHARS.
/// Used as a context prefix for the next chunk so boundary concepts aren't lost.
fn last_overlap(buf: &str) -> String {
    let last = buf.trim().rsplit("\n\n").next().unwrap_or("").trim();
    if last.len() <= CHUNK_OVERLAP_CHARS {
        last.to_string()
    } else {
        // Keep the last CHUNK_OVERLAP_CHARS chars (may cut mid-sentence — acceptable)
        last.chars().rev().take(CHUNK_OVERLAP_CHARS).collect::<String>()
            .chars().rev().collect()
    }
}

/// Split `content` into ≤MAX_CHUNK_CHARS chunks on paragraph boundaries with overlap.
/// Each chunk carries the last paragraph of the previous chunk as a context prefix,
/// so concepts that span boundaries remain retrievable from both sides.
fn chunk_text(content: &str, base_title: &str) -> Vec<(String, String)> {
    tracing::debug!("[knowledge] chunk_text len={} chunk_size={}", content.len(), MAX_CHUNK_CHARS);
    let mut out: Vec<(String, String)> = Vec::new();
    let mut buf = String::new();
    let mut overlap = String::new(); // tail of last flushed chunk
    let mut part = 0usize;

    for para in content.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if para.len() > MAX_CHUNK_CHARS {
            // Flush current buffer, then split oversized paragraph on sentences.
            flush_chunk(&buf, part, base_title, &mut out);
            if !buf.is_empty() { overlap = last_overlap(&buf); part += 1; buf.clear(); }

            let mut sent_buf = if overlap.is_empty() {
                String::new()
            } else {
                format!("{overlap}\n\n")
            };

            for sentence in para.split(". ") {
                if sent_buf.len() + sentence.len() + 2 > MAX_CHUNK_CHARS {
                    flush_chunk(&sent_buf, part, base_title, &mut out);
                    if !sent_buf.is_empty() {
                        overlap = last_overlap(&sent_buf);
                        part += 1;
                        sent_buf = if overlap.is_empty() {
                            String::new()
                        } else {
                            format!("{overlap}\n\n")
                        };
                    }
                }
                if !sent_buf.is_empty() { sent_buf.push_str(". "); }
                sent_buf.push_str(sentence);
            }
            buf = sent_buf;
        } else if buf.len() + para.len() + 2 > MAX_CHUNK_CHARS {
            flush_chunk(&buf, part, base_title, &mut out);
            overlap = last_overlap(&buf);
            part += 1;
            buf = if overlap.is_empty() {
                para.to_string()
            } else {
                format!("{overlap}\n\n{para}")
            };
        } else {
            if !buf.is_empty() { buf.push_str("\n\n"); }
            buf.push_str(para);
        }
    }
    flush_chunk(&buf, part, base_title, &mut out);

    if out.is_empty() {
        out.push((base_title.to_string(), content.trim().to_string()));
    }
    out
}

// ---------------------------------------------------------------------------
// Internal helpers — scoring
// ---------------------------------------------------------------------------

/// Score a chunk 0..1:
///   30 % information density  (unique-word ratio × log-length)
///   40 % recency              (exponential decay, 1-week half-life)
///   30 % source weight        (manual > memory > composio > message)
fn score_chunk(content: &str, source_type: &str, created_at: &str) -> f64 {
    // Density
    let words: Vec<&str> = content.split_whitespace().collect();
    let n = words.len() as f64;
    let unique = words.iter().collect::<std::collections::HashSet<_>>().len() as f64;
    let density = if n > 1.0 {
        (unique / n) * ((n + 1.0).ln() / 8.0).min(1.0)
    } else {
        0.1
    };

    // Recency — e^(–hours / 168)
    let hours_old = chrono::Utc::now()
        .signed_duration_since(
            chrono::DateTime::parse_from_rfc3339(created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        )
        .num_hours()
        .max(0) as f64;
    let recency = (-hours_old / 168.0_f64).exp();

    // Source weight
    let weight: f64 = match source_type {
        "manual"   => 0.9,
        "memory"   => 0.8,
        "meeting"  => 0.85,
        "composio" => 0.6,
        _          => 0.5, // message
    };

    (0.30 * density.min(1.0) + 0.40 * recency + 0.30 * weight).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Internal helpers — topic assignment + string utilities
// ---------------------------------------------------------------------------

/// Deterministic topic assignment without embeddings:
///   composio sources  → app name slug
///   tagged memories   → first non-system tag
///   messages          → "conversations"
///   everything else   → "notes"
fn assign_topic(source_type: &str, tags: &str, app_hint: Option<&str>) -> String {
    if source_type == "composio" {
        if let Some(app) = app_hint {
            return slug(app);
        }
    }
    for tag in tags.split(',') {
        let t = tag.trim().to_lowercase();
        if !t.is_empty() && !matches!(t.as_str(), "auto" | "reflection" | "composio") {
            return slug(&t);
        }
    }
    match source_type {
        "message" => "conversations".to_string(),
        _         => "notes".to_string(),
    }
}

/// Short deterministic 12-hex-char ID derived from an arbitrary string via FNV-1a.
fn short_hash(s: &str) -> String {
    let mut h: u64 = 14695981039346656037u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{:012x}", h)
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn topic_label(topic: &str) -> String {
    topic.split('-').map(capitalize).collect::<Vec<_>>().join(" ")
}

fn memory_title(content: &str, topic: &str) -> String {
    let first: String = content
        .trim()
        .trim_start_matches(['#', '-', '*', ' '])
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(60)
        .collect();
    if first.trim().is_empty() {
        format!("{} note", topic_label(topic))
    } else {
        first.trim().to_string()
    }
}

// ---------------------------------------------------------------------------
// ingest_all — canonicalize all sources → knowledge_chunks
// ---------------------------------------------------------------------------

/// Ingest all sources into knowledge_chunks.
/// Clears auto-generated chunks first, then re-processes memories and message threads.
/// Manual chunks (source_type='manual') are never touched.
pub async fn ingest_all() {
    tracing::info!("[knowledge] ingest_all started");

    if let Err(e) = db::knowledge_chunks_clear_auto() {
        tracing::error!("[knowledge] ingest_all: clear_auto error: {e}");
    }

    // ---- 1. Memories (including auto-reflection + composio) ----
    let memories = db::memories_list()
        .ok()
        .and_then(|v| v.get("result").and_then(|r| r.as_array()).cloned())
        .unwrap_or_default();

    let mut memory_rows: Vec<db::ChunkRow> = Vec::new();
    for mem in &memories {
        let id      = mem.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let content = mem.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let tags    = mem.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let created = mem.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() || content.is_empty() { continue; }

        let source_type = if tags.contains("composio") { "composio" } else { "memory" };
        let app_hint: Option<String> = tags.split(',')
            .map(str::trim)
            .find(|t| !matches!(*t, "" | "auto" | "composio" | "reflection"))
            .map(String::from);
        let topic = assign_topic(source_type, tags, app_hint.as_deref());
        let title = memory_title(content, &topic);
        let score = score_chunk(content, source_type, created);

        for (i, (chunk_title, chunk_content)) in chunk_text(content, &title).into_iter().enumerate() {
            let tok = estimate_tokens(&chunk_content);
            memory_rows.push(db::ChunkRow {
                id:          format!("mem-{id}-{i}"),
                source_type: source_type.to_string(),
                source_id:   id.to_string(),
                title:       chunk_title,
                content_md:  chunk_content,
                score,
                token_est:   tok,
                topic_group: topic.clone(),
                tags:        tags.to_string(),
                created_at:  created.to_string(),
            });
        }
    }
    let memory_chunk_count = memory_rows.len();
    if let Err(e) = db::knowledge_chunks_batch_insert(&memory_rows) {
        tracing::error!("[knowledge] ingest_all: memory batch insert error: {e}");
    }

    tracing::info!("[knowledge] ingest_all: {} memories → {memory_chunk_count} chunks ingesting source={:?} chunks={memory_chunk_count}", memories.len(), "memory");

    // ---- 2. Message threads (most recent 50) ----
    let threads = db::threads_list()
        .ok()
        .and_then(|v| v.get("result").and_then(|r| r.as_array()).cloned())
        .unwrap_or_default();

    let mut thread_rows: Vec<db::ChunkRow> = Vec::new();
    let mut thread_count = 0usize;
    for thread in threads.iter().take(50) {
        let thread_id    = thread.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let thread_title = thread.get("title").and_then(|v| v.as_str()).unwrap_or("Conversation");
        if thread_id.is_empty() { continue; }

        let msgs = db::messages_list(thread_id)
            .ok()
            .and_then(|v| v.get("result").and_then(|r| r.as_array()).cloned())
            .unwrap_or_default();
        if msgs.is_empty() { continue; }

        let display_title = if thread_title.is_empty() { "Conversation" } else { thread_title };
        let mut md = format!("# {display_title}\n\n");
        for msg in &msgs {
            let role    = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let speaker = if role == "assistant" { "**Youma**" } else { "**You**" };
            md.push_str(speaker);
            md.push_str(": ");
            md.push_str(content);
            md.push_str("\n\n");
        }

        let created = msgs.first()
            .and_then(|m| m.get("created_at").and_then(|v| v.as_str()))
            .unwrap_or("");
        let score = score_chunk(&md, "message", created);

        for (i, (chunk_title, chunk_content)) in chunk_text(&md, display_title).into_iter().enumerate() {
            let tok = estimate_tokens(&chunk_content);
            thread_rows.push(db::ChunkRow {
                id:          format!("thread-{thread_id}-{i}"),
                source_type: "message".to_string(),
                source_id:   thread_id.to_string(),
                title:       chunk_title,
                content_md:  chunk_content,
                score,
                token_est:   tok,
                topic_group: "conversations".to_string(),
                tags:        String::new(),
                created_at:  created.to_string(),
            });
        }
        thread_count += 1;
    }
    let thread_chunk_count = thread_rows.len();
    if let Err(e) = db::knowledge_chunks_batch_insert(&thread_rows) {
        tracing::error!("[knowledge] ingest_all: thread batch insert error: {e}");
    }

    tracing::info!("[knowledge] ingest_all: {thread_count} threads → {thread_chunk_count} chunks ingesting source={:?} chunks={thread_chunk_count}", "message");

    let total = db::knowledge_chunks_count().unwrap_or(0);
    tracing::info!("[knowledge] ingest_all done — {total} total chunks");
    let _ = db::settings_set("knowledge_last_ingest", &chrono::Utc::now().to_rfc3339());

    embed_all_pending().await;
}

// ---------------------------------------------------------------------------
// Document ingestion — manual source_type='manual' chunks
// ---------------------------------------------------------------------------

/// Ingest a completed meeting into the knowledge base.
/// source_type = "meeting", topic_group = "meetings", idempotent.
pub fn ingest_meeting(
    id: &str,
    title: &str,
    body: &str,
    tags: &[String],
    date: &str,
) -> Result<usize, String> {
    if body.trim().is_empty() { return Ok(0); }
    let tags_str = tags.join(",");
    {
        let conn = db::raw_conn()?;
        conn.execute(
            "DELETE FROM knowledge_fts WHERE chunk_id IN \
             (SELECT id FROM knowledge_chunks WHERE source_type='meeting' AND source_id=?1)",
            rusqlite::params![id],
        ).map_err(|e| format!("[knowledge] meeting fts delete: {e}"))?;
        conn.execute(
            "DELETE FROM knowledge_chunks WHERE source_type='meeting' AND source_id=?1",
            rusqlite::params![id],
        ).map_err(|e| format!("[knowledge] meeting chunks delete: {e}"))?;
    }
    let score = score_chunk(body, "meeting", date);
    let rows: Vec<db::ChunkRow> = chunk_text(body, title)
        .into_iter()
        .enumerate()
        .map(|(i, (chunk_title, chunk_content))| {
            let tok = estimate_tokens(&chunk_content);
            db::ChunkRow {
                id:          format!("meet-{}-{i}", short_hash(id)),
                source_type: "meeting".to_string(),
                source_id:   id.to_string(),
                title:       chunk_title,
                content_md:  chunk_content,
                score,
                token_est:   tok,
                topic_group: "meetings".to_string(),
                tags:        tags_str.clone(),
                created_at:  date.to_string(),
            }
        })
        .collect();
    let count = rows.len();
    db::knowledge_chunks_batch_insert(&rows)?;
    tracing::info!("[knowledge] ingest_meeting: {count} chunks id={id} title='{title}'");
    Ok(count)
}

/// Remove all KB chunks for a meeting (called on meeting delete).
pub fn delete_meeting_chunks(id: &str) -> Result<(), String> {
    let conn = db::raw_conn()?;
    conn.execute(
        "DELETE FROM knowledge_fts WHERE chunk_id IN \
         (SELECT id FROM knowledge_chunks WHERE source_type='meeting' AND source_id=?1)",
        rusqlite::params![id],
    ).map_err(|e| format!("[knowledge] meeting fts delete: {e}"))?;
    conn.execute(
        "DELETE FROM knowledge_chunks WHERE source_type='meeting' AND source_id=?1",
        rusqlite::params![id],
    ).map_err(|e| format!("[knowledge] meeting chunks delete: {e}"))?;
    tracing::debug!("[knowledge] delete_meeting_chunks id={id}");
    Ok(())
}

/// Ingest plain text or Markdown as first-class knowledge chunks.
/// `source_id` should be a stable key (file path, UUID, URL slug).
/// Existing chunks for the same source_id are replaced.
/// Returns number of chunks written.
pub fn ingest_document(
    title: &str,
    content: &str,
    tags: &str,
    source_id: &str,
    topic_override: Option<&str>,
) -> Result<usize, String> {
    if content.trim().is_empty() {
        return Err("document content is empty".to_string());
    }
    // Remove existing chunks + FTS entries for this source (idempotent re-import).
    {
        let conn = db::raw_conn()?;
        conn.execute(
            "DELETE FROM knowledge_fts WHERE chunk_id IN \
             (SELECT id FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1)",
            rusqlite::params![source_id],
        ).map_err(|e| format!("[knowledge] delete old fts: {e}"))?;
        conn.execute(
            "DELETE FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1",
            rusqlite::params![source_id],
        ).map_err(|e| format!("[knowledge] delete old chunks: {e}"))?;
    } // release lock before batch insert

    let now   = chrono::Utc::now().to_rfc3339();
    let topic = topic_override
        .map(String::from)
        .unwrap_or_else(|| assign_topic("manual", tags, None));
    let score = score_chunk(content, "manual", &now);

    let rows: Vec<db::ChunkRow> = chunk_text(content, title)
        .into_iter()
        .enumerate()
        .map(|(i, (chunk_title, chunk_content))| {
            let tok = estimate_tokens(&chunk_content);
            db::ChunkRow {
                id:          format!("doc-{}-{i}", short_hash(source_id)),
                source_type: "manual".to_string(),
                source_id:   source_id.to_string(),
                title:       chunk_title,
                content_md:  chunk_content,
                score,
                token_est:   tok,
                topic_group: topic.clone(),
                tags:        tags.to_string(),
                created_at:  now.clone(),
            }
        })
        .collect();

    let count = rows.len();
    db::knowledge_chunks_batch_insert(&rows)?;
    tracing::info!("[knowledge] ingest_document: {count} chunks title='{title}' source_id={source_id}");
    Ok(count)
}

/// Read a local file or directory and ingest as manual knowledge chunks.
/// L3: skips unchanged files based on mtime+size fingerprint.
/// Returns total chunk count (0 means file was unchanged — already up to date).
pub fn ingest_file(path: &str, tags: &str) -> Result<usize, String> {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        return ingest_directory(path, tags);
    }

    // L3 fingerprint check — skip only if file unchanged AND chunks actually exist.
    // If fingerprint matches but chunks are missing (e.g. previous failed ingest),
    // force re-ingest so the user always gets results from an explicit drop action.
    if let Ok(meta) = std::fs::metadata(p) {
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let fp = format!("{}:{}", mtime, meta.len());
        let fp_match = db::knowledge_file_fingerprint_get(path).as_deref() == Some(fp.as_str());
        let chunk_count = db::knowledge_chunks_count_for_source(path).unwrap_or(0);
        if fp_match && chunk_count > 0 {
            tracing::debug!("[knowledge] ingest_file skip (unchanged, {chunk_count} chunks) {path}");
            return Ok(0);
        }
        if fp_match {
            tracing::warn!("[knowledge] ingest_file: fingerprint match but 0 chunks found — re-ingesting {path}");
        }
        let content = crate::file_extract::extract_text(path)?;
        let title = p.file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string();
        let file_topic = if tags.is_empty() { Some(title.as_str()) } else { None };
        let n = ingest_document(&title, &content, tags, path, file_topic)?;
        let _ = db::knowledge_file_fingerprint_set(path, &fp);
        return Ok(n);
    }

    let content = crate::file_extract::extract_text(path)?;
    let title = p.file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string();
    let file_topic = if tags.is_empty() { Some(title.as_str()) } else { None };
    ingest_document(&title, &content, tags, path, file_topic)
}

const SUPPORTED_EXTS: &[&str] = &[
    "pdf", "docx", "xlsx", "xlsm", "ods",
    "html", "htm", "xhtml", "epub", "rtf",
    "txt", "md", "csv", "json", "yaml", "yml",
    "toml", "rs", "py", "js", "ts", "jsx", "tsx",
    "go", "java", "c", "cpp", "h", "cs", "rb", "sh",
];

fn is_supported(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Walk a directory recursively, extract text in parallel (L4/rayon),
/// then batch-insert all new/changed chunks in one SQLite transaction (L2).
/// L3: files whose mtime+size fingerprint is unchanged are skipped.
pub fn ingest_directory(dir: &str, tags: &str) -> Result<usize, String> {
    tracing::info!("[knowledge] ingest_directory dir={dir}");

    // 1. Collect all supported files + their current fingerprints.
    let all_files: Vec<(std::path::PathBuf, String)> = walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_supported(e.path()))
        .map(|e| {
            let fp = std::fs::metadata(e.path())
                .map(|m| {
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    format!("{}:{}", mtime, m.len())
                })
                .unwrap_or_default();
            (e.into_path(), fp)
        })
        .collect();

    // 2. L3 pre-filter: keep only files whose fingerprint changed OR that have no chunks.
    // Always re-ingest if fingerprint matches but chunks are missing (failed previous ingest).
    let to_process: Vec<(std::path::PathBuf, String)> = all_files
        .into_iter()
        .filter(|(p, fp)| {
            let ps = p.to_string_lossy().to_string();
            if fp.is_empty() { return true; }
            let fp_match = db::knowledge_file_fingerprint_get(&ps).as_deref() == Some(fp.as_str());
            if !fp_match { return true; } // fingerprint changed — process
            // Fingerprint matches — skip only if chunks exist
            let has_chunks = db::knowledge_chunks_count_for_source(&ps).unwrap_or(0) > 0;
            !has_chunks // re-ingest if no chunks despite matching fingerprint
        })
        .collect();

    if to_process.is_empty() {
        tracing::debug!("[knowledge] ingest_directory: all files up to date with chunks present");
        return Ok(0);
    }
    tracing::info!("[knowledge] ingest_directory: {} files need processing", to_process.len());

    // 3. L4: parallel text extraction (CPU-bound).
    let extracted: Vec<(std::path::PathBuf, String, Result<String, String>)> = to_process
        .par_iter()
        .map(|(path, fp)| {
            let ps = path.to_string_lossy().to_string();
            (path.clone(), fp.clone(), crate::file_extract::extract_text(&ps))
        })
        .collect();

    // 4. Delete old chunks+FTS for files we'll re-ingest.
    let successful_paths: Vec<String> = extracted
        .iter()
        .filter_map(|(p, _, r)| {
            if r.is_ok() { Some(p.to_string_lossy().to_string()) } else { None }
        })
        .collect();

    if !successful_paths.is_empty() {
        if let Ok(conn) = db::raw_conn() {
            for src in &successful_paths {
                let _ = conn.execute(
                    "DELETE FROM knowledge_fts WHERE chunk_id IN \
                     (SELECT id FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1)",
                    rusqlite::params![src],
                );
                let _ = conn.execute(
                    "DELETE FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1",
                    rusqlite::params![src],
                );
            }
        }
    } // lock released

    // 5. Build all ChunkRows sequentially (chunking is cheap).
    let now = chrono::Utc::now().to_rfc3339();
    let mut all_rows: Vec<db::ChunkRow> = Vec::new();
    let mut updated_fps: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (file_path, fp, extraction) in extracted {
        let path_str = file_path.to_string_lossy().to_string();
        match extraction {
            Err(e) => {
                tracing::warn!("[knowledge] ingest_directory skip {path_str}: {e}");
                errors.push(format!("{path_str}: {e}"));
            }
            Ok(content) if content.trim().is_empty() => {
                tracing::debug!("[knowledge] ingest_directory skip {path_str}: empty");
            }
            Ok(content) => {
                let title = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&path_str)
                    .to_string();
                let parent_topic: String = file_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .map(slug)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "documents".to_string());
                let topic = if !tags.is_empty() {
                    assign_topic("manual", tags, None)
                } else {
                    parent_topic
                };
                let score = score_chunk(&content, "manual", &now);
                for (i, (chunk_title, chunk_content)) in
                    chunk_text(&content, &title).into_iter().enumerate()
                {
                    let tok = estimate_tokens(&chunk_content);
                    all_rows.push(db::ChunkRow {
                        id:          format!("doc-{}-{i}", short_hash(&path_str)),
                        source_type: "manual".to_string(),
                        source_id:   path_str.clone(),
                        title:       chunk_title,
                        content_md:  chunk_content,
                        score,
                        token_est:   tok,
                        topic_group: topic.clone(),
                        tags:        tags.to_string(),
                        created_at:  now.clone(),
                    });
                }
                if !fp.is_empty() {
                    updated_fps.push((path_str, fp));
                }
            }
        }
    }

    // 6. Single batch insert for all files (L2).
    let total = all_rows.len();
    if !all_rows.is_empty() {
        db::knowledge_chunks_batch_insert(&all_rows)?;
        for (src, fp) in updated_fps {
            let _ = db::knowledge_file_fingerprint_set(&src, &fp);
        }
    }

    if total == 0 && !errors.is_empty() {
        return Err(format!(
            "No files ingested. {} error(s): {}",
            errors.len(),
            errors.first().unwrap_or(&String::new())
        ));
    }

    tracing::info!(
        "[knowledge] ingest_directory done dir={dir} total_chunks={total} errors={}",
        errors.len()
    );
    Ok(total)
}

// ---------------------------------------------------------------------------
// build_tree — AI-driven 2-level summary tree
// ---------------------------------------------------------------------------

/// Rebuild the branch + root summary tree from all knowledge chunks.
///
/// Algorithm:
///   1. Group chunks by topic_group (already sorted by score DESC via db).
///   2. Per group: ask the AI to summarize the top-10 chunks (≤6k chars input).
///      Creates a level-1 branch node.
///   3. Summarize all branch summaries → level-2 root node.
///
/// The tree is fully rebuilt (old branch + root nodes cleared first).
/// Run ingest_all() before this.
pub async fn build_tree() {
    tracing::info!("[knowledge] build_tree started");

    let grouped = match db::knowledge_chunks_by_topic() {
        Ok(g) if !g.is_empty() => g,
        Ok(_) => { tracing::debug!("[knowledge] build_tree: no chunks — skip"); return; }
        Err(e) => { tracing::error!("[knowledge] build_tree db error: {e}"); return; }
    };

    // Clear existing branch (1) and root (2) nodes.
    let _ = db::knowledge_tree_clear_above_level(1);

    let config = ai::load_config();
    let mut branch_node_ids: Vec<String> = Vec::new();
    let mut branch_labels_and_summaries: Vec<(String, String)> = Vec::new();

    for (topic, chunks) in &grouped {
        let label = topic_label(topic);
        let chunk_count = chunks.len();

        // Top-10 highest-scored chunks as summarization input.
        let top_content: String = chunks.iter()
            .take(10)
            .filter_map(|c| c.get("content_md").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let input: String = top_content.chars().take(6_000).collect();

        let system = "You are a knowledge base summarizer. \
Given notes from one topic, write 2-4 sentences of plain prose capturing the key \
facts, patterns, and actionable insights a person would want to recall. \
No bullet points. No headings. No preamble. Start with the summary.";

        let messages = vec![json!({
            "role": "user",
            "content": format!("Topic: {label}\nNotes ({chunk_count} items):\n\n{input}\n\nSummarize:")
        })];

        let summary = match ai::chat_completion(messages, system, &config).await {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            Ok(_)  => format!("{chunk_count} notes about {label}."),
            Err(e) => {
                tracing::error!("[knowledge] build_tree: AI summary error for {topic}: {e}");
                format!("{chunk_count} notes about {label}.")
            }
        };

        let node_id = format!("branch-{topic}");
        let chunk_ids: Vec<String> = chunks.iter()
            .filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let chunk_ids_json = serde_json::to_string(&chunk_ids).unwrap_or_default();

        let _ = db::knowledge_tree_node_upsert(
            &node_id, 1, &label, &summary, "[]", &chunk_ids_json, topic,
        );

        branch_node_ids.push(node_id);
        branch_labels_and_summaries.push((label.clone(), summary));
        tracing::debug!("[knowledge] build_tree: branch '{label}' ({chunk_count} chunks) done");
    }

    // Root node
    if !branch_labels_and_summaries.is_empty() {
        let branch_text: String = branch_labels_and_summaries.iter()
            .map(|(label, summary)| format!("**{label}**: {summary}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let branch_input: String = branch_text.chars().take(4_000).collect();

        let system = "You are summarizing a personal knowledge base. \
Given topic summaries, write 2-3 sentences describing what themes emerge and what \
this person is focused on. Be specific. No preamble. Start directly.";

        let messages = vec![json!({
            "role": "user",
            "content": format!("Topics:\n\n{branch_input}\n\nOverview:")
        })];

        let root_summary = match ai::chat_completion(messages, system, &config).await {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => format!(
                "Knowledge base with {} topics: {}.",
                branch_labels_and_summaries.len(),
                branch_labels_and_summaries.iter()
                    .map(|(l, _)| l.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let child_ids_json = serde_json::to_string(&branch_node_ids).unwrap_or_default();
        let _ = db::knowledge_tree_node_upsert(
            "root", 2, "Your Knowledge Base", &root_summary, &child_ids_json, "[]", "",
        );
        tracing::info!("[knowledge] build_tree: root node → {} branches", branch_node_ids.len());
    }

    let _ = db::settings_set("knowledge_last_build", &chrono::Utc::now().to_rfc3339());
    tracing::info!("[knowledge] build_tree done");
}

// ---------------------------------------------------------------------------
// sync_vault — write Obsidian-compatible .md files
// ---------------------------------------------------------------------------

/// Sync the entire knowledge base to `vault_dir` as Obsidian-compatible Markdown.
///
/// Vault layout:
///   INDEX.md              — root summary + topic table with [[wikilinks]]
///   topics/{slug}.md      — branch summary + links to note files
///   notes/{date}-{slug}-{id}.md — individual chunk files with YAML frontmatter
///
/// Returns the number of files written.
pub fn sync_vault(vault_dir: &str) -> Result<usize, String> {
    tracing::info!("[knowledge] sync_vault → {vault_dir}");

    let vault = std::path::Path::new(vault_dir);
    std::fs::create_dir_all(vault.join("topics"))
        .map_err(|e| format!("[knowledge] create topics/: {e}"))?;
    std::fs::create_dir_all(vault.join("notes"))
        .map_err(|e| format!("[knowledge] create notes/: {e}"))?;

    let grouped = db::knowledge_chunks_by_topic()
        .map_err(|e| format!("[knowledge] sync_vault chunks_by_topic: {e}"))?;

    let nodes: Vec<serde_json::Value> = db::knowledge_tree_nodes_list()
        .map(|v| v.get("result").and_then(|r| r.as_array()).cloned().unwrap_or_default())
        .unwrap_or_default();

    let now_str = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let today   = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut file_count = 0usize;

    // Branch summary look-up
    let branch_summary_of: std::collections::HashMap<String, String> = nodes.iter()
        .filter(|n| n.get("level").and_then(|v| v.as_i64()).unwrap_or(0) == 1)
        .map(|n| {
            let topic   = n.get("topic_group").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let summary = n.get("summary_md").and_then(|v| v.as_str()).unwrap_or("").to_string();
            (topic, summary)
        })
        .collect();

    // ---- Pass 1: write note files, collect wikilinks per topic ----
    let mut topic_links: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for (topic, chunks) in &grouped {
        let links = topic_links.entry(topic.clone()).or_default();

        for chunk in chunks {
            let chunk_id    = chunk.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let title       = chunk.get("title").and_then(|v| v.as_str()).unwrap_or("Note");
            let content     = chunk.get("content_md").and_then(|v| v.as_str()).unwrap_or("");
            let score       = chunk.get("score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let created_at  = chunk.get("created_at").and_then(|v| v.as_str()).unwrap_or(&today);
            let source_type = chunk.get("source_type").and_then(|v| v.as_str()).unwrap_or("note");
            let tags_raw    = chunk.get("tags").and_then(|v| v.as_str()).unwrap_or("");

            let date_prefix: String = created_at.chars().take(10).collect();

            // URL-safe filename: {date}-{title-slug}-{id-suffix}.md
            let title_slug: String = title.chars()
                .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|p| !p.is_empty())
                .take(6)
                .collect::<Vec<_>>()
                .join("-");
            let id_suffix: String = chunk_id.chars().filter(char::is_ascii_alphanumeric).take(6).collect();
            let filename = format!("{date_prefix}-{title_slug}-{id_suffix}.md");

            // Build tag list
            let mut tag_list: Vec<String> = vec![source_type.to_string(), topic.clone()];
            for t in tags_raw.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                tag_list.push(t.to_string());
            }
            tag_list.dedup();
            let tags_yaml = format!("[{}]", tag_list.join(", "));

            let note_md = format!(
                "---\ntitle: \"{title}\"\nsource: {source_type}\ntopic: {topic}\n\
                 score: {score:.2}\ndate: {date_prefix}\ntags: {tags_yaml}\n---\n\n{content}\n"
            );

            std::fs::write(vault.join("notes").join(&filename), &note_md)
                .map_err(|e| format!("[knowledge] write notes/{filename}: {e}"))?;

            let stem: String = filename.chars().take(filename.len().saturating_sub(3)).collect();
            let display: String = title.chars().take(50).collect();
            links.push(format!("[[notes/{stem}|{display}]]"));
            file_count += 1;
        }
    }

    // ---- Pass 2: write topic (branch) files ----
    let mut index_rows: Vec<String> = Vec::new();

    for (topic, chunks) in &grouped {
        let label   = topic_label(topic);
        let count   = chunks.len();
        let summary = branch_summary_of.get(topic.as_str())
            .cloned()
            .unwrap_or_else(|| format!("{count} notes."));

        let notes_section = {
            let links = topic_links.get(topic.as_str()).cloned().unwrap_or_default();
            if links.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n## Notes\n\n{}",
                    links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
                )
            }
        };

        let topic_md = format!(
            "---\ntitle: \"{label}\"\ntopic: {topic}\ncount: {count}\ndate: {today}\ntags: [topic]\n---\n\n\
             # {label}\n\n{summary}{notes_section}\n"
        );

        let topic_filename = format!("{topic}.md");
        std::fs::write(vault.join("topics").join(&topic_filename), &topic_md)
            .map_err(|e| format!("[knowledge] write topics/{topic_filename}: {e}"))?;

        let summary_short: String = summary.chars().take(80).collect();
        index_rows.push(format!("| [[topics/{topic}\\|{label}]] | {count} | {summary_short}… |"));
        file_count += 1;
    }

    // ---- Write INDEX.md ----
    let root_summary = nodes.iter()
        .find(|n| n.get("id").and_then(|v| v.as_str()) == Some("root"))
        .and_then(|n| n.get("summary_md").and_then(|v| v.as_str()))
        .unwrap_or("Your personal knowledge base — rebuilt by Youma.")
        .to_string();

    let table_section = if index_rows.is_empty() {
        "No topics yet. Chat with Youma or connect services to build your knowledge base.".to_string()
    } else {
        format!(
            "| Topic | Notes | Summary |\n|---|---|---|\n{}",
            index_rows.join("\n")
        )
    };

    let index_md = format!(
        "---\ntitle: \"Youma Knowledge Base\"\ndate: {today}\ntags: [index, youma, knowledge-base]\n---\n\n\
         # Youma Knowledge Base\n\n{root_summary}\n\n*Last rebuilt: {now_str}*\n\n\
         ## Topics\n\n{table_section}\n"
    );

    std::fs::write(vault.join("INDEX.md"), &index_md)
        .map_err(|e| format!("[knowledge] write INDEX.md: {e}"))?;
    file_count += 1;

    let _ = db::settings_set("knowledge_last_vault_sync", &chrono::Utc::now().to_rfc3339());
    let _ = db::settings_set("knowledge_vault_files_count", &file_count.to_string());

    tracing::info!("[knowledge] sync_vault done — {file_count} files written");
    Ok(file_count)
}

// ---------------------------------------------------------------------------
// Embedding generation — semantic RAG
// ---------------------------------------------------------------------------

/// Build the text fed to the embedding model.
/// Prepends topic label so the vector captures context, not just raw content.
fn embed_text(title: &str, content: &str, topic_group: &str) -> String {
    if topic_group.is_empty() {
        format!("{title}\n\n{content}")
    } else {
        format!("[{}] {title}\n\n{content}", topic_label(topic_group))
    }
}

/// Generate and store embeddings for all chunks of `source_id` that don't have one.
/// Called after `ingest_document` / `ingest_file` to enable semantic search immediately.
pub async fn embed_source_chunks(source_id: &str) {
    let config = crate::embeddings::load_embedding_config();
    if !config.is_configured() { return; }
    tracing::debug!("[knowledge] embed_source_chunks source={source_id} provider={}", config.provider);

    let chunks = match db::knowledge_chunks_for_source(source_id) {
        Ok(c) => c,
        Err(e) => { tracing::error!("[knowledge] embed_source_chunks: {e}"); return; }
    };

    let mut ok = 0usize;
    for (chunk_id, title, content, topic_group, _source_type) in &chunks {
        if db::embedding_exists(chunk_id) { continue; }
        let text = embed_text(title, content, topic_group);
        match crate::embeddings::generate_embedding(&text, &config).await {
            Ok(emb) => {
                let _ = db::embedding_set(chunk_id, &config.model, &emb);
                crate::vector_index::add(chunk_id, &emb);
                ok += 1;
            }
            Err(e) => tracing::error!("[knowledge] embed chunk={chunk_id}: {e}"),
        }
    }
    if ok > 0 {
        tracing::info!("[knowledge] embed_source_chunks: {ok}/{} chunks embedded", chunks.len());
    }
}

/// Generate embeddings for ALL chunks that don't have one yet.
/// Run on startup (delayed) and after `ingest_all()`.
pub async fn embed_all_pending() {
    let config = crate::embeddings::load_embedding_config();
    if !config.is_configured() {
        tracing::debug!("[knowledge] embed_all_pending: no embedding provider configured — skip");
        return;
    }

    let pending = match db::embeddings_pending() {
        Ok(p) => p,
        Err(e) => { tracing::error!("[knowledge] embed_all_pending: {e}"); return; }
    };
    if pending.is_empty() {
        tracing::debug!("[knowledge] embed_all_pending: all chunks already embedded");
        return;
    }

    tracing::info!("[knowledge] embed_all_pending: {} chunks to embed (provider={})", pending.len(), config.provider);
    let mut ok = 0usize;
    for (chunk_id, title, content, topic_group, _source_type) in &pending {
        let text = embed_text(title, content, topic_group);
        match crate::embeddings::generate_embedding(&text, &config).await {
            Ok(emb) => {
                let _ = db::embedding_set(chunk_id, &config.model, &emb);
                crate::vector_index::add(chunk_id, &emb);
                ok += 1;
            }
            Err(e) => tracing::error!("[knowledge] embed pending chunk={chunk_id}: {e}"),
        }
    }
    tracing::info!("[knowledge] embed_all_pending: {ok}/{} done", pending.len());
}

// ---------------------------------------------------------------------------
// Model change detection — purge stale embeddings when the model changes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- estimate_tokens -------------------------------------------------------

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_proportional() {
        // The implementation is len / 4.
        let s = "a".repeat(100);
        assert_eq!(estimate_tokens(&s), 25);
    }

    #[test]
    fn estimate_tokens_not_divisible() {
        // Integer division: 5 / 4 = 1
        assert_eq!(estimate_tokens("hello"), 1);
    }

    #[test]
    fn estimate_tokens_unicode_uses_byte_len() {
        // Each kanji is 3 UTF-8 bytes; &str::len() returns bytes, not char count.
        // "あいう" = 9 bytes → 9 / 4 = 2
        assert_eq!(estimate_tokens("あいう"), 9 / 4);
    }

    // ---- topic_label -----------------------------------------------------------

    #[test]
    fn topic_label_single_word() {
        assert_eq!(topic_label("notes"), "Notes");
    }

    #[test]
    fn topic_label_hyphenated() {
        assert_eq!(topic_label("hello-world"), "Hello World");
    }

    #[test]
    fn topic_label_empty() {
        assert_eq!(topic_label(""), "");
    }

    // ---- slug (private) --------------------------------------------------------

    #[test]
    fn slug_replaces_special_chars() {
        assert_eq!(slug("Hello World!"), "hello-world");
    }

    #[test]
    fn slug_collapses_consecutive_separators() {
        assert_eq!(slug("a--b"), "a-b");
    }

    #[test]
    fn slug_empty() {
        assert_eq!(slug(""), "");
    }

    // ---- short_hash (private) --------------------------------------------------

    #[test]
    fn short_hash_is_12_hex_chars() {
        let h = short_hash("some-source-id");
        // {:012x} pads to at least 12 hex chars; a u64 can be up to 16 hex chars.
        assert!(
            h.len() >= 12 && h.len() <= 16,
            "short_hash length should be 12-16 hex chars, got {}",
            h.len()
        );
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "short_hash should be hex, got: {h}");
    }

    #[test]
    fn short_hash_deterministic() {
        assert_eq!(short_hash("abc"), short_hash("abc"));
    }

    #[test]
    fn short_hash_differs_for_different_inputs() {
        assert_ne!(short_hash("abc"), short_hash("xyz"));
    }

    // ---- last_overlap (private) ------------------------------------------------

    #[test]
    fn last_overlap_short_paragraph() {
        let buf = "paragraph one\n\nparagraph two";
        // "paragraph two" is 13 chars < CHUNK_OVERLAP_CHARS (150) → returned as-is
        assert_eq!(last_overlap(buf), "paragraph two");
    }

    #[test]
    fn last_overlap_truncates_long_paragraph() {
        // Build a paragraph longer than CHUNK_OVERLAP_CHARS (150).
        let long_para = "x".repeat(300);
        let buf = format!("first\n\n{long_para}");
        let result = last_overlap(&buf);
        assert!(
            result.len() <= CHUNK_OVERLAP_CHARS,
            "overlap must be capped at CHUNK_OVERLAP_CHARS, got {}",
            result.len()
        );
    }

    #[test]
    fn last_overlap_empty_buf() {
        // Empty string → rsplit gives "" → trim is "" → returned as ""
        assert_eq!(last_overlap(""), "");
    }

    // ---- assign_topic (private) ------------------------------------------------

    #[test]
    fn assign_topic_composio_with_app_hint() {
        let topic = assign_topic("composio", "composio,gmail", Some("Gmail"));
        assert_eq!(topic, "gmail");
    }

    #[test]
    fn assign_topic_memory_with_tag() {
        let topic = assign_topic("memory", "work,reflection", None);
        // "reflection" is filtered out, "work" remains
        assert_eq!(topic, "work");
    }

    #[test]
    fn assign_topic_message_no_tags() {
        let topic = assign_topic("message", "", None);
        assert_eq!(topic, "conversations");
    }

    #[test]
    fn assign_topic_manual_no_tags() {
        let topic = assign_topic("manual", "", None);
        assert_eq!(topic, "notes");
    }

    #[test]
    fn assign_topic_filters_system_tags() {
        // Tags are only "auto" and "reflection" → fall through to "notes"
        let topic = assign_topic("memory", "auto,reflection", None);
        assert_eq!(topic, "notes");
    }

    // ---- chunk_text (private) --------------------------------------------------

    #[test]
    fn chunk_text_short_text_single_chunk() {
        let text = "Hello, world! This is a short paragraph.";
        let chunks = chunk_text(text, "Test");
        assert_eq!(chunks.len(), 1, "short text should produce a single chunk");
        assert_eq!(chunks[0].0, "Test", "chunk title should match base_title");
        assert!(chunks[0].1.contains("Hello"), "chunk content should contain original text");
    }

    #[test]
    fn chunk_text_empty_input_returns_one_empty_chunk() {
        // The fallback `out.push((base_title.to_string(), content.trim().to_string()))` fires.
        let chunks = chunk_text("", "Title");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1, "", "empty input → single chunk with empty content");
    }

    #[test]
    fn chunk_text_long_text_multiple_chunks() {
        // MAX_CHUNK_CHARS = 1_000. Build text > 2 000 chars across paragraphs.
        let para = "word ".repeat(50); // 250 chars per paragraph
        let text = (0..10).map(|_| para.as_str()).collect::<Vec<_>>().join("\n\n");
        let chunks = chunk_text(&text, "Long");
        assert!(
            chunks.len() > 1,
            "long text (> MAX_CHUNK_CHARS) should produce multiple chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn chunk_text_no_chunk_exceeds_max_size_plus_overlap() {
        // Chunks may exceed MAX_CHUNK_CHARS by one overlap prefix (up to CHUNK_OVERLAP_CHARS).
        let para = "word ".repeat(50); // 250 chars
        let text = (0..10).map(|_| para.as_str()).collect::<Vec<_>>().join("\n\n");
        let chunks = chunk_text(&text, "Bounds");
        for (i, (_, content)) in chunks.iter().enumerate() {
            assert!(
                content.len() <= MAX_CHUNK_CHARS + CHUNK_OVERLAP_CHARS + 4, // +4 for "\n\n"
                "chunk {i} content length {} exceeds allowed bound",
                content.len()
            );
        }
    }

    #[test]
    fn chunk_text_no_empty_content_chunks() {
        let para = "sentence ".repeat(30);
        let text = (0..8).map(|_| para.as_str()).collect::<Vec<_>>().join("\n\n");
        let chunks = chunk_text(&text, "NoEmpty");
        for (i, (_, content)) in chunks.iter().enumerate() {
            assert!(!content.trim().is_empty(), "chunk {i} should not be empty");
        }
    }

    #[test]
    fn chunk_text_part_suffix_on_continuation_chunks() {
        // Chunks after the first should be titled "Base (part N)".
        let para = "x ".repeat(100); // 200 chars
        let text = (0..8).map(|_| para.as_str()).collect::<Vec<_>>().join("\n\n");
        let chunks = chunk_text(&text, "Article");
        if chunks.len() > 1 {
            let second_title = &chunks[1].0;
            assert!(
                second_title.starts_with("Article (part "),
                "continuation chunk title should carry '(part N)' suffix, got: {second_title}"
            );
        }
    }

    #[test]
    fn chunk_text_oversized_paragraph_is_split_on_sentences() {
        // Single paragraph longer than MAX_CHUNK_CHARS → sentence-level splitting path.
        let long_para = (0..60).map(|i| format!("This is sentence number {i}")).collect::<Vec<_>>().join(". ");
        let chunks = chunk_text(&long_para, "BigPara");
        // Expect at least 1 chunk with non-empty content.
        assert!(!chunks.is_empty(), "oversized paragraph must produce at least one chunk");
        for (_, content) in &chunks {
            assert!(!content.trim().is_empty());
        }
    }
}

/// Check if the configured embedding model changed since last run.
/// If so, delete all stored embeddings so the HNSW index is rebuilt from scratch
/// with the new model's vectors (correct dimensions).
///
/// Called once at startup, before `rebuild_from_db()` and `embed_all_pending()`.
pub async fn check_model_change() {
    let config = crate::embeddings::load_embedding_config();
    if !config.is_configured() {
        return;
    }

    // Stable identifier: "provider/model" — catches both provider and model switches.
    let current_key = format!("{}/{}", config.provider, config.model);

    let stored_key = db::settings_get("embedding_model_active")
        .ok()
        .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_default();

    if stored_key == current_key {
        tracing::debug!("[knowledge] check_model_change: model unchanged ({current_key})");
        return;
    }

    if stored_key.is_empty() {
        // First time — just record the model, no purge needed.
        tracing::info!("[knowledge] check_model_change: first run, recording model={current_key}");
    } else {
        // Model changed — purge all stored embeddings to avoid dimension mismatch.
        tracing::warn!(
            "[knowledge] check_model_change: model changed ({stored_key} → {current_key}), purging embeddings"
        );
        match db::embeddings_clear_all() {
            Ok(n) => tracing::info!("[knowledge] check_model_change: purged {n} stale embeddings"),
            Err(e) => tracing::error!("[knowledge] check_model_change: purge error: {e}"),
        }
        // Reset HNSW index to empty (avoids serving stale vectors during this session).
        match crate::vector_index::rebuild_from_db() {
            Ok(n) => tracing::info!("[knowledge] check_model_change: HNSW rebuilt ({n} vectors after purge)"),
            Err(e) => tracing::error!("[knowledge] check_model_change: HNSW rebuild error: {e}"),
        }
    }

    // Persist current model key so next startup can detect future changes.
    let _ = db::settings_set("embedding_model_active", &current_key);
}
