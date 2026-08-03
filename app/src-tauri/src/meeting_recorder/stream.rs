//! cpal capture — mic and system-loopback share one code path.
//!
//! ## Why a dedicated OS thread per device
//!
//! cpal's WASAPI backend is not `Send`-safe: the `Device`/`Stream` objects
//! must be created, used, and dropped on the same OS thread. This mirrors
//! the existing pattern in `aura::voice::audio_capture::record_on_thread`
//! (root crate) — a dedicated `std::thread` builds the stream, plays it,
//! then blocks in a poll loop until a shared stop flag fires, at which
//! point the stream is dropped on that same thread.
//!
//! ## Why `ringbuf` instead of sending straight from the audio callback
//!
//! The cpal callback runs on the platform's real-time audio thread and
//! should never block or perform unbounded work. Rather than allocate and
//! push a `CapturedChunk` through an async channel on every single
//! callback, the callback only writes into a lock-free
//! [`ringbuf::HeapRb`] (`try`/`push_slice`, non-blocking, drops the
//! oldest-pending data only if the ring is completely full). The same
//! dedicated OS thread that owns the `Stream` drains the ring buffer
//! into `tokio::sync::mpsc` every ~20ms.
//!
//! ## System loopback = INPUT stream on an OUTPUT device
//!
//! System audio capture on Windows opens a cpal *input* stream on a
//! device that `devices.rs` enumerated as `DeviceType::Output`. This only
//! works because of the patched `cpal` fork pinned in
//! `Cargo.toml`'s `[patch.crates-io]` — stock cpal does not support
//! WASAPI loopback. Both device roles run through the exact same
//! `build_stream` below; the only per-role difference lives in
//! `devices.rs`'s config resolution.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use tokio::sync::mpsc::UnboundedSender;

use super::devices::DeviceType;

const LOG_PREFIX: &str = "[meeting-recorder:stream]";

/// One captured audio chunk pushed to the async mixing/VAD pipeline.
/// `samples` are normalized `f32` in `[-1.0, 1.0]`, mono is NOT assumed —
/// the mixer/resampler downstream handle channel count via the config
/// that produced this chunk.
#[derive(Debug, Clone)]
pub struct CapturedChunk {
    pub device_type: DeviceType,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Ring-buffer capacity: 10s of mono f32 at a generous 48kHz upper bound —
/// comfortably larger than what a 20ms drain interval ever needs, so the
/// real-time audio callback is never starved for vacant space.
const RING_BUFFER_CAPACITY_SAMPLES: usize = 48_000 * 10;
const DRAIN_INTERVAL: Duration = Duration::from_millis(20);

/// Build the cpal stream for `device`/`config`, start it, and block the
/// calling thread draining captured samples into `tx` until `stop_flag`
/// is set. Must be called on its own dedicated OS thread — see module doc.
pub fn run_capture_thread(
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    device_type: DeviceType,
    tx: UnboundedSender<CapturedChunk>,
    stop_flag: Arc<AtomicBool>,
) {
    let sample_rate = config.sample_rate().0;
    let rb = HeapRb::<f32>::new(RING_BUFFER_CAPACITY_SAMPLES);
    let (producer, mut consumer) = rb.split();

    let stream = match build_stream(&device, &config, device_type, producer) {
        Ok(s) => s,
        Err(err) => {
            log::error!("{LOG_PREFIX} failed to build {device_type:?} stream: {err}");
            return;
        }
    };

    if let Err(err) = stream.play() {
        log::error!("{LOG_PREFIX} failed to start {device_type:?} stream: {err}");
        return;
    }

    log::info!("{LOG_PREFIX} {device_type:?} capture started sample_rate={sample_rate}");

    let mut drain_buf = vec![0.0f32; RING_BUFFER_CAPACITY_SAMPLES];
    while !stop_flag.load(Ordering::SeqCst) {
        std::thread::sleep(DRAIN_INTERVAL);
        drain_into_channel(&mut consumer, &mut drain_buf, device_type, sample_rate, &tx);
    }
    // Final drain so the last ~20ms of audio before stop isn't lost.
    drain_into_channel(&mut consumer, &mut drain_buf, device_type, sample_rate, &tx);

    drop(stream);
    log::debug!("{LOG_PREFIX} {device_type:?} capture thread exiting");
}

fn drain_into_channel(
    consumer: &mut HeapCons<f32>,
    scratch: &mut [f32],
    device_type: DeviceType,
    sample_rate: u32,
    tx: &UnboundedSender<CapturedChunk>,
) {
    let available = consumer.occupied_len();
    if available == 0 {
        return;
    }
    let take = available.min(scratch.len());
    let popped = consumer.pop_slice(&mut scratch[..take]);
    if popped == 0 {
        return;
    }
    let samples = scratch[..popped].to_vec();
    if tx
        .send(CapturedChunk {
            device_type,
            samples,
            sample_rate,
        })
        .is_err()
    {
        log::debug!("{LOG_PREFIX} pipeline receiver dropped, {device_type:?} chunk discarded");
    }
}

/// Build (but do not start) the capture stream, wiring the cpal callback
/// to push normalized `f32` samples into `producer`. Handles the four
/// sample formats realistically seen on Windows WASAPI (F32/I16/I32/I8),
/// always normalizing to `[-1.0, 1.0]`.
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    device_type: DeviceType,
    mut producer: HeapProd<f32>,
) -> Result<cpal::Stream> {
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.clone().into();

    let err_fn = move |err| {
        log::error!("{LOG_PREFIX} stream error ({device_type:?}): {err}");
    };

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                push_samples(&mut producer, data, device_type);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let converted: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                push_samples(&mut producer, &converted, device_type);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            &stream_config,
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                let converted: Vec<f32> =
                    data.iter().map(|&s| s as f32 / i32::MAX as f32).collect();
                push_samples(&mut producer, &converted, device_type);
            },
            err_fn,
            None,
        ),
        SampleFormat::I8 => device.build_input_stream(
            &stream_config,
            move |data: &[i8], _: &cpal::InputCallbackInfo| {
                let converted: Vec<f32> = data.iter().map(|&s| s as f32 / 128.0).collect();
                push_samples(&mut producer, &converted, device_type);
            },
            err_fn,
            None,
        ),
        other => return Err(anyhow!("unsupported capture sample format: {other:?}")),
    }
    .map_err(|e| anyhow!("build_input_stream failed: {e}"))?;

    Ok(stream)
}

/// Push captured samples into the ring buffer. Never blocks: if the ring
/// is full (consumer stalled), the excess is dropped and logged — the
/// real-time audio thread must never wait.
fn push_samples(producer: &mut HeapProd<f32>, samples: &[f32], device_type: DeviceType) {
    if samples.is_empty() {
        return;
    }
    let written = producer.push_slice(samples);
    if written < samples.len() {
        log::warn!(
            "{LOG_PREFIX} {device_type:?} ring buffer overrun, dropped {} samples",
            samples.len() - written
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_roundtrip_preserves_samples() {
        let rb = HeapRb::<f32>::new(16);
        let (mut producer, mut consumer) = rb.split();
        let input = vec![0.1f32, 0.2, 0.3, 0.4];
        let written = producer.push_slice(&input);
        assert_eq!(written, input.len());

        let mut out = vec![0.0f32; 16];
        let popped = consumer.pop_slice(&mut out);
        assert_eq!(popped, input.len());
        assert_eq!(&out[..popped], input.as_slice());
    }

    #[test]
    fn drain_into_channel_forwards_available_samples() {
        let rb = HeapRb::<f32>::new(16);
        let (mut producer, mut consumer) = rb.split();
        producer.push_slice(&[1.0, 2.0, 3.0]);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut scratch = vec![0.0f32; 16];
        drain_into_channel(&mut consumer, &mut scratch, DeviceType::Input, 16_000, &tx);

        let chunk = rx.try_recv().expect("chunk forwarded");
        assert_eq!(chunk.samples, vec![1.0, 2.0, 3.0]);
        assert_eq!(chunk.sample_rate, 16_000);
        assert_eq!(chunk.device_type, DeviceType::Input);
    }

    #[test]
    fn drain_into_channel_noop_when_ring_empty() {
        let rb = HeapRb::<f32>::new(16);
        let (_producer, mut consumer) = rb.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut scratch = vec![0.0f32; 16];
        drain_into_channel(&mut consumer, &mut scratch, DeviceType::Output, 48_000, &tx);
        assert!(rx.try_recv().is_err());
    }
}
