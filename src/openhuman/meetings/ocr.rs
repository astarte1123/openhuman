//! OCR for periodic meeting-slide screenshots.
//!
//! **Why OCR instead of a vision LLM**: `inference::provider::traits::Provider`
//! (`ChatMessage.content: String`) has no real multimodal/image support —
//! `ProviderCapabilities.vision` exists as a flag but no adapter implements
//! it. The only working image->text path in this codebase is
//! `inference::local::service::vision_embed::vision_prompt`, which is
//! Ollama-local-only and not reliably available cross-platform. So this
//! module uses a real OCR engine instead.
//!
//! **Engine**: [`ocrs`](https://docs.rs/ocrs), a pure-Rust OCR engine backed
//! by `rten` — the crate author's own pure-Rust tensor runtime, *not*
//! Microsoft's onnxruntime. This sidesteps the `ORT_LIB_LOCATION`/MSVC-CRT
//! native-dependency pain already documented in `meetings::export`'s doc
//! comment for `whisper-rs-sys` and the historical `onnxruntime` builds —
//! `ocrs` + `rten` are plain Rust crates with no C toolchain requirement.
//!
//! **Models**: two small pretrained `.rten` files, downloaded on first use
//! (mirrors `app/src-tauri/src/meeting_recorder/whisper_engine.rs`'s
//! `ensure_model_downloaded` pattern — atomic `.part` -> rename, size
//! sanity-check, debug logging) into the caller-supplied `models_dir`
//! (`ops::meetings_append_screenshot` passes
//! `{workspace_dir}/meetings/models/`, the same directory whisper's model
//! already lives in on the Tauri-shell side):
//!
//! - `text-detection.rten` (~2.4MB) —
//!   <https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten>
//! - `text-recognition.rten` (~9.3MB) —
//!   <https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten>
//!
//! Both URLs come from `ocrs`'s own `ocrs/examples/download-models.sh` in
//! the upstream repository (robertknight/ocrs) — there is no crates.io or
//! HuggingFace distribution of these particular model weights.
//!
//! [`extract_text`] is a pure function (PNG bytes + a models directory in,
//! extracted text out) so it is unit-testable without touching the
//! meetings SQLite store or workspace layout — see `ocr_tests.rs`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

const LOG_PREFIX: &str = "[meetings:ocr]";

const DETECTION_MODEL_FILENAME: &str = "text-detection.rten";
const RECOGNITION_MODEL_FILENAME: &str = "text-recognition.rten";

const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

/// `text-detection.rten` is ~2.4MB; refuse anything drastically smaller —
/// almost certainly an HTML error page from a CDN redirect, not the real
/// model. Mirrors `whisper_engine::MIN_MODEL_BYTES`'s reasoning.
const MIN_DETECTION_MODEL_BYTES: u64 = 1024 * 1024;
/// `text-recognition.rten` is ~9.3MB.
const MIN_RECOGNITION_MODEL_BYTES: u64 = 4 * 1024 * 1024;

/// Resolved paths to both `.rten` model files, downloading each if it is
/// missing (or looks truncated) from `models_dir`. Cross-platform, network
/// I/O only — safe to call from an async context. Callers should follow
/// this with the CPU-bound [`extract_text`] on a blocking thread (see
/// `ops::meetings_append_screenshot`).
pub async fn ensure_models_downloaded(models_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    log::debug!("{LOG_PREFIX} ensure_models_downloaded: dir={}", models_dir.display());

    std::fs::create_dir_all(models_dir)
        .with_context(|| format!("failed to create OCR models dir: {}", models_dir.display()))?;

    let detection_path = ensure_model_file(
        models_dir,
        DETECTION_MODEL_FILENAME,
        DETECTION_MODEL_URL,
        MIN_DETECTION_MODEL_BYTES,
    )
    .await?;
    let recognition_path = ensure_model_file(
        models_dir,
        RECOGNITION_MODEL_FILENAME,
        RECOGNITION_MODEL_URL,
        MIN_RECOGNITION_MODEL_BYTES,
    )
    .await?;

    Ok((detection_path, recognition_path))
}

/// Downloads one `.rten` model file into `models_dir/filename` if it is not
/// already present at (at least) `min_bytes`. Writes to a `.part` sibling
/// first and renames atomically on success, so a process crash mid-download
/// never leaves a truncated file mistaken for a complete one on the next
/// call.
async fn ensure_model_file(
    models_dir: &Path,
    filename: &str,
    url: &str,
    min_bytes: u64,
) -> Result<PathBuf> {
    let final_path = models_dir.join(filename);

    if final_path.is_file() {
        let size_ok = std::fs::metadata(&final_path)
            .map(|m| m.len() >= min_bytes)
            .unwrap_or(false);
        if size_ok {
            log::debug!("{LOG_PREFIX} model already present at {}", final_path.display());
            return Ok(final_path);
        }
        log::warn!(
            "{LOG_PREFIX} existing model at {} is smaller than expected, re-downloading",
            final_path.display()
        );
    }

    log::info!("{LOG_PREFIX} downloading OCR model {filename} from {url}");
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("OCR model download request failed: {url}"))?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "OCR model download failed for {url} with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read OCR model download body: {url}"))?;
    if (bytes.len() as u64) < min_bytes {
        return Err(anyhow!(
            "downloaded OCR model {filename} is too small ({} bytes) — likely an error page, not the real model",
            bytes.len()
        ));
    }

    let tmp_path = final_path.with_extension("rten.part");
    std::fs::write(&tmp_path, &bytes)
        .with_context(|| format!("failed to write OCR model to {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("failed to finalize OCR model at {}", final_path.display()))?;

    log::info!(
        "{LOG_PREFIX} downloaded OCR model {filename} ({} bytes) to {}",
        bytes.len(),
        final_path.display()
    );
    Ok(final_path)
}

/// Runs OCR over a PNG-encoded image and returns the recognized text
/// (lines joined with `\n`, in reading order as determined by `ocrs`'s own
/// layout analysis). `models_dir` must already contain both `.rten` model
/// files — see [`ensure_models_downloaded`].
///
/// This is CPU-bound synchronous work (model load + inference) — callers on
/// an async runtime should run it via `tokio::task::spawn_blocking` (see
/// `ops::meetings_append_screenshot`).
pub fn extract_text(png_bytes: &[u8], models_dir: &Path) -> Result<String> {
    let detection_model_path = models_dir.join(DETECTION_MODEL_FILENAME);
    let recognition_model_path = models_dir.join(RECOGNITION_MODEL_FILENAME);

    log::debug!(
        "{LOG_PREFIX} extract_text: png_bytes={} detection_model={} recognition_model={}",
        png_bytes.len(),
        detection_model_path.display(),
        recognition_model_path.display()
    );

    // Decode first: a malformed PNG should fail fast with a clear decode
    // error rather than after paying for two model loads.
    let img = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .context("failed to decode screenshot PNG for OCR")?
        .into_rgb8();

    let detection_model = Model::load_file(&detection_model_path).with_context(|| {
        format!(
            "failed to load OCR detection model from {}",
            detection_model_path.display()
        )
    })?;
    let recognition_model = Model::load_file(&recognition_model_path).with_context(|| {
        format!(
            "failed to load OCR recognition model from {}",
            recognition_model_path.display()
        )
    })?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| anyhow!("failed to initialize OCR engine: {e}"))?;

    let img_source = ImageSource::from_bytes(img.as_raw(), img.dimensions())
        .map_err(|e| anyhow!("failed to prepare OCR image source: {e}"))?;
    let ocr_input = engine
        .prepare_input(img_source)
        .map_err(|e| anyhow!("failed to prepare OCR input: {e}"))?;

    let text = engine
        .get_text(&ocr_input)
        .map_err(|e| anyhow!("OCR text extraction failed: {e}"))?;

    log::debug!("{LOG_PREFIX} extract_text: extracted {} char(s)", text.len());
    Ok(text)
}

#[cfg(test)]
#[path = "ocr_tests.rs"]
mod tests;
