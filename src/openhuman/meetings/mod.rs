//! Meeting recordings, transcripts, and summaries.
//!
//! Phase 1 of the Meetily port: schema + SQLite storage + RPC CRUD +
//! LLM-backed summarization only. Audio capture and the Tauri shell wiring
//! are out of scope for this phase.
//!
//! Follows `openhuman::cron`'s module conventions: [`store`] owns the
//! `rusqlite` persistence, [`ops`] exposes async `meetings_*` functions that
//! wrap [`store`] results in [`crate::rpc::RpcOutcome`], and [`schemas`]
//! declares the `"meetings"` namespace controller schemas + handlers
//! consumed by `src/core/all.rs`. No `bus.rs` yet — this domain has no event
//! producer until a later phase wires up real audio capture.
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

mod export;
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
