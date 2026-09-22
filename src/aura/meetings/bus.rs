//! Event-bus publishing for the meetings domain.
//!
//! This is the domain's first event producer (Block 1 of the local-meeting
//! -capture design) — nothing inside `aura` subscribes to meeting events
//! yet, so there's no `<Purpose>Subscriber` here the way `cron::bus` /
//! `webhooks::bus` have one (both of those exist because something already
//! consumes their events). This file exists anyway as the one obvious place
//! for a future subscriber (e.g. a live-summary updater reacting to
//! transcript chunks as they arrive) to add an `EventHandler` impl, instead
//! of scattering `publish_global` calls inline in `capture::pipeline`.

use crate::aura::meetings::capture::AudioTrack;
use crate::aura::meetings::types::TranscriptChunk;
use crate::core::event_bus::{publish_global, DomainEvent};

const LOG_PREFIX: &str = "[meetings-capture]";

/// Publishes a [`DomainEvent::MeetingTranscriptChunkReceived`] for one
/// persisted transcript chunk.
///
/// Called by [`crate::aura::meetings::capture::pipeline`] right after
/// `ops::meetings_append_transcript_chunk` succeeds, so subscribers only
/// ever observe chunks that are already durably stored — there's no
/// "event fired but the write failed" race to handle downstream.
pub fn publish_transcript_chunk(meeting_id: &str, chunk: &TranscriptChunk, track: AudioTrack) {
    log::debug!(
        "{LOG_PREFIX} publishing MeetingTranscriptChunkReceived meeting_id={meeting_id} \
         chunk_id={} track={} text_chars={}",
        chunk.id,
        track.as_str(),
        chunk.text.len()
    );
    publish_global(DomainEvent::MeetingTranscriptChunkReceived {
        meeting_id: meeting_id.to_string(),
        chunk_id: chunk.id,
        start_ms: chunk.start_ms,
        end_ms: chunk.end_ms,
        text: chunk.text.clone(),
        speaker: chunk.speaker.clone(),
        track: track.as_str().to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chunk() -> TranscriptChunk {
        TranscriptChunk {
            id: 42,
            meeting_id: "meeting_1".into(),
            start_ms: 1_000,
            end_ms: 5_000,
            speaker: Some("microphone".into()),
            text: "hello team".into(),
            created_at: "2026-01-01T00:00:00+00:00".into(),
        }
    }

    #[test]
    fn publish_transcript_chunk_does_not_panic_without_a_subscriber() {
        // No event-bus subscriber is registered in this unit-test process —
        // publish_global() is documented as a safe no-op broadcast when the
        // global bus was never initialized (src/core/event_bus/bus.rs).
        publish_transcript_chunk("meeting_1", &sample_chunk(), AudioTrack::Microphone);
        publish_transcript_chunk("meeting_1", &sample_chunk(), AudioTrack::SystemAudio);
    }
}
