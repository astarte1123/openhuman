//! Meeting recordings, transcripts, and summaries.
//!
//! Phase 1 of the Meetily port: schema + SQLite storage + RPC CRUD +
//! LLM-backed summarization only. Local live audio capture is now wired up
//! (see [`capture`], Windows-only so far); other Tauri shell wiring (e.g.
//! Teams/Zoom bot capture) remains out of scope for later phases.
//!
//! Follows `aura::cron`'s module conventions: [`store`] owns the
//! `rusqlite` persistence, [`ops`] exposes async `meetings_*` functions that
//! wrap [`store`] results in [`crate::rpc::RpcOutcome`], and [`schemas`]
//! declares the `"meetings"` namespace controller schemas + handlers
//! consumed by `src/core/all.rs`. [`bus`] publishes this domain's first
//! event-bus producer (`DomainEvent::MeetingTranscriptChunkReceived`, fired
//! from [`capture::pipeline`] as live transcript chunks are persisted).
//!
//! `export` (private; used only by [`ops::meetings_export`]) renders a
//! meeting's transcript + summary to PDF/DOCX bytes via pure-Rust crates
//! (`printpdf`, `docx-rs`) — see its module doc for why those specific
//! crates were chosen.
//!
//! `ocr` (private; used only by [`ops::meetings_append_screenshot`]) runs
//! text extraction over periodically-captured meeting-slide screenshots via
//! the pure-Rust `ocrs` engine — see its module doc for why OCR was chosen
//! over a vision LLM, and for the pretrained model download URLs.
//!
//! Block 3 wires that periodic capture into the core-native local capture
//! lifecycle: [`ops::meetings_start_local_capture`] optionally starts
//! [`capture::pipeline`]'s sibling `capture::screenshot` task (private,
//! see `capture::mod`'s doc comment), which grabs the primary monitor every
//! ~20s, skips near-duplicate frames, and persists new ones via the
//! existing [`ops::meetings_append_screenshot`] — no new screenshot storage
//! or OCR path, just a new trigger for the one that already exists.
//!
//! `diarize` (private; used only by [`ops::meetings_diarize`], gated behind
//! the optional, off-by-default `diarize` Cargo feature) runs real
//! per-speaker diarization over a completed meeting's recorded
//! `system_audio` track, splitting the single `"system_audio"` speaker
//! label into `"system_audio:speaker_N"` per distinct remote voice — see
//! its module doc for what's ported from Minutes' `diarize.rs`, what's
//! deliberately left out, and why it needs real onnxruntime (via `ort`)
//! unlike every other meetings-domain ML dependency.

mod bus;
pub mod capture;
#[cfg(feature = "diarize")]
mod diarize;
mod export;
mod export_pptx;
mod ocr;
pub mod ops;
mod schemas;
mod store;
mod types;

pub use schemas::{
    all_controller_schemas as all_meetings_controller_schemas,
    all_registered_controllers as all_meetings_registered_controllers,
};
pub use types::{Meeting, MeetingScreenshot, MeetingStatus, TranscriptChunk};
