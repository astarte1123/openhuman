//! Bridges raw [`AudioChunk`]s from a capture backend to whisper + storage.
//!
//! [`run_capture_session`] wires a backend's realtime callback thread(s) to
//! an async tokio task ([`run_consumer_loop`]) that owns all the async work:
//! buffering, resampling, calling into whisper (off-thread via
//! `spawn_blocking`, mirroring `LocalAiService::transcribe_in_process_inner`
//! in `inference/local/service/speech.rs`), persisting via
//! `ops::meetings_append_transcript_chunk`, and publishing
//! `DomainEvent::MeetingTranscriptChunkReceived` via [`super::super::bus`].
//!
//! [`CaptureSession`]'s buffering/draining logic ([`CaptureSession::push`],
//! [`CaptureSession::drain_ready`], [`CaptureSession::drain_all`]) is pure —
//! no I/O, no whisper, no SQLite — specifically so it can be unit tested
//! without real audio hardware. The whisper/SQLite/event-bus side
//! ([`flush_and_transcribe`]) cannot be meaningfully unit tested here (it
//! needs a loaded whisper model and a live meeting row) and needs
//! hardware/integration verification instead.

use std::collections::HashMap;
use std::time::Duration;

use log::{debug, warn};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::aura::config::Config;
use crate::aura::inference::local::whisper_engine::{self, WhisperEngineHandle};
use crate::aura::meetings::bus;
use crate::aura::meetings::ops;
use crate::aura::voice::audio_capture::resample;

use super::{AudioCaptureBackend, AudioChunk, AudioTrack, CaptureHandle, LOG_PREFIX};

/// Whisper requires 16kHz mono input — see `whisper_engine::transcribe_pcm_f32`.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// How often the consumer loop flushes buffered audio to whisper when new
/// chunks keep arriving. On session end (capture channel closed) everything
/// buffered is flushed immediately regardless of this interval.
const FLUSH_INTERVAL: Duration = Duration::from_secs(4);

/// Minimum buffered samples (at [`WHISPER_SAMPLE_RATE`]) before a periodic
/// flush actually transcribes a track — avoids firing whisper on a near-empty
/// buffer right after a flush tick lands just after session start. 0.5s.
const MIN_FLUSH_SAMPLES: usize = (WHISPER_SAMPLE_RATE / 2) as usize;

/// Starts a capture backend and spawns the async consumer task that turns
/// its output into persisted, event-published transcript chunks.
///
/// Returns the [`CaptureHandle`] immediately once the backend's OS
/// thread(s) report they're up — the consumer loop keeps running in a
/// detached tokio task until the capture channel closes (i.e. until
/// `capture_handle.stop()` is called and the backend's threads exit,
/// dropping the channel's sender side).
///
/// `backend.start()` is synchronous and, on every current backend, blocks
/// on real device/permission I/O (OS permission checks, possibly shelling
/// out to open a settings prompt, device enumeration, `stream.play()`) via
/// a blocking rendezvous with the OS capture thread(s) it spawns — see
/// `capture::windows::spawn_mic_stream`. Running that inline on the async
/// caller's tokio worker thread would stall whatever else that worker is
/// scheduled to run for the duration of device setup, so it's offloaded to
/// `spawn_blocking`, mirroring [`flush_and_transcribe`]'s use of
/// `spawn_blocking` for its own blocking whisper call.
pub async fn run_capture_session(
    meeting_id: String,
    config: Config,
    whisper: WhisperEngineHandle,
    backend: Box<dyn AudioCaptureBackend>,
) -> Result<CaptureHandle, String> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AudioChunk>();

    debug!("{LOG_PREFIX} starting capture session meeting_id={meeting_id}");
    let start_meeting_id = meeting_id.clone();
    let capture_handle = tokio::task::spawn_blocking(move || backend.start(tx))
        .await
        .map_err(|e| {
            debug!(
                "{LOG_PREFIX} backend start task join error meeting_id={start_meeting_id}: {e}"
            );
            format!("capture backend start task join error: {e}")
        })?
        .map_err(|e| {
            debug!("{LOG_PREFIX} backend failed to start meeting_id={meeting_id}: {e}");
            e
        })?;

    tokio::spawn(run_consumer_loop(meeting_id, config, whisper, rx));

    Ok(capture_handle)
}

async fn run_consumer_loop(
    meeting_id: String,
    config: Config,
    whisper: WhisperEngineHandle,
    mut rx: UnboundedReceiver<AudioChunk>,
) {
    let mut session = CaptureSession::new(meeting_id.clone());
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // `interval()`'s first tick fires immediately — consume it so the first
    // real flush waits a full FLUSH_INTERVAL instead of firing right away.
    ticker.tick().await;

    loop {
        tokio::select! {
            maybe_chunk = rx.recv() => {
                match maybe_chunk {
                    Some(chunk) => session.push(chunk),
                    None => {
                        debug!(
                            "{LOG_PREFIX} capture channel closed meeting_id={meeting_id}, \
                             flushing remaining buffers"
                        );
                        flush_batch(&meeting_id, &config, &whisper, session.drain_all()).await;
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                flush_batch(&meeting_id, &config, &whisper, session.drain_ready()).await;
            }
        }
    }
    debug!("{LOG_PREFIX} capture session consumer exiting meeting_id={meeting_id}");
}

async fn flush_batch(
    meeting_id: &str,
    config: &Config,
    whisper: &WhisperEngineHandle,
    batch: Vec<(AudioTrack, Vec<f32>, i64)>,
) {
    for (track, samples, chunk_start_ms) in batch {
        if let Err(e) =
            flush_and_transcribe(meeting_id, config, whisper, track, samples, chunk_start_ms).await
        {
            warn!(
                "{LOG_PREFIX} flush failed meeting_id={meeting_id} track={} error={e}",
                track.as_str()
            );
        }
    }
}

/// Resamples-already-16kHz `samples` (see [`CaptureSession::push`], which
/// resamples on arrival) through whisper off the async runtime thread, then
/// persists + publishes on success. Silent/rejected transcriptions (empty
/// text — whisper-guard hallucination filtering already ran inside
/// `whisper_engine::transcribe_pcm_f32`) are skipped without persisting an
/// empty chunk.
async fn flush_and_transcribe(
    meeting_id: &str,
    config: &Config,
    whisper: &WhisperEngineHandle,
    track: AudioTrack,
    samples: Vec<f32>,
    chunk_start_ms: i64,
) -> Result<(), String> {
    let sample_count = samples.len();
    let duration_ms = (sample_count as f64 / WHISPER_SAMPLE_RATE as f64 * 1000.0) as i64;
    let end_ms = chunk_start_ms + duration_ms;

    debug!(
        "{LOG_PREFIX} transcribing meeting_id={meeting_id} track={} samples={sample_count} \
         start_ms={chunk_start_ms} end_ms={end_ms}",
        track.as_str()
    );

    let whisper_handle = whisper.clone();
    let transcription = tokio::task::spawn_blocking(move || {
        whisper_engine::transcribe_pcm_f32(&whisper_handle, &samples, None, None)
    })
    .await
    .map_err(|e| format!("whisper task join error: {e}"))?
    .map_err(|e| format!("transcription failed: {e}"))?;

    let text = transcription.text.trim().to_string();
    if text.is_empty() {
        debug!(
            "{LOG_PREFIX} empty transcription (silence or rejected segments), skipping persist \
             meeting_id={meeting_id} track={}",
            track.as_str()
        );
        return Ok(());
    }

    debug!(
        "{LOG_PREFIX} transcribed meeting_id={meeting_id} track={} chars={} avg_logprob={:?}",
        track.as_str(),
        text.len(),
        transcription.avg_logprob
    );

    let outcome = ops::meetings_append_transcript_chunk(
        config,
        meeting_id,
        chunk_start_ms,
        end_ms,
        &text,
        Some(track.as_str().to_string()),
    )
    .await?;

    bus::publish_transcript_chunk(meeting_id, &outcome.value, track);

    Ok(())
}

/// Per-track accumulation buffer. Samples are stored already resampled to
/// [`WHISPER_SAMPLE_RATE`] (resampling happens in [`CaptureSession::push`]
/// on arrival, using each chunk's own `sample_rate` — device rate can't
/// really change mid-callback-stream, but this avoids assuming it's
/// constant across the whole session).
struct TrackBuffer {
    samples: Vec<f32>,
    /// `captured_at_ms` of the first sample currently buffered — becomes
    /// the persisted transcript chunk's `start_ms`.
    chunk_start_ms: i64,
}

/// Owns per-track audio buffering for one meeting's capture session.
///
/// Kept deliberately free of I/O so [`CaptureSession::push`],
/// [`CaptureSession::drain_ready`], and [`CaptureSession::drain_all`] can be
/// unit tested without a whisper engine, SQLite, or real audio hardware —
/// see this module's doc comment.
pub struct CaptureSession {
    meeting_id: String,
    buffers: HashMap<AudioTrack, TrackBuffer>,
}

impl CaptureSession {
    pub fn new(meeting_id: String) -> Self {
        Self {
            meeting_id,
            buffers: HashMap::new(),
        }
    }

    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }

    /// Resamples `chunk` to [`WHISPER_SAMPLE_RATE`] and appends it to that
    /// track's buffer. A chunk that resamples to zero samples (shouldn't
    /// normally happen — `AudioChunk`s are dropped before sending if empty,
    /// see `capture::windows::push_chunk`) is ignored rather than starting
    /// an empty buffer.
    pub fn push(&mut self, chunk: AudioChunk) {
        let resampled = resample(&chunk.pcm_f32, chunk.sample_rate, WHISPER_SAMPLE_RATE);
        if resampled.is_empty() {
            return;
        }
        let track = chunk.track;
        let captured_at_ms = chunk.captured_at_ms;
        let buf = self.buffers.entry(track).or_insert_with(|| TrackBuffer {
            samples: Vec::new(),
            chunk_start_ms: captured_at_ms,
        });
        if buf.samples.is_empty() {
            buf.chunk_start_ms = captured_at_ms;
        }
        buf.samples.extend_from_slice(&resampled);
    }

    /// Drains every track whose buffer has at least [`MIN_FLUSH_SAMPLES`]
    /// accumulated, leaving smaller buffers untouched for the next flush.
    /// Returns `(track, samples, chunk_start_ms)` tuples in arbitrary order.
    pub fn drain_ready(&mut self) -> Vec<(AudioTrack, Vec<f32>, i64)> {
        let mut out = Vec::new();
        let ready_tracks: Vec<AudioTrack> = self
            .buffers
            .iter()
            .filter(|(_, buf)| buf.samples.len() >= MIN_FLUSH_SAMPLES)
            .map(|(track, _)| *track)
            .collect();
        for track in ready_tracks {
            if let Some(buf) = self.buffers.get_mut(&track) {
                let samples = std::mem::take(&mut buf.samples);
                let chunk_start_ms = buf.chunk_start_ms;
                out.push((track, samples, chunk_start_ms));
            }
        }
        out
    }

    /// Drains every track regardless of [`MIN_FLUSH_SAMPLES`] — used on
    /// session teardown so the last few hundred milliseconds aren't lost.
    /// Tracks with an empty buffer are skipped (nothing to flush).
    pub fn drain_all(&mut self) -> Vec<(AudioTrack, Vec<f32>, i64)> {
        let tracks: Vec<AudioTrack> = self.buffers.keys().copied().collect();
        let mut out = Vec::new();
        for track in tracks {
            if let Some(buf) = self.buffers.get_mut(&track) {
                if buf.samples.is_empty() {
                    continue;
                }
                let samples = std::mem::take(&mut buf.samples);
                let chunk_start_ms = buf.chunk_start_ms;
                out.push((track, samples, chunk_start_ms));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(track: AudioTrack, samples: Vec<f32>, sample_rate: u32, captured_at_ms: i64) -> AudioChunk {
        AudioChunk {
            pcm_f32: samples,
            sample_rate,
            track,
            captured_at_ms,
        }
    }

    #[test]
    fn push_resamples_to_whisper_rate() {
        let mut session = CaptureSession::new("meeting_1".into());
        // 32kHz input should roughly halve into the 16kHz buffer.
        let samples: Vec<f32> = (0..3200).map(|i| (i as f32 / 3200.0).sin()).collect();
        session.push(chunk(AudioTrack::Microphone, samples, 32_000, 0));

        let batch = session.drain_all();
        assert_eq!(batch.len(), 1);
        let (track, resampled, start_ms) = &batch[0];
        assert_eq!(*track, AudioTrack::Microphone);
        assert_eq!(*start_ms, 0);
        assert!(resampled.len() >= 1590 && resampled.len() <= 1610);
    }

    #[test]
    fn push_keeps_tracks_independent() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(
            AudioTrack::Microphone,
            vec![0.1; WHISPER_SAMPLE_RATE as usize],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.2; WHISPER_SAMPLE_RATE as usize],
            WHISPER_SAMPLE_RATE,
            0,
        ));

        let mut batch = session.drain_all();
        batch.sort_by_key(|(track, _, _)| track.as_str());
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].0, AudioTrack::Microphone);
        assert_eq!(batch[1].0, AudioTrack::SystemAudio);
    }

    #[test]
    fn drain_ready_leaves_small_buffers_untouched() {
        let mut session = CaptureSession::new("meeting_1".into());
        // Well under MIN_FLUSH_SAMPLES (0.5s @ 16kHz).
        session.push(chunk(AudioTrack::Microphone, vec![0.1; 100], WHISPER_SAMPLE_RATE, 0));

        assert!(session.drain_ready().is_empty());
        // Still buffered — drain_all should find it.
        let batch = session.drain_all();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].1.len(), 100);
    }

    #[test]
    fn drain_ready_flushes_once_threshold_reached() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(
            AudioTrack::Microphone,
            vec![0.1; MIN_FLUSH_SAMPLES],
            WHISPER_SAMPLE_RATE,
            250,
        ));

        let batch = session.drain_ready();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].0, AudioTrack::Microphone);
        assert_eq!(batch[0].1.len(), MIN_FLUSH_SAMPLES);
        assert_eq!(batch[0].2, 250);

        // Buffer was cleared by the drain.
        assert!(session.drain_all().is_empty());
    }

    #[test]
    fn chunk_start_ms_resets_after_each_drain() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(AudioTrack::Microphone, vec![0.1; 10], WHISPER_SAMPLE_RATE, 100));
        let _ = session.drain_all();

        session.push(chunk(AudioTrack::Microphone, vec![0.1; 10], WHISPER_SAMPLE_RATE, 900));
        let batch = session.drain_all();
        assert_eq!(batch[0].2, 900);
    }

    #[test]
    fn empty_chunk_after_resample_is_ignored() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(AudioTrack::Microphone, Vec::new(), WHISPER_SAMPLE_RATE, 0));
        assert!(session.drain_all().is_empty());
    }

    #[test]
    fn meeting_id_accessor_returns_constructor_value() {
        let session = CaptureSession::new("meeting_42".into());
        assert_eq!(session.meeting_id(), "meeting_42");
    }
}
