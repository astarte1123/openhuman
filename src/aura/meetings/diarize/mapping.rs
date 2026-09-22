//! Pure segment-to-chunk mapping — no ONNX, no I/O, fully unit-testable
//! without any model file on disk. This is the primary coverage-bearing
//! surface for the `diarize` feature's merge gate: [`engine`](super::engine)
//! needs a real ONNX runtime and model weights to exercise, but this module
//! needs neither.
//!
//! [`map_segments_to_chunks`] takes the speaker segments
//! [`super::engine::diarize_wav`] produced (start/end in seconds, one
//! opaque `speaker` label per segment) and a meeting's already-persisted
//! [`TranscriptChunk`] rows, and decides which chunks should have their
//! `speaker` field rewritten from `"system_audio"` to
//! `"system_audio:{label}"`.

use crate::aura::meetings::types::TranscriptChunk;

use super::engine::SpeakerSegment;

/// The only speaker label diarization is ever applied on top of — see
/// [`crate::aura::meetings::capture`]'s doc comment on why only
/// `system_audio` is diarized (the microphone track is the single local
/// user).
const DIARIZABLE_SPEAKER: &str = "system_audio";

/// For every chunk in `chunks` whose current `speaker` is exactly
/// `"system_audio"`, finds the diarization segment in `segments` with the
/// greatest time overlap (chunk `start_ms`/`end_ms` vs. segment
/// `start_secs`/`end_secs` converted to milliseconds) and, if any segment
/// overlaps at all, returns `(chunk.id, "system_audio:{segment.speaker}")`.
///
/// Chunks with no overlapping segment (e.g. the diarization pass found no
/// speech in that time range — a gap, or audio that didn't make it into the
/// WAV) are left out of the result entirely, rather than guessing; callers
/// should leave those chunks' `speaker` unchanged.
///
/// Chunks whose `speaker` is anything other than exactly `"system_audio"`
/// (already-diarized on a previous run, `"microphone"`, `None`, ...) are
/// never touched — this function is purely additive over a single
/// diarization pass.
pub fn map_segments_to_chunks(
    chunks: &[TranscriptChunk],
    segments: &[SpeakerSegment],
) -> Vec<(i64, String)> {
    let mut updates = Vec::new();

    for chunk in chunks {
        if chunk.speaker.as_deref() != Some(DIARIZABLE_SPEAKER) {
            continue;
        }

        let Some(best) = best_overlapping_segment(chunk, segments) else {
            continue;
        };

        updates.push((chunk.id, format!("{DIARIZABLE_SPEAKER}:{}", best.speaker)));
    }

    updates
}

/// Returns the segment in `segments` with the greatest positive time
/// overlap against `chunk`, or `None` if no segment overlaps at all. Ties
/// (equal overlap) keep whichever segment was seen first — segments are
/// expected to already be in chronological, non-overlapping order from
/// [`super::engine::diarize_wav`], so a genuine tie should not occur in
/// practice.
fn best_overlapping_segment<'a>(
    chunk: &TranscriptChunk,
    segments: &'a [SpeakerSegment],
) -> Option<&'a SpeakerSegment> {
    let chunk_start_ms = chunk.start_ms as f64;
    let chunk_end_ms = chunk.end_ms as f64;

    segments
        .iter()
        .filter_map(|seg| {
            let seg_start_ms = seg.start_secs * 1000.0;
            let seg_end_ms = seg.end_secs * 1000.0;
            let overlap_ms = (chunk_end_ms.min(seg_end_ms) - chunk_start_ms.max(seg_start_ms))
                .max(0.0);
            (overlap_ms > 0.0).then_some((seg, overlap_ms))
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(seg, _)| seg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: i64, start_ms: i64, end_ms: i64, speaker: Option<&str>) -> TranscriptChunk {
        TranscriptChunk {
            id,
            meeting_id: "meeting_1".into(),
            start_ms,
            end_ms,
            speaker: speaker.map(str::to_string),
            text: "hello".into(),
            created_at: "2026-01-01T00:00:00+00:00".into(),
        }
    }

    fn segment(speaker: &str, start_secs: f64, end_secs: f64) -> SpeakerSegment {
        SpeakerSegment {
            speaker: speaker.to_string(),
            start_secs,
            end_secs,
        }
    }

    #[test]
    fn assigns_the_fully_overlapping_segment() {
        let chunks = vec![chunk(1, 0, 2_000, Some("system_audio"))];
        let segments = vec![segment("speaker_1", 0.0, 3.0)];

        let updates = map_segments_to_chunks(&chunks, &segments);
        assert_eq!(updates, vec![(1, "system_audio:speaker_1".to_string())]);
    }

    #[test]
    fn picks_the_segment_with_greatest_overlap_among_several() {
        // Chunk spans 0..5000ms. speaker_1 covers 0..1000ms (1s overlap),
        // speaker_2 covers 1000..5000ms (4s overlap) — speaker_2 should win.
        let chunks = vec![chunk(1, 0, 5_000, Some("system_audio"))];
        let segments = vec![
            segment("speaker_1", 0.0, 1.0),
            segment("speaker_2", 1.0, 5.0),
        ];

        let updates = map_segments_to_chunks(&chunks, &segments);
        assert_eq!(updates, vec![(1, "system_audio:speaker_2".to_string())]);
    }

    #[test]
    fn leaves_non_system_audio_chunks_untouched() {
        let chunks = vec![
            chunk(1, 0, 2_000, Some("microphone")),
            chunk(2, 0, 2_000, None),
            chunk(3, 0, 2_000, Some("system_audio:speaker_1")),
        ];
        let segments = vec![segment("speaker_9", 0.0, 5.0)];

        assert!(map_segments_to_chunks(&chunks, &segments).is_empty());
    }

    #[test]
    fn chunk_with_no_overlapping_segment_is_left_out() {
        let chunks = vec![chunk(1, 10_000, 12_000, Some("system_audio"))];
        let segments = vec![segment("speaker_1", 0.0, 1.0)];

        assert!(map_segments_to_chunks(&chunks, &segments).is_empty());
    }

    #[test]
    fn empty_segments_produces_no_updates() {
        let chunks = vec![chunk(1, 0, 2_000, Some("system_audio"))];
        assert!(map_segments_to_chunks(&chunks, &[]).is_empty());
    }

    #[test]
    fn multiple_chunks_each_get_their_own_best_match() {
        let chunks = vec![
            chunk(1, 0, 1_000, Some("system_audio")),
            chunk(2, 1_000, 2_000, Some("system_audio")),
        ];
        let segments = vec![
            segment("speaker_1", 0.0, 1.0),
            segment("speaker_2", 1.0, 2.0),
        ];

        let mut updates = map_segments_to_chunks(&chunks, &segments);
        updates.sort_by_key(|(id, _)| *id);
        assert_eq!(
            updates,
            vec![
                (1, "system_audio:speaker_1".to_string()),
                (2, "system_audio:speaker_2".to_string()),
            ]
        );
    }
}
