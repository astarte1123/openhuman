// db.rs — SQLite-backed thread + message store for Youma.
//
// Uses a process-wide OnceLock<Mutex<Connection>> singleton so that
// every Tauri command shares the same open database handle.

use rusqlite::{Connection, params};
use serde_json::json;
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

fn conn() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    DB.get()
        .ok_or_else(|| {
            tracing::error!("[db] database not initialized — call db::init() first");
            "database not initialized — call db::init() first".to_string()
        })?
        .lock()
        .map_err(|e| {
            tracing::error!("[db] mutex poisoned: {e}");
            format!("db mutex poisoned: {e}")
        })
}

/// Public access to the raw connection guard — for use by knowledge.rs
/// when it needs to execute DDL/DML that doesn't warrant a dedicated helper.
pub fn raw_conn() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    conn()
}

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

/// Open (or create) the database at `db_path` and apply schema migrations.
/// Must be called once during Tauri setup before any other db function.
pub fn init(db_path: &str) -> Result<(), String> {
    tracing::info!("[db] opening database at {db_path}");

    let connection = Connection::open(db_path)
        .map_err(|e| format!("[db] failed to open {db_path}: {e}"))?;

    // Enable WAL + performance PRAGMAs.
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL; \
             PRAGMA foreign_keys=ON; \
             PRAGMA synchronous=NORMAL; \
             PRAGMA cache_size=-65536; \
             PRAGMA temp_store=MEMORY; \
             PRAGMA mmap_size=268435456;",
        )
        .map_err(|e| format!("[db] PRAGMA error: {e}"))?;

    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS threads (
               id         TEXT PRIMARY KEY,
               title      TEXT NOT NULL DEFAULT '',
               labels     TEXT NOT NULL DEFAULT '[]',
               created_at TEXT NOT NULL,
               deleted_at TEXT
             );
             CREATE TABLE IF NOT EXISTS messages (
               id         TEXT PRIMARY KEY,
               thread_id  TEXT NOT NULL,
               role       TEXT NOT NULL,
               content    TEXT NOT NULL,
               created_at TEXT NOT NULL,
               FOREIGN KEY(thread_id) REFERENCES threads(id)
             );
             CREATE TABLE IF NOT EXISTS memories (
               id         TEXT PRIMARY KEY,
               content    TEXT NOT NULL,
               tags       TEXT NOT NULL DEFAULT '',
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tasks (
               id         TEXT PRIMARY KEY,
               title      TEXT NOT NULL,
               status     TEXT NOT NULL DEFAULT 'todo',
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS skills (
               id          TEXT PRIMARY KEY,
               name        TEXT NOT NULL,
               description TEXT NOT NULL,
               category    TEXT NOT NULL,
               enabled     INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS app_settings (
               key   TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS composio_connections (
               id               TEXT PRIMARY KEY,
               app_name         TEXT NOT NULL,
               app_display_name TEXT NOT NULL,
               entity_id        TEXT NOT NULL DEFAULT 'default',
               status           TEXT NOT NULL DEFAULT 'INITIATED',
               redirect_url     TEXT,
               last_fetched_at  TEXT,
               created_at       TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS composio_apps_cache (
               name         TEXT PRIMARY KEY,
               display_name TEXT NOT NULL,
               description  TEXT NOT NULL DEFAULT '',
               auth_mode    TEXT NOT NULL DEFAULT 'OAUTH2',
               logo_url     TEXT NOT NULL DEFAULT '',
               cached_at    TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS knowledge_chunks (
               id          TEXT PRIMARY KEY,
               source_type TEXT NOT NULL,
               source_id   TEXT,
               title       TEXT NOT NULL DEFAULT '',
               content_md  TEXT NOT NULL,
               score       REAL NOT NULL DEFAULT 0.5,
               token_est   INTEGER NOT NULL DEFAULT 0,
               topic_group TEXT NOT NULL DEFAULT '',
               tags        TEXT NOT NULL DEFAULT '',
               created_at  TEXT NOT NULL,
               updated_at  TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS knowledge_tree_nodes (
               id          TEXT PRIMARY KEY,
               level       INTEGER NOT NULL DEFAULT 1,
               title       TEXT NOT NULL,
               summary_md  TEXT NOT NULL,
               child_ids   TEXT NOT NULL DEFAULT '[]',
               chunk_ids   TEXT NOT NULL DEFAULT '[]',
               topic_group TEXT NOT NULL DEFAULT '',
               created_at  TEXT NOT NULL,
               updated_at  TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS channel_messages (
               id          TEXT PRIMARY KEY,
               service     TEXT NOT NULL,
               external_id TEXT NOT NULL,
               sender      TEXT NOT NULL DEFAULT '',
               subject     TEXT NOT NULL DEFAULT '',
               body_enc    TEXT NOT NULL,
               received_at TEXT NOT NULL,
               read        INTEGER NOT NULL DEFAULT 0,
               metadata    TEXT NOT NULL DEFAULT '{}'
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_messages_ext
               ON channel_messages(service, external_id);
             CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
               chunk_id UNINDEXED,
               title,
               content_md,
               tokenize='unicode61 remove_diacritics 2'
             );
             CREATE TABLE IF NOT EXISTS knowledge_embeddings (
               chunk_id   TEXT PRIMARY KEY,
               model      TEXT NOT NULL,
               embedding  TEXT NOT NULL,
               created_at TEXT NOT NULL
             );",
        )
        .map_err(|e| format!("[db] schema migration failed: {e}"))?;

    // Additive migrations (ALTER TABLE IF NOT EXISTS not supported — ignore errors).
    // Each ALTER is separate so a pre-existing column only fails its own statement.
    let _ = connection.execute_batch("ALTER TABLE threads ADD COLUMN summary TEXT;");
    let _ = connection.execute_batch("ALTER TABLE threads ADD COLUMN summary_count INTEGER NOT NULL DEFAULT 0;");
    let _ = connection.execute_batch("ALTER TABLE threads ADD COLUMN reflection_count INTEGER NOT NULL DEFAULT 0;");

    // Populate FTS5 index from existing chunks on first migration.
    {
        let fts_n: i64 = connection
            .query_row("SELECT COUNT(*) FROM knowledge_fts", [], |r| r.get(0))
            .unwrap_or(0);
        let kc_n: i64 = connection
            .query_row("SELECT COUNT(*) FROM knowledge_chunks", [], |r| r.get(0))
            .unwrap_or(0);
        if fts_n == 0 && kc_n > 0 {
            tracing::info!("[db] migrating {kc_n} existing chunks into FTS5 index");
            let _ = connection.execute_batch(
                "INSERT INTO knowledge_fts(chunk_id, title, content_md) \
                 SELECT id, title, content_md FROM knowledge_chunks;",
            );
        }
    }

    DB.set(Mutex::new(connection))
        .map_err(|_| "[db] init() called more than once".to_string())?;

    seed_skills();

    tracing::info!("[db] initialised successfully");
    Ok(())
}

// ---------------------------------------------------------------------------
// Threads
// ---------------------------------------------------------------------------

/// Return all non-deleted threads ordered by creation time descending.
pub fn threads_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, labels, created_at \
             FROM threads \
             WHERE deleted_at IS NULL \
             ORDER BY created_at DESC",
        )
        .map_err(|e| format!("[db] threads_list prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("[db] threads_list query: {e}"))?;

    let mut threads = Vec::new();
    for row in rows {
        let (id, title, labels_raw, created_at) =
            row.map_err(|e| format!("[db] threads_list row: {e}"))?;
        let labels: serde_json::Value =
            serde_json::from_str(&labels_raw).unwrap_or(json!([]));
        threads.push(json!({
            "id": id,
            "title": title,
            "labels": labels,
            "created_at": created_at,
        }));
    }

    tracing::debug!("[db] threads_list → {} threads", threads.len());
    Ok(json!({ "result": threads }))
}

/// Insert a new thread and return the created object.
pub fn threads_create(labels: Vec<String>) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let labels_json = serde_json::to_string(&labels)
        .map_err(|e| format!("[db] labels serialize: {e}"))?;

    let conn = conn()?;
    conn.execute(
        "INSERT INTO threads (id, title, labels, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, "", labels_json, created_at],
    )
    .map_err(|e| format!("[db] threads_create insert: {e}"))?;

    tracing::debug!("[db] threads_create → id={id}");
    Ok(json!({
        "id": id,
        "title": "",
        "labels": labels,
        "created_at": created_at,
    }))
}

/// Update a thread's title.
pub fn thread_set_title(thread_id: &str, title: &str) -> Result<(), String> {
    let conn = conn()?;
    conn.execute(
        "UPDATE threads SET title = ?1 WHERE id = ?2",
        params![title, thread_id],
    )
    .map_err(|e| format!("[db] thread_set_title update: {e}"))?;
    tracing::debug!("[db] thread_set_title thread_id={thread_id}");
    Ok(())
}

/// Get the cached conversation summary for a thread.
/// Returns (summary_text, message_count_when_made).
pub fn thread_get_summary(thread_id: &str) -> Option<(String, i64)> {
    let conn = conn().ok()?;
    conn.query_row(
        "SELECT summary, summary_count FROM threads WHERE id = ?1",
        params![thread_id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
    )
    .ok()
    .and_then(|(s, c)| s.map(|text| (text, c)))
}

/// Store a conversation summary with the message count at time of generation.
pub fn thread_set_summary(thread_id: &str, summary: &str, count: i64) -> Result<(), String> {
    let conn = conn()?;
    conn.execute(
        "UPDATE threads SET summary = ?1, summary_count = ?2 WHERE id = ?3",
        params![summary, count, thread_id],
    )
    .map_err(|e| format!("[db] thread_set_summary: {e}"))?;
    tracing::debug!("[db] thread_set_summary thread_id={thread_id} count={count}");
    Ok(())
}

/// How many messages were present the last time we ran per-thread reflection.
pub fn thread_get_reflection_count(thread_id: &str) -> i64 {
    let conn = match conn() { Ok(c) => c, Err(_) => return 0 };
    conn.query_row(
        "SELECT reflection_count FROM threads WHERE id = ?1",
        params![thread_id],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0)
}

/// Persist the message count at which we last reflected on this thread.
pub fn thread_set_reflection_count(thread_id: &str, count: i64) {
    if let Ok(conn) = conn() {
        let _ = conn.execute(
            "UPDATE threads SET reflection_count = ?1 WHERE id = ?2",
            params![count, thread_id],
        );
        tracing::debug!("[db] thread_set_reflection_count thread_id={thread_id} count={count}");
    }
}

/// Total number of messages in a thread (both roles).
pub fn messages_count_for_thread(thread_id: &str) -> i64 {
    let conn = match conn() { Ok(c) => c, Err(_) => return 0 };
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE thread_id = ?1",
        params![thread_id],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0)
}

/// Return all messages in a thread as (role, content) pairs — lightweight version.
pub fn messages_list_raw(thread_id: &str) -> Result<Vec<(String, String)>, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare("SELECT role, content FROM messages WHERE thread_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| format!("[db] messages_list_raw prepare: {e}"))?;
    let rows = stmt
        .query_map(params![thread_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("[db] messages_list_raw query: {e}"))?;
    rows.map(|r| r.map_err(|e| format!("[db] messages_list_raw row: {e}")))
        .collect()
}

/// Return the last `n` messages in a thread as (role, content) pairs.
pub fn messages_last_n_raw(thread_id: &str, n: usize) -> Result<Vec<(String, String)>, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT role, content FROM messages \
             WHERE thread_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| format!("[db] messages_last_n_raw prepare: {e}"))?;
    let rows = stmt
        .query_map(params![thread_id, n as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("[db] messages_last_n_raw query: {e}"))?;
    let mut out: Vec<(String, String)> = rows
        .map(|r| r.map_err(|e| format!("[db] messages_last_n_raw row: {e}")))
        .collect::<Result<_, _>>()?;
    out.reverse(); // restore chronological order
    Ok(out)
}

/// Return all memories as (id, content) pairs for relevance scoring.
pub fn memories_list_raw() -> Result<Vec<(String, String)>, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare("SELECT id, content FROM memories ORDER BY created_at DESC")
        .map_err(|e| format!("[db] memories_list_raw prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("[db] memories_list_raw query: {e}"))?;
    rows.map(|r| r.map_err(|e| format!("[db] memories_list_raw row: {e}")))
        .collect()
}

/// Count messages in a thread.
pub fn messages_count(thread_id: &str) -> Result<i64, String> {
    let conn = conn()?;
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE thread_id = ?1",
            params![thread_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("[db] messages_count: {e}"))?;
    Ok(n)
}

/// Soft-delete a thread by setting deleted_at.
pub fn threads_delete(thread_id: &str, deleted_at: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let affected = conn
        .execute(
            "UPDATE threads SET deleted_at = ?1 WHERE id = ?2",
            params![deleted_at, thread_id],
        )
        .map_err(|e| format!("[db] threads_delete update: {e}"))?;

    tracing::debug!("[db] threads_delete thread_id={thread_id} rows={affected}");
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Return all messages in a thread ordered by creation time ascending.
pub fn messages_list(thread_id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, thread_id, role, content, created_at \
             FROM messages \
             WHERE thread_id = ?1 \
             ORDER BY created_at ASC",
        )
        .map_err(|e| format!("[db] messages_list prepare: {e}"))?;

    let rows = stmt
        .query_map(params![thread_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("[db] messages_list query: {e}"))?;

    let mut messages = Vec::new();
    for row in rows {
        let (id, tid, role, content, created_at) =
            row.map_err(|e| format!("[db] messages_list row: {e}"))?;
        messages.push(json!({
            "id": id,
            "thread_id": tid,
            "role": role,
            "content": content,
            "created_at": created_at,
        }));
    }

    tracing::debug!("[db] messages_list thread_id={thread_id} → {} messages", messages.len());
    Ok(json!({ "result": messages }))
}

/// Return the most recent `limit` messages across ALL threads (for background reflection).
pub fn recent_messages(limit: usize) -> Result<Vec<serde_json::Value>, String> {
    let conn = conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT role, content FROM messages \
             ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("[db] recent_messages prepare: {e}"))?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("[db] recent_messages query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        let (role, content) = row.map_err(|e| format!("[db] recent_messages row: {e}"))?;
        out.push(json!({"role": role, "content": content}));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Skills seed
// ---------------------------------------------------------------------------

fn seed_skills() {
    let conn = match conn() { Ok(c) => c, Err(_) => return };
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM skills", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 { return; }
    let defaults: &[(&str, &str, &str, &str, i32)] = &[
        ("web-search",     "Web Search",          "Search the web for information",                  "tools",        1),
        ("web-fetch",      "Web Fetch",            "Fetch and read content from URLs",                "tools",        1),
        ("voice-input",    "Voice Input",          "Speak to send messages (STT)",                   "voice",        1),
        ("voice-output",   "Voice Output",         "AI replies spoken aloud (TTS)",                  "voice",        1),
        ("memory-notes",   "Memory Notes",         "Save and recall personal notes",                 "intelligence", 1),
        ("task-manager",   "Task Manager",         "Create and track tasks",                         "intelligence", 1),
        ("markdown",       "Markdown Rendering",   "Rich text formatting in chat",                   "interface",    1),
        ("multi-provider", "Multi-Provider AI",    "Anthropic, OpenAI or Ollama backend",            "ai",           1),
    ];
    for (id, name, desc, cat, enabled) in defaults {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO skills (id, name, description, category, enabled) VALUES (?1,?2,?3,?4,?5)",
            params![id, name, desc, cat, enabled],
        );
    }
    tracing::info!("[db] seeded default skills");
}

// ---------------------------------------------------------------------------
// Memories
// ---------------------------------------------------------------------------

pub fn memories_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, content, tags, created_at FROM memories ORDER BY created_at DESC"
    ).map_err(|e| format!("[db] memories_list prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,String>(3)?))
    }).map_err(|e| format!("[db] memories_list query: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        let (id, content, tags, created_at) = row.map_err(|e| format!("[db] memories_list row: {e}"))?;
        items.push(json!({"id":id,"content":content,"tags":tags,"created_at":created_at}));
    }
    tracing::debug!("[db] memories_list → {}", items.len());
    Ok(json!({"result": items}))
}

pub fn memories_create(content: &str, tags: &str) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "INSERT INTO memories (id, content, tags, created_at) VALUES (?1,?2,?3,?4)",
        params![id, content, tags, created_at],
    ).map_err(|e| format!("[db] memories_create: {e}"))?;
    tracing::debug!("[db] memories_create id={id}");
    Ok(json!({"id":id,"content":content,"tags":tags,"created_at":created_at}))
}

pub fn memories_delete(id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("DELETE FROM memories WHERE id = ?1", params![id])
        .map_err(|e| format!("[db] memories_delete: {e}"))?;
    tracing::debug!("[db] memories_delete id={id}");
    Ok(json!({"ok":true}))
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

pub fn tasks_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, status, created_at FROM tasks ORDER BY created_at DESC"
    ).map_err(|e| format!("[db] tasks_list prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,String>(3)?))
    }).map_err(|e| format!("[db] tasks_list query: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        let (id, title, status, created_at) = row.map_err(|e| format!("[db] tasks_list row: {e}"))?;
        items.push(json!({"id":id,"title":title,"status":status,"created_at":created_at}));
    }
    Ok(json!({"result": items}))
}

pub fn tasks_create(title: &str) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "INSERT INTO tasks (id, title, status, created_at) VALUES (?1,?2,'todo',?3)",
        params![id, title, created_at],
    ).map_err(|e| format!("[db] tasks_create: {e}"))?;
    tracing::debug!("[db] tasks_create id={id}");
    Ok(json!({"id":id,"title":title,"status":"todo","created_at":created_at}))
}

pub fn tasks_update(id: &str, status: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("UPDATE tasks SET status=?1 WHERE id=?2", params![status, id])
        .map_err(|e| format!("[db] tasks_update: {e}"))?;
    let row = conn.query_row(
        "SELECT id, title, status, created_at FROM tasks WHERE id=?1",
        params![id],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,String>(3)?))
    ).map_err(|e| format!("[db] tasks_update fetch: {e}"))?;
    Ok(json!({"id":row.0,"title":row.1,"status":row.2,"created_at":row.3}))
}

pub fn tasks_delete(id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("DELETE FROM tasks WHERE id=?1", params![id])
        .map_err(|e| format!("[db] tasks_delete: {e}"))?;
    Ok(json!({"ok":true}))
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

pub fn skills_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, category, enabled FROM skills ORDER BY category, name"
    ).map_err(|e| format!("[db] skills_list prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,String>(3)?, row.get::<_,i32>(4)?))
    }).map_err(|e| format!("[db] skills_list query: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        let (id, name, desc, cat, enabled) = row.map_err(|e| format!("[db] skills_list row: {e}"))?;
        items.push(json!({"id":id,"name":name,"description":desc,"category":cat,"enabled":enabled==1}));
    }
    Ok(json!({"result": items}))
}

pub fn skills_toggle(id: &str, enabled: bool) -> Result<serde_json::Value, String> {
    let v: i32 = if enabled { 1 } else { 0 };
    let conn = conn()?;
    conn.execute("UPDATE skills SET enabled=?1 WHERE id=?2", params![v, id])
        .map_err(|e| format!("[db] skills_toggle: {e}"))?;
    let row = conn.query_row(
        "SELECT id, name, description, category, enabled FROM skills WHERE id=?1",
        params![id],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,String>(3)?, r.get::<_,i32>(4)?))
    ).map_err(|e| format!("[db] skills_toggle fetch: {e}"))?;
    Ok(json!({"id":row.0,"name":row.1,"description":row.2,"category":row.3,"enabled":row.4==1}))
}

// ---------------------------------------------------------------------------
// App settings
// ---------------------------------------------------------------------------

pub fn settings_get(key: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    match conn.query_row("SELECT value FROM app_settings WHERE key=?1", params![key], |r| r.get::<_,String>(0)) {
        Ok(v) => Ok(json!({"value": v})),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(json!({"value": null})),
        Err(e) => Err(format!("[db] settings_get: {e}")),
    }
}

pub fn settings_set(key: &str, value: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1,?2)", params![key, value])
        .map_err(|e| format!("[db] settings_set: {e}"))?;
    Ok(json!({"ok":true}))
}

// ---------------------------------------------------------------------------
// Composio connections
// ---------------------------------------------------------------------------

pub fn composio_connections_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, app_name, app_display_name, entity_id, status, redirect_url, last_fetched_at, created_at \
         FROM composio_connections ORDER BY created_at DESC",
    ).map_err(|e| format!("[db] composio_connections_list prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
        ))
    }).map_err(|e| format!("[db] composio_connections_list query: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        let (id, app_name, app_display_name, entity_id, status, redirect_url, last_fetched_at, created_at) =
            row.map_err(|e| format!("[db] composio_connections_list row: {e}"))?;
        items.push(json!({
            "id": id, "app_name": app_name, "app_display_name": app_display_name,
            "entity_id": entity_id, "status": status, "redirect_url": redirect_url,
            "last_fetched_at": last_fetched_at, "created_at": created_at,
        }));
    }
    tracing::debug!("[db] composio_connections_list → {}", items.len());
    Ok(json!({"result": items}))
}

pub fn composio_connection_upsert(
    id: &str, app_name: &str, app_display_name: &str, entity_id: &str,
    status: &str, redirect_url: Option<&str>,
) -> Result<serde_json::Value, String> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO composio_connections \
         (id, app_name, app_display_name, entity_id, status, redirect_url, created_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, app_name, app_display_name, entity_id, status, redirect_url, created_at],
    ).map_err(|e| format!("[db] composio_connection_upsert: {e}"))?;
    tracing::debug!("[db] composio_connection_upsert id={id} app={app_name} status={status}");
    Ok(json!({"id":id,"app_name":app_name,"app_display_name":app_display_name,
              "entity_id":entity_id,"status":status,"redirect_url":redirect_url,
              "created_at":created_at}))
}

pub fn composio_connection_set_status(id: &str, status: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute(
        "UPDATE composio_connections SET status=?1 WHERE id=?2",
        params![status, id],
    ).map_err(|e| format!("[db] composio_connection_set_status: {e}"))?;
    tracing::debug!("[db] composio_connection_set_status id={id} status={status}");
    Ok(json!({"ok":true}))
}

pub fn composio_connection_set_fetched(id: &str) -> Result<serde_json::Value, String> {
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "UPDATE composio_connections SET last_fetched_at=?1 WHERE id=?2",
        params![fetched_at, id],
    ).map_err(|e| format!("[db] composio_connection_set_fetched: {e}"))?;
    tracing::debug!("[db] composio_connection_set_fetched id={id}");
    Ok(json!({"ok":true}))
}

pub fn composio_connection_delete(id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute(
        "DELETE FROM composio_connections WHERE id=?1",
        params![id],
    ).map_err(|e| format!("[db] composio_connection_delete: {e}"))?;
    tracing::debug!("[db] composio_connection_delete id={id}");
    Ok(json!({"ok":true}))
}

pub fn composio_active_connections() -> Result<Vec<serde_json::Value>, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, app_name, app_display_name, entity_id, status, last_fetched_at \
         FROM composio_connections WHERE status='ACTIVE' ORDER BY app_name",
    ).map_err(|e| format!("[db] composio_active_connections prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    }).map_err(|e| format!("[db] composio_active_connections query: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, app_name, app_display_name, entity_id, status, last_fetched_at) =
            row.map_err(|e| format!("[db] composio_active_connections row: {e}"))?;
        out.push(json!({"id":id,"app_name":app_name,"app_display_name":app_display_name,
                        "entity_id":entity_id,"status":status,"last_fetched_at":last_fetched_at}));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Composio apps cache
// ---------------------------------------------------------------------------

pub fn composio_apps_cache_upsert(apps: &[serde_json::Value]) -> Result<(), String> {
    let cached_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    for app in apps {
        let name = app.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() { continue; }
        let display_name = app.get("display_name").and_then(|v| v.as_str()).unwrap_or(name);
        let description = app.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let auth_mode = app.get("auth_mode").and_then(|v| v.as_str()).unwrap_or("OAUTH2");
        let logo_url = app.get("logo_url").and_then(|v| v.as_str()).unwrap_or("");
        conn.execute(
            "INSERT OR REPLACE INTO composio_apps_cache \
             (name, display_name, description, auth_mode, logo_url, cached_at) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![name, display_name, description, auth_mode, logo_url, cached_at],
        ).map_err(|e| format!("[db] composio_apps_cache_upsert: {e}"))?;
    }
    tracing::debug!("[db] composio_apps_cache_upsert {} apps", apps.len());
    Ok(())
}

pub fn composio_apps_cache_get() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT name, display_name, description, auth_mode, logo_url, cached_at \
         FROM composio_apps_cache ORDER BY display_name",
    ).map_err(|e| format!("[db] composio_apps_cache_get prepare: {e}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    }).map_err(|e| format!("[db] composio_apps_cache_get query: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        let (name, display_name, description, auth_mode, logo_url, cached_at) =
            row.map_err(|e| format!("[db] composio_apps_cache_get row: {e}"))?;
        items.push(json!({"name":name,"display_name":display_name,"description":description,
                          "auth_mode":auth_mode,"logo_url":logo_url,"cached_at":cached_at}));
    }
    tracing::debug!("[db] composio_apps_cache_get → {} apps", items.len());
    Ok(json!({"result": items}))
}

// ---------------------------------------------------------------------------
// Knowledge base — chunks
// ---------------------------------------------------------------------------

/// Flat row for batch-inserting chunks + syncing FTS5.
pub struct ChunkRow {
    pub id:          String,
    pub source_type: String,
    pub source_id:   String,
    pub title:       String,
    pub content_md:  String,
    pub score:       f64,
    pub token_est:   usize,
    pub topic_group: String,
    pub tags:        String,
    pub created_at:  String,
}

/// Batch-insert (or replace) many chunks in a single transaction, keeping FTS5 in sync.
pub fn knowledge_chunks_batch_insert(chunks: &[ChunkRow]) -> Result<(), String> {
    if chunks.is_empty() { return Ok(()); }
    let mut guard = conn()?;
    let updated_at = chrono::Utc::now().to_rfc3339();
    let tx = guard
        .transaction()
        .map_err(|e| format!("[db] batch_insert begin: {e}"))?;
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR REPLACE INTO knowledge_chunks \
                 (id, source_type, source_id, title, content_md, score, token_est, \
                  topic_group, tags, created_at, updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            )
            .map_err(|e| format!("[db] batch_insert prepare: {e}"))?;
        let mut fts = tx
            .prepare(
                "INSERT INTO knowledge_fts(chunk_id, title, content_md) VALUES (?1,?2,?3)",
            )
            .map_err(|e| format!("[db] batch_insert fts_prepare: {e}"))?;
        for c in chunks {
            stmt.execute(params![
                c.id, c.source_type, c.source_id, c.title, c.content_md,
                c.score, c.token_est as i64, c.topic_group, c.tags, c.created_at, updated_at,
            ])
            .map_err(|e| format!("[db] batch_insert execute: {e}"))?;
            fts.execute(params![c.id, c.title, c.content_md])
                .map_err(|e| format!("[db] batch_insert fts_execute: {e}"))?;
        }
    }
    tx.commit()
        .map_err(|e| format!("[db] batch_insert commit: {e}"))?;
    tracing::debug!("[db] knowledge_chunks_batch_insert: {} chunks committed", chunks.len());
    Ok(())
}

/// Insert or replace a single knowledge chunk (kept for one-off use; batch callers use `knowledge_chunks_batch_insert`).
#[allow(dead_code)]
pub fn knowledge_chunk_upsert(
    id: &str,
    source_type: &str,
    source_id: &str,
    title: &str,
    content_md: &str,
    score: f64,
    token_est: usize,
    topic_group: &str,
    tags: &str,
    created_at: &str,
) -> Result<(), String> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_chunks \
         (id, source_type, source_id, title, content_md, score, token_est, \
          topic_group, tags, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            id, source_type, source_id, title, content_md,
            score, token_est as i64, topic_group, tags, created_at, updated_at
        ],
    ).map_err(|e| format!("[db] knowledge_chunk_upsert: {e}"))?;
    // Keep FTS in sync.
    conn.execute("DELETE FROM knowledge_fts WHERE chunk_id = ?1", params![id])
        .map_err(|e| format!("[db] knowledge_chunk_upsert fts_del: {e}"))?;
    conn.execute(
        "INSERT INTO knowledge_fts(chunk_id, title, content_md) VALUES (?1,?2,?3)",
        params![id, title, content_md],
    ).map_err(|e| format!("[db] knowledge_chunk_upsert fts_ins: {e}"))?;
    Ok(())
}

/// Delete all auto-generated chunks (message / memory / composio) before a rebuild.
/// Manual chunks (source_type = 'manual') are preserved.
pub fn knowledge_chunks_clear_auto() -> Result<(), String> {
    let conn = conn()?;
    conn.execute(
        "DELETE FROM knowledge_fts WHERE chunk_id IN \
         (SELECT id FROM knowledge_chunks WHERE source_type IN ('memory','message','composio'))",
        [],
    ).map_err(|e| format!("[db] knowledge_chunks_clear_auto fts: {e}"))?;
    conn.execute(
        "DELETE FROM knowledge_embeddings WHERE chunk_id IN \
         (SELECT id FROM knowledge_chunks WHERE source_type IN ('memory','message','composio'))",
        [],
    ).map_err(|e| format!("[db] knowledge_chunks_clear_auto emb: {e}"))?;
    conn.execute(
        "DELETE FROM knowledge_chunks WHERE source_type IN ('memory','message','composio')",
        [],
    ).map_err(|e| format!("[db] knowledge_chunks_clear_auto: {e}"))?;
    Ok(())
}

/// Count chunks for a specific manual source_id.
pub fn knowledge_chunks_count_for_source(source_id: &str) -> Result<i64, String> {
    let conn = conn()?;
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1",
            params![source_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("[db] knowledge_chunks_count_for_source: {e}"))?;
    Ok(n)
}

/// Count total chunks.
pub fn knowledge_chunks_count() -> Result<i64, String> {
    let conn = conn()?;
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM knowledge_chunks", [], |r| r.get(0))
        .map_err(|e| format!("[db] knowledge_chunks_count: {e}"))?;
    Ok(n)
}

/// Paginated list of chunks; optionally filtered by topic.
pub fn knowledge_chunks_list(
    topic: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut items = Vec::new();

    macro_rules! map_row {
        ($row:expr) => {{
            let child_ids_str: Option<String> = None; // placeholder — not in chunks
            let _ = child_ids_str;
            Ok((
                $row.get::<_, String>(0)?,
                $row.get::<_, String>(1)?,
                $row.get::<_, Option<String>>(2)?,
                $row.get::<_, String>(3)?,
                $row.get::<_, String>(4)?,
                $row.get::<_, f64>(5)?,
                $row.get::<_, i64>(6)?,
                $row.get::<_, String>(7)?,
                $row.get::<_, String>(8)?,
                $row.get::<_, String>(9)?,
            ))
        }};
    }

    let push_row = |items: &mut Vec<serde_json::Value>,
                    id: String, source_type: String, source_id: Option<String>,
                    title: String, content_md: String, score: f64, token_est: i64,
                    topic_group: String, tags: String, created_at: String| {
        items.push(json!({
            "id": id, "source_type": source_type, "source_id": source_id,
            "title": title, "content_md": content_md, "score": score,
            "token_est": token_est, "topic_group": topic_group,
            "tags": tags, "created_at": created_at,
        }));
    };

    if let Some(t) = topic {
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, title, content_md, score, token_est, \
             topic_group, tags, created_at FROM knowledge_chunks \
             WHERE topic_group = ?1 ORDER BY score DESC LIMIT ?2 OFFSET ?3",
        ).map_err(|e| format!("[db] knowledge_chunks_list prepare: {e}"))?;
        let rows = stmt.query_map(params![t, limit as i64, offset as i64], |row| map_row!(row))
            .map_err(|e| format!("[db] knowledge_chunks_list query: {e}"))?;
        for row in rows {
            let (id, st, sid, title, content, score, tok, tg, tags, created) =
                row.map_err(|e| format!("[db] knowledge_chunks_list row: {e}"))?;
            push_row(&mut items, id, st, sid, title, content, score, tok, tg, tags, created);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, title, content_md, score, token_est, \
             topic_group, tags, created_at FROM knowledge_chunks \
             ORDER BY score DESC LIMIT ?1 OFFSET ?2",
        ).map_err(|e| format!("[db] knowledge_chunks_list prepare: {e}"))?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| map_row!(row))
            .map_err(|e| format!("[db] knowledge_chunks_list query: {e}"))?;
        for row in rows {
            let (id, st, sid, title, content, score, tok, tg, tags, created) =
                row.map_err(|e| format!("[db] knowledge_chunks_list row: {e}"))?;
            push_row(&mut items, id, st, sid, title, content, score, tok, tg, tags, created);
        }
    }

    tracing::debug!("[db] knowledge_chunks_list → {} items", items.len());
    Ok(json!({ "result": items }))
}

/// Return chunk metadata for the Wiki graph — no content_md, top N per topic group.
/// Guarantees every document/topic is represented regardless of global score ranking.
pub fn knowledge_chunks_graph(per_topic: usize) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    // Load all chunks ordered by topic then score; take top per_topic per group in Rust.
    let mut stmt = conn.prepare(
        "SELECT id, source_type, source_id, title, score, topic_group, tags, created_at \
         FROM knowledge_chunks ORDER BY topic_group ASC, score DESC",
    ).map_err(|e| format!("[db] knowledge_chunks_graph prepare: {e}"))?;

    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, f64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
        ))
    }).map_err(|e| format!("[db] knowledge_chunks_graph query: {e}"))?;

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut items: Vec<serde_json::Value> = Vec::new();

    for row in rows {
        let (id, source_type, source_id, title, score, topic_group, tags, created_at) =
            row.map_err(|e| format!("[db] knowledge_chunks_graph row: {e}"))?;
        let n = counts.entry(topic_group.clone()).or_insert(0);
        if *n >= per_topic { continue; }
        *n += 1;
        items.push(json!({
            "id": id, "source_type": source_type, "source_id": source_id,
            "title": title, "score": score, "topic_group": topic_group,
            "tags": tags, "created_at": created_at,
            "content_md": "", // omitted for graph — fetch on demand via knowledge_chunk_get
        }));
    }

    tracing::debug!("[db] knowledge_chunks_graph → {} items ({} topics)", items.len(), counts.len());
    Ok(json!({ "result": items }))
}

/// Return a single chunk by id (full content, for the node detail panel).
pub fn knowledge_chunk_get(chunk_id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.query_row(
        "SELECT id, source_type, source_id, title, content_md, score, topic_group, tags, created_at \
         FROM knowledge_chunks WHERE id = ?1",
        params![chunk_id],
        |r| Ok(json!({
            "id":          r.get::<_, String>(0)?,
            "source_type": r.get::<_, String>(1)?,
            "source_id":   r.get::<_, Option<String>>(2)?,
            "title":       r.get::<_, String>(3)?,
            "content_md":  r.get::<_, String>(4)?,
            "score":       r.get::<_, f64>(5)?,
            "topic_group": r.get::<_, String>(6)?,
            "tags":        r.get::<_, String>(7)?,
            "created_at":  r.get::<_, String>(8)?,
        })),
    ).map_err(|e| format!("[db] knowledge_chunk_get {chunk_id}: {e}"))
}

/// Return all chunks grouped by topic_group, sorted by score DESC within each group.
/// Topic groups are returned in alphabetical order.
pub fn knowledge_chunks_by_topic() -> Result<Vec<(String, Vec<serde_json::Value>)>, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, source_type, source_id, title, content_md, score, token_est, \
         topic_group, tags, created_at FROM knowledge_chunks \
         ORDER BY topic_group ASC, score DESC",
    ).map_err(|e| format!("[db] knowledge_chunks_by_topic prepare: {e}"))?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, f64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
        ))
    }).map_err(|e| format!("[db] knowledge_chunks_by_topic query: {e}"))?;

    let mut grouped: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        std::collections::BTreeMap::new();

    for row in rows {
        let (id, source_type, source_id, title, content_md, score, token_est, topic_group, tags, created_at) =
            row.map_err(|e| format!("[db] knowledge_chunks_by_topic row: {e}"))?;
        grouped.entry(topic_group).or_default().push(json!({
            "id": id, "source_type": source_type, "source_id": source_id,
            "title": title, "content_md": content_md, "score": score,
            "token_est": token_est, "tags": tags, "created_at": created_at,
        }));
    }

    Ok(grouped.into_iter().collect())
}

/// Full-text search over knowledge_chunks.
///
/// Strategy (cascading):
///   1. FTS5 exact MATCH on all query terms (BM25-ranked).
///   2. If 0 results: FTS5 prefix MATCH (each word* individually OR'd).
///   3. Merge with top-scored chunks (by score column) to pad up to `limit`,
///      deduplicating by id so FTS hits aren't displaced.
pub fn knowledge_chunks_search(query: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
    let conn = conn()?;

    macro_rules! push_chunk_row {
        ($row:expr) => {
            Ok((
                $row.get::<_, String>(0)?,
                $row.get::<_, String>(1)?,
                $row.get::<_, String>(2)?,
                $row.get::<_, f64>(3)?,
                $row.get::<_, String>(4)?,
                $row.get::<_, String>(5)?,
                $row.get::<_, String>(6)?,
            ))
        };
    }

    fn make_json(row: &(String, String, String, f64, String, String, String)) -> serde_json::Value {
        json!({
            "id": row.0, "title": row.1, "content_md": row.2, "score": row.3,
            "topic_group": row.4, "tags": row.5, "created_at": row.6,
        })
    }

    // Sanitise: keep alphanumeric + hyphens (FTS5 tokenizer splits on hyphens too,
    // so "python-3" → "python 3" in the index; keep them as word separators).
    let terms: Vec<String> = query
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .map(String::from)
        .collect();

    tracing::debug!("[db] knowledge_chunks_search query={:?} terms={:?}", query, terms);

    // Shared FTS query helper: runs a MATCH and collects rows.
    let run_fts = |fts_q: &str, lim: usize| -> Vec<(String, String, String, f64, String, String, String)> {
        let sql = "SELECT kc.id, kc.title, kc.content_md, kc.score, \
                          kc.topic_group, kc.tags, kc.created_at \
                   FROM knowledge_fts \
                   JOIN knowledge_chunks kc ON kc.id = knowledge_fts.chunk_id \
                   WHERE knowledge_fts MATCH ?1 \
                   ORDER BY rank LIMIT ?2";
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map(params![fts_q, lim as i64], |r| push_chunk_row!(r)) {
                for row in rows.flatten() { out.push(row); }
            }
        }
        out
    };

    // Top-scored fallback (no query match needed).
    let run_top = |lim: usize| -> Vec<(String, String, String, f64, String, String, String)> {
        let sql = "SELECT id, title, content_md, score, topic_group, tags, created_at \
                   FROM knowledge_chunks ORDER BY score DESC LIMIT ?1";
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map(params![lim as i64], |r| push_chunk_row!(r)) {
                for row in rows.flatten() { out.push(row); }
            }
        }
        out
    };

    // ── Step 1: try exact FTS5 MATCH ──────────────────────────────────────────
    let mut hits: Vec<(String, String, String, f64, String, String, String)> = if terms.is_empty() {
        Vec::new()
    } else {
        let exact_q = terms.join(" ");
        run_fts(&exact_q, limit)
    };

    // ── Step 2: if no hits, try prefix MATCH (word* OR word*) ─────────────────
    if hits.is_empty() && !terms.is_empty() {
        // Build "term1* OR term2*" — FTS5 OR syntax
        let prefix_q = terms.iter()
            .map(|w| format!("{w}*"))
            .collect::<Vec<_>>()
            .join(" OR ");
        hits = run_fts(&prefix_q, limit);
        if !hits.is_empty() {
            tracing::debug!("[db] knowledge_chunks_search: exact→0, prefix({prefix_q:?})→{}", hits.len());
        }
    }

    // ── Step 3: per-term fallback — try each word individually ────────────────
    if hits.is_empty() && !terms.is_empty() {
        let mut seen = std::collections::HashSet::new();
        for term in &terms {
            for row in run_fts(term, limit) {
                if seen.insert(row.0.clone()) { hits.push(row); }
                if hits.len() >= limit { break; }
            }
            if hits.len() >= limit { break; }
        }
        if !hits.is_empty() {
            tracing::debug!("[db] knowledge_chunks_search: prefix→0, per-term→{}", hits.len());
        }
    }

    // ── Step 4: merge top-scored chunks to fill up to `limit` ────────────────
    // (ensures RAG always has context even for queries that don't match any text)
    let seen_ids: std::collections::HashSet<String> = hits.iter().map(|r| r.0.clone()).collect();
    let fill = limit.saturating_sub(hits.len());
    if fill > 0 {
        let top = run_top(limit);
        for row in top {
            if !seen_ids.contains(&row.0) {
                hits.push(row);
                if hits.len() >= limit { break; }
            }
        }
    }

    let items: Vec<serde_json::Value> = hits.iter().take(limit).map(make_json).collect();
    tracing::debug!("[db] knowledge_chunks_search → {} chunks (query={:?})", items.len(), query);
    Ok(items)
}

/// Delete a single chunk by ID.
pub fn knowledge_chunk_delete(id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("DELETE FROM knowledge_chunks WHERE id = ?1", params![id])
        .map_err(|e| format!("[db] knowledge_chunk_delete: {e}"))?;
    tracing::debug!("[db] knowledge_chunk_delete id={id}");
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// Knowledge base — tree nodes
// ---------------------------------------------------------------------------

/// Insert or replace a tree node.
pub fn knowledge_tree_node_upsert(
    id: &str,
    level: i32,
    title: &str,
    summary_md: &str,
    child_ids: &str,  // JSON array string
    chunk_ids: &str,  // JSON array string
    topic_group: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let conn = conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_tree_nodes \
         (id, level, title, summary_md, child_ids, chunk_ids, topic_group, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![id, level, title, summary_md, child_ids, chunk_ids, topic_group, now],
    ).map_err(|e| format!("[db] knowledge_tree_node_upsert: {e}"))?;
    Ok(())
}

/// Return all tree nodes, both branch (level=1) and root (level=2).
pub fn knowledge_tree_nodes_list() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, level, title, summary_md, child_ids, chunk_ids, topic_group, created_at, updated_at \
         FROM knowledge_tree_nodes ORDER BY level DESC, topic_group ASC",
    ).map_err(|e| format!("[db] knowledge_tree_nodes_list prepare: {e}"))?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    }).map_err(|e| format!("[db] knowledge_tree_nodes_list query: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (id, level, title, summary_md, child_ids_s, chunk_ids_s, topic_group, created_at, updated_at) =
            row.map_err(|e| format!("[db] knowledge_tree_nodes_list row: {e}"))?;
        let child_ids: serde_json::Value = serde_json::from_str(&child_ids_s).unwrap_or(json!([]));
        let chunk_ids: serde_json::Value = serde_json::from_str(&chunk_ids_s).unwrap_or(json!([]));
        items.push(json!({
            "id": id, "level": level,
            "title": title, "label": title,  // label = display name used by WikiTab graph
            "summary_md": summary_md,
            "child_ids": child_ids, "chunk_ids": chunk_ids,
            "topic_group": topic_group, "created_at": created_at, "updated_at": updated_at,
        }));
    }

    tracing::debug!("[db] knowledge_tree_nodes_list → {} nodes", items.len());
    Ok(json!({ "result": items }))
}

/// Delete all nodes at or above `min_level` (used to clear before a tree rebuild).
/// Call with min_level=1 to remove all branch + root nodes while keeping nothing.
pub fn knowledge_tree_clear_above_level(min_level: i32) -> Result<(), String> {
    let conn = conn()?;
    conn.execute(
        "DELETE FROM knowledge_tree_nodes WHERE level >= ?1",
        params![min_level],
    ).map_err(|e| format!("[db] knowledge_tree_clear_above_level: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Messages (existing)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Inbox — channel_messages
// ---------------------------------------------------------------------------

/// Insert a message; silently skips if (service, external_id) already exists.
/// Returns `true` if newly inserted.
pub fn inbox_message_upsert(
    id: &str,
    service: &str,
    external_id: &str,
    sender: &str,
    subject: &str,
    body_enc: &str,
    received_at: &str,
    metadata: &str,
) -> Result<bool, String> {
    let conn = conn()?;
    let rows = conn.execute(
        "INSERT OR IGNORE INTO channel_messages \
         (id, service, external_id, sender, subject, body_enc, received_at, metadata) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![id, service, external_id, sender, subject, body_enc, received_at, metadata],
    ).map_err(|e| format!("[db] inbox_message_upsert: {e}"))?;
    Ok(rows > 0)
}

/// Paginated list of inbox messages ordered by received_at DESC.
/// body_enc is returned as-is; caller decrypts.
pub fn inbox_messages_list(
    service: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut items = Vec::new();

    macro_rules! map_msg {
        ($row:expr) => {
            Ok((
                $row.get::<_, String>(0)?,
                $row.get::<_, String>(1)?,
                $row.get::<_, String>(2)?,
                $row.get::<_, String>(3)?,
                $row.get::<_, String>(4)?,
                $row.get::<_, String>(5)?,
                $row.get::<_, i32>(6)?,
                $row.get::<_, String>(7)?,
            ))
        };
    }

    if let Some(svc) = service {
        let mut stmt = conn.prepare(
            "SELECT id, service, sender, subject, body_enc, received_at, read, metadata \
             FROM channel_messages WHERE service=?1 ORDER BY received_at DESC LIMIT ?2 OFFSET ?3"
        ).map_err(|e| format!("[db] inbox_messages_list prepare: {e}"))?;
        let rows = stmt.query_map(params![svc, limit as i64, offset as i64], |row| map_msg!(row))
            .map_err(|e| format!("[db] inbox_messages_list query: {e}"))?;
        for row in rows {
            let (id, service, sender, subject, body_enc, received_at, read, metadata) =
                row.map_err(|e| format!("[db] inbox_messages_list row: {e}"))?;
            items.push(json!({"id":id,"service":service,"sender":sender,"subject":subject,
                              "body_enc":body_enc,"received_at":received_at,"read":read==1,"metadata":metadata}));
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, service, sender, subject, body_enc, received_at, read, metadata \
             FROM channel_messages ORDER BY received_at DESC LIMIT ?1 OFFSET ?2"
        ).map_err(|e| format!("[db] inbox_messages_list prepare: {e}"))?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| map_msg!(row))
            .map_err(|e| format!("[db] inbox_messages_list query: {e}"))?;
        for row in rows {
            let (id, service, sender, subject, body_enc, received_at, read, metadata) =
                row.map_err(|e| format!("[db] inbox_messages_list row: {e}"))?;
            items.push(json!({"id":id,"service":service,"sender":sender,"subject":subject,
                              "body_enc":body_enc,"received_at":received_at,"read":read==1,"metadata":metadata}));
        }
    }

    tracing::debug!("[db] inbox_messages_list → {} items", items.len());
    Ok(json!({ "result": items }))
}

pub fn inbox_message_mark_read(id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute("UPDATE channel_messages SET read=1 WHERE id=?1", params![id])
        .map_err(|e| format!("[db] inbox_message_mark_read: {e}"))?;
    Ok(json!({"ok": true}))
}

pub fn inbox_unread_count(service: Option<&str>) -> Result<i64, String> {
    let conn = conn()?;
    let n: i64 = if let Some(svc) = service {
        conn.query_row(
            "SELECT COUNT(*) FROM channel_messages WHERE read=0 AND service=?1",
            params![svc],
            |r| r.get(0),
        )
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM channel_messages WHERE read=0",
            [],
            |r| r.get(0),
        )
    }.map_err(|e| format!("[db] inbox_unread_count: {e}"))?;
    Ok(n)
}

/// List all distinct manual document sources (source_type='manual').
/// Returns one row per document: source_id, title, chunk_count, created_at, topic_group, tags.
pub fn knowledge_manual_sources() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT source_id, MIN(title), COUNT(*), MIN(created_at), MIN(topic_group), MIN(tags) \
         FROM knowledge_chunks WHERE source_type='manual' \
         GROUP BY source_id ORDER BY MIN(created_at) DESC",
    ).map_err(|e| format!("[db] knowledge_manual_sources prepare: {e}"))?;

    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
        ))
    }).map_err(|e| format!("[db] knowledge_manual_sources query: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (source_id, title, chunk_count, created_at, topic_group, tags) =
            row.map_err(|e| format!("[db] knowledge_manual_sources row: {e}"))?;
        items.push(json!({
            "source_id": source_id,
            "title": title,
            "chunk_count": chunk_count,
            "created_at": created_at,
            "topic_group": topic_group,
            "tags": tags,
        }));
    }
    tracing::debug!("[db] knowledge_manual_sources → {} docs", items.len());
    Ok(json!({ "result": items }))
}

/// Delete all manual chunks for `source_id`.
pub fn knowledge_source_delete(source_id: &str) -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute(
        "DELETE FROM knowledge_fts WHERE chunk_id IN \
         (SELECT id FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1)",
        params![source_id],
    ).map_err(|e| format!("[db] knowledge_source_delete fts: {e}"))?;
    conn.execute(
        "DELETE FROM knowledge_embeddings WHERE chunk_id IN \
         (SELECT id FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1)",
        params![source_id],
    ).map_err(|e| format!("[db] knowledge_source_delete emb: {e}"))?;
    let n = conn.execute(
        "DELETE FROM knowledge_chunks WHERE source_type='manual' AND source_id=?1",
        params![source_id],
    ).map_err(|e| format!("[db] knowledge_source_delete: {e}"))?;
    tracing::debug!("[db] knowledge_source_delete source_id={source_id} chunks_removed={n}");
    Ok(json!({ "ok": true, "chunks_removed": n }))
}

// ---------------------------------------------------------------------------
// File fingerprints — deduplication helpers (L3)
// ---------------------------------------------------------------------------

/// Return the stored mtime:size fingerprint for a source path, or None.
pub fn knowledge_file_fingerprint_get(source_id: &str) -> Option<String> {
    let key = format!("file_fp:{source_id}");
    settings_get(&key)
        .ok()
        .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
}

/// Persist a mtime:size fingerprint for a source path.
pub fn knowledge_file_fingerprint_set(source_id: &str, fingerprint: &str) -> Result<(), String> {
    let key = format!("file_fp:{source_id}");
    settings_set(&key, fingerprint).map(|_| ())
}

// ---------------------------------------------------------------------------
// Data reset
// ---------------------------------------------------------------------------

/// Delete all user-generated data (conversations, memories, tasks, knowledge,
/// inbox). Keeps app_settings (API keys, referral code, etc.).
pub fn data_reset() -> Result<serde_json::Value, String> {
    let conn = conn()?;
    conn.execute_batch(
        "DELETE FROM messages;
         DELETE FROM threads;
         DELETE FROM memories;
         DELETE FROM tasks;
         DELETE FROM knowledge_fts;
         DELETE FROM knowledge_embeddings;
         DELETE FROM knowledge_chunks;
         DELETE FROM knowledge_tree_nodes;
         DELETE FROM channel_messages;
         DELETE FROM composio_connections;
         DELETE FROM composio_apps_cache;",
    )
    .map_err(|e| format!("[db] data_reset: {e}"))?;
    tracing::info!("[db] data_reset: all user data cleared (settings retained)");
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// Knowledge embeddings — vector storage
// ---------------------------------------------------------------------------

/// Store a chunk's embedding vector.
pub fn embedding_set(chunk_id: &str, model: &str, embedding: &[f32]) -> Result<(), String> {
    let conn = conn()?;
    let now  = chrono::Utc::now().to_rfc3339();
    let emb_json = crate::embeddings::serialize(embedding);
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_embeddings (chunk_id, model, embedding, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![chunk_id, model, emb_json, now],
    ).map_err(|e| format!("[db] embedding_set: {e}"))?;
    Ok(())
}

/// Check if a chunk already has an embedding.
pub fn embedding_exists(chunk_id: &str) -> bool {
    conn().ok()
        .and_then(|c| c.query_row(
            "SELECT 1 FROM knowledge_embeddings WHERE chunk_id = ?1",
            params![chunk_id],
            |_| Ok(true),
        ).ok())
        .unwrap_or(false)
}

/// Return all (chunk_id, title, content_md, topic_group, embedding) rows
/// for semantic similarity ranking.
pub fn embeddings_all_with_meta() -> Result<Vec<(String, String, String, String, Vec<f32>)>, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT ke.chunk_id, kc.title, kc.content_md, kc.topic_group, ke.embedding \
         FROM knowledge_embeddings ke \
         JOIN knowledge_chunks kc ON kc.id = ke.chunk_id",
    ).map_err(|e| format!("[db] embeddings_all_with_meta prepare: {e}"))?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    }).map_err(|e| format!("[db] embeddings_all_with_meta query: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (cid, title, content, topic, emb_json) =
            row.map_err(|e| format!("[db] embeddings_all_with_meta row: {e}"))?;
        if let Some(emb) = crate::embeddings::deserialize(&emb_json) {
            out.push((cid, title, content, topic, emb));
        }
    }
    Ok(out)
}

/// Delete every row from knowledge_embeddings.
/// Called when the embedding model changes so stale vectors are purged before regeneration.
pub fn embeddings_clear_all() -> Result<usize, String> {
    let conn = conn()?;
    let n = conn.execute("DELETE FROM knowledge_embeddings", [])
        .map_err(|e| format!("[db] embeddings_clear_all: {e}"))?;
    tracing::info!("[db] embeddings_clear_all: removed {n} rows");
    Ok(n)
}

/// Return (chunk_id, title, content_md, topic_group, source_type) for chunks with no embedding.
pub fn embeddings_pending() -> Result<Vec<(String, String, String, String, String)>, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT kc.id, kc.title, kc.content_md, kc.topic_group, kc.source_type \
         FROM knowledge_chunks kc \
         LEFT JOIN knowledge_embeddings ke ON ke.chunk_id = kc.id \
         WHERE ke.chunk_id IS NULL",
    ).map_err(|e| format!("[db] embeddings_pending prepare: {e}"))?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    }).map_err(|e| format!("[db] embeddings_pending query: {e}"))?;
    let mut out = Vec::new();
    for row in rows { out.push(row.map_err(|e| format!("[db] embeddings_pending row: {e}"))?); }
    tracing::debug!("[db] embeddings_pending → {} chunks", out.len());
    Ok(out)
}

/// Return (chunk_id, title, content_md, topic_group, source_type) for a source_id.
pub fn knowledge_chunks_for_source(source_id: &str) -> Result<Vec<(String, String, String, String, String)>, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, content_md, topic_group, source_type \
         FROM knowledge_chunks WHERE source_id = ?1",
    ).map_err(|e| format!("[db] knowledge_chunks_for_source prepare: {e}"))?;
    let rows = stmt.query_map(params![source_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    }).map_err(|e| format!("[db] knowledge_chunks_for_source query: {e}"))?;
    let mut out = Vec::new();
    for row in rows { out.push(row.map_err(|e| format!("[db] knowledge_chunks_for_source row: {e}"))?); }
    Ok(out)
}

/// Search for the top-K most semantically similar chunks using the in-memory HNSW index.
/// Falls back to loading all embeddings from SQLite when the HNSW index isn't built yet.
pub fn knowledge_chunks_search_semantic(
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<serde_json::Value>, String> {
    tracing::debug!(
        "[db] knowledge_chunks_search_semantic: query_dims={} limit={} index_size={}",
        query_embedding.len(),
        limit,
        crate::vector_index::size(),
    );

    // Use in-memory HNSW index when available.
    let candidates: Vec<(String, f32)> = if crate::vector_index::is_ready() {
        crate::vector_index::search(query_embedding, limit)
    } else {
        // Fallback: load all embeddings from SQLite and do linear cosine scan.
        tracing::debug!("[db] knowledge_chunks_search_semantic: HNSW not ready, using linear scan");
        let rows = embeddings_all_with_meta()?;
        let mut scored: Vec<(String, f32)> = rows
            .into_iter()
            .map(|(chunk_id, _title, _content, _topic, vec)| {
                let sim = crate::embeddings::cosine_similarity(query_embedding, &vec);
                (chunk_id, sim)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    };

    if candidates.is_empty() {
        tracing::debug!("[db] knowledge_chunks_search_semantic: no candidates found");
        return Ok(vec![]);
    }

    // Fetch full chunk data for each candidate.
    let db_conn = conn()?;
    let mut results = Vec::with_capacity(candidates.len());

    for (chunk_id, similarity) in &candidates {
        let row = db_conn.query_row(
            "SELECT id, source_type, source_id, title, content_md, score, token_est, \
             topic_group, tags, created_at FROM knowledge_chunks WHERE id = ?1",
            params![chunk_id],
            |row| {
                Ok(json!({
                    "id":          row.get::<_, String>(0)?,
                    "source_type": row.get::<_, String>(1)?,
                    "source_id":   row.get::<_, String>(2)?,
                    "title":       row.get::<_, String>(3)?,
                    "content_md":  row.get::<_, String>(4)?,
                    "score":       row.get::<_, f64>(5)?,
                    "token_est":   row.get::<_, i64>(6)?,
                    "topic_group": row.get::<_, String>(7)?,
                    "tags":        row.get::<_, String>(8)?,
                    "created_at":  row.get::<_, String>(9)?,
                    "similarity":  similarity,
                }))
            },
        );
        match row {
            Ok(v) => results.push(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                tracing::warn!("[db] knowledge_chunks_search_semantic: chunk {chunk_id} not found in DB");
            }
            Err(e) => tracing::error!("[db] knowledge_chunks_search_semantic: row error: {e}"),
        }
    }

    tracing::debug!("[db] knowledge_chunks_search_semantic: {} results", results.len());
    Ok(results)
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Env var settings — encrypted storage in app_settings under "env.*" keys
// ---------------------------------------------------------------------------

/// Store an env var value encrypted in app_settings under key "env.<name>".
/// Empty value stores empty string (= cleared). Non-empty value is encrypted.
pub fn settings_set_env(env_key: &str, plain: &str) -> Result<serde_json::Value, String> {
    let db_key = format!("env.{env_key}");
    let stored = if plain.is_empty() {
        String::new()
    } else {
        let enc = crate::encryption::encrypt_str(crate::encryption::key(), plain);
        format!("ENC:{enc}")
    };
    let conn = conn()?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![db_key, stored],
    ).map_err(|e| { tracing::error!("[db] settings_set_env: {e}"); format!("[db] settings_set_env: {e}") })?;
    tracing::debug!("[db] settings_set_env key={env_key} encrypted={}", !plain.is_empty());
    Ok(serde_json::json!({"ok": true}))
}

/// Read and decrypt an env var value from app_settings ("env.<name>").
/// Returns empty string if not found or value is empty.
pub fn settings_get_env(env_key: &str) -> Result<String, String> {
    let db_key = format!("env.{env_key}");
    let conn = conn()?;
    let value: String = match conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![db_key],
        |r| r.get::<_, String>(0),
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(String::new()),
        Err(e) => return Err(format!("[db] settings_get_env: {e}")),
    };
    if value.is_empty() {
        return Ok(String::new());
    }
    if let Some(enc) = value.strip_prefix("ENC:") {
        crate::encryption::decrypt_str(crate::encryption::key(), enc)
            .map_err(|e| format!("[db] settings_get_env decrypt: {e}"))
    } else {
        Ok(value) // plaintext fallback for migration compatibility
    }
}

/// Load all env.* settings from DB, decrypt, and apply to process environment.
/// Returns the number of env vars successfully loaded.
pub fn load_env_settings_from_db() -> Result<usize, String> {
    let conn = conn()?;
    let mut stmt = conn.prepare(
        "SELECT key, value FROM app_settings WHERE key LIKE 'env.%' AND value != ''"
    ).map_err(|e| format!("[db] load_env_settings: {e}"))?;
    let rows: Vec<(String, String)> = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| format!("[db] load_env_settings query: {e}"))?
    .filter_map(|r| r.ok())
    .collect();
    let mut count = 0usize;
    for (db_key, stored) in rows {
        let env_key = db_key.trim_start_matches("env.");
        if env_key.is_empty() { continue; }
        let plain = if let Some(enc) = stored.strip_prefix("ENC:") {
            match crate::encryption::decrypt_str(crate::encryption::key(), enc) {
                Ok(v) => v,
                Err(e) => { tracing::warn!("[db] load_env_settings decrypt failed for {env_key}: {e}"); continue; }
            }
        } else {
            stored // plaintext fallback
        };
        if plain.is_empty() { continue; }
        std::env::set_var(env_key, &plain);
        count += 1;
        tracing::debug!("[db] loaded env var from DB: {env_key}");
    }
    Ok(count)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use tempfile::TempDir;

    static INIT: Once = Once::new();
    // SAFETY: written once inside INIT.call_once (single-threaded init), then
    // only read (for lifetime extension). Tests run in the same process.
    static mut TEST_DIR: Option<TempDir> = None;

    fn setup() {
        INIT.call_once(|| {
            let dir = TempDir::new().expect("tempdir");
            let dir_str = dir.path().to_str().expect("valid utf-8 path");
            crate::encryption::init(dir_str);
            let db_path = dir.path().join("test.db");
            let db_str = db_path.to_str().expect("valid utf-8 db path");
            init(db_str).expect("db init");
            // SAFETY: test-only, single-threaded init via Once
            unsafe { TEST_DIR = Some(dir); }
        });
    }

    #[test]
    fn env_round_trip_encrypt_decrypt() {
        setup();
        let key = "TEST_API_KEY_ROUNDTRIP";
        let value = "sk-test-1234567890abcdef";
        settings_set_env(key, value).expect("set");
        let got = settings_get_env(key).expect("get");
        assert_eq!(got, value, "decrypted value should match original");
        // Verify the stored value is encrypted (not plaintext)
        let db_conn = conn().expect("conn");
        let stored: String = db_conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            rusqlite::params![format!("env.{key}")],
            |r| r.get(0),
        ).expect("stored value");
        assert!(stored.starts_with("ENC:"), "stored value should be encrypted, got: {stored}");
    }

    #[test]
    fn env_empty_value_clears() {
        setup();
        let key = "TEST_CLEAR_KEY";
        settings_set_env(key, "some_value").expect("set");
        settings_set_env(key, "").expect("clear");
        let got = settings_get_env(key).expect("get");
        assert_eq!(got, "", "cleared key should return empty string");
    }

    #[test]
    fn env_not_found_returns_empty() {
        setup();
        let got = settings_get_env("NONEXISTENT_KEY_XYZ_123").expect("get");
        assert_eq!(got, "", "missing key should return empty string");
    }

    #[test]
    fn env_load_from_db_sets_process_env() {
        setup();
        let key = "TEST_LOAD_ENV_VAR";
        let value = "load_test_value_42";
        // Ensure clean state before setting
        std::env::remove_var(key);
        settings_set_env(key, value).expect("set");
        let count = load_env_settings_from_db().expect("load");
        assert!(count >= 1, "at least one var should be loaded, got {count}");
        let env_val = std::env::var(key).expect("env var should be set after load");
        assert_eq!(env_val, value);
        std::env::remove_var(key); // cleanup
    }

    #[test]
    fn env_overwrite_updates_value() {
        setup();
        let key = "TEST_OVERWRITE_KEY";
        settings_set_env(key, "first_value").expect("set first");
        settings_set_env(key, "second_value").expect("set second");
        let got = settings_get_env(key).expect("get");
        assert_eq!(got, "second_value");
    }

    #[test]
    fn env_special_chars_preserved() {
        setup();
        let key = "TEST_SPECIAL_CHARS";
        let value = "sk-test!@#value_with_special-chars_plus=more";
        settings_set_env(key, value).expect("set");
        let got = settings_get_env(key).expect("get");
        assert_eq!(got, value, "special characters should be preserved");
    }
}

/// Insert a new message and return the created object.
pub fn message_append(
    thread_id: &str,
    role: &str,
    content: &str,
) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let conn = conn()?;
    conn.execute(
        "INSERT INTO messages (id, thread_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, thread_id, role, content, created_at],
    )
    .map_err(|e| format!("[db] message_append insert: {e}"))?;

    tracing::debug!("[db] message_append thread_id={thread_id} role={role} id={id}");
    Ok(json!({
        "id": id,
        "thread_id": thread_id,
        "role": role,
        "content": content,
        "created_at": created_at,
    }))
}
