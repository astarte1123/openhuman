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
//!
//! `screenshot` (private; used only by [`pipeline::run_capture_session`])
//! is Block 3: a sibling periodic task, spawned alongside the audio consumer
//! loop for the same session, that captures the primary monitor every ~20s,
//! skips near-duplicate frames, and persists new ones via the existing
//! [`crate::aura::meetings::ops::meetings_append_screenshot`] — see that
//! module's doc comment for the full design and for why it isn't
//! platform-gated to Windows the way audio capture currently is.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{debug, warn};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

pub mod pipeline;
#[cfg(feature = "diarize")]
pub mod raw_audio_sink;
/// Periodic slide-screenshot capture (Block 3), wired into
/// [`pipeline::run_capture_session`] — see that module's doc comment.
/// Private: nothing outside `capture` needs it directly.
mod screenshot;
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
/// [`CaptureHandle::stop_and_wait_for_drain`] additionally waits for the
/// consumer loop to actually finish flushing/finalizing before returning —
/// see its own doc comment for when that matters.
///
/// Dropping a `CaptureHandle` without calling `stop()` leaks the running
/// capture threads — callers (`ops::meetings_stop_local_capture`, and
/// `meetings_finish`/`meetings_delete` force-stopping an active session) must
/// always call `stop()` (or `stop_and_wait_for_drain()`) explicitly.
pub struct CaptureHandle {
    tracks: Vec<(Arc<AtomicBool>, &'static str)>,
    /// Resolves once [`super::pipeline`]'s consumer loop has drained every
    /// buffered chunk and — under the `diarize` feature — finalized the raw
    /// `system_audio` WAV, after every track's stop flag has been observed.
    /// `None` for a handle nothing has wired this signal into yet (e.g. a
    /// bare `CaptureHandle::new()` built directly in a unit test) — see
    /// [`CaptureHandle::stop_and_wait_for_drain`]'s doc comment for why this
    /// exists.
    finished_rx: Option<oneshot::Receiver<()>>,
    /// Sibling tasks spawned for this session whose *task completion*
    /// [`CaptureHandle::stop_and_wait_for_drain`] must also wait for, beyond
    /// [`finished_rx`](Self::finished_rx) — currently just
    /// [`super::screenshot::spawn_capture_task`]'s `JoinHandle`. Its stop
    /// flag is already registered via [`CaptureHandle::add_track`] (so
    /// `stop()`/`signal_stop` tells it to exit), but a stop flag only signals
    /// *intent*; without awaiting the task itself, `stop_and_wait_for_drain`
    /// could return — and a caller like `ops::meetings_finish` could flip the
    /// meeting to `Completed` — while that task is still mid-cycle (in-flight
    /// capture/OCR/write). `label` is used only for logging.
    extra_join_handles: Vec<(tokio::task::JoinHandle<()>, &'static str)>,
}

impl CaptureHandle {
    fn new() -> Self {
        Self {
            tracks: Vec::new(),
            finished_rx: None,
            extra_join_handles: Vec::new(),
        }
    }

    /// Registers one track's stop flag so [`CaptureHandle::stop`] can signal
    /// it. `label` is used only for logging.
    fn add_track(&mut self, stop_flag: Arc<AtomicBool>, label: &'static str) {
        self.tracks.push((stop_flag, label));
    }

    /// Attaches the consumer loop's drain-completion signal — see
    /// [`finished_rx`](Self::finished_rx)'s doc comment. Called once by
    /// [`super::pipeline::run_capture_session`] right after spawning the
    /// consumer task, before the handle is returned to its caller.
    pub(crate) fn set_finished_signal(&mut self, rx: oneshot::Receiver<()>) {
        self.finished_rx = Some(rx);
    }

    /// Registers a sibling task's `JoinHandle` so
    /// [`CaptureHandle::stop_and_wait_for_drain`] also waits for it to
    /// actually finish — see
    /// [`extra_join_handles`](Self::extra_join_handles)'s doc comment.
    pub(crate) fn add_join_handle(
        &mut self,
        handle: tokio::task::JoinHandle<()>,
        label: &'static str,
    ) {
        self.extra_join_handles.push((handle, label));
    }

    /// True when this handle has at least one live track — used by backends
    /// to decide whether a partial setup failure (e.g. loopback device
    /// missing) should still count as a usable session.
    pub fn has_tracks(&self) -> bool {
        !self.tracks.is_empty()
    }

    fn signal_stop(&self) {
        debug!(
            "{LOG_PREFIX} stop requested for {} track(s)",
            self.tracks.len()
        );
        for (flag, label) in &self.tracks {
            debug!("{LOG_PREFIX} signalling stop for track={label}");
            flag.store(true, Ordering::SeqCst);
        }
    }

    pub fn stop(self) -> Result<(), String> {
        self.signal_stop();
        Ok(())
    }

    /// Same as [`stop`](Self::stop) but also waits, up to `timeout`, for the
    /// consumer loop to actually finish draining its buffers — and, under
    /// the `diarize` feature, finalizing the raw `system_audio` WAV (`hound`
    /// rewriting its header with the final sample count) — before returning.
    ///
    /// [`crate::aura::meetings::ops::meetings_finish`] uses this instead of
    /// plain [`stop`](Self::stop) specifically because a client's natural
    /// next call is `aura.meetings_diarize` against that same WAV file: the
    /// WAV finalize happens asynchronously, well after `stop()` merely flips
    /// a flag (see [`super::pipeline`]'s consumer loop), so without waiting
    /// here `meetings_diarize` could open the file before its header was
    /// ever rewritten and silently read zero/truncated samples.
    ///
    /// Also awaits every sibling task registered via
    /// [`CaptureHandle::add_join_handle`] (currently just the screenshot
    /// capture task, when the session has one) to actually finish — not just
    /// observe its stop flag — so a caller like `ops::meetings_finish` never
    /// marks the meeting `Completed` while a sibling task is still mid-cycle
    /// (e.g. mid capture/OCR/write). Both the audio-drain wait and the
    /// sibling-task waits share the single `timeout` budget below.
    ///
    /// Best-effort: a missing signal (handle built without one, e.g. a unit
    /// test), a dropped sender (consumer task panicked before sending), a
    /// sibling task join error, or an overall timeout are all logged and
    /// treated as "stopped, drain outcome unknown" rather than propagated as
    /// a hard error — every track's stop flag is signalled unconditionally
    /// before any of that is checked, so the capture threads are never left
    /// running regardless of how this returns.
    pub async fn stop_and_wait_for_drain(self, timeout: Duration) -> Result<(), String> {
        self.signal_stop();
        let extra_join_handles = self.extra_join_handles;
        let drain = async move {
            match self.finished_rx {
                Some(finished_rx) => match finished_rx.await {
                    Ok(()) => debug!(
                        "{LOG_PREFIX} stop_and_wait_for_drain: consumer loop drained and finalized"
                    ),
                    Err(_) => warn!(
                        "{LOG_PREFIX} stop_and_wait_for_drain: finished-signal sender dropped \
                         without completing (consumer task may have panicked) — proceeding \
                         without drain confirmation"
                    ),
                },
                None => debug!(
                    "{LOG_PREFIX} stop_and_wait_for_drain: no finished-signal attached, cannot \
                     wait for drain"
                ),
            }

            for (handle, label) in extra_join_handles {
                match handle.await {
                    Ok(()) => debug!(
                        "{LOG_PREFIX} stop_and_wait_for_drain: sibling task '{label}' finished"
                    ),
                    Err(e) => warn!(
                        "{LOG_PREFIX} stop_and_wait_for_drain: sibling task '{label}' join \
                         error: {e} — proceeding without its completion confirmed"
                    ),
                }
            }
        };

        match tokio::time::timeout(timeout, drain).await {
            Ok(()) => Ok(()),
            Err(_) => {
                warn!(
                    "{LOG_PREFIX} stop_and_wait_for_drain: timed out after {timeout:?} waiting \
                     for the capture session to drain/finalize — proceeding without drain \
                     confirmation"
                );
                Ok(())
            }
        }
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

    // ── stop_and_wait_for_drain (meetings_finish / meetings_diarize race) ──

    #[tokio::test]
    async fn stop_and_wait_for_drain_without_signal_returns_immediately() {
        let mut handle = CaptureHandle::new();
        let flag = Arc::new(AtomicBool::new(false));
        handle.add_track(flag.clone(), "microphone");

        // No `set_finished_signal` call — mirrors a bare handle in a test
        // that never wired up `pipeline::run_capture_session`.
        handle
            .stop_and_wait_for_drain(Duration::from_millis(50))
            .await
            .expect("must not error when no signal is attached");

        assert!(
            flag.load(Ordering::SeqCst),
            "stop flags must still be signalled"
        );
    }

    #[tokio::test]
    async fn stop_and_wait_for_drain_resolves_once_finished_signal_fires() {
        let mut handle = CaptureHandle::new();
        let flag = Arc::new(AtomicBool::new(false));
        handle.add_track(flag.clone(), "system_audio");

        let (tx, rx) = oneshot::channel();
        handle.set_finished_signal(rx);

        // Simulate the consumer loop finishing its drain/finalize shortly
        // after observing the stop flag.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let _ = tx.send(());
        });

        handle
            .stop_and_wait_for_drain(Duration::from_secs(5))
            .await
            .expect("must resolve once the finished signal fires");

        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn stop_and_wait_for_drain_times_out_without_hanging_forever() {
        let mut handle = CaptureHandle::new();
        let flag = Arc::new(AtomicBool::new(false));
        handle.add_track(flag.clone(), "system_audio");

        let (tx, rx) = oneshot::channel::<()>();
        handle.set_finished_signal(rx);
        // Sender deliberately never fires and is kept alive for the whole
        // call — this exercises the timeout branch, not the dropped-sender
        // branch.
        let _keep_alive = tx;

        handle
            .stop_and_wait_for_drain(Duration::from_millis(20))
            .await
            .expect("a timeout must be swallowed, not propagated as an error");

        assert!(
            flag.load(Ordering::SeqCst),
            "stop flags must still be signalled"
        );
    }

    #[tokio::test]
    async fn stop_and_wait_for_drain_tolerates_dropped_sender() {
        let mut handle = CaptureHandle::new();
        let flag = Arc::new(AtomicBool::new(false));
        handle.add_track(flag.clone(), "system_audio");

        let (tx, rx) = oneshot::channel::<()>();
        handle.set_finished_signal(rx);
        drop(tx); // Simulates the consumer task panicking before signalling.

        handle
            .stop_and_wait_for_drain(Duration::from_secs(5))
            .await
            .expect("a dropped sender must be swallowed, not propagated as an error");

        assert!(flag.load(Ordering::SeqCst));
    }
}
