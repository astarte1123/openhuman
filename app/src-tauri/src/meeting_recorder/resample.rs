//! Resample arbitrary-rate mono `f32` audio to the VAD's fixed 16 kHz
//! input rate, using `rubato`'s windowed-sinc resampler.
//!
//! Each captured device stream (mic, system-loopback) gets its own
//! [`Resampler16k`] instance sized for that device's negotiated sample
//! rate — mic and system-loopback devices are not guaranteed to share a
//! sample rate, so resampling happens per-device *before* the two streams
//! are mixed, rather than once after mixing. This keeps
//! `AudioMixerRingBuffer` operating on a single canonical 16 kHz timeline
//! throughout (window math, VAD, and Whisper all just see 16 kHz mono).

use anyhow::{anyhow, Result};
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use super::vad::VAD_SAMPLE_RATE;

/// Fixed input chunk size fed to the sinc resampler. Arbitrary-length
/// pushes are buffered internally and drained in chunks of this size;
/// any remainder waits for the next `push()` call.
const RESAMPLE_CHUNK_FRAMES: usize = 1024;

/// Resamples mono `f32` audio to [`VAD_SAMPLE_RATE`]. A no-op passthrough
/// when the source is already at the target rate.
pub enum Resampler16k {
    Passthrough,
    Resample {
        resampler: SincFixedIn<f32>,
        input_buffer: Vec<f32>,
    },
}

impl Resampler16k {
    pub fn new(source_rate: u32) -> Result<Self> {
        if source_rate == VAD_SAMPLE_RATE {
            return Ok(Self::Passthrough);
        }
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = VAD_SAMPLE_RATE as f64 / source_rate as f64;
        let resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, RESAMPLE_CHUNK_FRAMES, 1)
            .map_err(|e| {
                anyhow!("failed to build resampler ({source_rate}Hz -> {VAD_SAMPLE_RATE}Hz): {e}")
            })?;
        Ok(Self::Resample {
            resampler,
            input_buffer: Vec::with_capacity(RESAMPLE_CHUNK_FRAMES * 2),
        })
    }

    /// Push `samples` (mono, at the source rate this resampler was built
    /// for) and return however many 16 kHz samples became available.
    /// Buffers any remainder smaller than the internal chunk size for the
    /// next call — callers must keep calling `push` (even with silence)
    /// to eventually flush a trailing partial chunk; see `flush`.
    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>> {
        match self {
            Self::Passthrough => Ok(samples.to_vec()),
            Self::Resample {
                resampler,
                input_buffer,
            } => {
                input_buffer.extend_from_slice(samples);
                drain_full_chunks(resampler, input_buffer)
            }
        }
    }

    /// Flush any buffered remainder (zero-padded to the resampler's fixed
    /// chunk size) at end of stream. Introduces a small amount of
    /// trailing silence into the resampled output — acceptable since it's
    /// at most one chunk (~1024 source-rate samples) at the very end of a
    /// recording.
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        match self {
            Self::Passthrough => Ok(Vec::new()),
            Self::Resample {
                resampler,
                input_buffer,
            } => {
                if input_buffer.is_empty() {
                    return Ok(Vec::new());
                }
                input_buffer.resize(RESAMPLE_CHUNK_FRAMES, 0.0);
                drain_full_chunks(resampler, input_buffer)
            }
        }
    }
}

fn drain_full_chunks(resampler: &mut SincFixedIn<f32>, input_buffer: &mut Vec<f32>) -> Result<Vec<f32>> {
    let mut out = Vec::new();
    while input_buffer.len() >= RESAMPLE_CHUNK_FRAMES {
        let chunk: Vec<f32> = input_buffer.drain(..RESAMPLE_CHUNK_FRAMES).collect();
        let result = resampler
            .process(&[chunk], None)
            .map_err(|e| anyhow!("resample failed: {e}"))?;
        if let Some(channel0) = result.into_iter().next() {
            out.extend(channel0);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_source_matches_target_rate() {
        let mut resampler = Resampler16k::new(VAD_SAMPLE_RATE).expect("passthrough construction");
        let input = vec![0.1f32, 0.2, 0.3];
        let out = resampler.push(&input).expect("push");
        assert_eq!(out, input);
    }

    #[test]
    fn resample_downsamples_48k_to_16k_roughly_one_third_length() {
        let mut resampler = Resampler16k::new(48_000).expect("resampler construction");
        // A few chunks worth of a low-frequency sine wave.
        let input: Vec<f32> = (0..RESAMPLE_CHUNK_FRAMES * 4)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();
        let out = resampler.push(&input).expect("push");
        // 48kHz -> 16kHz is a 1/3 ratio; allow generous tolerance for
        // sinc filter warm-up/latency.
        let expected = input.len() / 3;
        assert!(
            out.len() > expected / 2 && out.len() < expected * 2,
            "expected roughly {expected} samples, got {}",
            out.len()
        );
    }

    #[test]
    fn flush_drains_a_short_trailing_remainder() {
        let mut resampler = Resampler16k::new(48_000).expect("resampler construction");
        // Less than one full chunk — nothing should come out of push().
        let input = vec![0.1f32; 100];
        let out = resampler.push(&input).expect("push");
        assert!(out.is_empty());
        let flushed = resampler.flush().expect("flush");
        assert!(!flushed.is_empty());
    }
}
