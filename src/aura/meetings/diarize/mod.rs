//! Real per-speaker diarization for a meeting's recorded `system_audio`
//! track — splitting the single `"system_audio"` speaker label into
//! `"system_audio:speaker_N"` per distinct remote voice.
//!
//! Gated entirely behind the optional, off-by-default `diarize` Cargo
//! feature (see `Cargo.toml`'s `[features]` section). When the feature is
//! off, this module (and everything it declares) is not compiled at all —
//! `meetings::mod` only declares `mod diarize;` under
//! `#[cfg(feature = "diarize")]`.
//!
//! ## Honesty about scope (read before assuming this is Minutes-equivalent)
//!
//! This is a deliberately scoped MVP port of the ONNX inference core from
//! `pyannote-rs`-based diarization — modeled closely on
//! `diarize_with_pyannote_rs` in the reference Minutes codebase
//! (`crates/core/src/diarize.rs`), adapted to this repo's simpler
//! single-file-per-meeting capture model. What it does NOT have, unlike
//! that reference implementation:
//!
//! - **No stem/source separation.** Minutes discovers separate per-source
//!   audio *files* (mic stem, system stem, per-participant stems on some
//!   platforms) and diarizes each independently. This repo has exactly one
//!   `system_audio` waveform per meeting (see
//!   [`super::capture::raw_audio_sink`]) — there is nothing to separate.
//! - **No degraded-capture / ML-bleed detection.** Minutes has heuristics
//!   for detecting when one audio stem has "bled" into another (e.g. a
//!   quiet mic picking up system audio) and adjusts accordingly. Not
//!   ported.
//! - **No cross-meeting speaker enrollment or naming.** Every meeting is
//!   diarized independently; `speaker_1` in one meeting has no relationship
//!   to `speaker_1` in another. Minutes retains confirmed speaker embeddings
//!   across meetings to recognize the same person by name; this MVP does
//!   not.
//! - **No Python-subprocess fallback engine, no alternate/community model
//!   set, no capability-bound audio tokens, no process-global worker
//!   lease with panic/timeout handling.** [`engine`] uses a single
//!   Instant-based deadline guard and this crate's own `ACTIVE_CAPTURES`-style
//!   single-caller idiom instead (see `ops::meetings_diarize`).
//!
//! What it DOES do, faithfully: real ONNX speech segmentation
//! (pyannote `segmentation-3.0`), real speaker-embedding extraction
//! (wespeaker CAM++), and the same running-template cosine-similarity
//! clustering + merge pass + temporal label-fill that Minutes uses to turn
//! "N speech segments" into "K speaker identities" — see [`engine`].
//!
//! ## Pipeline
//!
//! 1. [`super::capture::pipeline`] streams the meeting's raw `system_audio`
//!    waveform to `{workspace_dir}/meetings/audio/{meeting_id}.system.wav`
//!    (see [`crate::aura::meetings::ops::system_audio_wav_path`]) whenever
//!    this feature is compiled in.
//! 2. [`diarize_meeting_system_audio`] (called from `ops::meetings_diarize`,
//!    reachable only via the explicit `aura.meetings_diarize` RPC — never
//!    auto-triggered from capture or `meetings_finish`) ensures the ONNX
//!    models are present ([`models::ensure_models_downloaded`]), then runs
//!    [`engine::diarize_wav`] off the async runtime thread.
//! 3. [`mapping::map_segments_to_chunks`] — pure, model-free, fully unit
//!    tested — maps the resulting speaker segments back onto this meeting's
//!    persisted `TranscriptChunk` rows by time overlap, producing the
//!    `(chunk_id, "system_audio:speaker_N")` updates that
//!    `store::update_transcript_chunk_speakers` persists.

pub mod engine;
pub mod mapping;
pub mod models;

use std::path::Path;
use std::time::Duration;

use log::debug;

use crate::aura::config::Config;

pub(crate) const LOG_PREFIX: &str = "[meetings-diarization]";

/// How long a single `meetings_diarize` call is allowed to run ONNX
/// inference before [`engine::diarize_wav`] gives up and returns an error.
/// Generous — this is an explicit, user-triggered offline batch pass over a
/// finished recording, not something on a request's critical path.
const DIARIZATION_DEADLINE: Duration = Duration::from_secs(15 * 60);

/// Ensures the diarization ONNX models are present, then runs the full
/// segmentation + embedding + clustering pipeline over `wav_path` off the
/// async runtime thread (mirrors [`crate::aura::meetings::ocr`]'s
/// `spawn_blocking` use for its own CPU-bound ONNX/tensor inference).
pub async fn diarize_meeting_system_audio(
    config: &Config,
    meeting_id: &str,
    wav_path: &Path,
) -> Result<Vec<engine::SpeakerSegment>, String> {
    let models_dir = config.workspace_dir.join("meetings").join("models");
    debug!(
        "{LOG_PREFIX} diarize_meeting_system_audio: start meeting_id={meeting_id} \
         wav_path={} models_dir={}",
        wav_path.display(),
        models_dir.display()
    );

    models::ensure_models_downloaded(&models_dir)
        .await
        .map_err(|e| format!("failed to prepare diarization models: {e}"))?;

    let wav_path = wav_path.to_path_buf();
    let deadline = DIARIZATION_DEADLINE;
    let segments =
        tokio::task::spawn_blocking(move || engine::diarize_wav(&wav_path, &models_dir, deadline))
            .await
            .map_err(|e| format!("diarization task panicked: {e}"))??;

    debug!(
        "{LOG_PREFIX} diarize_meeting_system_audio: done meeting_id={meeting_id} \
         segments={}",
        segments.len()
    );

    Ok(segments)
}
