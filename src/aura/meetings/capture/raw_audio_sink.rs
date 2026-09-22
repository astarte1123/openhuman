//! Incremental WAV writer for the `system_audio` capture track, used only by
//! the opt-in `diarize` feature.
//!
//! **Why this exists**: [`super::pipeline`]'s whisper flush path takes
//! ownership of each buffered chunk, transcribes it, and discards the raw
//! samples — see `flush_and_transcribe`'s doc comment. Real per-speaker
//! diarization (splitting `"system_audio"` into `"system_audio:speaker_N"`,
//! see [`crate::aura::meetings::diarize`]) needs the *raw*, un-chunked
//! system-audio waveform for the whole meeting, not whisper's already-cut
//! transcript segments — segmentation and speaker embedding both need
//! continuous audio to do their own windowing.
//!
//! **Why only `system_audio`**: the microphone track is the local user —
//! there's exactly one speaker on it and diarizing it would be pointless.
//! Only [`super::pipeline::CaptureSession`]'s `system_audio` buffer feeds
//! this writer.
//!
//! **Why incremental, not buffer-then-write**: a meeting can run for hours;
//! holding the whole recording in memory (`Vec<f32>` growing unbounded for
//! the session's lifetime) risks multi-hundred-MB allocations on long calls.
//! [`RawAudioWriter::append_f32`] streams each already-resampled 16kHz chunk
//! straight to disk via `hound::WavWriter`, which itself buffers through a
//! `BufWriter` and writes PCM samples incrementally.
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavSpec, WavWriter};
use log::debug;

use super::LOG_PREFIX;

/// Sample rate every appended chunk must already be at — [`super::pipeline`]
/// resamples to this (whisper's required rate) before this writer ever sees
/// a sample, so no resampling happens here.
const SAMPLE_RATE_HZ: u32 = 16_000;

/// Streams raw f32 PCM samples to a mono 16-bit PCM WAV file on disk,
/// incrementally, without holding the full recording in memory.
///
/// 16-bit int (not float) output because that's what
/// [`crate::aura::meetings::diarize::engine`]'s segmentation/embedding
/// pipeline expects to read back (mirroring Minutes' own diarization input
/// contract) and it keeps the file half the size of 32-bit float PCM.
pub struct RawAudioWriter {
    writer: WavWriter<BufWriter<File>>,
    path: PathBuf,
    samples_written: u64,
}

impl RawAudioWriter {
    /// Creates (or truncates) a mono 16kHz 16-bit PCM WAV file at `path`.
    /// Creates any missing parent directories first.
    pub fn create(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        }
        let spec = WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE_HZ,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let writer = WavWriter::create(path, spec)
            .map_err(|e| format!("failed to create WAV writer at {}: {e}", path.display()))?;
        debug!(
            "{LOG_PREFIX} raw_audio_sink: created writer path={}",
            path.display()
        );
        Ok(Self {
            writer,
            path: path.to_path_buf(),
            samples_written: 0,
        })
    }

    /// Appends already-16kHz f32 samples (range roughly [-1.0, 1.0]),
    /// converting to 16-bit PCM. Out-of-range samples are clamped rather
    /// than left to overflow/wrap on cast.
    pub fn append_f32(&mut self, samples: &[f32]) -> Result<(), String> {
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm = (clamped * i16::MAX as f32) as i16;
            self.writer
                .write_sample(pcm)
                .map_err(|e| format!("failed to write WAV sample: {e}"))?;
        }
        self.samples_written += samples.len() as u64;
        Ok(())
    }

    /// Finalizes the WAV file (writes the header with the final sample
    /// count) and returns its path.
    pub fn finish(self) -> Result<PathBuf, String> {
        let path = self.path.clone();
        let samples_written = self.samples_written;
        self.writer
            .finalize()
            .map_err(|e| format!("failed to finalize WAV at {}: {e}", path.display()))?;
        debug!(
            "{LOG_PREFIX} raw_audio_sink: finalized path={} samples_written={samples_written}",
            path.display()
        );
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_append_finish_roundtrips_to_a_valid_wav() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("system.wav");

        let mut writer = RawAudioWriter::create(&path).expect("create writer");
        writer
            .append_f32(&[0.0, 0.5, -0.5, 1.0, -1.0])
            .expect("append 1");
        writer.append_f32(&[0.25]).expect("append 2");
        let finished_path = writer.finish().expect("finish");
        assert_eq!(finished_path, path);

        let reader = hound::WavReader::open(&path).expect("reopen wav");
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, SAMPLE_RATE_HZ);
        assert_eq!(spec.bits_per_sample, 16);

        let samples: Vec<i16> = reader
            .into_samples::<i16>()
            .map(|s| s.expect("sample"))
            .collect();
        assert_eq!(samples.len(), 6);
    }

    #[test]
    fn out_of_range_samples_are_clamped_not_wrapped() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("clamped.wav");

        let mut writer = RawAudioWriter::create(&path).expect("create writer");
        writer.append_f32(&[5.0, -5.0]).expect("append");
        writer.finish().expect("finish");

        let reader = hound::WavReader::open(&path).expect("reopen wav");
        let samples: Vec<i16> = reader
            .into_samples::<i16>()
            .map(|s| s.expect("sample"))
            .collect();
        assert_eq!(samples, vec![i16::MAX, -i16::MAX]);
    }

    #[test]
    fn empty_recording_still_produces_a_valid_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("empty.wav");

        let writer = RawAudioWriter::create(&path).expect("create writer");
        writer.finish().expect("finish empty");

        let reader = hound::WavReader::open(&path).expect("reopen empty wav");
        assert_eq!(reader.duration(), 0);
    }
}
