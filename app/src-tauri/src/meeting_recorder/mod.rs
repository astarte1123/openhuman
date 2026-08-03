//! Meeting recording pipeline: local audio capture (mic + system
//! loopback) -> mixing -> voice-activity detection -> in-process Whisper
//! transcription -> push to the `aura::meetings` core domain.
//!
//! Phase 2+3 of the Meetily integration plan (Phase 1 — the `meetings`
//! core RPC surface — already exists at `src/aura/meetings/`). This
//! module is the shell-side audio pipeline that feeds it.
//!
//! **Windows only** for now. macOS/Linux calls fail fast with an explicit
//! "not yet implemented for this platform" error (see `devices.rs` /
//! `whisper_engine.rs`) rather than pretending to support it — no
//! noise-suppression/loudness-normalization either (declared follow-up,
//! non-blocking), no GPU acceleration (CPU-only whisper.cpp), no speaker
//! diarization.
//!
//! ## Module map
//!
//! - [`devices`] — WASAPI device enumeration + resolution (Windows), with
//!   explicit non-Windows stubs.
//! - [`stream`] — cpal capture (mic + system-loopback share one code
//!   path), bridged off the real-time audio thread via a lock-free
//!   `ringbuf` ring buffer.
//! - [`mixer`] — fixed-window two-source mixer (sum + soft-clip).
//! - [`resample`] — per-device resampling to the VAD's fixed 16kHz input
//!   rate (`rubato`).
//! - [`vad`] — Silero VAD wrapper, buffers to 30ms frames, filters out
//!   sub-50ms segments.
//! - [`whisper_engine`] — in-process whisper.cpp (CPU-only) + GGML model
//!   download/cache.
//! - [`screenshot`] — periodic primary-monitor capture + perceptual dedup,
//!   pushing new slide frames to the core `meetings` domain for OCR.
//! - [`recorder`] — orchestration, Tauri command surface, core RPC push.

mod devices;
mod mixer;
pub mod recorder;
mod resample;
mod screenshot;
mod stream;
mod vad;
mod whisper_engine;

pub use recorder::MeetingRecorderState;
