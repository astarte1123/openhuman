use super::*;
use crate::aura::config::Config;
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

#[tokio::test]
async fn meetings_create_defaults_title_and_source() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let out = meetings_create(&config, None, None).await.unwrap();
    assert_eq!(out.value.title, "Untitled meeting");
    assert_eq!(out.value.source, "manual");
    assert!(out.logs.iter().any(|l| l.contains("meeting created")));
}

#[tokio::test]
async fn meetings_create_trims_and_uses_provided_fields() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let out = meetings_create(
        &config,
        Some("  Kickoff  ".to_string()),
        Some("zoom".to_string()),
    )
    .await
    .unwrap();
    assert_eq!(out.value.title, "Kickoff");
    assert_eq!(out.value.source, "zoom");
}

#[tokio::test]
async fn meetings_list_returns_created_meetings() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_list(&config).await.unwrap();
    assert!(out.value.iter().any(|m| m.id == created.id));
}

#[tokio::test]
async fn meetings_get_rejects_empty_meeting_id() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let err = meetings_get(&config, "  ").await.unwrap_err();
    assert!(err.contains("Missing 'meeting_id'"));
}

#[tokio::test]
async fn meetings_get_returns_meeting_and_chunks() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_append_transcript_chunk(&config, &created.id, 0, 1_000, "hello", None)
        .await
        .unwrap();

    let out = meetings_get(&config, &created.id).await.unwrap();
    assert_eq!(out.value.meeting.id, created.id);
    assert_eq!(out.value.chunks.len(), 1);
    assert_eq!(out.value.chunks[0].text, "hello");
}

#[tokio::test]
async fn meetings_update_title_rejects_empty_title() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let err = meetings_update_title(&config, &created.id, "   ")
        .await
        .unwrap_err();
    assert!(err.contains("must not be empty"));
}

#[tokio::test]
async fn meetings_update_title_renames_meeting() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_update_title(&config, &created.id, "Renamed")
        .await
        .unwrap();
    assert_eq!(out.value.title, "Renamed");
}

#[tokio::test]
async fn meetings_delete_reports_true_when_removed() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_delete(&config, &created.id).await.unwrap();
    assert_eq!(out.value["deleted"], serde_json::json!(true));
}

#[tokio::test]
async fn meetings_delete_reports_false_when_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let out = meetings_delete(&config, "does-not-exist").await.unwrap();
    assert_eq!(out.value["deleted"], serde_json::json!(false));
}

#[tokio::test]
async fn meetings_append_transcript_chunk_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_append_transcript_chunk(&config, "does-not-exist", 0, 1_000, "hi", None)
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_append_transcript_chunk_errors_when_meeting_not_recording() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_finish(&config, &created.id, 1_000).await.unwrap();

    let err = meetings_append_transcript_chunk(&config, &created.id, 0, 1_000, "hi", None)
        .await
        .unwrap_err();
    assert!(err.contains("meeting is not recording"));
}

#[tokio::test]
async fn meetings_finish_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_finish(&config, "does-not-exist", 1_000)
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_finish_errors_when_already_completed() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_finish(&config, &created.id, 1_000).await.unwrap();

    let err = meetings_finish(&config, &created.id, 2_000)
        .await
        .unwrap_err();
    assert!(err.contains("meeting is not recording"));
}

#[tokio::test]
async fn meetings_finish_marks_completed_with_duration() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_finish(&config, &created.id, 5_000).await.unwrap();
    assert_eq!(out.value.status, MeetingStatus::Completed);
    assert_eq!(out.value.duration_ms, Some(5_000));
}

#[tokio::test]
async fn meetings_summarize_errors_when_no_transcript() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let err = meetings_summarize(&config, &created.id).await.unwrap_err();
    assert!(err.contains("no transcript to summarize"));
}

#[tokio::test]
async fn meetings_summarize_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_summarize(&config, "does-not-exist")
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_search_rejects_empty_query() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_search(&config, "   ".to_string())
        .await
        .unwrap_err();
    assert!(err.contains("must not be empty"));
}

#[tokio::test]
async fn meetings_export_rejects_unsupported_format() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let err = meetings_export(&config, &created.id, "txt")
        .await
        .unwrap_err();
    assert!(
        err.contains("unsupported export format"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn meetings_export_rejects_missing_meeting_id() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_export(&config, "   ", "pdf").await.unwrap_err();
    assert!(err.contains("Missing 'meeting_id'"));
}

#[tokio::test]
async fn meetings_export_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_export(&config, "does-not-exist", "pdf")
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_export_pdf_writes_nonempty_file_and_returns_its_path() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, Some("Export me".to_string()), None)
        .await
        .unwrap()
        .value;
    meetings_append_transcript_chunk(&config, &created.id, 0, 1_000, "hello there", None)
        .await
        .unwrap();

    let out = meetings_export(&config, &created.id, "pdf").await.unwrap();
    let path = out.value["path"]
        .as_str()
        .expect("path in export result")
        .to_string();

    let metadata = std::fs::metadata(&path).expect("exported PDF file must exist on disk");
    assert!(metadata.len() > 0, "exported PDF must be non-empty");
    assert!(path.ends_with(".pdf"));
    assert!(out.logs.iter().any(|l| l.contains("meeting exported")));
}

#[tokio::test]
async fn meetings_export_docx_writes_nonempty_file_and_returns_its_path() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, Some("Export me too".to_string()), None)
        .await
        .unwrap()
        .value;
    meetings_append_transcript_chunk(&config, &created.id, 0, 1_000, "hello there", None)
        .await
        .unwrap();

    let out = meetings_export(&config, &created.id, "DOCX").await.unwrap();
    let path = out.value["path"]
        .as_str()
        .expect("path in export result")
        .to_string();

    let metadata = std::fs::metadata(&path).expect("exported DOCX file must exist on disk");
    assert!(metadata.len() > 0, "exported DOCX must be non-empty");
    // Format is lower-cased before building the file name, even though the
    // caller passed it upper-case.
    assert!(path.ends_with(".docx"));
}

#[tokio::test]
async fn meetings_export_works_with_zero_transcript_chunks() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_export(&config, &created.id, "pdf").await.unwrap();
    let path = out.value["path"]
        .as_str()
        .expect("path in export result")
        .to_string();
    let metadata = std::fs::metadata(&path).expect("exported PDF file must exist on disk");
    assert!(
        metadata.len() > 0,
        "exported PDF must be non-empty even with no transcript"
    );
}

// ── append_screenshot ────────────────────────────────────────────

/// Renders a small PNG (base64-encoded) containing `text`, using the same
/// embedded test font + drawing primitives as `ocr_tests.rs`.
fn base64_png_with_text(text: &str) -> String {
    use ab_glyph::FontRef;
    use image::{Rgb, RgbImage};
    use imageproc::drawing::draw_text_mut;

    const FONT_BYTES: &[u8] = include_bytes!("test_fixtures/DejaVuSansMono.ttf");
    let font = FontRef::try_from_slice(FONT_BYTES).expect("embedded test font must parse");
    let mut img = RgbImage::from_pixel(700, 120, Rgb([255, 255, 255]));
    draw_text_mut(&mut img, Rgb([0, 0, 0]), 10, 30, 48.0, &font, text);

    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .expect("encode synthetic test image as PNG");
    BASE64.encode(png_bytes)
}

#[tokio::test]
async fn meetings_append_screenshot_rejects_empty_meeting_id() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_append_screenshot(&config, "  ", 0, "AAAA")
        .await
        .unwrap_err();
    assert!(err.contains("Missing 'meeting_id'"));
}

#[tokio::test]
async fn meetings_append_screenshot_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_append_screenshot(&config, "does-not-exist", 0, "AAAA")
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_append_screenshot_errors_when_meeting_not_recording() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_finish(&config, &created.id, 1_000).await.unwrap();

    let err = meetings_append_screenshot(&config, &created.id, 0, "AAAA")
        .await
        .unwrap_err();
    assert!(err.contains("meeting is not recording"));
}

#[tokio::test]
async fn meetings_append_screenshot_rejects_invalid_base64() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let err = meetings_append_screenshot(&config, &created.id, 0, "not-valid-base64!!")
        .await
        .unwrap_err();
    assert!(
        err.contains("invalid 'image_base64'"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn meetings_append_screenshot_writes_file_runs_ocr_and_is_visible_via_get() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let image_base64 = base64_png_with_text("Slide Title");
    // Downloads the OCR models on first use (network required for this test).
    let out = meetings_append_screenshot(&config, &created.id, 20_000, &image_base64)
        .await
        .expect("append_screenshot should succeed");

    assert_eq!(out.value.meeting_id, created.id);
    assert_eq!(out.value.captured_at_ms, 20_000);
    assert!(
        std::path::Path::new(&out.value.image_path).is_file(),
        "screenshot PNG should be written to disk at {}",
        out.value.image_path
    );
    // Assert on "title" rather than "slide": this is a real OCR engine, not a
    // fixture-matching stub, and it can legitimately misread a single glyph
    // (e.g. "Slide" -> "Stide" — lowercase "l" confused for "t") without that
    // indicating a pipeline bug. "title" is checked instead because it isn't
    // adjacent to the ambiguous glyph in this fixture's text, so this test
    // still verifies the full append -> OCR -> store round trip without being
    // flaky on incidental OCR model imprecision.
    assert!(
        out.value.ocr_text.to_lowercase().contains("title"),
        "expected OCR text to mention 'title': {:?}",
        out.value.ocr_text
    );
    assert!(out.logs.iter().any(|l| l.contains("screenshot appended")));

    let fetched = meetings_get(&config, &created.id).await.unwrap();
    assert_eq!(fetched.value.screenshots.len(), 1);
    assert_eq!(fetched.value.screenshots[0].id, out.value.id);
}

#[tokio::test]
async fn meetings_search_finds_matching_chunk() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, Some("Search test".to_string()), None)
        .await
        .unwrap()
        .value;
    meetings_append_transcript_chunk(
        &config,
        &created.id,
        0,
        1_000,
        "we shipped the release",
        None,
    )
    .await
    .unwrap();
    meetings_append_transcript_chunk(
        &config,
        &created.id,
        1_000,
        2_000,
        "lunch plans for later",
        None,
    )
    .await
    .unwrap();

    let out = meetings_search(&config, "release".to_string())
        .await
        .unwrap();
    assert_eq!(out.value.len(), 1);
    assert_eq!(out.value[0].meeting_id, created.id);
    assert!(out.value[0].snippet.contains("<mark>"));
    assert!(out.logs.iter().any(|l| l.contains("1 result")));
}

// ── local capture (start/stop) ──────────────────────────────────────────
//
// These cover the validation branches only — anything that would reach
// `capture::default_backend()` needs real audio hardware and can't be
// exercised in a unit test (see the module doc comment on
// `capture::windows` for why, and CLAUDE.md's debug-logging rule for how
// failures there are meant to be diagnosed instead: verbose `[meetings-capture]`
// logs plus manual verification).

#[tokio::test]
async fn meetings_start_local_capture_rejects_empty_meeting_id() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_start_local_capture(&config, "   ", None)
        .await
        .unwrap_err();
    assert!(err.contains("Missing 'meeting_id'"));
}

#[tokio::test]
async fn meetings_start_local_capture_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_start_local_capture(&config, "does-not-exist", None)
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_start_local_capture_errors_when_meeting_not_recording() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_finish(&config, &created.id, 1_000).await.unwrap();

    let err = meetings_start_local_capture(&config, &created.id, None)
        .await
        .unwrap_err();
    assert!(err.contains("meeting is not recording"));
}

#[tokio::test]
async fn meetings_start_local_capture_rejects_meeting_recorder_source() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, Some("meeting_recorder".to_string()))
        .await
        .unwrap()
        .value;
    assert_eq!(created.status, MeetingStatus::Recording);

    let err = meetings_start_local_capture(&config, &created.id, None)
        .await
        .unwrap_err();
    assert!(err.contains("meeting_recorder"));

    // Confirm this really is a rejection, not a registry side effect: a
    // meeting the shell already owns must never occupy an `ACTIVE_CAPTURES`
    // slot as a result of this call.
    assert!(!ACTIVE_CAPTURES.lock().unwrap().contains_key(&created.id));
}

// `capture_screenshots` reaches the same "meeting is not recording" /
// "meeting not found" validation regardless of its value — these two cases
// confirm `Some(true)` / `Some(false)` are both accepted params that don't
// change (or skip) that validation. Whether a `capture_screenshots=false`
// session actually skips spawning the screenshot task can't be asserted
// here without real audio hardware to reach `capture::default_backend()`
// (see this file's "local capture" section doc comment) — that behavior is
// covered by `capture::screenshot`'s own unit tests plus manual
// verification.
#[tokio::test]
async fn meetings_start_local_capture_accepts_explicit_capture_screenshots_true() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;
    meetings_finish(&config, &created.id, 1_000).await.unwrap();

    let err = meetings_start_local_capture(&config, &created.id, Some(true))
        .await
        .unwrap_err();
    assert!(err.contains("meeting is not recording"));
}

#[tokio::test]
async fn meetings_start_local_capture_accepts_explicit_capture_screenshots_false() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_start_local_capture(&config, "does-not-exist", Some(false))
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_stop_local_capture_rejects_empty_meeting_id() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_stop_local_capture(&config, "  ")
        .await
        .unwrap_err();
    assert!(err.contains("Missing 'meeting_id'"));
}

#[tokio::test]
async fn meetings_stop_local_capture_errors_when_meeting_missing() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);

    let err = meetings_stop_local_capture(&config, "does-not-exist")
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn meetings_stop_local_capture_reports_false_when_nothing_active() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let out = meetings_stop_local_capture(&config, &created.id)
        .await
        .unwrap();
    assert_eq!(out.value["stopped"], serde_json::json!(false));
}

#[test]
fn force_stop_capture_returns_false_when_registry_empty() {
    let stopped = force_stop_capture("no-such-meeting").unwrap();
    assert!(!stopped);
}

// ── capture reservation / TOCTOU-race regression coverage ───────────────
//
// `ACTIVE_CAPTURES` is a process-wide static, so these tests manipulate it
// directly with distinctive, test-scoped keys and clean up after
// themselves so they don't bleed into other tests sharing the registry.
// They can't exercise the `CaptureSlot::Active` branch without real audio
// hardware to produce a `capture::CaptureHandle` (its constructor is
// private outside `capture`) — see the module doc comment atop this file's
// "local capture" section.

#[tokio::test]
async fn meetings_start_local_capture_rejects_when_reservation_already_starting() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    // Simulate a concurrent `meetings_start_local_capture` call that has
    // already reserved this meeting_id's slot (see `CaptureSlot::Starting`'s
    // doc comment on the check-then-insert race this closes) — without
    // needing real audio hardware to actually start a session.
    {
        let mut registry = ACTIVE_CAPTURES.lock().unwrap();
        registry.insert(created.id.clone(), CaptureSlot::Starting);
    }

    let err = meetings_start_local_capture(&config, &created.id, None)
        .await
        .unwrap_err();
    assert!(err.contains("already has an active local capture session"));

    ACTIVE_CAPTURES.lock().unwrap().remove(&created.id);
}

#[test]
fn force_stop_capture_leaves_starting_reservation_untouched() {
    let meeting_id = "force-stop-starting-test-meeting";
    ACTIVE_CAPTURES
        .lock()
        .unwrap()
        .insert(meeting_id.to_string(), CaptureSlot::Starting);

    let stopped = force_stop_capture(meeting_id).unwrap();
    assert!(!stopped, "a Starting reservation has no handle to stop yet");
    assert!(
        matches!(
            ACTIVE_CAPTURES.lock().unwrap().get(meeting_id),
            Some(CaptureSlot::Starting)
        ),
        "force_stop_capture must not remove a Starting slot it can't stop"
    );

    ACTIVE_CAPTURES.lock().unwrap().remove(meeting_id);
}

#[test]
fn release_capture_reservation_removes_starting_slot() {
    let meeting_id = "release-reservation-test-meeting";
    ACTIVE_CAPTURES
        .lock()
        .unwrap()
        .insert(meeting_id.to_string(), CaptureSlot::Starting);

    release_capture_reservation(meeting_id);

    assert!(ACTIVE_CAPTURES.lock().unwrap().get(meeting_id).is_none());
}

#[test]
fn release_capture_reservation_is_a_no_op_when_nothing_reserved() {
    // Just asserts this doesn't panic when the key is absent.
    release_capture_reservation("no-such-meeting-for-release-test");
}

#[tokio::test]
async fn stop_all_active_captures_drains_starting_reservations() {
    let meeting_id = "shutdown-hook-starting-test-meeting";
    ACTIVE_CAPTURES
        .lock()
        .unwrap()
        .insert(meeting_id.to_string(), CaptureSlot::Starting);

    stop_all_active_captures().await;

    assert!(ACTIVE_CAPTURES.lock().unwrap().get(meeting_id).is_none());
}

#[test]
fn register_shutdown_hook_once_is_idempotent() {
    // Calling this more than once (e.g. two overlapping
    // `meetings_start_local_capture` calls) must not panic or double-register
    // the hook.
    register_shutdown_hook_once();
    register_shutdown_hook_once();
}

#[tokio::test]
async fn meetings_finish_and_delete_tolerate_no_active_capture() {
    // meetings_finish / meetings_delete both call force_stop_capture()
    // unconditionally now — this exercises that path with nothing
    // registered, i.e. the overwhelmingly common case (most meetings finish
    // without ever using local capture).
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    let created = meetings_create(&config, None, None).await.unwrap().value;

    let finished = meetings_finish(&config, &created.id, 1_000).await.unwrap();
    assert_eq!(finished.value.status, MeetingStatus::Completed);

    let deleted = meetings_delete(&config, &created.id).await.unwrap();
    assert_eq!(deleted.value["deleted"], serde_json::json!(true));
}
