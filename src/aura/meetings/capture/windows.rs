//! Windows local meeting-audio capture.
//!
//! Two independent tracks, each on its own dedicated OS thread (audio APIs
//! aren't `Send`-friendly across an async runtime — `cpal::Stream` and
//! WASAPI COM interfaces must be created, used, and dropped on the thread
//! that created them):
//!
//! - **Microphone** — via `cpal` (WASAPI under the hood), reusing the same
//!   device-selection and channel-mixdown idioms as
//!   [`crate::aura::voice::audio_capture`].
//! - **System audio (loopback)** — via the `wasapi` crate directly. `cpal`
//!   0.15 has no first-class WASAPI loopback capture mode; real loopback
//!   needs `IAudioClient::Initialize` with `AUDCLNT_STREAMFLAGS_LOOPBACK`,
//!   which the `wasapi` crate sets automatically when you open the default
//!   *Render* (output/speaker) device's `IAudioClient` but initialize it for
//!   `Direction::Capture` (confirmed against the crate's v0.24.0 source —
//!   see the `wasapi` dependency comment in the root `Cargo.toml`, and
//!   `examples/record.rs` in that crate's repo, which documents the same
//!   Render-device/Capture-direction combination in a comment). This is the
//!   single biggest unknown flagged in the meeting-capture design doc; the
//!   API usage here is verified against the crate's actual source, but the
//!   *behavior* (does it reliably deliver system audio, at what latency, does
//!   it survive a default-output-device change mid-meeting) is not — see the
//!   "Needs hardware verification" note below.
//!
//! **Needs hardware verification.** Neither track can be meaningfully unit
//! tested here — both talk to real OS audio devices. `cargo check` proves
//! this module compiles; someone must actually run a meeting capture with a
//! real microphone (speaking near it) and, separately, with real system
//! audio playing (e.g. a YouTube video) to confirm both tracks produce
//! sane, correctly-leveled, in-order samples. Known untested edge cases:
//! default-device changes mid-capture (e.g. Bluetooth headset disconnect),
//! machines with no output device at all, and exclusive-mode conflicts with
//! another application holding the loopback source device.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use log::{debug, error, warn};
use tokio::sync::mpsc::UnboundedSender;

use crate::aura::voice::audio_capture::to_mono;

use super::{AudioCaptureBackend, AudioChunk, AudioTrack, CaptureHandle, LOG_PREFIX};

/// Poll interval for both capture threads' stop-flag checks, and the WASAPI
/// event-wait timeout on the loopback thread.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Loopback capture format. 48kHz/stereo/f32 is the format almost every
/// Windows audio engine mixes at internally, so this avoids the audio
/// engine having to resample on our behalf; [`super::pipeline`] resamples
/// down to whisper's 16kHz mono afterwards regardless.
const LOOPBACK_SAMPLE_RATE: u32 = 48_000;
const LOOPBACK_CHANNELS: usize = 2;
const LOOPBACK_STOREBITS: usize = 32;
const LOOPBACK_VALIDBITS: usize = 32;

pub struct WindowsAudioCapture;

impl AudioCaptureBackend for WindowsAudioCapture {
    fn start(&self, sink: UnboundedSender<AudioChunk>) -> Result<CaptureHandle, String> {
        let mut handle = CaptureHandle::new();
        let session_start = Instant::now();

        debug!("{LOG_PREFIX} starting Windows capture backend");

        // Microphone is required — a meeting capture with no mic track is
        // pointless, so its setup failure fails the whole start() call.
        let mic_stop = spawn_mic_stream(sink.clone(), session_start).map_err(|e| {
            error!("{LOG_PREFIX}[mic] setup failed: {e}");
            format!("microphone capture failed to start: {e}")
        })?;
        handle.add_track(mic_stop, "microphone");
        debug!("{LOG_PREFIX}[mic] capture thread started");

        // System-audio loopback is best-effort: some dev machines (and some
        // CI/VM images) have no default output device, or COM activation
        // fails for reasons unrelated to the mic path. A meeting with
        // mic-only capture is still useful, so a loopback failure is logged
        // and swallowed rather than failing start().
        match spawn_loopback_stream(sink, session_start) {
            Ok(loopback_stop) => {
                handle.add_track(loopback_stop, "system_audio");
                debug!("{LOG_PREFIX}[loopback] capture thread started");
            }
            Err(e) => {
                warn!(
                    "{LOG_PREFIX}[loopback] system-audio capture unavailable, \
                     continuing mic-only: {e}"
                );
            }
        }

        Ok(handle)
    }
}

/// Pushes one chunk onto `sink`, stamping it with elapsed time since
/// `session_start`. `send` on an unbounded channel never blocks, so this is
/// safe to call from a realtime-ish audio callback/poll thread. Empty
/// chunks are dropped without sending (nothing useful to transcribe).
fn push_chunk(
    sink: &UnboundedSender<AudioChunk>,
    pcm_f32: Vec<f32>,
    sample_rate: u32,
    track: AudioTrack,
    session_start: Instant,
) {
    if pcm_f32.is_empty() {
        return;
    }
    let captured_at_ms = session_start.elapsed().as_millis() as i64;
    // A send error means the pipeline's receiver was dropped (session
    // already tearing down) — drop the chunk silently rather than logging
    // at audio-callback rate.
    let _ = sink.send(AudioChunk {
        pcm_f32,
        sample_rate,
        track,
        captured_at_ms,
    });
}

// ── Microphone (cpal) ───────────────────────────────────────────────────

fn spawn_mic_stream(
    sink: UnboundedSender<AudioChunk>,
    session_start: Instant,
) -> Result<Arc<AtomicBool>, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();
    let (setup_tx, setup_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    std::thread::Builder::new()
        .name("meeting-capture-mic".into())
        .spawn(move || run_mic_capture_thread(sink, stop_flag_thread, setup_tx, session_start))
        .map_err(|e| format!("failed to spawn microphone capture thread: {e}"))?;

    match setup_rx.recv() {
        Ok(Ok(())) => Ok(stop_flag),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("microphone capture thread exited before signalling readiness".to_string()),
    }
}

fn run_mic_capture_thread(
    sink: UnboundedSender<AudioChunk>,
    stop_flag: Arc<AtomicBool>,
    setup_tx: std::sync::mpsc::SyncSender<Result<(), String>>,
    session_start: Instant,
) {
    // Cross-platform microphone permission pre-check — same gate
    // `voice::audio_capture::record_on_thread` uses, so meeting capture
    // respects the same OS-level permission state instead of failing with
    // an opaque cpal error.
    use crate::aura::accessibility::{
        detect_microphone_permission, microphone_denied_message, request_microphone_access,
        PermissionState,
    };

    let mic_perm = detect_microphone_permission();
    debug!("{LOG_PREFIX}[mic] permission state: {mic_perm:?}");
    match mic_perm {
        PermissionState::Unknown => {
            request_microphone_access();
            let updated = detect_microphone_permission();
            debug!("{LOG_PREFIX}[mic] permission after request: {updated:?}");
            if matches!(updated, PermissionState::Denied | PermissionState::Unknown) {
                let msg = microphone_denied_message();
                error!("{LOG_PREFIX}[mic] {msg}");
                let _ = setup_tx.send(Err(msg));
                return;
            }
        }
        PermissionState::Denied => {
            let msg = microphone_denied_message();
            error!("{LOG_PREFIX}[mic] {msg}");
            let _ = setup_tx.send(Err(msg));
            return;
        }
        _ => {} // Granted or Unsupported — proceed normally.
    }

    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            let msg = "no default audio input device found".to_string();
            error!("{LOG_PREFIX}[mic] {msg}");
            let _ = setup_tx.send(Err(msg));
            return;
        }
    };
    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
    debug!("{LOG_PREFIX}[mic] using input device: {device_name}");

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("no default input config available: {e}");
            error!("{LOG_PREFIX}[mic] {msg}");
            let _ = setup_tx.send(Err(msg));
            return;
        }
    };

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    debug!(
        "{LOG_PREFIX}[mic] config: rate={sample_rate} channels={channels} format={sample_format:?}"
    );

    let stream: Result<cpal::Stream, String> = match sample_format {
        SampleFormat::F32 => {
            let tx = sink.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono = to_mono(data, channels);
                        push_chunk(
                            &tx,
                            mono,
                            sample_rate,
                            AudioTrack::Microphone,
                            session_start,
                        );
                    },
                    |err| error!("{LOG_PREFIX}[mic] stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("failed to build f32 mic stream: {e}"))
        }
        SampleFormat::I16 => {
            let tx = sink.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let floats: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let mono = to_mono(&floats, channels);
                        push_chunk(
                            &tx,
                            mono,
                            sample_rate,
                            AudioTrack::Microphone,
                            session_start,
                        );
                    },
                    |err| error!("{LOG_PREFIX}[mic] stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("failed to build i16 mic stream: {e}"))
        }
        SampleFormat::U16 => {
            let tx = sink.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let floats: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();
                        let mono = to_mono(&floats, channels);
                        push_chunk(
                            &tx,
                            mono,
                            sample_rate,
                            AudioTrack::Microphone,
                            session_start,
                        );
                    },
                    |err| error!("{LOG_PREFIX}[mic] stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("failed to build u16 mic stream: {e}"))
        }
        other => Err(format!("unsupported mic sample format: {other:?}")),
    };

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            error!("{LOG_PREFIX}[mic] {e}");
            let _ = setup_tx.send(Err(e));
            return;
        }
    };

    if let Err(e) = stream.play() {
        let msg = format!("failed to start mic stream: {e}");
        error!("{LOG_PREFIX}[mic] {msg}");
        let _ = setup_tx.send(Err(msg));
        return;
    }

    let _ = setup_tx.send(Ok(()));

    while !stop_flag.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);
    }
    debug!("{LOG_PREFIX}[mic] stop flag detected, tearing down stream");
    drop(stream);
}

// ── System audio (WASAPI loopback via the `wasapi` crate) ──────────────

fn spawn_loopback_stream(
    sink: UnboundedSender<AudioChunk>,
    session_start: Instant,
) -> Result<Arc<AtomicBool>, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();
    let (setup_tx, setup_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    std::thread::Builder::new()
        .name("meeting-capture-loopback".into())
        .spawn(move || run_loopback_capture_thread(sink, stop_flag_thread, setup_tx, session_start))
        .map_err(|e| format!("failed to spawn loopback capture thread: {e}"))?;

    match setup_rx.recv() {
        Ok(Ok(())) => Ok(stop_flag),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("loopback capture thread exited before signalling readiness".to_string()),
    }
}

fn run_loopback_capture_thread(
    sink: UnboundedSender<AudioChunk>,
    stop_flag: Arc<AtomicBool>,
    setup_tx: std::sync::mpsc::SyncSender<Result<(), String>>,
    session_start: Instant,
) {
    // WASAPI/COM must be initialized on the thread that uses it. This
    // thread owns every wasapi call below (setup + the whole polling loop)
    // and calls `wasapi::deinitialize()` on every exit path, so a
    // per-thread MTA init/deinit pair is correct here — mirrors the
    // `wasapi` crate's own examples (`initialize_mta().ok()?` in `main()`).
    if let Err(e) = wasapi::initialize_mta().ok() {
        let msg = format!("COM MTA initialization failed: {e}");
        error!("{LOG_PREFIX}[loopback] {msg}");
        let _ = setup_tx.send(Err(msg));
        return;
    }

    let setup = (|| -> Result<(wasapi::AudioClient, wasapi::AudioCaptureClient, wasapi::Handle), String> {
        let enumerator = wasapi::DeviceEnumerator::new()
            .map_err(|e| format!("failed to create device enumerator: {e}"))?;
        // Loopback capture opens the *Render* (output/speaker) endpoint's
        // audio client, then initializes it for `Direction::Capture` — the
        // `wasapi` crate sets `AUDCLNT_STREAMFLAGS_LOOPBACK` automatically
        // for exactly this Render-device/Capture-direction/Shared-mode
        // combination (see this module's doc comment for the source trace).
        let device = enumerator
            .get_default_device(&wasapi::Direction::Render)
            .map_err(|e| format!("no default output device for loopback capture: {e}"))?;
        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|e| format!("failed to activate loopback audio client: {e}"))?;

        let desired_format = wasapi::WaveFormat::new(
            LOOPBACK_STOREBITS,
            LOOPBACK_VALIDBITS,
            &wasapi::SampleType::Float,
            LOOPBACK_SAMPLE_RATE as usize,
            LOOPBACK_CHANNELS,
            None,
        );

        let (_default_period, min_period) = audio_client
            .get_device_period()
            .map_err(|e| format!("failed to query loopback device period: {e}"))?;

        let stream_mode = wasapi::StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: min_period,
        };

        audio_client
            .initialize_client(&desired_format, &wasapi::Direction::Capture, &stream_mode)
            .map_err(|e| format!("failed to initialize loopback client: {e}"))?;

        let event_handle = audio_client
            .set_get_eventhandle()
            .map_err(|e| format!("failed to get loopback event handle: {e}"))?;

        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|e| format!("failed to get loopback capture client: {e}"))?;

        audio_client
            .start_stream()
            .map_err(|e| format!("failed to start loopback stream: {e}"))?;

        Ok((audio_client, capture_client, event_handle))
    })();

    let (audio_client, capture_client, event_handle) = match setup {
        Ok(v) => v,
        Err(e) => {
            error!("{LOG_PREFIX}[loopback] setup failed: {e}");
            let _ = setup_tx.send(Err(e));
            wasapi::deinitialize();
            return;
        }
    };

    debug!(
        "{LOG_PREFIX}[loopback] stream started: rate={LOOPBACK_SAMPLE_RATE} \
         channels={LOOPBACK_CHANNELS}"
    );
    let _ = setup_tx.send(Ok(()));

    let bytes_per_frame = LOOPBACK_CHANNELS * (LOOPBACK_STOREBITS / 8);
    let mut byte_buffer: VecDeque<u8> = VecDeque::new();

    while !stop_flag.load(Ordering::SeqCst) {
        // Timeout is expected while no system audio is playing (WASAPI
        // still delivers periodic silent buffers in most cases, but a slow
        // device period can legitimately time out this wait) — just
        // re-check the stop flag and keep polling rather than treating a
        // timeout as fatal.
        if event_handle
            .wait_for_event(POLL_INTERVAL.as_millis() as u32)
            .is_err()
        {
            continue;
        }

        if let Err(e) = capture_client.read_from_device_to_deque(&mut byte_buffer) {
            warn!("{LOG_PREFIX}[loopback] read error, stopping capture: {e}");
            break;
        }

        let usable = byte_buffer.len() - (byte_buffer.len() % bytes_per_frame);
        if usable == 0 {
            continue;
        }
        let frame_bytes: Vec<u8> = byte_buffer.drain(..usable).collect();
        let samples_f32: Vec<f32> = frame_bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let mono = to_mono(&samples_f32, LOOPBACK_CHANNELS);
        push_chunk(
            &sink,
            mono,
            LOOPBACK_SAMPLE_RATE,
            AudioTrack::SystemAudio,
            session_start,
        );
    }

    if let Err(e) = audio_client.stop_stream() {
        warn!("{LOG_PREFIX}[loopback] failed to stop stream cleanly: {e}");
    }
    debug!("{LOG_PREFIX}[loopback] capture thread exiting");
    wasapi::deinitialize();
}
