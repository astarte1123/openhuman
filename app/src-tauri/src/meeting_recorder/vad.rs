//! Silero VAD wrapper.
//!
//! Buffers 16 kHz mono input (already resampled upstream — see
//! `meeting_recorder::resample`) into fixed 480-sample (30ms) chunks —
//! Silero's native internal frame size — and surfaces completed speech
//! segments once a `VadTransition::SpeechEnd` fires. Segments shorter
//! than 800 samples (50ms) are dropped before reaching Whisper; they are
//! almost never usable transcription input and are mostly false triggers.
//!
//! Uses the `static-model` feature of the `silero` crate (embeds the
//! ONNX VAD model directly in the binary, ~1.7MB), so there is no
//! separate VAD model file to download or manage — only the Whisper GGML
//! model (see `whisper_engine.rs`) needs a download step.

use std::time::Duration;

use anyhow::{anyhow, Result};
use silero_rs::{VadConfig, VadSession, VadTransition};

/// Fixed VAD input rate. Anything captured at a different rate must be
/// resampled upstream (see `resample::Resampler16k`) before reaching
/// [`SileroVad::process`].
pub const VAD_SAMPLE_RATE: u32 = 16_000;

/// Silero's native internal frame size (30ms @ 16kHz).
const CHUNK_SAMPLES: usize = 480;

/// Segments shorter than this (50ms @ 16kHz) are discarded before being
/// handed to Whisper.
const MIN_SEGMENT_SAMPLES: usize = 800;

/// One detected speech segment, ready for transcription.
#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub samples: Vec<f32>,
    pub start_timestamp_ms: f64,
    pub end_timestamp_ms: f64,
}

pub struct SileroVad {
    session: VadSession,
    pending: Vec<f32>,
}

impl SileroVad {
    pub fn new() -> Result<Self> {
        let config = VadConfig {
            sample_rate: VAD_SAMPLE_RATE as usize,
            positive_speech_threshold: 0.50,
            negative_speech_threshold: 0.35,
            redemption_time: Duration::from_millis(400),
            pre_speech_pad: Duration::from_millis(300),
            post_speech_pad: Duration::from_millis(400),
            min_speech_time: Duration::from_millis(250),
        };
        let session = VadSession::new(config)
            .map_err(|e| anyhow!("failed to initialize Silero VAD session: {e}"))?;
        Ok(Self {
            session,
            pending: Vec::with_capacity(CHUNK_SAMPLES * 2),
        })
    }

    /// Feed 16 kHz mono samples (mixed mic+system audio). Internally
    /// buffers to Silero's native 30ms frame size and returns any speech
    /// segments (>= 50ms) that completed as a result.
    pub fn process(&mut self, samples_16k: &[f32]) -> Result<Vec<SpeechSegment>> {
        self.pending.extend_from_slice(samples_16k);
        let mut segments = Vec::new();

        while self.pending.len() >= CHUNK_SAMPLES {
            let chunk: Vec<f32> = self.pending.drain(..CHUNK_SAMPLES).collect();
            let transitions = self
                .session
                .process(&chunk)
                .map_err(|e| anyhow!("silero vad process failed: {e}"))?;

            for transition in transitions {
                if let VadTransition::SpeechEnd {
                    start_timestamp_ms,
                    end_timestamp_ms,
                    samples,
                } = transition
                {
                    if samples.len() < MIN_SEGMENT_SAMPLES {
                        log::debug!(
                            "[meeting-recorder:vad] discarding short segment ({} samples < {MIN_SEGMENT_SAMPLES})",
                            samples.len()
                        );
                        continue;
                    }
                    segments.push(SpeechSegment {
                        samples,
                        start_timestamp_ms: start_timestamp_ms as f64,
                        end_timestamp_ms: end_timestamp_ms as f64,
                    });
                }
            }
        }

        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_initializes_without_a_model_file() {
        // `static-model` embeds the ONNX model, so construction must
        // succeed with no filesystem/network dependency.
        assert!(SileroVad::new().is_ok());
    }

    #[test]
    fn silence_produces_no_segments() {
        let mut vad = SileroVad::new().expect("vad init");
        let silence = vec![0.0f32; VAD_SAMPLE_RATE as usize]; // 1s of silence
        let segments = vad.process(&silence).expect("process");
        assert!(segments.is_empty());
    }

    #[test]
    fn sub_chunk_input_is_buffered_not_dropped() {
        let mut vad = SileroVad::new().expect("vad init");
        // Less than one 30ms frame — must not panic or error, just buffer.
        let tiny = vec![0.0f32; 10];
        let segments = vad.process(&tiny).expect("process");
        assert!(segments.is_empty());
    }
}
