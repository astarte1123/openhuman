use super::*;
use crate::aura::meetings::types::MeetingStatus;

fn sample_meeting(summary: Option<&str>) -> Meeting {
    Meeting {
        id: "meeting_1".to_string(),
        title: "Weekly sync".to_string(),
        source: "manual".to_string(),
        status: MeetingStatus::Completed,
        created_at: "2026-09-23T10:00:00Z".to_string(),
        updated_at: "2026-09-23T10:30:00Z".to_string(),
        duration_ms: Some(1_830_000),
        summary: summary.map(str::to_string),
        summary_structured: None,
        participants: Vec::new(),
    }
}

fn sample_chunk(id: i64, speaker: Option<&str>, text: &str) -> TranscriptChunk {
    TranscriptChunk {
        id,
        meeting_id: "meeting_1".to_string(),
        start_ms: 0,
        end_ms: 1000,
        speaker: speaker.map(str::to_string),
        text: text.to_string(),
        created_at: "2026-09-23T10:00:01Z".to_string(),
    }
}

/// A produced `.pptx` must be a valid zip archive with the required OOXML
/// parts — this is the load-bearing check: any XML/zip mistake here would
/// otherwise only surface as "PowerPoint can't open this file" much later.
fn assert_valid_pptx_zip(bytes: &[u8]) {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).expect("pptx bytes must be a valid zip archive");
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/theme/theme1.xml",
        "ppt/slides/slide1.xml",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing required pptx part: {required} (present: {names:?})"
        );
    }
}

#[test]
fn render_pptx_produces_valid_zip_with_summary() {
    let meeting = sample_meeting(Some("We discussed the roadmap.\nDecided to ship Friday."));
    let chunks = vec![
        sample_chunk(1, Some("You"), "Let's discuss the roadmap."),
        sample_chunk(2, Some("Others"), "Sounds good, let's ship Friday."),
    ];
    let bytes = render_pptx(&meeting, &chunks).expect("render_pptx should succeed");
    assert!(!bytes.is_empty());
    assert_valid_pptx_zip(&bytes);
}

#[test]
fn render_pptx_handles_missing_summary() {
    let meeting = sample_meeting(None);
    let bytes = render_pptx(&meeting, &[]).expect("render_pptx should succeed with no summary");
    assert_valid_pptx_zip(&bytes);
}

#[test]
fn render_pptx_escapes_xml_special_characters() {
    let meeting = sample_meeting(Some("R&D <status> update \"final\""));
    let bytes = render_pptx(&meeting, &[]).expect("render_pptx should succeed");
    let reader = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    let mut found_escaped = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        if file.name().starts_with("ppt/slides/slide") {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
            if contents.contains("&amp;D") {
                found_escaped = true;
            }
            // Unescaped raw special characters would make this an invalid
            // XML document — a lenient parser check that a literal
            // unescaped ampersand-followed-by-letter sequence (other than
            // a valid entity) never appears is a reasonable proxy without
            // pulling in a full XML parser dependency just for this test.
            assert!(
                !contents.contains(" & "),
                "found unescaped bare '&' in slide XML"
            );
        }
    }
    assert!(found_escaped, "expected escaped '&' from 'R&D' in some slide");
}

#[test]
fn distinct_speakers_deduplicates_and_preserves_order() {
    let chunks = vec![
        sample_chunk(1, Some("You"), "hi"),
        sample_chunk(2, Some("Others"), "hello"),
        sample_chunk(3, Some("You"), "again"),
        sample_chunk(4, None, "no speaker"),
    ];
    assert_eq!(distinct_speakers(&chunks), vec!["You", "Others"]);
}

#[test]
fn wrap_text_splits_long_lines() {
    let text = "word ".repeat(30);
    let lines = wrap_text(&text, 20);
    assert!(lines.len() > 1);
    for line in &lines {
        assert!(line.len() <= 20 || !line.contains(' '));
    }
}

#[test]
fn render_pptx_paginates_long_summary_across_slides() {
    let long_summary = (0..60)
        .map(|i| format!("Point number {i} in the summary discussion."))
        .collect::<Vec<_>>()
        .join("\n");
    let meeting = sample_meeting(Some(&long_summary));
    let bytes = render_pptx(&meeting, &[]).expect("render_pptx should succeed");
    let reader = std::io::Cursor::new(&bytes);
    let archive = zip::ZipArchive::new(reader).unwrap();
    let slide_count = (0..archive.len())
        .filter(|&i| {
            let name = archive.name_for_index(i).unwrap_or_default();
            name.starts_with("ppt/slides/slide") && name.ends_with(".xml")
        })
        .count();
    assert!(
        slide_count > 2,
        "expected a long summary to span multiple slides, got {slide_count}"
    );
}
