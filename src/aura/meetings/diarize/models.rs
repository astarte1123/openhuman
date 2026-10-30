//! ONNX model download/cache for [`super::engine`].
//!
//! Mirrors [`crate::aura::meetings::ocr`]'s `ensure_models_downloaded`
//! pattern exactly (atomic `.part` -> rename, size sanity-check, verbose
//! logging) — the two model files are public releases from the
//! `thewh1teagle/pyannote-rs` GitHub project (the same crate this feature
//! depends on for its embedding extractor), so this needs no proprietary or
//! bundled asset:
//!
//! - `segmentation-3.0.onnx` (~5.7MB) — pyannote speech segmentation model.
//! - `wespeaker_en_voxceleb_CAM++.onnx` (~28MB) — wespeaker CAM++ speaker
//!   embedding model.
//!
//! Both URLs match the ones the reference Minutes implementation downloads
//! (`D:\minutes-main\crates\core\src\diarize.rs`'s `SEGMENTATION_MODEL_URL` /
//! `CAM_PP.url`) — same upstream release, not re-hosted.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::LOG_PREFIX;

pub const SEGMENTATION_MODEL_FILENAME: &str = "segmentation-3.0.onnx";
pub const EMBEDDING_MODEL_FILENAME: &str = "wespeaker_en_voxceleb_CAM++.onnx";

const SEGMENTATION_MODEL_URL: &str =
    "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx";
const EMBEDDING_MODEL_URL: &str =
    "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/wespeaker_en_voxceleb_CAM++.onnx";

/// `segmentation-3.0.onnx` is ~5.7MB; refuse anything drastically smaller —
/// almost certainly an HTML error page from a CDN redirect. Mirrors
/// `ocr::MIN_DETECTION_MODEL_BYTES`'s reasoning.
const MIN_SEGMENTATION_MODEL_BYTES: u64 = 1024 * 1024;
/// `wespeaker_en_voxceleb_CAM++.onnx` is ~28MB.
const MIN_EMBEDDING_MODEL_BYTES: u64 = 8 * 1024 * 1024;

/// True when both model files already exist in `models_dir` at (at least)
/// their expected minimum size — a cheap pre-check callers can use before
/// deciding whether a `meetings_diarize` call will need to hit the network.
pub fn models_installed(models_dir: &Path) -> bool {
    file_looks_complete(
        &models_dir.join(SEGMENTATION_MODEL_FILENAME),
        MIN_SEGMENTATION_MODEL_BYTES,
    ) && file_looks_complete(
        &models_dir.join(EMBEDDING_MODEL_FILENAME),
        MIN_EMBEDDING_MODEL_BYTES,
    )
}

fn file_looks_complete(path: &Path, min_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() >= min_bytes)
        .unwrap_or(false)
}

/// Resolved paths to both ONNX model files, downloading each if missing (or
/// truncated) from `models_dir`. Network I/O only — safe to call from an
/// async context; CPU-bound ONNX inference happens later, off-thread, in
/// [`super::engine`].
pub async fn ensure_models_downloaded(models_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    log::debug!(
        "{LOG_PREFIX} models::ensure_models_downloaded: dir={}",
        models_dir.display()
    );

    std::fs::create_dir_all(models_dir).with_context(|| {
        format!(
            "failed to create diarization models dir: {}",
            models_dir.display()
        )
    })?;

    let segmentation_path = ensure_model_file(
        models_dir,
        SEGMENTATION_MODEL_FILENAME,
        SEGMENTATION_MODEL_URL,
        MIN_SEGMENTATION_MODEL_BYTES,
    )
    .await?;
    let embedding_path = ensure_model_file(
        models_dir,
        EMBEDDING_MODEL_FILENAME,
        EMBEDDING_MODEL_URL,
        MIN_EMBEDDING_MODEL_BYTES,
    )
    .await?;

    Ok((segmentation_path, embedding_path))
}

async fn ensure_model_file(
    models_dir: &Path,
    filename: &str,
    url: &str,
    min_bytes: u64,
) -> Result<PathBuf> {
    let final_path = models_dir.join(filename);

    if file_looks_complete(&final_path, min_bytes) {
        log::debug!(
            "{LOG_PREFIX} diarization model already present at {}",
            final_path.display()
        );
        return Ok(final_path);
    }
    if final_path.is_file() {
        log::warn!(
            "{LOG_PREFIX} existing diarization model at {} is smaller than expected, \
             re-downloading",
            final_path.display()
        );
    }

    log::info!("{LOG_PREFIX} downloading diarization model {filename} from {url}");
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("diarization model download request failed: {url}"))?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "diarization model download failed for {url} with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read diarization model download body: {url}"))?;
    if (bytes.len() as u64) < min_bytes {
        return Err(anyhow!(
            "downloaded diarization model {filename} is too small ({} bytes) — likely an \
             error page, not the real model",
            bytes.len()
        ));
    }

    let tmp_path = final_path.with_extension("onnx.part");
    std::fs::write(&tmp_path, &bytes).with_context(|| {
        format!(
            "failed to write diarization model to {}",
            tmp_path.display()
        )
    })?;
    std::fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "failed to finalize diarization model at {}",
            final_path.display()
        )
    })?;

    log::info!(
        "{LOG_PREFIX} downloaded diarization model {filename} ({} bytes) to {}",
        bytes.len(),
        final_path.display()
    );
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn models_installed_false_when_directory_empty() {
        let dir = tempdir().expect("tempdir");
        assert!(!models_installed(dir.path()));
    }

    #[test]
    fn models_installed_false_when_files_are_truncated_stubs() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join(SEGMENTATION_MODEL_FILENAME),
            b"not a real model",
        )
        .expect("write stub");
        std::fs::write(
            dir.path().join(EMBEDDING_MODEL_FILENAME),
            b"not a real model",
        )
        .expect("write stub");
        assert!(!models_installed(dir.path()));
    }

    #[test]
    fn models_installed_true_once_both_files_meet_the_size_floor() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join(SEGMENTATION_MODEL_FILENAME),
            vec![0u8; MIN_SEGMENTATION_MODEL_BYTES as usize],
        )
        .expect("write stub");
        std::fs::write(
            dir.path().join(EMBEDDING_MODEL_FILENAME),
            vec![0u8; MIN_EMBEDDING_MODEL_BYTES as usize],
        )
        .expect("write stub");
        assert!(models_installed(dir.path()));
    }
}
