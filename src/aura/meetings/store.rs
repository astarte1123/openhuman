//! SQLite-backed storage for meetings and their transcript chunks.
//!
//! Mirrors the `cron` domain's storage conventions: one `rusqlite` connection
//! opened per call via [`with_connection`], schema created idempotently with
//! `CREATE TABLE IF NOT EXISTS`, and `anyhow::Result` + `.context(...)` for
//! error reporting.
//!
//! Transcript text is also indexed for full-text search via an "external
//! content" FTS5 virtual table (`meeting_transcript_fts`), kept in sync with
//! `meeting_transcript_chunks` by triggers rather than duplicating the text —
//! see [`search_transcripts`] and [`backfill_missing_fts_rows`].

use crate::aura::config::Config;
use crate::aura::meetings::types::{
    Meeting, MeetingScreenshot, MeetingStatus, TranscriptChunk,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Creates a new meeting row in `recording` status and returns it.
pub fn create_meeting(config: &Config, title: &str, source: &str) -> Result<Meeting> {
    let now = Utc::now().to_rfc3339();
    let id = format!("meeting_{}", Uuid::new_v4());

    log::debug!("[meetings] creating meeting id={id} title={title} source={source}");

    with_connection(config, |conn| {
        conn.execute(
            "INSERT INTO meetings (id, title, source, status, created_at, updated_at, duration_ms, summary)
             VALUES (?1, ?2, ?3, 'recording', ?4, ?4, NULL, NULL)",
            params![id, title, source, now],
        )
        .context("Failed to insert meeting")?;
        Ok(())
    })?;

    get_meeting(config, &id)
}

/// Lists all meetings, most recently created first.
pub fn list_meetings(config: &Config) -> Result<Vec<Meeting>> {
    with_connection(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, source, status, created_at, updated_at, duration_ms, summary
             FROM meetings ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], map_meeting_row)?;
        let mut meetings = Vec::new();
        for row in rows {
            meetings.push(row?);
        }
        Ok(meetings)
    })
}

/// Fetches a single meeting by id. Errors clearly if it does not exist.
pub fn get_meeting(config: &Config, meeting_id: &str) -> Result<Meeting> {
    with_connection(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, source, status, created_at, updated_at, duration_ms, summary
             FROM meetings WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![meeting_id])?;
        if let Some(row) = rows.next()? {
            map_meeting_row(row).map_err(Into::into)
        } else {
            anyhow::bail!("Meeting '{meeting_id}' not found")
        }
    })
}

/// Renames a meeting and bumps `updated_at`.
pub fn update_title(config: &Config, meeting_id: &str, title: &str) -> Result<Meeting> {
    let now = Utc::now().to_rfc3339();
    let changed = with_connection(config, |conn| {
        conn.execute(
            "UPDATE meetings SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, meeting_id],
        )
        .context("Failed to update meeting title")
    })?;

    if changed == 0 {
        anyhow::bail!("Meeting '{meeting_id}' not found");
    }

    get_meeting(config, meeting_id)
}

/// Deletes a meeting. Transcript chunks cascade-delete via
/// `ON DELETE CASCADE` (requires `PRAGMA foreign_keys = ON`, set in
/// [`with_connection`]). Returns whether a row was actually removed.
pub fn delete_meeting(config: &Config, meeting_id: &str) -> Result<bool> {
    let changed = with_connection(config, |conn| {
        conn.execute("DELETE FROM meetings WHERE id = ?1", params![meeting_id])
            .context("Failed to delete meeting")
    })?;

    log::info!("[meetings] deleted meeting id={meeting_id} rows_removed={changed}");
    Ok(changed > 0)
}

/// Appends one transcript chunk to a meeting and touches its `updated_at`.
///
/// Both writes happen on the same connection so a failure on either half
/// leaves the meeting's `updated_at` and its chunk list consistent with each
/// other (no partial write straddling two separate connections).
pub fn append_transcript_chunk(
    config: &Config,
    meeting_id: &str,
    start_ms: i64,
    end_ms: i64,
    speaker: Option<&str>,
    text: &str,
) -> Result<TranscriptChunk> {
    let now = Utc::now().to_rfc3339();

    let chunk_id = with_connection(config, |conn| {
        conn.execute(
            "INSERT INTO meeting_transcript_chunks (meeting_id, start_ms, end_ms, speaker, text, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![meeting_id, start_ms, end_ms, speaker, text, now],
        )
        .context("Failed to insert transcript chunk")?;
        let id = conn.last_insert_rowid();

        conn.execute(
            "UPDATE meetings SET updated_at = ?1 WHERE id = ?2",
            params![now, meeting_id],
        )
        .context("Failed to touch meeting updated_at after appending transcript chunk")?;

        Ok(id)
    })?;

    log::debug!(
        "[meetings] appended transcript chunk id={chunk_id} meeting_id={meeting_id} start_ms={start_ms} end_ms={end_ms}"
    );

    Ok(TranscriptChunk {
        id: chunk_id,
        meeting_id: meeting_id.to_string(),
        start_ms,
        end_ms,
        speaker: speaker.map(str::to_string),
        text: text.to_string(),
        created_at: now,
    })
}

/// One full-text search hit against a meeting transcript.
///
/// Defined here (rather than in `types.rs`) because it is purely a query
/// result shape for [`search_transcripts`] — like `ops::MeetingWithChunks`,
/// it is not a row that round-trips back into the store.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub meeting_id: String,
    pub meeting_title: String,
    pub chunk_id: i64,
    pub start_ms: i64,
    /// FTS5 `snippet()` output: a short excerpt around the match with
    /// `<mark>...</mark>` wrapped around the matched terms.
    pub snippet: String,
}

/// Full-text searches every meeting transcript via the `meeting_transcript_fts`
/// virtual table (kept in sync with `meeting_transcript_chunks` by triggers —
/// see [`with_connection`]), ordered by BM25 relevance (best match first).
///
/// `query` is passed to FTS5's `MATCH` operator, so it supports the full FTS5
/// query syntax (`AND`/`OR`/`NOT`, `"exact phrases"`, `prefix*`). User input
/// that doesn't parse as a valid MATCH expression (e.g. bare punctuation,
/// dangling boolean operators, unbalanced quotes) is retried once as a
/// literal quoted phrase rather than surfacing a raw SQL error to the RPC
/// caller — see [`is_fts5_syntax_error`] for why we detect this by message
/// rather than by rusqlite error variant.
pub fn search_transcripts(config: &Config, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    log::debug!("[meetings] search_transcripts query={query:?} limit={limit}");

    with_connection(config, |conn| run_transcript_search(conn, query, limit))
}

fn run_transcript_search(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
    match run_fts_match(conn, query, limit) {
        Ok(results) => Ok(results),
        Err(err) if is_fts5_syntax_error(&err) => {
            log::debug!(
                "[meetings] search_transcripts: query {query:?} rejected as FTS5 syntax ({err}); \
                 retrying as a literal phrase"
            );
            // Quote the raw input as a single phrase, doubling any embedded
            // `"` per FTS5's phrase-escaping rule, so arbitrary user text
            // (including pure punctuation) is always a syntactically valid
            // MATCH expression. Worst case this returns zero rows instead of
            // an error.
            let literal = format!("\"{}\"", query.replace('"', "\"\""));
            run_fts_match(conn, &literal, limit)
                .context("Failed to search transcripts even as a literal phrase")
        }
        Err(err) => Err(err),
    }
}

fn run_fts_match(conn: &Connection, match_expr: &str, limit: i64) -> Result<Vec<SearchResult>> {
    // `bm25(meeting_transcript_fts)` is used explicitly instead of the FTS5
    // `rank` hidden column: `rank` is only recognized when it appears
    // unqualified in a plain `ORDER BY` on the FTS table itself, and this
    // query joins `meeting_transcript_chunks` and `meetings` alongside it —
    // some SQLite builds report "no such column: rank" once other tables are
    // in scope. `bm25()` is a normal scalar function and has no such
    // ambiguity. Lower bm25 scores are more relevant, hence ascending order.
    let mut stmt = conn.prepare(
        "SELECT c.meeting_id, m.title, c.id, c.start_ms,
                snippet(meeting_transcript_fts, 0, '<mark>', '</mark>', '…', 10)
         FROM meeting_transcript_fts
         JOIN meeting_transcript_chunks c ON c.id = meeting_transcript_fts.rowid
         JOIN meetings m ON m.id = c.meeting_id
         WHERE meeting_transcript_fts MATCH ?1
         ORDER BY bm25(meeting_transcript_fts)
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![match_expr, limit], |row| {
        Ok(SearchResult {
            meeting_id: row.get(0)?,
            meeting_title: row.get(1)?,
            chunk_id: row.get(2)?,
            start_ms: row.get(3)?,
            snippet: row.get(4)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Detects FTS5 MATCH syntax errors by message rather than by rusqlite error
/// variant/code: rusqlite surfaces them as a generic
/// `rusqlite::Error::SqliteFailure` with `ErrorCode::Unknown` — there is no
/// dedicated variant/code to match on, only the message SQLite's FTS5 module
/// generates. That message varies by failure shape — dangling boolean
/// operators like `AND` produce `"fts5: syntax error near \"AND\""`, but bare
/// punctuation runs like `***` produce `"unknown special query: **"` instead
/// (no `"fts5:"` prefix at all). Checking the whole error chain (not just the
/// top-level `Display`) keeps this working even if a caller later wraps the
/// error with `.context(...)`.
fn is_fts5_syntax_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        let msg = cause.to_string();
        msg.contains("fts5: syntax error") || msg.contains("unknown special query")
    })
}

/// Lists a meeting's transcript chunks ordered by `start_ms` ascending.
pub fn list_transcript_chunks(config: &Config, meeting_id: &str) -> Result<Vec<TranscriptChunk>> {
    with_connection(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, start_ms, end_ms, speaker, text, created_at
             FROM meeting_transcript_chunks WHERE meeting_id = ?1 ORDER BY start_ms ASC",
        )?;

        let rows = stmt.query_map(params![meeting_id], map_chunk_row)?;
        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?);
        }
        Ok(chunks)
    })
}

/// Inserts one captured-and-OCR'd meeting screenshot row.
pub fn insert_screenshot(
    config: &Config,
    meeting_id: &str,
    captured_at_ms: i64,
    image_path: &str,
    ocr_text: &str,
) -> Result<MeetingScreenshot> {
    let now = Utc::now().to_rfc3339();

    let id = with_connection(config, |conn| {
        conn.execute(
            "INSERT INTO meeting_screenshots (meeting_id, captured_at_ms, image_path, ocr_text, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meeting_id, captured_at_ms, image_path, ocr_text, now],
        )
        .context("Failed to insert meeting screenshot")?;
        Ok(conn.last_insert_rowid())
    })?;

    log::debug!(
        "[meetings] inserted screenshot id={id} meeting_id={meeting_id} captured_at_ms={captured_at_ms} \
         ocr_text_chars={}",
        ocr_text.len()
    );

    Ok(MeetingScreenshot {
        id,
        meeting_id: meeting_id.to_string(),
        captured_at_ms,
        image_path: image_path.to_string(),
        ocr_text: ocr_text.to_string(),
        created_at: now,
    })
}

/// Lists a meeting's screenshots ordered by `captured_at_ms` ascending.
pub fn list_screenshots(config: &Config, meeting_id: &str) -> Result<Vec<MeetingScreenshot>> {
    with_connection(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, captured_at_ms, image_path, ocr_text, created_at
             FROM meeting_screenshots WHERE meeting_id = ?1 ORDER BY captured_at_ms ASC",
        )?;

        let rows = stmt.query_map(params![meeting_id], map_screenshot_row)?;
        let mut screenshots = Vec::new();
        for row in rows {
            screenshots.push(row?);
        }
        Ok(screenshots)
    })
}

/// Marks a meeting `completed` with a final duration.
pub fn finish_meeting(config: &Config, meeting_id: &str, duration_ms: i64) -> Result<Meeting> {
    let now = Utc::now().to_rfc3339();
    let changed = with_connection(config, |conn| {
        conn.execute(
            "UPDATE meetings SET status = 'completed', duration_ms = ?1, updated_at = ?2 WHERE id = ?3",
            params![duration_ms, now, meeting_id],
        )
        .context("Failed to finish meeting")
    })?;

    if changed == 0 {
        anyhow::bail!("Meeting '{meeting_id}' not found");
    }

    get_meeting(config, meeting_id)
}

/// Persists a generated summary for a meeting.
pub fn update_summary(config: &Config, meeting_id: &str, summary: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let changed = with_connection(config, |conn| {
        conn.execute(
            "UPDATE meetings SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![summary, now, meeting_id],
        )
        .context("Failed to save meeting summary")
    })?;

    if changed == 0 {
        anyhow::bail!("Meeting '{meeting_id}' not found");
    }
    Ok(())
}

fn map_meeting_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Meeting> {
    Ok(Meeting {
        id: row.get(0)?,
        title: row.get(1)?,
        source: row.get(2)?,
        status: MeetingStatus::parse(&row.get::<_, String>(3)?),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        duration_ms: row.get(6)?,
        summary: row.get(7)?,
    })
}

fn map_chunk_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranscriptChunk> {
    Ok(TranscriptChunk {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        start_ms: row.get(2)?,
        end_ms: row.get(3)?,
        speaker: row.get(4)?,
        text: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_screenshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MeetingScreenshot> {
    Ok(MeetingScreenshot {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        captured_at_ms: row.get(2)?,
        image_path: row.get(3)?,
        ocr_text: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn with_connection<T>(config: &Config, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let db_path = config.workspace_dir.join("meetings").join("meetings.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create meetings directory: {}", parent.display())
        })?;
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open meetings DB: {}", db_path.display()))?;

    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS meetings (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            source      TEXT NOT NULL DEFAULT 'manual',
            status      TEXT NOT NULL DEFAULT 'recording',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            duration_ms INTEGER,
            summary     TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_meetings_created_at ON meetings(created_at);

        CREATE TABLE IF NOT EXISTS meeting_transcript_chunks (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            meeting_id TEXT NOT NULL,
            start_ms   INTEGER NOT NULL,
            end_ms     INTEGER NOT NULL,
            speaker    TEXT,
            text       TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_transcript_chunks_meeting_id ON meeting_transcript_chunks(meeting_id);

        CREATE TABLE IF NOT EXISTS meeting_screenshots (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            meeting_id     TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
            captured_at_ms INTEGER NOT NULL,
            image_path     TEXT NOT NULL,
            ocr_text       TEXT NOT NULL,
            created_at     TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_screenshots_meeting_id ON meeting_screenshots(meeting_id);

        -- Full-text index over transcript chunk text, in 'external content'
        -- mode: the FTS5 shadow tables index the text but do not duplicate
        -- it — `content_rowid='id'` ties each FTS row to
        -- `meeting_transcript_chunks.id`, and the triggers below keep the
        -- index in sync on every insert/update/delete of that table.
        CREATE VIRTUAL TABLE IF NOT EXISTS meeting_transcript_fts USING fts5(
            text,
            content='meeting_transcript_chunks',
            content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS meeting_transcript_chunks_ai AFTER INSERT ON meeting_transcript_chunks BEGIN
            INSERT INTO meeting_transcript_fts(rowid, text) VALUES (new.id, new.text);
        END;
        CREATE TRIGGER IF NOT EXISTS meeting_transcript_chunks_ad AFTER DELETE ON meeting_transcript_chunks BEGIN
            INSERT INTO meeting_transcript_fts(meeting_transcript_fts, rowid, text) VALUES ('delete', old.id, old.text);
        END;
        CREATE TRIGGER IF NOT EXISTS meeting_transcript_chunks_au AFTER UPDATE ON meeting_transcript_chunks BEGIN
            INSERT INTO meeting_transcript_fts(meeting_transcript_fts, rowid, text) VALUES ('delete', old.id, old.text);
            INSERT INTO meeting_transcript_fts(rowid, text) VALUES (new.id, new.text);
        END;",
    )
    .context("Failed to initialize meetings schema")?;

    backfill_missing_fts_rows(&conn)?;

    f(&conn)
}

/// Idempotent migration step for databases that already had rows in
/// `meeting_transcript_chunks` before `meeting_transcript_fts` existed (this
/// domain shipped before FTS5 search was added, and real recordings were
/// already stored). The triggers created above only fire for *future*
/// writes, so any chunk whose `id` has no matching row in the FTS index
/// (`rowid NOT IN ...`) is backfilled here.
///
/// This runs on every [`with_connection`] call rather than gating on "was
/// the FTS table just created", which would need extra state threaded out of
/// `execute_batch`'s single opaque `Result<()>`. Instead the migration is
/// naturally idempotent: once every chunk has a matching FTS row, the
/// `WHERE ... NOT IN` anti-join matches nothing and the `INSERT` is a no-op
/// `SELECT` over an already-synced table — mirrors how `cron::store`'s
/// `add_column_if_missing` re-checks schema state on every connection open
/// instead of only "the first time".
fn backfill_missing_fts_rows(conn: &Connection) -> Result<()> {
    // A plain `SELECT rowid FROM meeting_transcript_fts` (no MATCH) against an
    // *external content* FTS5 table is not reliable for detecting which rows
    // are actually indexed — verified empirically: after removing a row via
    // the `'delete'` special command, `COUNT(*)` on the FTS table still
    // reported the old count within the very same connection. So a `WHERE id
    // NOT IN (SELECT rowid FROM meeting_transcript_fts)` anti-join here would
    // never detect a genuinely missing row.
    //
    // `INSERT OR IGNORE` sidesteps this: fts5's `xUpdate` reports a
    // constraint violation for a duplicate rowid, which SQLite's core honors
    // as an ordinary `ON CONFLICT` — so already-indexed rows are silently
    // skipped and only rows with no matching FTS entry actually get
    // inserted, without ever needing to read the FTS table itself.
    let backfilled = conn
        .execute(
            "INSERT OR IGNORE INTO meeting_transcript_fts(rowid, text)
             SELECT id, text FROM meeting_transcript_chunks",
            [],
        )
        .context("Failed to backfill meeting_transcript_fts for pre-existing transcript chunks")?;

    if backfilled > 0 {
        log::info!(
            "[meetings] backfilled {backfilled} transcript chunk(s) into meeting_transcript_fts"
        );
    }

    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
