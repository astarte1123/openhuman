//! Data types for the meetings domain: meeting metadata and transcript chunks.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a meeting recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    /// Actively capturing a live meeting; transcript chunks may still be appended.
    Recording,
    /// Recording finished successfully.
    Completed,
    /// Recording ended abnormally.
    Error,
}

impl MeetingStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Completed => "completed",
            Self::Error => "error",
        }
    }

    /// Parses a raw DB value, falling back to [`MeetingStatus::Recording`] (the
    /// table default) for anything unrecognized.
    pub(crate) fn parse(raw: &str) -> Self {
        match raw {
            "completed" => Self::Completed,
            "error" => Self::Error,
            _ => Self::Recording,
        }
    }
}

/// A single meeting recording session and its lifecycle metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub source: String,
    pub status: MeetingStatus,
    /// RFC3339 timestamp.
    pub created_at: String,
    /// RFC3339 timestamp.
    pub updated_at: String,
    pub duration_ms: Option<i64>,
    /// Free-form prose summary. Predates [`MeetingSummary`] (Block 4) — every
    /// meeting summarized before that still has *only* this field populated,
    /// and every meeting summarized after it still gets this field populated
    /// too (rendered from `summary_structured` by
    /// `ops::render_structured_summary_prose`), so old and new clients alike
    /// can always display a sensible flat summary without knowing about
    /// `summary_structured`. See `store::update_summary_structured`'s doc
    /// comment.
    pub summary: Option<String>,
    /// Structured decisions + action items extracted alongside `summary` by
    /// `aura.meetings_summarize_structured` (Block 4). `None` for a meeting
    /// that was never (re-)summarized after this field was introduced, or
    /// whose stored JSON failed to parse (see
    /// `store::parse_summary_structured_column`) — both cases fall back to
    /// treating the meeting as if it only ever had a plain-text `summary`.
    pub summary_structured: Option<MeetingSummary>,
}

/// One action item extracted from a meeting's structured summary: what needs
/// doing, plus who's on the hook for it, if the transcript made that clear.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionItem {
    pub text: String,
    /// Owner name/handle as mentioned in the transcript, if the model could
    /// attribute the action item to someone. `None` when unclear rather than
    /// guessing — an incorrectly-attributed action item is worse than an
    /// unattributed one.
    pub owner: Option<String>,
}

/// Structured meeting summary: a prose overview plus explicit decisions and
/// action items, as opposed to [`Meeting::summary`]'s flat prose. Produced by
/// `ops::meetings_summarize_structured` (Block 4) and persisted (as JSON) in
/// the `meetings.summary_structured` column via
/// `store::update_summary_structured`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MeetingSummary {
    /// Short prose recap of the discussion, same register as the old
    /// LLM-generated `Meeting::summary` prose.
    pub overview: String,
    /// Concrete decisions made during the meeting, one per entry.
    pub decisions: Vec<String>,
    /// Action items called out during the meeting, with an owner when the
    /// transcript made one clear.
    pub action_items: Vec<ActionItem>,
}

/// One ordered slice of a meeting's transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptChunk {
    pub id: i64,
    pub meeting_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// `"microphone"` or `"system_audio"` from live capture (see
    /// `capture::AudioTrack::as_str`). Under the optional `diarize` build
    /// feature, a completed meeting's `"system_audio"` chunks can be
    /// rewritten to `"system_audio:speaker_N"` (one label per distinct
    /// remote voice identified) by the `aura.meetings_diarize` RPC — see
    /// `crate::aura::meetings::diarize`. `"microphone"` chunks are never
    /// diarized (there is exactly one local speaker on that track).
    pub speaker: Option<String>,
    pub text: String,
    /// RFC3339 timestamp.
    pub created_at: String,
}

/// One periodically-captured screenshot of a meeting's shared slides, plus
/// its OCR'd text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingScreenshot {
    pub id: i64,
    pub meeting_id: String,
    /// Milliseconds since the meeting started, matching how
    /// [`TranscriptChunk::start_ms`] is measured.
    pub captured_at_ms: i64,
    /// Absolute path of the saved PNG on disk.
    pub image_path: String,
    pub ocr_text: String,
    /// RFC3339 timestamp.
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_status_parse_and_as_str_roundtrip() {
        assert_eq!(MeetingStatus::parse("recording").as_str(), "recording");
        assert_eq!(MeetingStatus::parse("completed").as_str(), "completed");
        assert_eq!(MeetingStatus::parse("error").as_str(), "error");
    }

    #[test]
    fn meeting_status_parse_falls_back_to_recording_for_unknown() {
        assert_eq!(MeetingStatus::parse(""), MeetingStatus::Recording);
        assert_eq!(MeetingStatus::parse("garbage"), MeetingStatus::Recording);
    }

    #[test]
    fn meeting_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&MeetingStatus::Recording).unwrap(),
            "\"recording\""
        );
        assert_eq!(
            serde_json::to_string(&MeetingStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&MeetingStatus::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn meeting_roundtrips_through_json() {
        let meeting = Meeting {
            id: "meeting_1".into(),
            title: "Standup".into(),
            source: "manual".into(),
            status: MeetingStatus::Recording,
            created_at: "2026-01-01T00:00:00+00:00".into(),
            updated_at: "2026-01-01T00:00:00+00:00".into(),
            duration_ms: None,
            summary: None,
            summary_structured: None,
        };
        let v = serde_json::to_value(&meeting).unwrap();
        assert_eq!(v["status"], "recording");
        let back: Meeting = serde_json::from_value(v).unwrap();
        assert_eq!(back.id, meeting.id);
        assert_eq!(back.status, meeting.status);
        assert_eq!(back.summary_structured, None);
    }

    #[test]
    fn meeting_with_structured_summary_roundtrips_through_json() {
        let meeting = Meeting {
            id: "meeting_2".into(),
            title: "Planning".into(),
            source: "manual".into(),
            status: MeetingStatus::Completed,
            created_at: "2026-01-01T00:00:00+00:00".into(),
            updated_at: "2026-01-01T00:00:00+00:00".into(),
            duration_ms: Some(60_000),
            summary: Some("We discussed the roadmap.".into()),
            summary_structured: Some(MeetingSummary {
                overview: "We discussed the roadmap.".into(),
                decisions: vec!["Ship v2 next sprint".into()],
                action_items: vec![ActionItem {
                    text: "Write the migration guide".into(),
                    owner: Some("Alex".into()),
                }],
            }),
        };
        let v = serde_json::to_value(&meeting).unwrap();
        let back: Meeting = serde_json::from_value(v).unwrap();
        assert_eq!(back.summary_structured, meeting.summary_structured);
        let structured = back.summary_structured.unwrap();
        assert_eq!(structured.decisions, vec!["Ship v2 next sprint"]);
        assert_eq!(structured.action_items[0].owner.as_deref(), Some("Alex"));
    }

    #[test]
    fn meeting_summary_default_is_empty() {
        let summary = MeetingSummary::default();
        assert_eq!(summary.overview, "");
        assert!(summary.decisions.is_empty());
        assert!(summary.action_items.is_empty());
    }

    #[test]
    fn action_item_without_owner_roundtrips_as_null() {
        let item = ActionItem {
            text: "Follow up with legal".into(),
            owner: None,
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["owner"], serde_json::Value::Null);
        let back: ActionItem = serde_json::from_value(v).unwrap();
        assert_eq!(back, item);
    }

    #[test]
    fn meeting_screenshot_roundtrips_through_json() {
        let screenshot = MeetingScreenshot {
            id: 1,
            meeting_id: "meeting_1".into(),
            captured_at_ms: 20_000,
            image_path: "/tmp/meeting_1/20000.png".into(),
            ocr_text: "Slide title".into(),
            created_at: "2026-01-01T00:00:00+00:00".into(),
        };
        let v = serde_json::to_value(&screenshot).unwrap();
        assert_eq!(v["captured_at_ms"], 20_000);
        let back: MeetingScreenshot = serde_json::from_value(v).unwrap();
        assert_eq!(back.id, screenshot.id);
        assert_eq!(back.ocr_text, screenshot.ocr_text);
    }
}
