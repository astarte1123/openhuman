//! Meeting recorder orchestration: cpal capture -> mixer -> resample ->
//! VAD -> whisper -> core RPC push, wired as Tauri commands.
//!
//! ## Shell -> core RPC (STEP 0 finding)
//!
//! The existing `meet_audio` module (live meet-agent audio loop) talks to
//! the in-process core over plain HTTP JSON-RPC-over-loopback:
//! `crate::core_rpc::core_rpc_url_value()` (`http://127.0.0.1:<port>/rpc`)
//! POSTed with a bearer token from `crate::core_rpc::apply_auth`
//! (`AURA_CORE_TOKEN`). There is no native-event-bus path reachable
//! from the Tauri shell process — `request_native_global` /
//! `register_native_global` (`src/core/event_bus/native_request.rs`) are
//! in-process-only inside the Rust core and are never exposed across the
//! shell/core boundary. So this module mirrors `meet_audio::rpc_call`'s
//! HTTP JSON-RPC helper exactly (see `rpc_call` below) rather than
//! inventing a new transport.
//!
//! ## Platform gating
//!
//! This file intentionally has almost no `#[cfg(target_os = "windows")]`
//! blocks. Both `devices::configure_platform_audio` /
//! `devices::get_device` and `whisper_engine::WhisperEngine` already
//! return an explicit "not yet implemented for this platform" error on
//! non-Windows internally — [`meeting_recorder_start`] calls
//! `configure_platform_audio` as its very first step, so a non-Windows
//! build fails fast there with a clear message, without ever touching
//! cpal capture, the model downloader, or core RPC. This keeps the
//! platform stub in exactly one place (`devices.rs`) instead of scattered
//! across the orchestration code.
//!
//! ## cpal lifecycle
//!
//! Mirrors `aura::voice::audio_capture::record_on_thread` (root
//! crate): each device's cpal `Device`/`Stream` is created, played, and
//! dropped on its own dedicated OS thread (WASAPI backends are not
//! `Send`-safe), signaled to stop via a shared `Arc<AtomicBool>`. Captured
//! samples cross into the async pipeline over a `tokio::sync::mpsc`
//! channel fed by `stream::run_capture_thread`'s ring-buffer drain.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::mpsc::UnboundedReceiver;

use super::devices::{self, AudioDevice, DeviceType};
use super::mixer::AudioMixerRingBuffer;
use super::resample::Resampler16k;
use super::screenshot;
use super::stream::{self, CapturedChunk};
use super::vad::{SileroVad, VAD_SAMPLE_RATE};
use super::whisper_engine::{self, WhisperEngine};

const LOG_PREFIX: &str = "[meeting-recorder]";

/// Wire-friendly device listing for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub device_type: String,
}

impl From<AudioDevice> for AudioDeviceInfo {
    fn from(d: AudioDevice) -> Self {
        let device_type = match d.device_type {
            DeviceType::Input => "input",
            DeviceType::Output => "output",
        }
        .to_string();
        Self {
            name: d.name,
            device_type,
        }
    }
}

struct ActiveSession {
    meeting_id: String,
    started_at: Instant,
    stop_flag: Arc<AtomicBool>,
    pipeline_task: tauri::async_runtime::JoinHandle<()>,
    /// Periodic primary-monitor screenshot capture for slide OCR (see
    /// `screenshot` module doc). Same start/stop lifecycle as
    /// `pipeline_task`: spawned in `meeting_recorder_start`, `.abort()`-ed
    /// in `meeting_recorder_stop`.
    screenshot_task: tauri::async_runtime::JoinHandle<()>,
}

/// Tauri-managed state (`app.manage(...)`), following the same pattern as
/// `meet_audio::MeetAudioState` / `meet_call::MeetCallState` — not a
/// standalone global `static Mutex` (unlike upstream Meetily).
#[derive(Default)]
pub struct MeetingRecorderState {
    inner: Mutex<Option<ActiveSession>>,
}

impl MeetingRecorderState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[tauri::command]
pub fn meeting_recorder_list_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let host = cpal::default_host();
    devices::configure_platform_audio(&host)
        .map(|list| list.into_iter().map(AudioDeviceInfo::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn meeting_recorder_start<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, MeetingRecorderState>,
    mic_device_name: Option<String>,
    system_device_name: Option<String>,
    title: Option<String>,
) -> Result<String, String> {
    {
        let guard = state.inner.lock().unwrap();
        if guard.is_some() {
            return Err("a meeting recording is already in progress".to_string());
        }
    }

    log::info!(
        "{LOG_PREFIX} start requested mic={mic_device_name:?} system={system_device_name:?}"
    );

    // 1) Resolve devices. On unsupported platforms this fails immediately
    // with a clear "not yet implemented" error — nothing below runs.
    let host = cpal::default_host();
    let available = devices::configure_platform_audio(&host).map_err(|e| e.to_string())?;

    let mic_device = pick_device(&available, DeviceType::Input, mic_device_name.as_deref())
        .ok_or_else(|| "no microphone device available".to_string())?;
    let system_device = pick_device(
        &available,
        DeviceType::Output,
        system_device_name.as_deref(),
    )
    .ok_or_else(|| "no system-audio (loopback) device available".to_string())?;

    let (mic_cpal_device, mic_config) =
        devices::get_device(&mic_device).map_err(|e| e.to_string())?;
    let (system_cpal_device, system_config) =
        devices::get_device(&system_device).map_err(|e| e.to_string())?;

    // 2) Create the meeting via core RPC first, so the very first
    // transcript-chunk push can't race an unfinished create.
    let create_result = rpc_call(
        "aura.meetings_create",
        serde_json::json!({ "title": title, "source": "meeting_recorder" }),
    )
    .await
    .map_err(|e| format!("meetings_create failed: {e}"))?;
    // `RpcOutcome<Meeting>::into_cli_compatible_json` wraps as
    // `{"result": <Meeting>, "logs": [...]}` whenever logs are non-empty —
    // `meetings_create` always attaches one ("meeting created: <id>"), so
    // the id lives at `result.id`, not `result.meeting.id`.
    let meeting_id = create_result
        .get("result")
        .and_then(|m| m.get("id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| "meetings_create response missing result.id".to_string())?
        .to_string();
    log::info!("{LOG_PREFIX} created meeting_id={meeting_id}");

    // 3) Resolve (downloading if needed) the whisper model before
    // starting capture, so the first speech segment doesn't stall behind
    // a multi-hundred-MB download. On non-Windows this is harmless to run
    // (just stages a file), even though `WhisperEngine::load` afterwards
    // will refuse to actually use it.
    let workspace_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("app_local_data_dir: {e}"))?;
    let model_path = whisper_engine::ensure_model_downloaded(&workspace_dir)
        .await
        .map_err(|e| format!("whisper model setup failed: {e}"))?;
    let engine = Arc::new(
        WhisperEngine::load(&model_path).map_err(|e| format!("whisper load failed: {e}"))?,
    );

    // 4) Spawn capture threads (mic + system loopback).
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<CapturedChunk>();

    spawn_capture_thread(
        mic_cpal_device,
        mic_config,
        DeviceType::Input,
        tx.clone(),
        stop_flag.clone(),
    );
    spawn_capture_thread(
        system_cpal_device,
        system_config,
        DeviceType::Output,
        tx,
        stop_flag.clone(),
    );

    // 5) Spawn the async pipeline: mixer -> resample -> VAD -> whisper -> RPC push.
    let pipeline_meeting_id = meeting_id.clone();
    let pipeline_handle =
        tauri::async_runtime::spawn(run_pipeline(rx, engine, pipeline_meeting_id));

    // 6) Spawn the sibling periodic screenshot-capture task (slide OCR).
    let screenshot_handle = screenshot::spawn_capture_task(meeting_id.clone());

    *state.inner.lock().unwrap() = Some(ActiveSession {
        meeting_id: meeting_id.clone(),
        started_at: Instant::now(),
        stop_flag,
        pipeline_task: pipeline_handle,
        screenshot_task: screenshot_handle,
    });

    Ok(meeting_id)
}

#[tauri::command]
pub async fn meeting_recorder_stop(
    state: tauri::State<'_, MeetingRecorderState>,
) -> Result<(), String> {
    let session = state.inner.lock().unwrap().take();
    let Some(session) = session else {
        return Err("no meeting recording in progress".to_string());
    };

    session.stop_flag.store(true, Ordering::SeqCst);
    // Give the capture threads a moment to unwind (each polls its stop
    // flag every 20ms — see `stream::run_capture_thread`) so any audio
    // already in flight gets one last chance to reach the mixer/VAD
    // before we abort the pipeline task.
    tokio::time::sleep(Duration::from_millis(200)).await;
    session.pipeline_task.abort();
    session.screenshot_task.abort();

    let duration_ms = session.started_at.elapsed().as_millis() as i64;
    rpc_call(
        "aura.meetings_finish",
        serde_json::json!({
            "meeting_id": session.meeting_id,
            "duration_ms": duration_ms,
        }),
    )
    .await
    .map_err(|e| format!("meetings_finish failed: {e}"))?;

    log::info!(
        "{LOG_PREFIX} stopped meeting_id={} duration_ms={duration_ms}",
        session.meeting_id
    );
    Ok(())
}

fn pick_device(
    available: &[AudioDevice],
    device_type: DeviceType,
    requested_name: Option<&str>,
) -> Option<AudioDevice> {
    if let Some(name) = requested_name {
        if let Some(found) = available
            .iter()
            .find(|d| d.device_type == device_type && d.name == name)
        {
            return Some(found.clone());
        }
        log::warn!(
            "{LOG_PREFIX} requested {device_type:?} device '{name}' not found, falling back to first available"
        );
    }
    available
        .iter()
        .find(|d| d.device_type == device_type)
        .cloned()
}

fn spawn_capture_thread(
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    device_type: DeviceType,
    tx: tokio::sync::mpsc::UnboundedSender<CapturedChunk>,
    stop_flag: Arc<AtomicBool>,
) {
    let thread_name = format!("meeting-recorder-{device_type:?}");
    if let Err(err) = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            stream::run_capture_thread(device, config, device_type, tx, stop_flag);
        })
    {
        log::error!("{LOG_PREFIX} failed to spawn {device_type:?} capture thread: {err}");
    }
}

/// Speaker label attached to transcript chunks captured from the
/// microphone device (i.e. this machine's user).
const SPEAKER_MIC: &str = "You";
/// Speaker label attached to transcript chunks captured from the
/// system-loopback device (i.e. everyone else on the call).
const SPEAKER_SYSTEM: &str = "Others";

/// Drives one recording session end-to-end: drains captured chunks,
/// resamples each device stream to 16kHz independently (mic and
/// system-loopback devices aren't guaranteed to share a sample rate), then
/// runs VAD and transcription **per source** rather than summing mic +
/// system-loopback into one mono stream first.
///
/// Two independent `SileroVad` instances are used (VAD carries per-stream
/// state) so each source's speech segments are detected and transcribed on
/// their own — this both avoids the audio-quality loss from summing two
/// simultaneous speakers into one waveform (whisper does markedly worse on
/// overlapping-speech mixes) and gives real, non-heuristic 2-way speaker
/// attribution for free: mic segments are tagged [`SPEAKER_MIC`] ("You"),
/// system-loopback segments [`SPEAKER_SYSTEM`] ("Others"). This does not
/// distinguish between multiple *other* participants on the same call —
/// that needs true multi-speaker diarization (see
/// `src/aura/meetings/diarize`, not yet wired into this pipeline).
async fn run_pipeline(
    mut rx: UnboundedReceiver<CapturedChunk>,
    engine: Arc<WhisperEngine>,
    meeting_id: String,
) {
    let mut mixer = AudioMixerRingBuffer::new(VAD_SAMPLE_RATE);
    let mut vad_mic = match SileroVad::new() {
        Ok(v) => v,
        Err(err) => {
            log::error!("{LOG_PREFIX} failed to initialize mic VAD, pipeline stopping: {err}");
            return;
        }
    };
    let mut vad_system = match SileroVad::new() {
        Ok(v) => v,
        Err(err) => {
            log::error!("{LOG_PREFIX} failed to initialize system VAD, pipeline stopping: {err}");
            return;
        }
    };
    let mut mic_resampler: Option<(u32, Resampler16k)> = None;
    let mut system_resampler: Option<(u32, Resampler16k)> = None;

    while let Some(chunk) = rx.recv().await {
        let resampler_slot = match chunk.device_type {
            DeviceType::Input => &mut mic_resampler,
            DeviceType::Output => &mut system_resampler,
        };
        let resampled = match resample_chunk(resampler_slot, chunk.sample_rate, &chunk.samples) {
            Ok(s) => s,
            Err(err) => {
                log::warn!(
                    "{LOG_PREFIX} resample failed for {:?}: {err}",
                    chunk.device_type
                );
                continue;
            }
        };
        if resampled.is_empty() {
            continue;
        }
        mixer.add_samples(chunk.device_type, &resampled);

        while mixer.can_mix() {
            let (mic_window, system_window) = mixer.extract_window();

            match vad_mic.process(&mic_window) {
                Ok(segments) => {
                    for segment in segments {
                        transcribe_and_push(&engine, &meeting_id, segment, SPEAKER_MIC).await;
                    }
                }
                Err(err) => log::warn!("{LOG_PREFIX} mic vad process failed: {err}"),
            }

            match vad_system.process(&system_window) {
                Ok(segments) => {
                    for segment in segments {
                        transcribe_and_push(&engine, &meeting_id, segment, SPEAKER_SYSTEM).await;
                    }
                }
                Err(err) => log::warn!("{LOG_PREFIX} system vad process failed: {err}"),
            }
        }
    }
    log::debug!("{LOG_PREFIX} pipeline task exiting (capture channel closed)");
}

async fn transcribe_and_push(
    engine: &Arc<WhisperEngine>,
    meeting_id: &str,
    segment: super::vad::SpeechSegment,
    speaker: &str,
) {
    let engine = engine.clone();
    // Whisper inference is CPU-bound and synchronous — run it off the
    // async runtime thread so the pipeline keeps draining the capture
    // channel while transcription is in flight.
    let text = match tokio::task::spawn_blocking(move || engine.transcribe(&segment.samples)).await
    {
        Ok(Ok(text)) => text,
        Ok(Err(err)) => {
            log::warn!("{LOG_PREFIX} whisper transcription failed: {err}");
            return;
        }
        Err(join_err) => {
            log::warn!("{LOG_PREFIX} whisper task panicked: {join_err}");
            return;
        }
    };
    if text.trim().is_empty() {
        return;
    }
    if let Err(err) = rpc_call(
        "aura.meetings_append_transcript_chunk",
        serde_json::json!({
            "meeting_id": meeting_id,
            "start_ms": segment.start_timestamp_ms as i64,
            "end_ms": segment.end_timestamp_ms as i64,
            "text": text,
            "speaker": speaker,
        }),
    )
    .await
    {
        log::warn!("{LOG_PREFIX} append_transcript_chunk failed: {err}");
    }
}

fn resample_chunk(
    slot: &mut Option<(u32, Resampler16k)>,
    source_rate: u32,
    samples: &[f32],
) -> anyhow::Result<Vec<f32>> {
    if slot.as_ref().map(|(rate, _)| *rate) != Some(source_rate) {
        *slot = Some((source_rate, Resampler16k::new(source_rate)?));
    }
    let (_, resampler) = slot.as_mut().expect("slot just populated above");
    resampler.push(samples)
}

/// Minimal JSON-RPC helper — mirrors `meet_audio::rpc_call` exactly (see
/// module doc's STEP 0 finding). Kept as a private, module-local copy
/// rather than importing `meet_audio`'s, matching the existing project
/// convention of each shell feature module owning its own small RPC
/// helper (see also `telegram_scanner::mod.rs`).
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

    fn device(name: &str, device_type: DeviceType) -> AudioDevice {
        AudioDevice {
            name: name.to_string(),
            device_type,
        }
    }

    #[test]
    fn pick_device_prefers_requested_name_when_present() {
        let available = vec![
            device("Mic A", DeviceType::Input),
            device("Mic B", DeviceType::Input),
        ];
        let picked = pick_device(&available, DeviceType::Input, Some("Mic B")).unwrap();
        assert_eq!(picked.name, "Mic B");
    }

    #[test]
    fn pick_device_falls_back_to_first_available_when_name_missing() {
        let available = vec![device("Speakers", DeviceType::Output)];
        let picked = pick_device(&available, DeviceType::Output, Some("nonexistent")).unwrap();
        assert_eq!(picked.name, "Speakers");
    }

    #[test]
    fn pick_device_returns_none_when_type_absent() {
        let available = vec![device("Mic A", DeviceType::Input)];
        assert!(pick_device(&available, DeviceType::Output, None).is_none());
    }

    #[test]
    fn audio_device_info_maps_device_type_to_lowercase_strings() {
        let input_info: AudioDeviceInfo = device("Mic", DeviceType::Input).into();
        let output_info: AudioDeviceInfo = device("Speakers", DeviceType::Output).into();
        assert_eq!(input_info.device_type, "input");
        assert_eq!(output_info.device_type, "output");
    }

    #[test]
    fn meeting_recorder_state_starts_empty() {
        let state = MeetingRecorderState::new();
        assert!(state.inner.lock().unwrap().is_none());
    }
}
