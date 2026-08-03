//! Periodic screenshot capture of the primary monitor during an active
//! meeting recording, pushed to the core `meetings` domain for OCR.
//!
//! Spawned as a sibling task to `recorder.rs`'s audio `pipeline_task`, tied
//! to the same `ActiveSession` lifecycle (see `recorder::ActiveSession`):
//! [`spawn_capture_task`] is called from `meeting_recorder_start`, and the
//! returned `JoinHandle` is stored and `.abort()`-ed from
//! `meeting_recorder_stop`, exactly like `pipeline_task`. There is no
//! internal stop signal — plain task cancellation on `.abort()` is enough
//! since this loop has no in-flight resource (no open stream/socket) that
//! needs a graceful unwind.
//!
//! ## Cadence + dedup
//!
//! Captures a candidate frame every [`CAPTURE_INTERVAL`]. Before saving +
//! OCR-ing a new frame, it is compared against the last *saved* frame with
//! a cheap perceptual diff: both are downscaled to a small fixed-size
//! grayscale thumbnail and compared by mean absolute pixel difference
//! ([`mean_abs_diff`]). Only a frame that differs enough from the last
//! saved one (or is the very first frame) is pushed — this is plain
//! arithmetic on pixel buffers, no perceptual-hashing crate needed for a
//! "did the slide change" check at 20-second granularity.
//!
//! ## Capture + core RPC
//!
//! Uses [`xcap::Monitor`] (primary monitor only for v1 — see
//! [`capture_primary_monitor_png`]) to grab a frame, encodes it as PNG,
//! base64s it, and calls the new `openhuman.meetings_append_screenshot`
//! core RPC. [`rpc_call`] is a module-local copy of
//! `recorder::rpc_call`'s minimal JSON-RPC-over-HTTP helper — see that
//! module's doc comment ("STEP 0 finding") for why the Tauri shell talks to
//! the in-process core over plain HTTP rather than the native event bus,
//! and for the existing-project convention of each shell feature module
//! owning its own small copy of this helper (e.g. `telegram_scanner`)
//! rather than sharing one.

use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

const LOG_PREFIX: &str = "[meeting-recorder:screenshot]";

/// How often to capture a candidate frame while recording.
const CAPTURE_INTERVAL: Duration = Duration::from_secs(20);

/// Both dimensions of the downscaled grayscale thumbnail used for the
/// cheap perceptual dedup comparison.
const DIFF_THUMBNAIL_SIZE: u32 = 32;

/// Mean absolute pixel difference (0-255 scale) a candidate frame's
/// thumbnail must exceed, versus the last *saved* frame's thumbnail, to be
/// considered a genuinely new slide rather than a near-duplicate (cursor
/// blink, minor render jitter, etc.).
const DIFF_THRESHOLD: f64 = 8.0;

/// Spawns the periodic screenshot-capture task for one recording session.
/// Callers store the returned handle on `ActiveSession` and `.abort()` it
/// on stop — mirrors how `recorder::meeting_recorder_start` spawns and
/// stores `pipeline_task`.
pub fn spawn_capture_task(meeting_id: String) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(run_capture_loop(meeting_id))
}

async fn run_capture_loop(meeting_id: String) {
    log::info!("{LOG_PREFIX} capture loop starting meeting_id={meeting_id}");
    let started_at = Instant::now();
    let mut last_saved_thumbnail: Option<Vec<u8>> = None;

    loop {
        tokio::time::sleep(CAPTURE_INTERVAL).await;

        let captured_at_ms = started_at.elapsed().as_millis() as i64;
        let png_bytes = match capture_primary_monitor_png() {
            Ok(bytes) => bytes,
            Err(err) => {
                log::warn!("{LOG_PREFIX} screenshot capture failed: {err}");
                continue;
            }
        };

        let thumbnail = downscale_grayscale_thumbnail(&png_bytes);
        let is_near_duplicate = match (&last_saved_thumbnail, &thumbnail) {
            (Some(prev), Some(curr)) => mean_abs_diff(prev, curr) <= DIFF_THRESHOLD,
            _ => false,
        };
        if is_near_duplicate {
            log::debug!(
                "{LOG_PREFIX} skipping near-duplicate frame meeting_id={meeting_id} \
                 captured_at_ms={captured_at_ms}"
            );
            continue;
        }

        log::debug!(
            "{LOG_PREFIX} pushing new frame meeting_id={meeting_id} captured_at_ms={captured_at_ms} \
             png_bytes={}",
            png_bytes.len()
        );
        match push_screenshot(&meeting_id, captured_at_ms, &png_bytes).await {
            Ok(()) => {
                // Only advance the dedup baseline on a successful push, so a
                // transient RPC failure retries against the same reference
                // frame next tick instead of silently adopting a frame the
                // core never actually stored.
                last_saved_thumbnail = thumbnail;
            }
            Err(err) => {
                log::warn!("{LOG_PREFIX} append_screenshot failed: {err}");
            }
        }
    }
}

/// Captures the primary monitor (falling back to the first enumerated
/// monitor if none is marked primary) and encodes the frame as PNG bytes.
fn capture_primary_monitor_png() -> anyhow::Result<Vec<u8>> {
    let monitors =
        xcap::Monitor::all().map_err(|e| anyhow::anyhow!("failed to enumerate monitors: {e}"))?;
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("no monitors available to capture"));
    }
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .unwrap_or(&monitors[0]);

    let image = monitor
        .capture_image()
        .map_err(|e| anyhow::anyhow!("failed to capture monitor image: {e}"))?;

    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| anyhow::anyhow!("failed to encode screenshot as PNG: {e}"))?;
    Ok(png_bytes)
}

/// Downscales a PNG-encoded frame to a fixed-size grayscale thumbnail for
/// cheap dedup comparison. Returns `None` if the bytes don't decode (in
/// which case dedup is skipped for that frame — see call site).
fn downscale_grayscale_thumbnail(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png).ok()?;
    let thumbnail = img
        .resize_exact(
            DIFF_THUMBNAIL_SIZE,
            DIFF_THUMBNAIL_SIZE,
            image::imageops::FilterType::Triangle,
        )
        .into_luma8();
    Some(thumbnail.into_raw())
}

/// Mean absolute per-pixel difference between two equal-length grayscale
/// pixel buffers. Mismatched lengths (shouldn't happen — both thumbnails
/// are always [`DIFF_THUMBNAIL_SIZE`] x [`DIFF_THUMBNAIL_SIZE`]) are
/// treated as "maximally different" so a shape mismatch never causes a
/// frame to be wrongly skipped as a duplicate.
fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return f64::MAX;
    }
    let sum: u64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as i64 - *y as i64).unsigned_abs())
        .sum();
    sum as f64 / a.len() as f64
}

async fn push_screenshot(meeting_id: &str, captured_at_ms: i64, png_bytes: &[u8]) -> Result<(), String> {
    let image_base64 = BASE64.encode(png_bytes);
    rpc_call(
        "openhuman.meetings_append_screenshot",
        serde_json::json!({
            "meeting_id": meeting_id,
            "captured_at_ms": captured_at_ms,
            "image_base64": image_base64,
        }),
    )
    .await
    .map(|_| ())
}

/// Minimal JSON-RPC helper — module-local copy of `recorder::rpc_call` (see
/// this module's doc comment for why each shell feature module owns its
/// own copy rather than sharing one).
async fn rpc_call(method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let url = crate::core_rpc::core_rpc_url_value();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let req = crate::core_rpc::apply_auth(client.post(&url))
        .map_err(|e| format!("prepare {url}: {e}"))?;
    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;
    let status = resp.status();
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("decode {status}: {e}"))?;
    if !status.is_success() {
        return Err(format!("{status}: {v}"));
    }
    if let Some(err) = v.get("error") {
        return Err(format!("rpc error: {err}"));
    }
    Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_abs_diff_is_zero_for_identical_buffers() {
        let a = vec![10u8, 20, 30, 40];
        assert_eq!(mean_abs_diff(&a, &a), 0.0);
    }

    #[test]
    fn mean_abs_diff_computes_average_of_absolute_differences() {
        let a = vec![0u8, 0, 0, 0];
        let b = vec![10u8, 20, 30, 40];
        // (10 + 20 + 30 + 40) / 4 = 25
        assert_eq!(mean_abs_diff(&a, &b), 25.0);
    }

    #[test]
    fn mean_abs_diff_treats_mismatched_lengths_as_maximally_different() {
        let a = vec![1u8, 2, 3];
        let b = vec![1u8, 2];
        assert_eq!(mean_abs_diff(&a, &b), f64::MAX);
    }

    #[test]
    fn mean_abs_diff_treats_empty_buffers_as_maximally_different() {
        assert_eq!(mean_abs_diff(&[], &[]), f64::MAX);
    }

    #[test]
    fn downscale_grayscale_thumbnail_returns_none_for_invalid_png() {
        assert!(downscale_grayscale_thumbnail(b"not a png").is_none());
    }

    #[test]
    fn downscale_grayscale_thumbnail_returns_fixed_size_buffer_for_valid_png() {
        let img = image::RgbImage::from_pixel(64, 64, image::Rgb([200, 100, 50]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .unwrap();

        let thumbnail = downscale_grayscale_thumbnail(&png_bytes).expect("valid PNG should decode");
        assert_eq!(thumbnail.len(), (DIFF_THUMBNAIL_SIZE * DIFF_THUMBNAIL_SIZE) as usize);
    }
}
