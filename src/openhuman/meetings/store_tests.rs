use super::*;
use crate::openhuman::config::Config;
use tempfile::TempDir;

fn test_config(tmp: &TempDir) -> Config {
    let config = Config {
        workspace_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    };
    std::fs::create_dir_all(&config.workspace_dir).unwrap();
    config
}

// ── create / get ─────────────────────────────────────────────────

#[test]
fn create_and_get_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let created = create_meeting(&config, "Standup", "manual").unwrap();
    assert_eq!(created.title, "Standup");
    assert_eq!(created.source, "manual");
    assert_eq!(created.status, MeetingStatus::Recording);
    assert!(created.duration_ms.is_none());
    assert!(created.summary.is_none());

    let fetched = get_meeting(&config, &created.id).unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, "Standup");
    assert_eq!(fetched.created_at, created.created_at);
}

#[test]
fn get_missing_meeting_errors() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = get_meeting(&config, "does-not-exist").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── list ─────────────────────────────────────────────────────────

#[test]
fn list_orders_by_created_at_desc() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let first = create_meeting(&config, "First", "manual").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = create_meeting(&config, "Second", "manual").unwrap();

    let listed = list_meetings(&config).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(
        listed[0].id, second.id,
        "most recently created meeting should be first"
    );
    assert_eq!(listed[1].id, first.id);
}

#[test]
fn list_is_empty_for_fresh_workspace() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    assert!(list_meetings(&config).unwrap().is_empty());
}

// ── transcript chunks ────────────────────────────────────────────

#[test]
fn append_transcript_chunk_and_list_orders_by_start_ms() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Planning", "manual").unwrap();

    // Insert out of order to prove the read path sorts by start_ms, not insertion order.
    let second = append_transcript_chunk(
        &config,
        &meeting.id,
        5_000,
        8_000,
        Some("bob"),
        "second chunk",
    )
    .unwrap();
    let first = append_transcript_chunk(
        &config,
        &meeting.id,
        0,
        4_000,
        Some("alice"),
        "first chunk",
    )
    .unwrap();

    let chunks = list_transcript_chunks(&config, &meeting.id).unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].id, first.id);
    assert_eq!(chunks[0].text, "first chunk");
    assert_eq!(chunks[0].speaker.as_deref(), Some("alice"));
    assert_eq!(chunks[1].id, second.id);
    assert_eq!(chunks[1].text, "second chunk");
}

#[test]
fn append_transcript_chunk_touches_meeting_updated_at() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Sync", "manual").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(5));
    append_transcript_chunk(&config, &meeting.id, 0, 1_000, None, "hi").unwrap();

    let touched = get_meeting(&config, &meeting.id).unwrap();
    assert!(
        touched.updated_at > meeting.updated_at,
        "appending a chunk should bump updated_at"
    );
}

#[test]
fn list_transcript_chunks_is_empty_for_meeting_with_no_chunks() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Quiet meeting", "manual").unwrap();

    assert!(list_transcript_chunks(&config, &meeting.id)
        .unwrap()
        .is_empty());
}

// ── delete ───────────────────────────────────────────────────────

#[test]
fn delete_meeting_cascades_transcript_chunks() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Retro", "manual").unwrap();
    append_transcript_chunk(&config, &meeting.id, 0, 1_000, None, "hello").unwrap();

    let deleted = delete_meeting(&config, &meeting.id).unwrap();
    assert!(deleted);

    assert!(get_meeting(&config, &meeting.id).is_err());
    assert!(
        list_transcript_chunks(&config, &meeting.id).unwrap().is_empty(),
        "transcript chunks must cascade-delete with their meeting"
    );
}

#[test]
fn delete_meeting_cascades_screenshots() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Slides retro", "manual").unwrap();
    insert_screenshot(&config, &meeting.id, 20_000, "/tmp/20000.png", "Slide text").unwrap();

    let deleted = delete_meeting(&config, &meeting.id).unwrap();
    assert!(deleted);

    assert!(
        list_screenshots(&config, &meeting.id).unwrap().is_empty(),
        "screenshots must cascade-delete with their meeting"
    );
}

// ── screenshots ──────────────────────────────────────────────────

#[test]
fn insert_screenshot_and_list_orders_by_captured_at_ms() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Slide deck review", "manual").unwrap();

    // Insert out of order to prove the read path sorts by captured_at_ms,
    // not insertion order.
    let second =
        insert_screenshot(&config, &meeting.id, 40_000, "/tmp/40000.png", "Second slide").unwrap();
    let first =
        insert_screenshot(&config, &meeting.id, 20_000, "/tmp/20000.png", "First slide").unwrap();

    let screenshots = list_screenshots(&config, &meeting.id).unwrap();
    assert_eq!(screenshots.len(), 2);
    assert_eq!(screenshots[0].id, first.id);
    assert_eq!(screenshots[0].ocr_text, "First slide");
    assert_eq!(screenshots[0].captured_at_ms, 20_000);
    assert_eq!(screenshots[1].id, second.id);
    assert_eq!(screenshots[1].ocr_text, "Second slide");
}

#[test]
fn list_screenshots_is_empty_for_meeting_with_no_screenshots() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Quiet meeting", "manual").unwrap();

    assert!(list_screenshots(&config, &meeting.id).unwrap().is_empty());
}

#[test]
fn delete_missing_meeting_returns_false() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let deleted = delete_meeting(&config, "does-not-exist").unwrap();
    assert!(!deleted);
}

// ── update_title ─────────────────────────────────────────────────

#[test]
fn update_title_changes_title() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Old title", "manual").unwrap();

    let updated = update_title(&config, &meeting.id, "New title").unwrap();
    assert_eq!(updated.id, meeting.id);
    assert_eq!(updated.title, "New title");

    let fetched = get_meeting(&config, &meeting.id).unwrap();
    assert_eq!(fetched.title, "New title");
}

#[test]
fn update_title_missing_meeting_errors() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = update_title(&config, "does-not-exist", "New title").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── finish ───────────────────────────────────────────────────────

#[test]
fn finish_meeting_sets_completed_status_and_duration() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Standup", "manual").unwrap();
    assert_eq!(meeting.status, MeetingStatus::Recording);

    let finished = finish_meeting(&config, &meeting.id, 42_000).unwrap();
    assert_eq!(finished.status, MeetingStatus::Completed);
    assert_eq!(finished.duration_ms, Some(42_000));
}

#[test]
fn finish_missing_meeting_errors() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = finish_meeting(&config, "does-not-exist", 1_000).unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── update_summary ───────────────────────────────────────────────

#[test]
fn update_summary_persists_summary_text() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Deep dive", "manual").unwrap();

    update_summary(&config, &meeting.id, "Key takeaway: ship it.").unwrap();

    let fetched = get_meeting(&config, &meeting.id).unwrap();
    assert_eq!(fetched.summary.as_deref(), Some("Key takeaway: ship it."));
}

#[test]
fn update_summary_missing_meeting_errors() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = update_summary(&config, "does-not-exist", "summary").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── search_transcripts ──────────────────────────────────────────

#[test]
fn search_transcripts_finds_only_the_matching_chunk() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Roadmap sync", "manual").unwrap();

    let roadmap_chunk = append_transcript_chunk(
        &config,
        &meeting.id,
        0,
        4_000,
        Some("alice"),
        "Let's discuss the quarterly roadmap.",
    )
    .unwrap();
    append_transcript_chunk(
        &config,
        &meeting.id,
        4_000,
        8_000,
        Some("bob"),
        "I think the budget needs revisiting.",
    )
    .unwrap();
    append_transcript_chunk(
        &config,
        &meeting.id,
        8_000,
        12_000,
        Some("carol"),
        "Sounds good, let's move to Q&A.",
    )
    .unwrap();

    let results = search_transcripts(&config, "roadmap", 50).unwrap();
    assert_eq!(
        results.len(),
        1,
        "only the chunk mentioning 'roadmap' should match"
    );
    assert_eq!(results[0].meeting_id, meeting.id);
    assert_eq!(results[0].meeting_title, "Roadmap sync");
    assert_eq!(results[0].chunk_id, roadmap_chunk.id);
    assert_eq!(results[0].start_ms, 0);
    assert!(
        results[0].snippet.contains("<mark>") && results[0].snippet.contains("</mark>"),
        "snippet should highlight the match: {}",
        results[0].snippet
    );

    let budget_results = search_transcripts(&config, "budget", 50).unwrap();
    assert_eq!(budget_results.len(), 1);
    assert_ne!(
        budget_results[0].chunk_id, roadmap_chunk.id,
        "'budget' must not match the roadmap chunk"
    );
}

#[test]
fn search_transcripts_blank_query_returns_no_results_without_error() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Blank query test", "manual").unwrap();
    append_transcript_chunk(&config, &meeting.id, 0, 1_000, None, "some content").unwrap();

    let results = search_transcripts(&config, "   ", 50).unwrap();
    assert!(results.is_empty());
}

#[test]
fn search_transcripts_malformed_query_degrades_to_zero_results_instead_of_erroring() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Punctuation test", "manual").unwrap();
    append_transcript_chunk(&config, &meeting.id, 0, 1_000, None, "This mentions a quote mark.")
        .unwrap();

    // Bare FTS5 operators / dangling punctuation would otherwise be a MATCH
    // syntax error; verify the literal-phrase retry recovers instead of
    // propagating a SQL error to the caller.
    let results = search_transcripts(&config, "***", 50).unwrap();
    assert!(
        results.is_empty(),
        "malformed input should degrade to zero results, not error"
    );

    let dangling_operator = search_transcripts(&config, "AND", 50).unwrap();
    assert!(dangling_operator.is_empty());
}

#[test]
fn search_transcripts_backfills_rows_missing_from_fts_index() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let meeting = create_meeting(&config, "Backfill check", "manual").unwrap();
    let chunk =
        append_transcript_chunk(&config, &meeting.id, 0, 1_000, None, "backfill sentinel text")
            .unwrap();

    // Simulate rows written before the FTS5 table/triggers existed by
    // stripping this chunk back out of the FTS index directly, bypassing the
    // normal insert path (which always keeps chunks and the FTS index in
    // sync via triggers).
    let db_path = config.workspace_dir.join("meetings").join("meetings.db");
    let raw = Connection::open(&db_path).unwrap();
    raw.execute(
        "INSERT INTO meeting_transcript_fts(meeting_transcript_fts, rowid, text) VALUES ('delete', ?1, ?2)",
        params![chunk.id, "backfill sentinel text"],
    )
    .unwrap();
    drop(raw);

    // search_transcripts opens a fresh connection, which re-runs the
    // idempotent backfill (see `backfill_missing_fts_rows`) before executing
    // the search — the chunk should be found again even though nothing
    // explicitly re-inserted it.
    let results = search_transcripts(&config, "sentinel", 50).unwrap();
    assert_eq!(
        results.len(),
        1,
        "backfill should restore the FTS row on the next connection open"
    );
    assert_eq!(results[0].chunk_id, chunk.id);
}
