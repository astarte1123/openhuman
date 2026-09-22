//! Local audio capture backend contract for live meeting transcription.
//!
//! [`AudioCaptureBackend`] is the platform-agnostic capture contract; each OS
//! gets its own implementation behind [`default_backend`]. Block 1 of the
//! meeting-capture design only ships the Windows backend ([`windows`],
//! WASAPI via `cpal` for the microphone + the `wasapi` crate for system-audio
//! loopback) — macOS and Linux capture is a later, separate block and
//! [`default_backend`] returns a stub that always errors on those platforms.
//!
//! Capture backends push raw, un-resampled [`AudioChunk`]s onto an unbounded
//! channel from a realtime-ish OS audio callback thread; [`pipeline`] owns
//! the async consumer side (buffering, resampling, whisper transcription,
//! persistence, event-bus publish). Keeping capture and pipeline in separate
//! modules keeps the callback-thread code — which must never block, allocate
//! unpredictably, or await — clearly separated from the tokio-task code that
//! does all of that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::debug;
use tokio::sync::mpsc::UnboundedSender;

pub mod pipeline;
#[cfg(target_os = "windows")]
pub mod windows;

pub(crate) const LOG_PREFIX: &str = "[meetings-capture]";

/// Which audio path a captured chunk came from.
///
/// Kept separate — never mixed at capture time — so a later
/// diarization/OCR-aligned UI can distinguish "you" from "them" (or drive
/// per-track sensitivity). Mixing mic + system audio at capture time is a
/// lossy operation that can't be undone downstream; the whisper pipeline can
/// accept either track independently, or a caller can mix later if desired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioTrack {
    Microphone,
    SystemAudio,
}

impl AudioTrack {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::SystemAudio => "system_audio",
        }
    }
}

/// One raw, un-resampled slice of captured audio pushed off a capture
/// backend's dedicated OS thread.
///
/// `sample_rate` matches whatever the device negotiated (commonly 48kHz for
/// both WASAPI microphone and loopback endpoints on Windows) —
/// [`pipeline::CaptureSession`] resamples to whisper's required 16kHz mono
/// before transcription; capture backends must not resample themselves so
/// this type stays a pure data carrier.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub pcm_f32: Vec<f32>,
    pub sample_rate: u32,
    pub track: AudioTrack,
    /// Milliseconds since the capture session started (monotonic), used to
    /// stamp transcript chunk start/end offsets.
    pub captured_at_ms: i64,
}

/// Handle to a running capture session covering one or more tracks (e.g.
/// microphone + system audio). Calling [`CaptureHandle::stop`] signals every
/// underlying capture thread to stop; it does not block waiting for the
/// threads to actually exit (they poll their stop flag and tear down their
/// OS stream on their own dedicated thread, mirroring
/// [`crate::aura::voice::audio_capture`]'s recording lifecycle).
///
/// Dropping a `CaptureHandle` without calling `stop()` leaks the running
/// capture threads — callers (`ops::meetings_stop_local_capture`, and
/// `meetings_finish`/`meetings_delete` force-stopping an active session) must
/// always call `stop()` explicitly.
pub struct CaptureHandle {
    tracks: Vec<(Arc<AtomicBool>, &'static str)>,
}

impl CaptureHandle {
    fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    /// Registers one track's stop flag so [`CaptureHandle::stop`] can signal
    /// it. `label` is used only for logging.
    fn add_track(&mut self, stop_flag: Arc<AtomicBool>, label: &'static str) {
        self.tracks.push((stop_flag, label));
    }

    /// True when this handle has at least one live track — used by backends
    /// to decide whether a partial setup failure (e.g. loopback device
    /// missing) should still count as a usable session.
    pub fn has_tracks(&self) -> bool {
        !self.tracks.is_empty()
    }

    pub fn stop(self) -> Result<(), String> {
        debug!(
            "{LOG_PREFIX} stop requested for {} track(s)",
            self.tracks.len()
        );
        for (flag, label) in &self.tracks {
            debug!("{LOG_PREFIX} signalling stop for track={label}");
            flag.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Platform-agnostic capture backend contract.
///
/// Implementations run on dedicated OS threads (audio APIs generally aren't
/// `Send`-friendly across an async runtime — `cpal::Stream` and WASAPI COM
/// interfaces both must be created, used, and dropped on the same thread)
/// and push chunks onto `sink` non-blockingly from that thread's audio
/// callback.
pub trait AudioCaptureBackend: Send {
    fn start(&self, sink: UnboundedSender<AudioChunk>) -> Result<CaptureHandle, String>;
}

#[cfg(target_os = "windows")]
pub fn default_backend() -> Box<dyn AudioCaptureBackend> {
    Box::new(windows::WindowsAudioCapture)
}

/// Other platforms don't have local meeting-audio capture wired up yet
/// (see the meeting-capture design's block 10 — macOS/Linux capture is a
/// separate, later phase). Returns a backend whose `start()` always errors
/// so callers get a clear message instead of a platform-specific compile
/// failure.
#[cfg(not(target_os = "windows"))]
pub fn default_backend() -> Box<dyn AudioCaptureBackend> {
    Box::new(UnsupportedPlatformBackend)
}

#[cfg(not(target_os = "windows"))]
struct UnsupportedPlatformBackend;

#[cfg(not(target_os = "windows"))]
impl AudioCaptureBackend for UnsupportedPlatformBackend {
    fn start(&self, _sink: UnboundedSender<AudioChunk>) -> Result<CaptureHandle, String> {
        Err("local meeting audio capture is not implemented on this platform yet".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_track_as_str() {
        assert_eq!(AudioTrack::Microphone.as_str(), "microphone");
        assert_eq!(AudioTrack::SystemAudio.as_str(), "system_audio");
    }

    #[test]
    fn capture_handle_stop_signals_every_track() {
        let mut handle = CaptureHandle::new();
        let flag_a = Arc::new(AtomicBool::new(false));
        let flag_b = Arc::new(AtomicBool::new(false));
        handle.add_track(flag_a.clone(), "microphone");
        handle.add_track(flag_b.clone(), "system_audio");

        assert!(handle.has_tracks());
        handle.stop().expect("stop should not fail");

        assert!(flag_a.load(Ordering::SeqCst));
        assert!(flag_b.load(Ordering::SeqCst));
    }

    #[test]
    fn capture_handle_with_no_tracks_reports_empty() {
        let handle = CaptureHandle::new();
        assert!(!handle.has_tracks());
        // Stopping an empty handle is a no-op, not an error.
        handle.stop().expect("stop of empty handle should not fail");
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn unsupported_platform_backend_errors_clearly() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let backend = UnsupportedPlatformBackend;
        let err = backend.start(tx).unwrap_err();
        assert!(err.contains("not implemented on this platform"));
    }
}
