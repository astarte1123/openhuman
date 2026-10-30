//! ONNX inference core: speech segmentation, speaker-embedding extraction,
//! and running-template clustering. This is the ~850-line self-contained
//! slice of Minutes' `diarize.rs` this feature actually ports (segmentation
//! + `BoundedEmbeddingExtractor`-equivalent + `diarize_with_pyannote_rs`'s
//! clustering) — see `super`'s module doc for what was deliberately left
//! out (stem separation, degraded-capture detection, cross-meeting speaker
//! enrollment, the Python fallback engine, alternate model sets, the
//! process-global worker lease).
//!
//! **Why segmentation is hand-rolled against raw `ort` instead of calling
//! `pyannote_rs::get_segments`**: that function has a confirmed upstream bug
//! — it casts raw `i16` samples to `f32` (`x as f32`) without dividing by
//! `32768.0`, so the segmentation model receives values wildly outside the
//! `[-1.0, 1.0]` range it was trained on and reports no speech at all for
//! normal audio. [`segment_speech`] below mirrors `get_segments`'s sliding-
//! window logic but feeds correctly normalized `f32` PCM. This mirrors
//! Minutes' own `diarize.rs` doing exactly the same thing for exactly the
//! same reason (see that file's `segment_speech` doc comment) — this is not
//! an invented workaround, it's the same fix applied independently against
//! the same confirmed bug.
//!
//! Embedding extraction, by contrast, uses `pyannote_rs::EmbeddingExtractor`
//! directly (no known bug in that path) rather than hand-rolling the
//! `knf-rs` fbank + raw-session plumbing Minutes' `BoundedEmbeddingExtractor`
//! uses for its own inference-deadline support — this MVP has no
//! mid-inference cancellation watchdog (see [`diarize_wav`]'s doc comment),
//! so that extra plumbing has no payoff here.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use log::debug;

use super::LOG_PREFIX;

/// pyannote `segmentation-3.0`'s expected ONNX input/output tensor names.
const SEGMENTATION_INPUT_NAME: &str = "input";
const SEGMENTATION_OUTPUT_NAME: &str = "output";

/// Hop between segmentation output frames (270 samples @ 16kHz = 16.875ms),
/// derived from the model's sincnet + temporal-pooling stride.
const FRAME_SIZE: usize = 270;
/// Receptive-field offset — how many input samples precede the center of
/// the first output frame (721 samples @ 16kHz = 45ms).
const FRAME_START: usize = 721;
/// The model's fixed-length input window, in seconds.
const WINDOW_SECONDS: usize = 10;

/// Safety cap mirroring Minutes' `MAX_DIARIZATION_SEGMENTS` — a segmentation
/// pass that reports more turns than this on a single meeting is almost
/// certainly pathological input (e.g. white noise), not real speech.
const MAX_SPEECH_SEGMENTS: usize = 20_000;
/// Safety cap mirroring Minutes' `MAX_SPEAKER_TEMPLATES`.
const MAX_SPEAKER_TEMPLATES: usize = 64;
/// Longest slice of a single speech segment actually fed to the embedding
/// model — mirrors Minutes' `MAX_EMBEDDING_SECONDS`; a segment longer than
/// this contributes only its first 60s to its embedding.
const MAX_EMBEDDING_SECONDS: usize = 60;

/// Minimum cosine similarity to an existing speaker template for a new
/// segment to be assigned to it rather than starting a new speaker. Matches
/// Minutes' documented default (`config.diarization.threshold == 0.4`) —
/// this repo has no per-user-configurable diarization threshold yet.
const CLUSTER_THRESHOLD: f32 = 0.4;
/// Floor for the post-clustering template-merge pass, so an aggressively low
/// `CLUSTER_THRESHOLD` can never merge genuinely different speakers.
const MERGE_THRESHOLD_FLOOR: f32 = 0.3;
/// How much lower than `CLUSTER_THRESHOLD` the merge pass's threshold sits.
const MERGE_THRESHOLD_MARGIN: f32 = 0.05;

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// One diarized speech turn: an opaque per-meeting-local speaker label
/// (`"speaker_1"`, `"speaker_2"`, ...) plus its time range in seconds,
/// relative to the start of the WAV file [`diarize_wav`] was run against.
/// [`super::mapping::map_segments_to_chunks`] turns this into the
/// `"system_audio:speaker_N"` label persisted on transcript chunks — labels
/// here have no `system_audio:` prefix and no relationship across separate
/// [`diarize_wav`] calls (see `super`'s "no cross-meeting speaker
/// enrollment" honesty note).
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerSegment {
    pub speaker: String,
    pub start_secs: f64,
    pub end_secs: f64,
}

#[derive(Debug, Clone)]
struct SpeechSegment {
    start_secs: f64,
    end_secs: f64,
    start_sample: usize,
    end_sample: usize,
}

/// Runs the full segmentation -> embedding -> clustering pipeline over a WAV
/// file and returns one [`SpeakerSegment`] per detected speech turn.
/// CPU/ONNX-bound and synchronous — callers must run this via
/// `spawn_blocking` (see [`super::diarize_meeting_system_audio`]).
///
/// `deadline` bounds the coarse wall-clock budget checked between pipeline
/// stages — it is **not** a hard mid-inference cutoff like the reference
/// implementation's `OrtInferenceDeadline` watchdog thread (see `super`'s
/// module doc); a single stuck ONNX call inside [`segment_speech`] or the
/// embedding loop can still run past it. Good enough for an explicit,
/// user-triggered offline batch pass; not a real-time guarantee.
pub fn diarize_wav(
    wav_path: &Path,
    models_dir: &Path,
    deadline: Duration,
) -> Result<Vec<SpeakerSegment>, String> {
    let start = Instant::now();
    let seg_model = models_dir.join(super::models::SEGMENTATION_MODEL_FILENAME);
    let emb_model = models_dir.join(super::models::EMBEDDING_MODEL_FILENAME);

    if !seg_model.is_file() {
        return Err(format!(
            "segmentation model missing at {}",
            seg_model.display()
        ));
    }
    if !emb_model.is_file() {
        return Err(format!(
            "embedding model missing at {}",
            emb_model.display()
        ));
    }

    let (samples, sample_rate) = load_audio(wav_path)?;
    debug!(
        "{LOG_PREFIX} engine::diarize_wav: loaded {} sample(s) @ {sample_rate}Hz from {}",
        samples.len(),
        wav_path.display()
    );
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    check_deadline(start, deadline)?;
    let raw_segments = segment_speech(&samples, sample_rate, &seg_model)
        .map_err(|e| format!("speech segmentation failed: {e}"))?;
    debug!(
        "{LOG_PREFIX} engine::diarize_wav: {} raw speech segment(s)",
        raw_segments.len()
    );
    if raw_segments.is_empty() {
        return Ok(Vec::new());
    }

    check_deadline(start, deadline)?;
    let segments = merge_short_segments(raw_segments, sample_rate);
    debug!(
        "{LOG_PREFIX} engine::diarize_wav: {} segment(s) after short-segment merge",
        segments.len()
    );

    check_deadline(start, deadline)?;
    let labels = cluster_segments(&samples, sample_rate, &segments, &emb_model)?;
    // See `count_real_speakers`'s doc comment: a segment labeled
    // `UNKNOWN_SPEAKER_LABEL` means no real speaker template was ever built
    // for it (too short to embed reliably), not a genuine first speaker —
    // this must not be counted as one.
    let speaker_count = count_real_speakers(labels.iter().map(String::as_str));

    let result: Vec<SpeakerSegment> = segments
        .iter()
        .zip(labels.iter())
        .map(|(seg, label)| SpeakerSegment {
            speaker: label.clone(),
            start_secs: seg.start_secs,
            end_secs: seg.end_secs,
        })
        .collect();

    debug!(
        "{LOG_PREFIX} engine::diarize_wav: done in {:?}, {} segment(s), {speaker_count} distinct speaker(s)",
        start.elapsed(),
        result.len()
    );
    Ok(result)
}

fn check_deadline(start: Instant, deadline: Duration) -> Result<(), String> {
    if start.elapsed() > deadline {
        return Err("diarization deadline exceeded".to_string());
    }
    Ok(())
}

/// Reads `wav_path` and returns mono `f32` PCM at [`TARGET_SAMPLE_RATE`],
/// down-mixing multi-channel audio by averaging channels and resampling if
/// the file's native rate differs. [`crate::aura::meetings::capture::raw_audio_sink`]
/// always writes mono 16kHz already, so the resample path is a defensive
/// fallback for a WAV that ended up on disk some other way, not the normal
/// case.
fn load_audio(wav_path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(wav_path)
        .map_err(|e| format!("failed to open WAV {}: {e}", wav_path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    if channels == 0 {
        return Err("WAV has zero channels".to_string());
    }

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("failed to read WAV float samples: {e}"))?,
        hound::SampleFormat::Int => {
            let max_value = (1i64 << spec.bits_per_sample.saturating_sub(1).max(1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_value))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("failed to read WAV int samples: {e}"))?
        }
    };

    let mono = downmix(&interleaved, channels);
    let resampled = if spec.sample_rate == TARGET_SAMPLE_RATE {
        mono
    } else {
        crate::aura::voice::audio_capture::resample(&mono, spec.sample_rate, TARGET_SAMPLE_RATE)
    };

    Ok((resampled, TARGET_SAMPLE_RATE))
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Runs the segmentation ONNX model directly with correctly normalized
/// `f32` audio (see this module's doc comment for why `pyannote_rs::get_segments`
/// is bypassed). Mirrors the reference implementation's sliding-window
/// logic exactly.
fn segment_speech(
    samples: &[f32],
    sample_rate: u32,
    model_path: &Path,
) -> Result<Vec<SpeechSegment>, Box<dyn std::error::Error>> {
    use ndarray::{Array1, ArrayViewD, Axis, IxDyn};
    use ort::session::builder::GraphOptimizationLevel;
    use ort::session::Session;

    let mut session = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(1)?
        .with_inter_threads(1)?
        .commit_from_file(model_path)?;

    let actual_input = session
        .inputs
        .first()
        .map(|input| input.name.as_str())
        .ok_or("segmentation model has no input")?;
    if actual_input != SEGMENTATION_INPUT_NAME {
        return Err(format!(
            "segmentation model input mismatch: expected '{SEGMENTATION_INPUT_NAME}', found '{actual_input}'"
        )
        .into());
    }
    if !session
        .outputs
        .iter()
        .any(|output| output.name == SEGMENTATION_OUTPUT_NAME)
    {
        return Err(format!(
            "segmentation model output mismatch: expected '{SEGMENTATION_OUTPUT_NAME}'"
        )
        .into());
    }

    let window_size = sample_rate as usize * WINDOW_SECONDS;
    let mut result = Vec::new();
    let mut is_speeching = false;
    let mut offset = FRAME_START;
    let mut start_offset = 0usize;
    let mut final_window = vec![0.0f32; window_size];

    for window_start in (0..samples.len()).step_by(window_size) {
        let window_end = (window_start + window_size).min(samples.len());
        let window: &[f32] = if window_end - window_start == window_size {
            &samples[window_start..window_end]
        } else {
            final_window.iter_mut().for_each(|s| *s = 0.0);
            let tail = &samples[window_start..window_end];
            final_window[..tail.len()].copy_from_slice(tail);
            final_window.as_slice()
        };

        let array = Array1::from_iter(window.iter().copied());
        let array = array.view().insert_axis(Axis(0)).insert_axis(Axis(1));

        let inputs = ort::inputs![ort::value::TensorRef::from_array_view(array.into_dyn())
            .map_err(|e| format!("tensor prep: {e:?}"))?];

        let outputs = session.run(inputs)?;
        let out = outputs
            .get(SEGMENTATION_OUTPUT_NAME)
            .ok_or("segmentation model output tensor is missing")?;
        let (shape, data) = out
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("tensor extract: {e:?}"))?;
        let shape_slice: Vec<usize> = (0..shape.len()).map(|i| shape[i] as usize).collect();
        let view = ArrayViewD::<f32>::from_shape(IxDyn(&shape_slice), data)
            .map_err(|e| format!("ndarray shape: {e}"))?;

        for row in view.outer_iter() {
            for sub_row in row.axis_iter(Axis(0)) {
                let max_index = sub_row
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                if max_index != 0 {
                    if !is_speeching {
                        start_offset = offset;
                        is_speeching = true;
                    }
                } else if is_speeching {
                    let start_secs = start_offset as f64 / sample_rate as f64;
                    let end_secs = offset as f64 / sample_rate as f64;
                    let start_sample = start_offset.min(samples.len().saturating_sub(1));
                    let end_sample = offset.min(samples.len());
                    result.push(SpeechSegment {
                        start_secs,
                        end_secs,
                        start_sample,
                        end_sample,
                    });
                    if result.len() > MAX_SPEECH_SEGMENTS {
                        return Err("diarization segment budget exceeded".into());
                    }
                    is_speeching = false;
                }
                offset += FRAME_SIZE;
            }
        }
    }

    // Flush trailing speech that never hit a silence frame before the
    // recording ended.
    if is_speeching {
        let start_secs = start_offset as f64 / sample_rate as f64;
        let end_secs = offset as f64 / sample_rate as f64;
        let start_sample = start_offset.min(samples.len().saturating_sub(1));
        let end_sample = samples.len();
        result.push(SpeechSegment {
            start_secs,
            end_secs,
            start_sample,
            end_sample,
        });
        if result.len() > MAX_SPEECH_SEGMENTS {
            return Err("diarization segment budget exceeded".into());
        }
    }

    Ok(result)
}

/// Merges speech segments separated by short gaps and absorbs tiny segments
/// into their neighbor, reducing over-fragmentation from the frame-level
/// segmentation model so embeddings get longer, more stable audio to work
/// with. Mirrors the reference implementation's constants and logic exactly.
fn merge_short_segments(segments: Vec<SpeechSegment>, sample_rate: u32) -> Vec<SpeechSegment> {
    if segments.is_empty() {
        return segments;
    }

    let max_gap_samples = (sample_rate as f64 * 0.3) as usize; // 300ms gap tolerance
    let min_dur_samples = (sample_rate as f64 * 0.5) as usize; // 0.5s minimum
    let max_short_gap_samples = (sample_rate as f64 * 1.0) as usize; // 1s ceiling for absorbing short segments

    let mut merged: Vec<SpeechSegment> = Vec::new();
    let mut current = segments[0].clone();

    for seg in segments.iter().skip(1) {
        let gap = seg.start_sample.saturating_sub(current.end_sample);
        let current_dur = current.end_sample.saturating_sub(current.start_sample);
        let should_merge = gap <= max_gap_samples
            || (current_dur < min_dur_samples && gap <= max_short_gap_samples);

        if should_merge {
            current.end_secs = seg.end_secs;
            current.end_sample = seg.end_sample;
        } else {
            merged.push(current);
            current = seg.clone();
        }
    }
    merged.push(current);
    merged
}

/// Extracts a speaker embedding per segment (skipping segments too short for
/// a reliable embedding) and clusters them via running-template cosine
/// similarity, then a merge pass, then temporal forward/backward label fill
/// for segments that were skipped — mirrors
/// `diarize_with_pyannote_rs`'s clustering logic in the reference
/// implementation.
fn cluster_segments(
    samples: &[f32],
    sample_rate: u32,
    segments: &[SpeechSegment],
    emb_model: &Path,
) -> Result<Vec<String>, String> {
    let mut extractor = pyannote_rs::EmbeddingExtractor::new(emb_model)
        .map_err(|e| format!("failed to load embedding model: {e}"))?;

    let min_embed_samples = (sample_rate as f64 * 1.5) as usize;
    let max_embed_samples = sample_rate as usize * MAX_EMBEDDING_SECONDS;

    // speaker_templates[i] = (running-average embedding, segment count)
    let mut speaker_templates: Vec<(Vec<f32>, usize)> = Vec::new();
    // Per-segment: which speaker template index it was assigned, or `None`
    // for segments too short to embed reliably (they inherit a label later).
    let mut seg_speaker_ids: Vec<Option<usize>> = Vec::with_capacity(segments.len());

    for seg in segments {
        let embed_end = seg
            .end_sample
            .min(seg.start_sample.saturating_add(max_embed_samples));
        let slice = samples
            .get(seg.start_sample..embed_end)
            .ok_or("speech segment sample range out of bounds")?;

        if slice.len() < min_embed_samples {
            seg_speaker_ids.push(None);
            continue;
        }

        let seg_i16: Vec<i16> = slice
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        let raw_embedding: Vec<f32> = extractor
            .compute(&seg_i16)
            .map_err(|e| format!("embedding extraction failed: {e}"))?
            .collect();
        let embedding = l2_normalize(&raw_embedding);

        let mut best_id = None;
        let mut best_sim = CLUSTER_THRESHOLD;
        for (id, (template, _)) in speaker_templates.iter().enumerate() {
            let sim = cosine_similarity(&embedding, template);
            if sim > best_sim {
                best_sim = sim;
                best_id = Some(id);
            }
        }

        let speaker_id = match best_id {
            Some(id) => {
                let (template, count) = &mut speaker_templates[id];
                let old_count = *count as f32;
                let new_count = old_count + 1.0;
                for (i, val) in embedding.iter().enumerate() {
                    template[i] = (template[i] * old_count + val) / new_count;
                }
                *count += 1;
                id
            }
            None => {
                if speaker_templates.len() >= MAX_SPEAKER_TEMPLATES {
                    return Err("diarization speaker-template budget exceeded".to_string());
                }
                let id = speaker_templates.len();
                speaker_templates.push((embedding, 1));
                id
            }
        };

        seg_speaker_ids.push(Some(speaker_id));
    }

    Ok(resolve_speaker_labels(speaker_templates, seg_speaker_ids))
}

/// Merges near-duplicate speaker templates (catches early segments that
/// created separate speakers before converging as more data arrived), then
/// assigns compact `speaker_N` labels and fills in labels for segments that
/// were skipped during embedding (too short) by inheriting the nearest
/// *temporal* neighbor's label.
fn resolve_speaker_labels(
    speaker_templates: Vec<(Vec<f32>, usize)>,
    seg_speaker_ids: Vec<Option<usize>>,
) -> Vec<String> {
    let merge_threshold = (CLUSTER_THRESHOLD - MERGE_THRESHOLD_MARGIN).max(MERGE_THRESHOLD_FLOOR);
    let num_templates = speaker_templates.len();
    let mut merge_map: Vec<usize> = (0..num_templates).collect();

    for i in 0..num_templates {
        for j in (i + 1)..num_templates {
            if merge_map[j] != j {
                continue; // already merged
            }
            let ri = merge_map[i];
            let sim = cosine_similarity(&speaker_templates[ri].0, &speaker_templates[j].0);
            if sim > merge_threshold {
                merge_map[j] = ri;
            }
        }
    }
    // Resolve transitive merges (e.g. 3->2->1 becomes 3->1, 2->1).
    for i in 0..num_templates {
        let mut root = merge_map[i];
        let mut steps = 0;
        while merge_map[root] != root && steps < num_templates {
            root = merge_map[root];
            steps += 1;
        }
        merge_map[i] = root;
    }

    let mut canonical_to_label: HashMap<usize, String> = HashMap::new();
    let mut next_label = 1usize;
    for &canonical in &merge_map {
        canonical_to_label.entry(canonical).or_insert_with(|| {
            let label = format!("speaker_{next_label}");
            next_label += 1;
            label
        });
    }

    let resolved_labels: Vec<Option<String>> = seg_speaker_ids
        .iter()
        .map(|raw_id| raw_id.map(|id| canonical_to_label[&merge_map[id]].clone()))
        .collect();

    let mut last_known: Option<String> = None;
    let mut final_labels: Vec<String> = Vec::with_capacity(resolved_labels.len());
    for label in &resolved_labels {
        if let Some(l) = label {
            last_known = Some(l.clone());
        }
        final_labels.push(
            last_known
                .clone()
                .unwrap_or_else(|| UNKNOWN_SPEAKER_LABEL.to_string()),
        );
    }
    // Backward pass: fix leading skipped segments (before any known label).
    if let Some(first_known) = resolved_labels.iter().find_map(|l| l.as_ref()) {
        for label in &mut final_labels {
            if label == UNKNOWN_SPEAKER_LABEL {
                *label = first_known.clone();
            } else {
                break;
            }
        }
    }

    final_labels
}

/// Placeholder speaker label [`resolve_speaker_labels`] assigns to a segment
/// that never inherited a real speaker template — either every segment in
/// the meeting was shorter than `cluster_segments`'s `min_embed_samples`
/// floor (no template ever created) or, transiently, a segment precedes the
/// first one that did resolve (fixed up by the backward pass above). Exposed
/// (`pub(crate)`, not private) so [`count_real_speakers`] and callers outside
/// this module (`ops::meetings_diarize`) can consistently exclude it from
/// "how many speakers did diarization find" — see that function's doc
/// comment for why counting it as a real speaker is wrong.
pub(crate) const UNKNOWN_SPEAKER_LABEL: &str = "speaker_unknown";

/// Counts distinct *real* speaker identities among `labels` — pure and
/// input-shape-agnostic so both [`diarize_wav`]'s own internal count and
/// `ops::meetings_diarize`'s `speakers_found` (computed over
/// `SpeakerSegment::speaker` strings, a different shape) can share one
/// definition instead of two HashSet-over-strings computations silently
/// drifting apart.
///
/// Labels equal to [`UNKNOWN_SPEAKER_LABEL`] are excluded: every segment
/// only ever gets that placeholder when `cluster_segments` built zero real
/// speaker templates for the whole meeting (every speech segment was too
/// short to embed reliably), meaning diarization did not actually identify
/// anyone — counting the repeated placeholder string as one distinct speaker
/// would fabricate a single-speaker identity where none was detected.
pub(crate) fn count_real_speakers<'a>(labels: impl IntoIterator<Item = &'a str>) -> usize {
    labels
        .into_iter()
        .filter(|label| *label != UNKNOWN_SPEAKER_LABEL)
        .collect::<std::collections::HashSet<_>>()
        .len()
}

/// L2-normalize a vector to unit length. Returns the input unchanged if it
/// has zero norm (avoids NaN propagation from a divide-by-zero).
fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(
        start_secs: f64,
        end_secs: f64,
        start_sample: usize,
        end_sample: usize,
    ) -> SpeechSegment {
        SpeechSegment {
            start_secs,
            end_secs,
            start_sample,
            end_sample,
        }
    }

    // ── Pure helpers: no ONNX, no model files ───────────────────────────

    #[test]
    fn l2_normalize_produces_unit_length() {
        let v = l2_normalize(&[3.0, 4.0]);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_zero_vector_stays_zero() {
        assert_eq!(l2_normalize(&[0.0, 0.0]), vec![0.0, 0.0]);
    }

    #[test]
    fn cosine_similarity_identical_vectors_is_one() {
        let a = [1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors_is_zero() {
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_mismatched_lengths_returns_zero() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
    }

    #[test]
    fn downmix_stereo_averages_channels() {
        let interleaved = vec![1.0, -1.0, 0.5, 0.5];
        assert_eq!(downmix(&interleaved, 2), vec![0.0, 0.5]);
    }

    #[test]
    fn downmix_mono_is_a_no_op() {
        let samples = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix(&samples, 1), samples);
    }

    #[test]
    fn merge_short_segments_bridges_a_short_gap() {
        let segments = vec![seg(0.0, 1.0, 0, 16_000), seg(1.1, 2.0, 17_600, 32_000)];
        let merged = merge_short_segments(segments, 16_000);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_sample, 0);
        assert_eq!(merged[0].end_sample, 32_000);
    }

    #[test]
    fn merge_short_segments_keeps_a_long_gap_separate() {
        let segments = vec![seg(0.0, 1.0, 0, 16_000), seg(5.0, 6.0, 80_000, 96_000)];
        let merged = merge_short_segments(segments, 16_000);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_short_segments_empty_input_stays_empty() {
        assert!(merge_short_segments(Vec::new(), 16_000).is_empty());
    }

    // ── Label resolution (running-template clustering output shaping) ──

    #[test]
    fn resolve_speaker_labels_assigns_compact_labels_per_template() {
        let templates = vec![(vec![1.0, 0.0], 1), (vec![0.0, 1.0], 1)];
        let ids = vec![Some(0), Some(1), Some(0)];
        let labels = resolve_speaker_labels(templates, ids);
        assert_eq!(labels, vec!["speaker_1", "speaker_2", "speaker_1"]);
    }

    #[test]
    fn resolve_speaker_labels_merges_near_duplicate_templates() {
        // Two templates that are nearly identical (cosine sim > merge
        // threshold, which floors at 0.3) should collapse to one label.
        let templates = vec![(vec![1.0, 0.0], 1), (vec![0.99, 0.01], 1)];
        let ids = vec![Some(0), Some(1)];
        let labels = resolve_speaker_labels(templates, ids);
        assert_eq!(labels[0], labels[1]);
    }

    #[test]
    fn resolve_speaker_labels_fills_forward_for_skipped_segments() {
        let templates = vec![(vec![1.0, 0.0], 1)];
        let ids = vec![Some(0), None, None];
        let labels = resolve_speaker_labels(templates, ids);
        assert_eq!(labels, vec!["speaker_1", "speaker_1", "speaker_1"]);
    }

    #[test]
    fn resolve_speaker_labels_backfills_leading_skipped_segments() {
        let templates = vec![(vec![1.0, 0.0], 1)];
        let ids = vec![None, None, Some(0)];
        let labels = resolve_speaker_labels(templates, ids);
        assert_eq!(labels, vec!["speaker_1", "speaker_1", "speaker_1"]);
    }

    #[test]
    fn resolve_speaker_labels_all_skipped_segments_stay_placeholder() {
        let templates: Vec<(Vec<f32>, usize)> = Vec::new();
        let ids = vec![None, None];
        let labels = resolve_speaker_labels(templates, ids);
        assert_eq!(labels, vec!["speaker_unknown", "speaker_unknown"]);
    }

    // ── count_real_speakers (regression: placeholder must not count) ───

    #[test]
    fn count_real_speakers_excludes_all_unknown_placeholders() {
        // Mirrors every speech segment being shorter than the embedding
        // floor: zero real templates were ever built, so this must report
        // zero speakers, not one.
        let labels = vec![UNKNOWN_SPEAKER_LABEL, UNKNOWN_SPEAKER_LABEL];
        assert_eq!(count_real_speakers(labels), 0);
    }

    #[test]
    fn count_real_speakers_counts_distinct_real_labels() {
        let labels = vec!["speaker_1", "speaker_2", "speaker_1"];
        assert_eq!(count_real_speakers(labels), 2);
    }

    #[test]
    fn count_real_speakers_ignores_placeholder_mixed_with_real_labels() {
        // A leading/trailing placeholder that the backward-fill pass didn't
        // reach mixed in with genuine speakers — only the real ones count.
        let labels = vec![UNKNOWN_SPEAKER_LABEL, "speaker_1", "speaker_1"];
        assert_eq!(count_real_speakers(labels), 1);
    }

    #[test]
    fn count_real_speakers_empty_input_is_zero() {
        assert_eq!(count_real_speakers(Vec::new()), 0);
    }

    #[test]
    fn diarize_wav_errors_clearly_when_models_are_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("does_not_matter.wav");
        let err = diarize_wav(&wav_path, dir.path(), Duration::from_secs(60)).unwrap_err();
        assert!(err.contains("model missing"), "unexpected error: {err}");
    }
}
