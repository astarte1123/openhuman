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
    pub summary: Option<String>,
}

/// One ordered slice of a meeting's transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptChunk {
    pub id: i64,
    pub meeting_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
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
        };
        let v = serde_json::to_value(&meeting).unwrap();
        assert_eq!(v["status"], "recording");
        let back: Meeting = serde_json::from_value(v).unwrap();
        assert_eq!(back.id, meeting.id);
        assert_eq!(back.status, meeting.status);
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
