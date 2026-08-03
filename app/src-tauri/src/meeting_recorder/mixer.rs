//! Two-source audio mixer: buffers mic + system-loopback chunks into
//! fixed-size windows and mixes them down to one mono stream.
//!
//! Ported faithfully from Meetily's `AudioMixerRingBuffer` /
//! `SimpleAudioMixer`. Despite the historical "ducking" naming carried
//! over from the original project, the real algorithm performs no
//! RMS-based ducking at all — it buffers to a fixed window, zero-pads
//! whichever side is shorter, sums the two, and soft-clips proportionally.

use std::collections::VecDeque;

use super::devices::DeviceType;

/// Window length used by [`AudioMixerRingBuffer::can_mix`] /
/// [`AudioMixerRingBuffer::extract_window`]. The historical Meetily code
/// comments called this "50ms" but the real window used at runtime is
/// 600ms — kept at parity here rather than the misleading comment.
const WINDOW_MS: u64 = 600;

/// Safety multiplier for the max buffer size before the oldest samples
/// start getting dropped (~4.8s of buffering headroom at a 600ms window).
const MAX_BUFFER_WINDOWS: usize = 8;

/// Buffers mic + system-loopback samples (both assumed to already be at
/// the same sample rate — callers resample upstream, see
/// `meeting_recorder::resample`) and hands out fixed-length windows for
/// mixing.
pub struct AudioMixerRingBuffer {
    mic_buffer: VecDeque<f32>,
    system_buffer: VecDeque<f32>,
    window_size_samples: usize,
    max_buffer_size: usize,
}

impl AudioMixerRingBuffer {
    pub fn new(sample_rate: u32) -> Self {
        let window_size_samples = (sample_rate as u64 * WINDOW_MS / 1000) as usize;
        Self {
            mic_buffer: VecDeque::with_capacity(window_size_samples * 2),
            system_buffer: VecDeque::with_capacity(window_size_samples * 2),
            window_size_samples,
            max_buffer_size: window_size_samples * MAX_BUFFER_WINDOWS,
        }
    }

    pub fn window_size_samples(&self) -> usize {
        self.window_size_samples
    }

    /// Append samples for one device's buffer. Drops the oldest samples
    /// (with a warning) if the buffer grows past its safety cap.
    pub fn add_samples(&mut self, device_type: DeviceType, samples: &[f32]) {
        let buffer = match device_type {
            DeviceType::Input => &mut self.mic_buffer,
            DeviceType::Output => &mut self.system_buffer,
        };
        buffer.extend(samples.iter().copied());
        if buffer.len() > self.max_buffer_size {
            let excess = buffer.len() - self.max_buffer_size;
            log::warn!(
                "[meeting-recorder:mixer] {device_type:?} buffer overflow ({} > {}), dropping {excess} oldest samples",
                buffer.len(),
                self.max_buffer_size
            );
            buffer.drain(..excess);
        }
    }

    /// True once either buffer has accumulated a full window.
    pub fn can_mix(&self) -> bool {
        self.mic_buffer.len() >= self.window_size_samples
            || self.system_buffer.len() >= self.window_size_samples
    }

    /// Drain one window from both buffers, zero-padding the shorter one so
    /// both outputs are always exactly `window_size_samples` long.
    pub fn extract_window(&mut self) -> (Vec<f32>, Vec<f32>) {
        let mic = drain_window(&mut self.mic_buffer, self.window_size_samples);
        let system = drain_window(&mut self.system_buffer, self.window_size_samples);
        (mic, system)
    }
}

fn drain_window(buffer: &mut VecDeque<f32>, window_size_samples: usize) -> Vec<f32> {
    let take = buffer.len().min(window_size_samples);
    let mut out: Vec<f32> = buffer.drain(..take).collect();
    if out.len() < window_size_samples {
        out.resize(window_size_samples, 0.0);
    }
    out
}

/// Sums the two mixer windows and applies proportional soft-clipping.
/// Despite the name (kept for continuity with the ported original), this
/// performs no RMS-based ducking — just sum + clip.
pub struct SimpleAudioMixer;

impl SimpleAudioMixer {
    pub fn mix_window(&mut self, mic: &[f32], system: &[f32]) -> Vec<f32> {
        mic.iter()
            .zip(system.iter())
            .map(|(&m, &s)| {
                let sum = m + s;
                if sum.abs() > 1.0 {
                    sum / sum.abs()
                } else {
                    sum
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_size_is_600ms() {
        let mixer = AudioMixerRingBuffer::new(16_000);
        assert_eq!(mixer.window_size_samples(), 9_600);
    }

    #[test]
    fn can_mix_false_when_both_buffers_below_window() {
        let mut mixer = AudioMixerRingBuffer::new(16_000);
        mixer.add_samples(DeviceType::Input, &vec![0.1; 100]);
        assert!(!mixer.can_mix());
    }

    #[test]
    fn can_mix_true_once_one_buffer_reaches_window() {
        let mut mixer = AudioMixerRingBuffer::new(16_000);
        let window = mixer.window_size_samples();
        mixer.add_samples(DeviceType::Input, &vec![0.1; window]);
        assert!(mixer.can_mix());
    }

    #[test]
    fn extract_window_pads_shorter_buffer_with_zeros() {
        let mut mixer = AudioMixerRingBuffer::new(16_000);
        let window = mixer.window_size_samples();
        mixer.add_samples(DeviceType::Input, &vec![0.5; window]);
        // system buffer stays empty
        let (mic, system) = mixer.extract_window();
        assert_eq!(mic.len(), window);
        assert_eq!(system.len(), window);
        assert!(system.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn extract_window_drains_exactly_one_window_leaving_remainder() {
        let mut mixer = AudioMixerRingBuffer::new(16_000);
        let window = mixer.window_size_samples();
        mixer.add_samples(DeviceType::Input, &vec![0.5; window + 50]);
        let (mic, _system) = mixer.extract_window();
        assert_eq!(mic.len(), window);
        assert_eq!(mixer.mic_buffer.len(), 50);
    }

    #[test]
    fn overflow_drops_oldest_samples() {
        let mut mixer = AudioMixerRingBuffer::new(16_000);
        let window = mixer.window_size_samples();
        let over = window * (MAX_BUFFER_WINDOWS + 1);
        mixer.add_samples(DeviceType::Input, &vec![1.0; over]);
        assert!(mixer.mic_buffer.len() <= window * MAX_BUFFER_WINDOWS);
    }

    #[test]
    fn mix_window_sums_without_clipping_when_under_threshold() {
        let mut mixer = SimpleAudioMixer;
        let mic = vec![0.2f32, -0.2];
        let system = vec![0.1f32, -0.1];
        let mixed = mixer.mix_window(&mic, &system);
        assert!((mixed[0] - 0.3).abs() < 1e-6);
        assert!((mixed[1] + 0.3).abs() < 1e-6);
    }

    #[test]
    fn mix_window_soft_clips_when_sum_exceeds_unity() {
        let mut mixer = SimpleAudioMixer;
        let mixed_pos = mixer.mix_window(&[0.8], &[0.8]);
        assert_eq!(mixed_pos, vec![1.0_f32]);

        let mixed_neg = mixer.mix_window(&[-0.8], &[-0.8]);
        assert_eq!(mixed_neg, vec![-1.0_f32]);
    }

    #[test]
    fn mix_window_handles_empty_input() {
        let mut mixer = SimpleAudioMixer;
        let mixed = mixer.mix_window(&[], &[]);
        assert!(mixed.is_empty());
    }
}
