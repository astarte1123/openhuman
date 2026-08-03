//! In-process whisper.cpp transcription (CPU-only) for meeting recordings.
//!
//! **Windows only** for this phase — `whisper-rs` is a Windows-only Cargo
//! dependency here (see `Cargo.toml`), pinned to the same `0.16` version
//! already resolved transitively via `aura_core` so whisper.cpp is
//! only ever compiled once for this crate graph. Non-Windows builds get
//! an explicit "not yet implemented" stub — no pretend support.
//!
//! CPU-only by design for this phase: `WhisperContextParameters::use_gpu`
//! is hardcoded `false`. No vulkan/cuda/metal feature is enabled.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

const LOG_PREFIX: &str = "[meeting-recorder:whisper]";
const MODEL_FILENAME: &str = "ggml-base.bin";
const MODEL_DOWNLOAD_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
/// `ggml-base.bin` is ~148MB; refuse anything drastically smaller — almost
/// certainly an HTML error page from a CDN redirect, not the real model.
const MIN_MODEL_BYTES: u64 = 50 * 1024 * 1024;

/// RMS / peak-below-threshold guard: skip calling whisper entirely for
/// near-silent segments rather than risk a hallucinated transcript.
const SILENCE_RMS_THRESHOLD: f32 = 0.001;
const SILENCE_PEAK_THRESHOLD: f32 = 0.005;

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    pub struct WhisperEngine {
        context: WhisperContext,
    }

    impl WhisperEngine {
        pub fn load(model_path: &Path) -> Result<Self> {
            if !model_path.is_file() {
                return Err(anyhow!(
                    "whisper model not found at {}",
                    model_path.display()
                ));
            }
            let mut params = WhisperContextParameters::default();
            params.use_gpu(false); // CPU-only — no vulkan/cuda/metal in this phase.
            let model_str = model_path.to_str().ok_or_else(|| {
                anyhow!("model path is not valid UTF-8: {}", model_path.display())
            })?;
            let context = WhisperContext::new_with_params(model_str, params)
                .map_err(|e| anyhow!("failed to load whisper model: {e}"))?;
            log::info!("{LOG_PREFIX} model loaded from {}", model_path.display());
            Ok(Self { context })
        }

        /// Transcribe one VAD-detected speech segment (16 kHz mono f32).
        /// Returns an empty string (not an error) for near-silent audio —
        /// callers should treat that as "nothing to append", not a failure.
        pub fn transcribe(&self, audio_f32_16k: &[f32]) -> Result<String> {
            if is_silent_audio(audio_f32_16k) {
                log::debug!(
                    "{LOG_PREFIX} skipping silent segment ({} samples)",
                    audio_f32_16k.len()
                );
                return Ok(String::new());
            }

            let mut state = self
                .context
                .create_state()
                .map_err(|e| anyhow!("failed to create whisper state: {e}"))?;

            let mut params = FullParams::new(SamplingStrategy::BeamSearch {
                beam_size: 5,
                patience: 1.0,
            });
            params.set_language(None); // auto-detect
            params.set_translate(false);
            params.set_no_timestamps(true);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true); // suppress non-speech tokens
            params.set_temperature(0.0);
            params.set_entropy_thold(2.4);
            params.set_logprob_thold(-1.0);
            params.set_no_speech_thold(0.55);
            params.set_max_len(200);
            params.set_single_segment(false);

            let n_threads = std::thread::available_parallelism()
                .map(|n| n.get().min(4) as i32)
                .unwrap_or(2);
            params.set_n_threads(n_threads);

            state
                .full(params, audio_f32_16k)
                .map_err(|e| anyhow!("whisper inference failed: {e}"))?;

            let mut text = String::new();
            for segment in state.as_iter() {
                match segment.to_str() {
                    Ok(s) => text.push_str(s),
                    Err(e) => log::debug!("{LOG_PREFIX} skipping malformed segment: {e}"),
                }
            }
            Ok(text.trim().to_string())
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::WhisperEngine;

#[cfg(not(target_os = "windows"))]
pub struct WhisperEngine;

#[cfg(not(target_os = "windows"))]
impl WhisperEngine {
    pub fn load(_model_path: &Path) -> Result<Self> {
        Err(anyhow!(
            "Meeting transcription (whisper) is not yet implemented for this platform"
        ))
    }

    pub fn transcribe(&self, _audio_f32_16k: &[f32]) -> Result<String> {
        Err(anyhow!(
            "Meeting transcription (whisper) is not yet implemented for this platform"
        ))
    }
}

fn is_silent_audio(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_sq / samples.len() as f32).sqrt();
    let peak = samples.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
    rms < SILENCE_RMS_THRESHOLD && peak < SILENCE_PEAK_THRESHOLD
}

/// Resolve (and, if missing, download) the GGML model file into
/// `{workspace_dir}/meetings/models/ggml-base.bin`. Cross-platform:
/// harmless to call on non-Windows (just stages the file), even though
/// [`WhisperEngine::load`] will refuse to use it there.
pub async fn ensure_model_downloaded(workspace_dir: &Path) -> Result<PathBuf> {
    let model_dir = workspace_dir.join("meetings").join("models");
    let model_path = model_dir.join(MODEL_FILENAME);

    if model_path.is_file() {
        let size_ok = std::fs::metadata(&model_path)
            .map(|m| m.len() >= MIN_MODEL_BYTES)
            .unwrap_or(false);
        if size_ok {
            log::debug!(
                "{LOG_PREFIX} model already present at {}",
                model_path.display()
            );
            return Ok(model_path);
        }
        log::warn!(
            "{LOG_PREFIX} existing model at {} is smaller than expected, re-downloading",
            model_path.display()
        );
    }

    std::fs::create_dir_all(&model_dir)
        .map_err(|e| anyhow!("failed to create model dir {}: {e}", model_dir.display()))?;

    log::info!("{LOG_PREFIX} downloading whisper model from {MODEL_DOWNLOAD_URL}");
    let response = reqwest::get(MODEL_DOWNLOAD_URL)
        .await
        .map_err(|e| anyhow!("model download request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "model download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow!("failed to read model download body: {e}"))?;
    if (bytes.len() as u64) < MIN_MODEL_BYTES {
        return Err(anyhow!(
            "downloaded model is too small ({} bytes) — likely an error page, not the real model",
            bytes.len()
        ));
    }

    // Write to a temp file first, then rename — avoids leaving a
    // truncated model file behind if the process dies mid-write.
    let tmp_path = model_path.with_extension("bin.part");
    std::fs::write(&tmp_path, &bytes)
        .map_err(|e| anyhow!("failed to write model to {}: {e}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &model_path)
        .map_err(|e| anyhow!("failed to finalize model at {}: {e}", model_path.display()))?;

    log::info!(
        "{LOG_PREFIX} model downloaded ({} bytes) to {}",
        bytes.len(),
        model_path.display()
    );
    Ok(model_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_silent_audio_true_for_zeros() {
        assert!(is_silent_audio(&vec![0.0; 1600]));
    }

    #[test]
    fn is_silent_audio_false_for_loud_tone() {
        let samples: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        assert!(!is_silent_audio(&samples));
    }

    #[test]
    fn is_silent_audio_true_for_empty() {
        assert!(is_silent_audio(&[]));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn whisper_engine_stub_errors_on_non_windows() {
        let err = WhisperEngine::load(Path::new("does-not-matter.bin")).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }
}
