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
//!
//! ## Turn-boundary heuristic (`system_audio` only) — not diarization
//!
//! `speaker` (see `TranscriptChunk::speaker` in `types.rs`) already carries
//! the mic-vs-system-audio split end to end — that part of the
//! meeting-capture design is fully wired and needs no change. What this
//! module adds on top is [`CaptureSession::take_pause_boundary_ready`]: a
//! pause/energy heuristic, scoped to the `system_audio` track only, that
//! flushes a buffer early the moment a long-enough silence gap is observed
//! instead of waiting for the periodic [`FLUSH_INTERVAL`] tick or
//! [`MIN_FLUSH_SAMPLES`] threshold. This only changes *chunk boundaries* —
//! it cannot tell which remote participant is speaking, so every chunk it
//! produces is still labeled the single `"system_audio"` speaker. It exists
//! purely so one long `system_audio` transcript chunk is less likely to
//! silently merge two different remote speakers' turns together; real
//! per-remote-speaker diarization (splitting `"system_audio"` into
//! individually-clustered speakers) is a separate, later, explicitly
//! scoped block — see the design doc's native-dependency discussion for why
//! it isn't attempted here.

use std::collections::HashMap;
use std::time::Duration;

use log::{debug, warn};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::aura::config::Config;
use crate::aura::inference::local::whisper_engine::{self, WhisperEngineHandle};
use crate::aura::meetings::bus;
use crate::aura::meetings::ops;
use crate::aura::voice::audio_capture::resample;

#[cfg(feature = "diarize")]
use super::raw_audio_sink::RawAudioWriter;
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

/// Grep-friendly prefix for the turn-boundary heuristic's own logs, kept
/// distinct from [`LOG_PREFIX`] (`[meetings-capture]`) so raw-capture noise
/// and turn-splitting decisions can be filtered independently. Not
/// diarization — see this module's doc comment.
const DIARIZATION_LOG_PREFIX: &str = "[meetings-diarization]";

/// Below this absolute sample amplitude, a resampled sample counts as
/// "silence" for [`CaptureSession::take_pause_boundary_ready`]'s trailing-
/// silence run. Deliberately conservative (near the noise floor rather than
/// a perceptual loudness cutoff) — this only decides *where* to cut a
/// buffer, never *whether* to transcribe it; whisper-guard's own
/// empty-transcription filtering (see [`flush_and_transcribe`]) is what
/// actually decides "was there speech".
const SILENCE_AMPLITUDE_THRESHOLD: f32 = 0.01;

/// How much trailing silence on the `system_audio` track counts as a likely
/// turn boundary. 600ms sits comfortably above a normal mid-sentence
/// breath/pause but well under a full flush cycle, so an early flush here
/// actually shortens the amount of unrelated audio that can accumulate on
/// either side of a turn change.
const PAUSE_BOUNDARY_SILENCE_MS: u64 = 600;

/// [`PAUSE_BOUNDARY_SILENCE_MS`] expressed in samples at
/// [`WHISPER_SAMPLE_RATE`] (600ms @ 16kHz = 9_600 samples).
const PAUSE_BOUNDARY_SILENCE_SAMPLES: usize = 9_600;

/// Minimum non-trailing-silence ("voiced prefix") content required before
/// [`CaptureSession::take_pause_boundary_ready`] honors a pause boundary.
/// Without this floor, a `system_audio` buffer that is mostly or entirely
/// silence (e.g. system audio muted for a long stretch) would trigger an
/// early flush — and therefore a whisper call — roughly every
/// [`PAUSE_BOUNDARY_SILENCE_MS`], instead of waiting for the much less
/// frequent [`FLUSH_INTERVAL`] tick the way silent audio always has. 250ms.
const MIN_VOICED_PREFIX_SAMPLES: usize = (WHISPER_SAMPLE_RATE / 4) as usize;

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
    let mut capture_handle = tokio::task::spawn_blocking(move || backend.start(tx))
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

    // Lets a caller that force-stops this session (`CaptureHandle::
    // stop_and_wait_for_drain`, used by `ops::meetings_finish`) actually wait
    // for `run_consumer_loop` below to drain its buffers and finalize the raw
    // system_audio WAV, instead of racing a follow-up `meetings_diarize` call
    // against an in-flight `hound::WavWriter::finalize()` — see
    // `CaptureHandle::stop_and_wait_for_drain`'s doc comment.
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    capture_handle.set_finished_signal(finished_rx);

    tokio::spawn(run_consumer_loop(meeting_id, config, whisper, rx, finished_tx));

    Ok(capture_handle)
}

async fn run_consumer_loop(
    meeting_id: String,
    config: Config,
    whisper: WhisperEngineHandle,
    mut rx: UnboundedReceiver<AudioChunk>,
    finished_tx: tokio::sync::oneshot::Sender<()>,
) {
    let mut session = CaptureSession::new(meeting_id.clone());

    // Only under the opt-in `diarize` feature: start buffering the raw
    // system_audio waveform to disk so an offline `meetings.diarize` call can
    // run real per-speaker clustering over it later — see
    // `crate::aura::meetings::diarize` and `raw_audio_sink`'s doc comments.
    // Best-effort: a writer-creation failure (e.g. unwritable workspace dir)
    // only disables diarization for this session, it must never abort
    // capture/transcription, which has nothing to do with this path.
    #[cfg(feature = "diarize")]
    {
        let wav_path = ops::system_audio_wav_path(&config, &meeting_id);
        match RawAudioWriter::create(&wav_path) {
            Ok(writer) => {
                debug!(
                    "{DIARIZATION_LOG_PREFIX} raw system_audio capture enabled meeting_id={meeting_id} \
                     path={}",
                    wav_path.display()
                );
                session.attach_raw_audio_writer(writer);
            }
            Err(e) => warn!(
                "{DIARIZATION_LOG_PREFIX} failed to open raw audio sink meeting_id={meeting_id} \
                 path={} error={e} — this meeting will not be diarizable",
                wav_path.display()
            ),
        }
    }

    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // `interval()`'s first tick fires immediately — consume it so the first
    // real flush waits a full FLUSH_INTERVAL instead of firing right away.
    ticker.tick().await;

    loop {
        tokio::select! {
            maybe_chunk = rx.recv() => {
                match maybe_chunk {
                    Some(chunk) => {
                        session.push(chunk);
                        if let Some(ready) = session.take_pause_boundary_ready() {
                            let (track, samples, chunk_start_ms) = &ready;
                            debug!(
                                "{DIARIZATION_LOG_PREFIX} pause boundary detected \
                                 meeting_id={meeting_id} track={} samples={} \
                                 start_ms={chunk_start_ms} — flushing early as a likely \
                                 turn change (not per-speaker diarization)",
                                track.as_str(),
                                samples.len()
                            );
                            flush_batch(&meeting_id, &config, &whisper, vec![ready]).await;
                        }
                    }
                    None => {
                        debug!(
                            "{LOG_PREFIX} capture channel closed meeting_id={meeting_id}, \
                             flushing remaining buffers"
                        );
                        flush_batch(&meeting_id, &config, &whisper, session.drain_all()).await;
                        #[cfg(feature = "diarize")]
                        match session.finish_raw_audio() {
                            Some(Ok(path)) => debug!(
                                "{DIARIZATION_LOG_PREFIX} raw system_audio capture finalized \
                                 meeting_id={meeting_id} path={}",
                                path.display()
                            ),
                            Some(Err(e)) => warn!(
                                "{DIARIZATION_LOG_PREFIX} failed to finalize raw audio sink \
                                 meeting_id={meeting_id} error={e}"
                            ),
                            None => {}
                        }
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
    // Signals `CaptureHandle::stop_and_wait_for_drain` (if a caller is
    // awaiting it) that every buffer has been drained and — under the
    // `diarize` feature — the raw system_audio WAV has been finalized. Sent
    // unconditionally on every exit from the loop above (there is only one:
    // the `None` branch after flushing/finalizing, immediately above the
    // `break`); a dropped receiver (no caller waiting) is a normal, silent
    // no-op for `oneshot::Sender::send`.
    let _ = finished_tx.send(());
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
    /// Count of consecutive trailing samples at the end of `samples` whose
    /// absolute amplitude is below [`SILENCE_AMPLITUDE_THRESHOLD`]. Only
    /// maintained for the `system_audio` track (see
    /// [`CaptureSession::push`]) — used by
    /// [`CaptureSession::take_pause_boundary_ready`] to detect a likely turn
    /// boundary. Reset to 0 whenever the buffer is drained or restarted.
    trailing_silence_samples: usize,
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
    /// Raw `system_audio` waveform sink for the opt-in `diarize` feature —
    /// `None` whenever the feature is off, or on (best-effort) writer-open
    /// failure. See [`RawAudioWriter`] and this module's doc comment.
    #[cfg(feature = "diarize")]
    raw_audio: Option<RawAudioWriter>,
}

impl CaptureSession {
    pub fn new(meeting_id: String) -> Self {
        Self {
            meeting_id,
            buffers: HashMap::new(),
            #[cfg(feature = "diarize")]
            raw_audio: None,
        }
    }

    /// Attaches a raw-audio writer that [`CaptureSession::push`] forwards
    /// every resampled `system_audio` chunk to, for later offline
    /// diarization. Only compiled under the `diarize` feature.
    #[cfg(feature = "diarize")]
    pub fn attach_raw_audio_writer(&mut self, writer: RawAudioWriter) {
        self.raw_audio = Some(writer);
    }

    /// Takes and finalizes the raw-audio writer, if one was attached and is
    /// still present. Returns `None` when no writer was ever attached (or it
    /// was already finalized), `Some(Ok(path))` on a successful finalize, and
    /// `Some(Err(_))` if finalizing failed. Only compiled under the
    /// `diarize` feature; idempotent — a second call always returns `None`.
    #[cfg(feature = "diarize")]
    pub fn finish_raw_audio(&mut self) -> Option<Result<std::path::PathBuf, String>> {
        self.raw_audio.take().map(RawAudioWriter::finish)
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
            trailing_silence_samples: 0,
        });
        if buf.samples.is_empty() {
            buf.chunk_start_ms = captured_at_ms;
            buf.trailing_silence_samples = 0;
        }
        buf.samples.extend_from_slice(&resampled);

        // Trailing-silence tracking is only meaningful (and only used) for
        // system_audio's turn-boundary heuristic — see this module's doc
        // comment. Skipping it for microphone avoids pointless per-sample
        // work on a track nothing reads it from.
        if track == AudioTrack::SystemAudio {
            for &sample in &resampled {
                if sample.abs() < SILENCE_AMPLITUDE_THRESHOLD {
                    buf.trailing_silence_samples += 1;
                } else {
                    buf.trailing_silence_samples = 0;
                }
            }

            // Opt-in `diarize` feature only: mirror every resampled
            // system_audio sample into the raw-audio sink (independent of,
            // and never interfering with, the whisper-chunk buffer above).
            // Best-effort — a write failure here disables diarization for
            // the rest of the session but must never affect transcription.
            #[cfg(feature = "diarize")]
            if let Some(writer) = self.raw_audio.as_mut() {
                if let Err(e) = writer.append_f32(&resampled) {
                    warn!(
                        "{DIARIZATION_LOG_PREFIX} raw audio sink write failed meeting_id={} \
                         error={e} — disabling diarization for the rest of this session",
                        self.meeting_id
                    );
                    self.raw_audio = None;
                }
            }
        }
    }

    /// Turn-boundary heuristic for `system_audio` only — see this module's
    /// doc comment. Returns `Some` (draining that buffer, resetting its
    /// silence run) the moment the track has both a plausible amount of
    /// voiced content ([`MIN_VOICED_PREFIX_SAMPLES`]) and a long-enough
    /// trailing silence run ([`PAUSE_BOUNDARY_SILENCE_SAMPLES`]) to call it a
    /// likely turn change. Returns `None` otherwise, including when the
    /// buffer is silent from the start (nothing voiced to flush early) —
    /// that case is left for the normal periodic [`CaptureSession::drain_ready`]
    /// tick, so a muted `system_audio` track doesn't trigger a whisper call
    /// roughly every [`PAUSE_BOUNDARY_SILENCE_MS`].
    pub fn take_pause_boundary_ready(&mut self) -> Option<(AudioTrack, Vec<f32>, i64)> {
        let buf = self.buffers.get_mut(&AudioTrack::SystemAudio)?;
        let voiced_prefix = buf.samples.len().saturating_sub(buf.trailing_silence_samples);
        if buf.trailing_silence_samples < PAUSE_BOUNDARY_SILENCE_SAMPLES
            || voiced_prefix < MIN_VOICED_PREFIX_SAMPLES
        {
            return None;
        }
        let samples = std::mem::take(&mut buf.samples);
        let chunk_start_ms = buf.chunk_start_ms;
        buf.trailing_silence_samples = 0;
        Some((AudioTrack::SystemAudio, samples, chunk_start_ms))
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

    // ── Turn-boundary heuristic (system_audio pause detection) ─────────

    #[test]
    fn pause_boundary_fires_after_voiced_audio_then_silence_gap() {
        let mut session = CaptureSession::new("meeting_1".into());
        // Enough "voiced" (loud) content to clear MIN_VOICED_PREFIX_SAMPLES.
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.5; MIN_VOICED_PREFIX_SAMPLES],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        assert!(session.take_pause_boundary_ready().is_none());

        // Not quite enough trailing silence yet.
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES - 1],
            WHISPER_SAMPLE_RATE,
            500,
        ));
        assert!(session.take_pause_boundary_ready().is_none());

        // One more silent sample crosses the pause-boundary threshold.
        session.push(chunk(AudioTrack::SystemAudio, vec![0.0], WHISPER_SAMPLE_RATE, 1_100));
        let ready = session
            .take_pause_boundary_ready()
            .expect("pause boundary should fire once trailing silence threshold is crossed");
        assert_eq!(ready.0, AudioTrack::SystemAudio);
        assert_eq!(
            ready.1.len(),
            MIN_VOICED_PREFIX_SAMPLES + PAUSE_BOUNDARY_SILENCE_SAMPLES
        );
        assert_eq!(ready.2, 0);

        // Buffer (and its silence run) was reset by the drain.
        assert!(session.take_pause_boundary_ready().is_none());
        assert!(session.drain_all().is_empty());
    }

    #[test]
    fn pause_boundary_never_fires_for_microphone_track() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(
            AudioTrack::Microphone,
            vec![0.5; MIN_VOICED_PREFIX_SAMPLES],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        session.push(chunk(
            AudioTrack::Microphone,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES + 100],
            WHISPER_SAMPLE_RATE,
            500,
        ));
        // Only system_audio is tracked — a microphone-only session has no
        // system_audio buffer at all yet.
        assert!(session.take_pause_boundary_ready().is_none());
    }

    #[test]
    fn pause_boundary_does_not_fire_on_silence_alone() {
        let mut session = CaptureSession::new("meeting_1".into());
        // Entirely silent system_audio (e.g. nothing playing) should never
        // trigger an early flush — that would call whisper roughly every
        // PAUSE_BOUNDARY_SILENCE_MS on pure silence. It's left for the
        // normal periodic drain_ready/FLUSH_INTERVAL cadence instead.
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES * 3],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        assert!(session.take_pause_boundary_ready().is_none());
        // Still there for the periodic path.
        let batch = session.drain_ready();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].0, AudioTrack::SystemAudio);
    }

    #[test]
    fn pause_boundary_silence_run_resets_on_new_voiced_audio() {
        let mut session = CaptureSession::new("meeting_1".into());
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.5; MIN_VOICED_PREFIX_SAMPLES],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        // Silence, but interrupted by more voiced audio before the pause
        // threshold is reached — the trailing-silence run must reset rather
        // than carry over from before the interruption.
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES - 1],
            WHISPER_SAMPLE_RATE,
            500,
        ));
        session.push(chunk(AudioTrack::SystemAudio, vec![0.5; 10], WHISPER_SAMPLE_RATE, 1_100));
        assert!(session.take_pause_boundary_ready().is_none());

        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES - 1],
            WHISPER_SAMPLE_RATE,
            1_200,
        ));
        // Still short of the threshold — the reset from the voiced
        // interruption means this run hasn't crossed it yet.
        assert!(session.take_pause_boundary_ready().is_none());
    }

    #[test]
    fn pause_boundary_requires_minimum_voiced_prefix() {
        let mut session = CaptureSession::new("meeting_1".into());
        // A short burst of voiced audio, under MIN_VOICED_PREFIX_SAMPLES,
        // followed by a long silence should not fire — there isn't enough
        // real content to justify an early whisper call.
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.5; MIN_VOICED_PREFIX_SAMPLES / 2],
            WHISPER_SAMPLE_RATE,
            0,
        ));
        session.push(chunk(
            AudioTrack::SystemAudio,
            vec![0.0; PAUSE_BOUNDARY_SILENCE_SAMPLES + 100],
            WHISPER_SAMPLE_RATE,
            500,
        ));
        assert!(session.take_pause_boundary_ready().is_none());
    }
}
