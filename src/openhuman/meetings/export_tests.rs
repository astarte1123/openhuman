use super::*;
use crate::openhuman::meetings::types::MeetingStatus;

fn sample_meeting() -> Meeting {
    Meeting {
        id: "meeting_export_test".into(),
        title: "Sprint planning".into(),
        source: "manual".into(),
        status: MeetingStatus::Completed,
        created_at: "2026-01-01T00:00:00+00:00".into(),
        updated_at: "2026-01-01T00:10:00+00:00".into(),
        duration_ms: Some(9_000),
        summary: Some("We reviewed the backlog and assigned tickets.".into()),
    }
}

fn sample_chunks() -> Vec<TranscriptChunk> {
    vec![
        TranscriptChunk {
            id: 1,
            meeting_id: "meeting_export_test".into(),
            start_ms: 0,
            end_ms: 4_000,
            speaker: Some("alice".into()),
            text: "Let's review the backlog.".into(),
            created_at: "2026-01-01T00:00:01+00:00".into(),
        },
        TranscriptChunk {
            id: 2,
            meeting_id: "meeting_export_test".into(),
            start_ms: 4_000,
            end_ms: 9_000,
            speaker: None,
            text: "Sounds good, I will take the first ticket.".into(),
            created_at: "2026-01-01T00:00:05+00:00".into(),
        },
    ]
}

// ── render_pdf ───────────────────────────────────────────────────

#[test]
fn render_pdf_produces_nonempty_pdf_bytes_with_chunks_and_summary() {
    let meeting = sample_meeting();
    let chunks = sample_chunks();

    let bytes = render_pdf(&meeting, &chunks).unwrap();

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF-"), "output must be a PDF file");
}

#[test]
fn render_pdf_handles_zero_chunks_without_panicking() {
    let mut meeting = sample_meeting();
    meeting.summary = None;

    let bytes = render_pdf(&meeting, &[]).unwrap();

    assert!(!bytes.is_empty());
    assert!(
        bytes.starts_with(b"%PDF-"),
        "output must still be a valid PDF file"
    );
}

#[test]
fn render_pdf_paginates_a_very_long_transcript() {
    let meeting = sample_meeting();
    // Enough chunks (each wrapping to multiple lines) to overflow a single
    // A4 page and force at least one page break through `PdfLayout`.
    let chunks: Vec<TranscriptChunk> = (0..400)
        .map(|i| TranscriptChunk {
            id: i,
            meeting_id: meeting.id.clone(),
            start_ms: i * 1_000,
            end_ms: (i + 1) * 1_000,
            speaker: Some("alice".into()),
            text: "This is a moderately long transcript line meant to force pagination.".into(),
            created_at: meeting.created_at.clone(),
        })
        .collect();

    let bytes = render_pdf(&meeting, &chunks).unwrap();

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF-"));
}

// ── render_docx ──────────────────────────────────────────────────

#[test]
fn render_docx_produces_nonempty_docx_bytes_with_chunks_and_summary() {
    let meeting = sample_meeting();
    let chunks = sample_chunks();

    let bytes = render_docx(&meeting, &chunks).unwrap();

    assert!(!bytes.is_empty());
    // DOCX is a zip archive; every zip starts with the local-file-header
    // signature "PK\x03\x04".
    assert!(
        bytes.starts_with(b"PK\x03\x04"),
        "output must be a zip/docx file"
    );
}

#[test]
fn render_docx_handles_zero_chunks_without_panicking() {
    let mut meeting = sample_meeting();
    meeting.summary = None;

    let bytes = render_docx(&meeting, &[]).unwrap();

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

// ── helpers ──────────────────────────────────────────────────────

#[test]
fn transcript_line_uses_speaker_and_unknown_fallback() {
    let with_speaker = TranscriptChunk {
        id: 1,
        meeting_id: "m".into(),
        start_ms: 0,
        end_ms: 1_000,
        speaker: Some("bob".into()),
        text: "hello".into(),
        created_at: "2026-01-01T00:00:00+00:00".into(),
    };
    assert_eq!(transcript_line(&with_speaker), "[bob]: hello");

    let without_speaker = TranscriptChunk {
        speaker: None,
        ..with_speaker
    };
    assert_eq!(transcript_line(&without_speaker), "[unknown]: hello");
}

#[test]
fn non_empty_summary_filters_blank_and_missing() {
    let mut meeting = sample_meeting();
    meeting.summary = None;
    assert_eq!(non_empty_summary(&meeting), None);

    meeting.summary = Some("   ".into());
    assert_eq!(non_empty_summary(&meeting), None);

    meeting.summary = Some("  Real summary  ".into());
    assert_eq!(non_empty_summary(&meeting), Some("Real summary"));
}

#[test]
fn format_duration_ms_formats_minutes_and_hours() {
    assert_eq!(format_duration_ms(5_000), "0:05");
    assert_eq!(format_duration_ms(65_000), "1:05");
    assert_eq!(format_duration_ms(3_661_000), "1:01:01");
    assert_eq!(format_duration_ms(-500), "0:00");
}

#[test]
fn format_meta_line_includes_duration_only_when_present() {
    let mut meeting = sample_meeting();
    meeting.duration_ms = None;
    let line = format_meta_line(&meeting);
    assert!(line.contains("Source: manual"));
    assert!(line.contains("Created: 2026-01-01T00:00:00+00:00"));
    assert!(!line.contains("Duration:"));

    meeting.duration_ms = Some(9_000);
    assert!(format_meta_line(&meeting).contains("Duration: 0:09"));
}

#[test]
fn wrap_text_breaks_on_word_boundaries_within_budget() {
    let text = "one two three four five six seven eight nine ten";
    let lines = wrap_text(text, 12);
    assert!(lines.iter().all(|l| l.len() <= 12 || !l.contains(' ')));
    assert_eq!(lines.join(" "), text);
}

#[test]
fn wrap_text_empty_input_yields_single_blank_line() {
    assert_eq!(wrap_text("", 80), vec![String::new()]);
    assert_eq!(wrap_text("   ", 80), vec![String::new()]);
}
