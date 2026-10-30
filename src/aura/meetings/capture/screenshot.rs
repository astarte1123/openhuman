//! Periodic slide-screenshot capture for the core-native meeting capture
//! lifecycle (Block 3: periodic slide capture + OCR, wired into
//! [`super::pipeline::run_capture_session`]).
//!
//! Mirrors the Tauri shell's `meeting_recorder::screenshot` capture loop
//! (`app/src-tauri/src/meeting_recorder/screenshot.rs`) — same cadence, same
//! cheap-perceptual-diff dedup strategy, same `xcap` capture crate — but
//! runs natively inside the core process alongside
//! [`super::pipeline::run_consumer_loop`]'s audio consumer task, instead of
//! a separate shell process polling the primary monitor and pushing frames
//! back into this same core over its own HTTP JSON-RPC hop. There is no
//! extra transport here: [`push_screenshot`] calls
//! [`ops::meetings_append_screenshot`] directly, in-process — see that
//! function's doc comment for storage + OCR (reused unchanged, not
//! reimplemented).
//!
//! ## Lifecycle wiring
//!
//! [`spawn_capture_task`] is called once by
//! [`super::pipeline::run_capture_session`], right after the audio capture
//! backend comes up, with a dedicated `Arc<AtomicBool>` stop flag that is
//! also registered on the session's [`super::CaptureHandle`] via
//! `add_track` — so `CaptureHandle::stop()` signals this task's shutdown
//! exactly the same way it signals every audio capture thread's, with no
//! separate stop path to remember. Its `JoinHandle` is additionally
//! registered via `CaptureHandle::add_join_handle`, so
//! `CaptureHandle::stop_and_wait_for_drain()` (used by
//! `ops::meetings_finish`) waits for this task to actually finish an
//! in-flight capture/OCR/write cycle, not just for the stop flag to be
//! observed — otherwise the meeting could flip to `Completed` while a slide
//! captured right at the end was still being persisted, and get silently
//! dropped by `ops::meetings_append_screenshot`'s "meeting is not recording"
//! check. Opt-out (per meeting) via `meetings_start_local_capture`'s
//! `capture_screenshots` param — see `ops::meetings_start_local_capture`'s
//! doc comment.
//!
//! ## Cadence + duplicate detection + cap
//!
//! Captures a candidate frame every [`CAPTURE_INTERVAL`]. The stop flag is
//! polled far more often ([`STOP_POLL_INTERVAL`]) than that so
//! `CaptureHandle::stop()` doesn't have to wait a full capture interval to
//! take effect. Before persisting + OCR-ing a new frame, it is compared
//! against the last *persisted* frame with a cheap perceptual fingerprint:
//! both are downscaled to a small fixed-size grayscale thumbnail and
//! compared by mean absolute pixel difference ([`mean_abs_diff`]) — plain
//! arithmetic on pixel buffers, no perceptual-hashing crate needed for a
//! "did the slide change" check at this granularity. Only a frame that
//! differs enough from the last persisted one (or is the very first frame)
//! is pushed to [`ops::meetings_append_screenshot`]. Duplicate detection
//! alone can't bound total disk usage for legitimately-changing content
//! (scrolling docs, video, live code demos), so
//! [`MAX_SCREENSHOTS_PER_MEETING`] additionally hard-caps how many frames one
//! session will ever persist. The monitor capture + PNG encode + thumbnail
//! downscale are all synchronous, CPU/IO-bound work, so they run inside
//! `spawn_blocking` (via [`capture_frame_blocking`]) rather than directly on
//! this async task, mirroring `ops::meetings_append_screenshot`'s OCR call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use log::{debug, warn};

use crate::aura::config::Config;
use crate::aura::meetings::ops;

/// Grep-friendly prefix for every log line in this module (and the couple
/// of call-site log lines in `pipeline.rs` / `ops.rs` about starting or
/// skipping this task) — kept distinct from `capture::LOG_PREFIX`
/// (`[meetings-capture]`, audio-focused) so slide-capture activity can be
/// filtered independently, per CLAUDE.md's debug-logging convention.
const LOG_PREFIX: &str = "[meetings-slides]";

/// How often to capture a candidate frame while a local capture session is
/// running. Matches the Tauri shell's `meeting_recorder::screenshot`
/// cadence.
const CAPTURE_INTERVAL: Duration = Duration::from_secs(20);

/// How often the loop wakes up to check whether it's been stopped. Much
/// shorter than [`CAPTURE_INTERVAL`] so `CaptureHandle::stop()` is honored
/// promptly instead of after up to a full capture interval.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Both dimensions of the downscaled grayscale thumbnail used for the cheap
/// duplicate-detection comparison.
const DIFF_THUMBNAIL_SIZE: u32 = 32;

/// Mean absolute pixel difference (0-255 scale) a candidate frame's
/// thumbnail must exceed, versus the last *persisted* frame's thumbnail, to
/// be considered a genuinely new slide rather than a near-duplicate (cursor
/// blink, minor render jitter, etc.).
const DIFF_THRESHOLD: f64 = 8.0;

/// Upper bound on how many full-resolution screenshots one meeting's session
/// will persist. The near-duplicate check above only filters near-static
/// frames — it does nothing to bound output for content that legitimately
/// keeps changing (a scrolling document, a video tile, a live code demo), so
/// without a hard cap a very long meeting with continuously-changing content
/// could accumulate unbounded disk usage (full-screen PNGs, one every
/// [`CAPTURE_INTERVAL`]). At the default 20s cadence this caps a single
/// meeting at roughly 2.75 hours of active slide capture; once reached, the
/// capture loop keeps running (so it still honors the stop flag promptly)
/// but stops persisting new frames, logging a single warning rather than one
/// per skipped tick.
const MAX_SCREENSHOTS_PER_MEETING: usize = 500;

/// Spawns the periodic screenshot-capture task for one local capture
/// session. `stop_flag` is the same flag the caller registers on the
/// session's [`super::CaptureHandle`] via `add_track` — this task exits
/// (within [`STOP_POLL_INTERVAL`]) once that flag is set, with no separate
/// cancellation mechanism.
pub fn spawn_capture_task(
    meeting_id: String,
    config: Config,
    stop_flag: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run_capture_loop(meeting_id, config, stop_flag))
}

async fn run_capture_loop(meeting_id: String, config: Config, stop_flag: Arc<AtomicBool>) {
    debug!("{LOG_PREFIX} capture loop starting meeting_id={meeting_id}");
    let started_at = Instant::now();
    let mut last_saved_thumbnail: Option<Vec<u8>> = None;
    let mut elapsed_since_capture = Duration::ZERO;
    let mut persisted_count: usize = 0;
    let mut cap_warned = false;

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            debug!("{LOG_PREFIX} stop observed, exiting capture loop meeting_id={meeting_id}");
            break;
        }

        tokio::time::sleep(STOP_POLL_INTERVAL).await;
        elapsed_since_capture += STOP_POLL_INTERVAL;
        if !should_capture(elapsed_since_capture, CAPTURE_INTERVAL) {
            continue;
        }
        elapsed_since_capture = Duration::ZERO;

        if persisted_count >= MAX_SCREENSHOTS_PER_MEETING {
            if !cap_warned {
                warn!(
                    "{LOG_PREFIX} reached MAX_SCREENSHOTS_PER_MEETING={MAX_SCREENSHOTS_PER_MEETING} \
                     meeting_id={meeting_id}, no further slides will be captured for this session"
                );
                cap_warned = true;
            }
            continue;
        }

        let captured_at_ms = started_at.elapsed().as_millis() as i64;
        // Screen capture (xcap) + PNG encode + downscale/resize are all
        // synchronous, CPU/IO-bound work — run them off the async runtime
        // thread via `spawn_blocking` so a slow monitor capture or encode
        // can't stall other work scheduled on this same worker (e.g. this
        // loop's own stop-flag polling, or other in-flight RPC calls),
        // mirroring `ops::meetings_append_screenshot`'s OCR call.
        let (png_bytes, thumbnail) = match tokio::task::spawn_blocking(capture_frame_blocking).await
        {
            Ok(Ok(pair)) => pair,
            Ok(Err(err)) => {
                warn!("{LOG_PREFIX} screenshot capture failed meeting_id={meeting_id} error={err}");
                continue;
            }
            Err(join_err) => {
                warn!(
                    "{LOG_PREFIX} screenshot capture task panicked meeting_id={meeting_id} \
                     error={join_err}"
                );
                continue;
            }
        };

        if is_near_duplicate(last_saved_thumbnail.as_deref(), thumbnail.as_deref()) {
            debug!(
                "{LOG_PREFIX} skipping near-duplicate frame meeting_id={meeting_id} \
                 captured_at_ms={captured_at_ms}"
            );
            continue;
        }

        debug!(
            "{LOG_PREFIX} pushing new frame meeting_id={meeting_id} captured_at_ms={captured_at_ms} \
             png_bytes={}",
            png_bytes.len()
        );
        match push_screenshot(&config, &meeting_id, captured_at_ms, &png_bytes).await {
            Ok(()) => {
                // Only advance the dedup baseline on a successful persist,
                // so a transient failure (e.g. a momentary OCR/store error)
                // retries against the same reference frame next tick
                // instead of silently adopting a frame that was never
                // actually stored.
                last_saved_thumbnail = thumbnail;
                persisted_count += 1;
            }
            Err(err) => {
                warn!("{LOG_PREFIX} append_screenshot failed meeting_id={meeting_id} error={err}");
            }
        }
    }
}

/// Pure timer decision, extracted from [`run_capture_loop`] so it's unit
/// testable without a real tokio runtime or waiting on real timers: has
/// enough time elapsed since the last capture attempt to fire another one?
fn should_capture(elapsed_since_last_capture: Duration, interval: Duration) -> bool {
    elapsed_since_last_capture >= interval
}

/// Blocking entry point run inside `spawn_blocking` by [`run_capture_loop`]:
/// captures + PNG-encodes the current frame, then immediately derives its
/// dedup thumbnail from those same in-memory bytes — bundled into one
/// blocking call (rather than two separate `spawn_blocking` round-trips) so
/// the PNG bytes never need to be cloned just to cross the blocking-task
/// boundary twice.
fn capture_frame_blocking() -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let png_bytes = capture_primary_monitor_png()?;
    let thumbnail = downscale_grayscale_thumbnail(&png_bytes);
    Ok((png_bytes, thumbnail))
}

/// Captures the primary monitor (falling back to the first enumerated
/// monitor if none is marked primary) and encodes the frame as PNG bytes.
/// Needs a real display — see this module's doc comment and
/// `capture::windows`'s "needs hardware verification" note for the same
/// caveat on the audio side. Synchronous/blocking — callers must run this on
/// a blocking-safe thread (see [`capture_frame_blocking`]), never directly on
/// an async runtime worker.
fn capture_primary_monitor_png() -> Result<Vec<u8>, String> {
    let monitors =
        xcap::Monitor::all().map_err(|e| format!("failed to enumerate monitors: {e}"))?;
    if monitors.is_empty() {
        return Err("no monitors available to capture".to_string());
    }
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .unwrap_or(&monitors[0]);

    let image = monitor
        .capture_image()
        .map_err(|e| format!("failed to capture monitor image: {e}"))?;

    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("failed to encode screenshot as PNG: {e}"))?;
    Ok(png_bytes)
}

/// Downscales a PNG-encoded frame to a fixed-size grayscale thumbnail for
/// cheap duplicate-detection comparison. Returns `None` if the bytes don't
/// decode (in which case dedup is skipped for that frame — see call site,
/// which then always treats it as new rather than risk silently dropping a
/// real slide change).
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

/// Duplicate-detection decision: `true` only when both thumbnails are
/// present and their [`mean_abs_diff`] is at or below [`DIFF_THRESHOLD`].
/// Missing either side (no prior frame yet, or the current frame failed to
/// decode) always reports "not a duplicate" — an undecodable frame must
/// never be silently dropped as though it matched the last one.
fn is_near_duplicate(prev_thumbnail: Option<&[u8]>, curr_thumbnail: Option<&[u8]>) -> bool {
    match (prev_thumbnail, curr_thumbnail) {
        (Some(prev), Some(curr)) => mean_abs_diff(prev, curr) <= DIFF_THRESHOLD,
        _ => false,
    }
}

/// Mean absolute per-pixel difference between two equal-length grayscale
/// pixel buffers. Mismatched lengths (shouldn't happen — both thumbnails
/// are always [`DIFF_THUMBNAIL_SIZE`] x [`DIFF_THUMBNAIL_SIZE`]) are treated
/// as "maximally different" so a shape mismatch never causes a frame to be
/// wrongly skipped as a duplicate.
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

/// Base64-encodes `png_bytes` and persists it via the existing
/// [`ops::meetings_append_screenshot`] — reusing its screenshot-storage +
/// OCR pipeline unchanged rather than duplicating either.
async fn push_screenshot(
    config: &Config,
    meeting_id: &str,
    captured_at_ms: i64,
    png_bytes: &[u8],
) -> Result<(), String> {
    let image_base64 = BASE64.encode(png_bytes);
    ops::meetings_append_screenshot(config, meeting_id, captured_at_ms, &image_base64)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── duplicate-detection hash logic ──────────────────────────────────

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
    fn is_near_duplicate_true_when_diff_at_or_below_threshold() {
        let prev = vec![100u8; 16];
        // Every pixel differs by exactly DIFF_THRESHOLD, so the mean
        // absolute difference lands exactly on the threshold — the `<=`
        // comparison in `is_near_duplicate` must still call this a
        // duplicate.
        let curr: Vec<u8> = prev.iter().map(|v| v + DIFF_THRESHOLD as u8).collect();
        assert!(is_near_duplicate(Some(&prev), Some(&curr)));
    }

    #[test]
    fn is_near_duplicate_false_when_diff_exceeds_threshold() {
        let prev = vec![0u8; 16];
        let curr = vec![255u8; 16];
        assert!(!is_near_duplicate(Some(&prev), Some(&curr)));
    }

    #[test]
    fn is_near_duplicate_false_when_no_prior_frame() {
        let curr = vec![10u8; 16];
        assert!(!is_near_duplicate(None, Some(&curr)));
    }

    #[test]
    fn is_near_duplicate_false_when_current_frame_undecodable() {
        let prev = vec![10u8; 16];
        assert!(!is_near_duplicate(Some(&prev), None));
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
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let thumbnail = downscale_grayscale_thumbnail(&png_bytes).expect("valid PNG should decode");
        assert_eq!(
            thumbnail.len(),
            (DIFF_THUMBNAIL_SIZE * DIFF_THUMBNAIL_SIZE) as usize
        );
    }

    // ── timer/interval logic ────────────────────────────────────────────

    #[test]
    fn should_capture_false_before_interval_elapsed() {
        assert!(!should_capture(Duration::from_secs(19), CAPTURE_INTERVAL));
    }

    #[test]
    fn should_capture_true_once_interval_elapsed() {
        assert!(should_capture(Duration::from_secs(20), CAPTURE_INTERVAL));
        assert!(should_capture(Duration::from_secs(25), CAPTURE_INTERVAL));
    }

    #[test]
    fn should_capture_false_at_zero_elapsed() {
        assert!(!should_capture(Duration::ZERO, CAPTURE_INTERVAL));
    }
}
