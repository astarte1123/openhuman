// meetings/diarize.rs — Speaker diarization pipeline using local ONNX models.
//
// Compile-time feature gate: `diarize` feature enables the full pipeline.
// Without the feature, public types are still defined but `diarize()` and
// `decode_audio()` return an Err immediately, and `ensure_models()` is a no-op.
//
// Pipeline (when `diarize` feature is enabled):
//   1. Decode audio (any format symphonia handles) → mono f32 + i16 samples
//   2. Resample to 16 kHz if needed
//   3. Run pyannote segmentation ONNX model in 10-second windows to detect speech frames
//   4. Extract speaker embeddings via pyannote-rs EmbeddingExtractor for each segment
//   5. Cluster embeddings by cosine similarity → SPEAKER_0, SPEAKER_1, …
//   6. Merge Whisper timed segments with diarization result

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public types (always available regardless of feature flag)
// ---------------------------------------------------------------------------

/// A speaker segment produced by the diarization pipeline.
#[derive(Debug, Clone)]
pub struct SpeakerSegment {
    pub speaker: String, // "SPEAKER_0", "SPEAKER_1", …
    pub start: f64,      // seconds
    pub end: f64,        // seconds
}

/// A timed transcript segment from Whisper verbose_json output.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TimedTranscriptSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Model URLs and filenames
// ---------------------------------------------------------------------------

const SEG_MODEL_FILENAME: &str = "segmentation-3.0.onnx";
const EMB_MODEL_FILENAME: &str = "wespeaker_en_voxceleb_CAM++.onnx";

const SEG_MODEL_URL: &str = "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx";
const EMB_MODEL_URL: &str = "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/wespeaker_en_voxceleb_CAM++.onnx";

// ---------------------------------------------------------------------------
// Model management (always available — download is pure HTTP via reqwest)
// ---------------------------------------------------------------------------

/// Download ONNX models to `{data_dir}/models/` if not already present.
/// Returns `(seg_model_path, emb_model_path)`.
pub async fn ensure_models(data_dir: &str) -> Result<(PathBuf, PathBuf), String> {
    let models_dir = PathBuf::from(data_dir).join("models");
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("[diarize] create models dir: {e}"))?;

    let seg_path = models_dir.join(SEG_MODEL_FILENAME);
    let emb_path = models_dir.join(EMB_MODEL_FILENAME);

    download_if_missing(&seg_path, SEG_MODEL_URL, "segmentation").await?;
    download_if_missing(&emb_path, EMB_MODEL_URL, "embedding").await?;

    Ok((seg_path, emb_path))
}

async fn download_if_missing(dest: &Path, url: &str, label: &str) -> Result<(), String> {
    if dest.exists() {
        eprintln!(
            "[diarize] {} model already present: {}",
            label,
            dest.display()
        );
        return Ok(());
    }

    eprintln!("[diarize] downloading {} model from {url}", label);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("[diarize] build client: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("[diarize] download {label}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "[diarize] download {label}: HTTP {}",
            resp.status()
        ));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("[diarize] read download body {label}: {e}"))?;

    std::fs::write(dest, &bytes)
        .map_err(|e| format!("[diarize] write model {label}: {e}"))?;

    eprintln!(
        "[diarize] {} model saved ({} bytes) → {}",
        label,
        bytes.len(),
        dest.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Transcript merge (always available — no native dependencies)
// ---------------------------------------------------------------------------

/// Merge Whisper timed segments with speaker diarization result.
///
/// For each Whisper segment, finds the `SpeakerSegment` with the maximum
/// overlap with [start, end]. Maps SPEAKER_N → `attendee_names[N]` if
/// available, else falls back to "Parlante N+1". Groups consecutive segments
/// from the same speaker.
///
/// Output format:
/// ```text
/// [Alice]: Some text here.
///
/// [Bob]: Other text here.
/// ```
pub fn merge_transcript(
    whisper_segments: &[TimedTranscriptSegment],
    speaker_segments: &[SpeakerSegment],
    attendee_names: &[String],
) -> String {
    if whisper_segments.is_empty() {
        return String::new();
    }

    // Map each Whisper segment to a speaker label.
    let labeled: Vec<(&TimedTranscriptSegment, String)> = whisper_segments
        .iter()
        .map(|ws| {
            let speaker = find_dominant_speaker(ws.start, ws.end, speaker_segments);
            let label = match speaker {
                Some(sp) => {
                    // Parse SPEAKER_N suffix.
                    let n: Option<usize> = sp
                        .strip_prefix("SPEAKER_")
                        .and_then(|s| s.parse().ok());
                    match n {
                        Some(idx) if idx < attendee_names.len() => {
                            attendee_names[idx].clone()
                        }
                        Some(idx) => format!("Parlante {}", idx + 1),
                        None => sp,
                    }
                }
                None => String::new(), // no speaker → no label
            };
            (ws, label)
        })
        .collect();

    // Group consecutive same-speaker runs.
    let mut output = String::new();
    let mut i = 0usize;

    while i < labeled.len() {
        let (ws, ref label) = labeled[i];
        let mut text = ws.text.trim().to_string();
        let mut j = i + 1;

        // Extend run while speaker is the same.
        while j < labeled.len() && &labeled[j].1 == label {
            let next_text = labeled[j].0.text.trim();
            if !next_text.is_empty() {
                text.push(' ');
                text.push_str(next_text);
            }
            j += 1;
        }

        if !output.is_empty() {
            output.push_str("\n\n");
        }

        if label.is_empty() {
            output.push_str(text.trim());
        } else {
            output.push_str(&format!("[{label}]: {}", text.trim()));
        }

        i = j;
    }

    output
}

/// Find the speaker with the maximum overlap with [ws_start, ws_end].
fn find_dominant_speaker(
    ws_start: f64,
    ws_end: f64,
    speaker_segments: &[SpeakerSegment],
) -> Option<String> {
    if speaker_segments.is_empty() {
        return None;
    }

    let mut best_speaker: Option<&str> = None;
    let mut best_overlap = 0.0f64;

    for ss in speaker_segments {
        let overlap_start = ws_start.max(ss.start);
        let overlap_end = ws_end.min(ss.end);
        let overlap = (overlap_end - overlap_start).max(0.0);
        if overlap > best_overlap {
            best_overlap = overlap;
            best_speaker = Some(&ss.speaker);
        }
    }

    best_speaker.map(String::from)
}

// ---------------------------------------------------------------------------
// Audio decoding — gated on `diarize` feature
// ---------------------------------------------------------------------------

/// Decode audio bytes (any format symphonia supports) into:
///   - `f32_samples`: normalized [-1.0, 1.0] mono at original sample rate
///   - `i16_samples`: mono i16 at original sample rate (for embedding extractor)
///   - `sample_rate`: original sample rate in Hz
///
/// Requires the `diarize` feature. Returns `Err` immediately without it.
pub fn decode_audio(audio_bytes: &[u8]) -> Result<(Vec<f32>, Vec<i16>, u32), String> {
    #[cfg(not(feature = "diarize"))]
    {
        let _ = audio_bytes;
        return Err("[diarize] compiled without 'diarize' feature — audio decoding unavailable".to_string());
    }

    #[cfg(feature = "diarize")]
    decode_audio_impl(audio_bytes)
}

#[cfg(feature = "diarize")]
fn decode_audio_impl(audio_bytes: &[u8]) -> Result<(Vec<f32>, Vec<i16>, u32), String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    eprintln!("[diarize] decode_audio bytes={}", audio_bytes.len());

    let cursor = std::io::Cursor::new(audio_bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let hint = Hint::new();
    let format_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default();
    let dec_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &meta_opts)
        .map_err(|e| format!("[diarize] probe audio format: {e}"))?;

    let mut format = probed.format;

    // Pick the default audio track.
    let track = format
        .default_track()
        .ok_or_else(|| "[diarize] no audio track found".to_string())?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(16000);
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1);
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| format!("[diarize] build decoder: {e}"))?;

    let mut f32_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                break;
            }
            Err(e) => return Err(format!("[diarize] read packet: {e}")),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("[diarize] decode packet: {e}"))?;

        let spec = *decoded.spec();
        let frames = decoded.frames();
        let mut buf: SampleBuffer<f32> = SampleBuffer::new(frames as u64, spec);
        buf.copy_interleaved_ref(decoded);
        let interleaved = buf.samples();

        // Downmix to mono: average all channels.
        if channels == 1 {
            f32_samples.extend_from_slice(interleaved);
        } else {
            let ch = channels as usize;
            for frame in interleaved.chunks_exact(ch) {
                let mono = frame.iter().sum::<f32>() / ch as f32;
                f32_samples.push(mono);
            }
        }
    }

    eprintln!(
        "[diarize] decoded {} f32 samples sample_rate={sample_rate}",
        f32_samples.len()
    );

    // Convert f32 → i16 for embedding extractor (expects PCM i16 at same rate).
    let i16_samples: Vec<i16> = f32_samples
        .iter()
        .map(|&s| {
            let clamped = s.clamp(-1.0, 1.0);
            (clamped * i16::MAX as f32) as i16
        })
        .collect();

    Ok((f32_samples, i16_samples, sample_rate))
}

// ---------------------------------------------------------------------------
// Resampling (linear interpolation) — gated on `diarize` feature
// ---------------------------------------------------------------------------

#[cfg(feature = "diarize")]
fn resample_mono(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate {
        return samples.to_vec();
    }
    let ratio = src_rate as f64 / dst_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = pos - idx as f64;
        let s0 = samples.get(idx).copied().unwrap_or(0.0);
        let s1 = samples.get(idx + 1).copied().unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac as f32);
    }
    out
}

#[cfg(feature = "diarize")]
fn resample_mono_i16(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<i16> {
    let resampled_f32 = resample_mono(samples, src_rate, dst_rate);
    resampled_f32
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect()
}

// ---------------------------------------------------------------------------
// Diarization entry point — gated on `diarize` feature
// ---------------------------------------------------------------------------

/// Run speaker diarization on decoded audio.
///
/// Requires the `diarize` feature. Returns `Err` immediately without it.
///
/// Algorithm:
/// 1. Resample to 16 kHz if needed.
/// 2. Run segmentation ONNX model in 10s windows (stride 270 frames at 16kHz).
///    Threshold activations at 0.5 to identify speech frames, group into segments.
/// 3. For each segment >= 1.5s, extract a speaker embedding via pyannote-rs.
/// 4. Cluster embeddings by cosine similarity (threshold 0.4) with running templates.
/// 5. Sort clusters by first-appearance → SPEAKER_0, SPEAKER_1, …
pub fn diarize(
    f32_samples: &[f32],
    i16_samples: &[i16],
    sample_rate: u32,
    seg_model_path: &Path,
    emb_model_path: &Path,
) -> Result<Vec<SpeakerSegment>, String> {
    #[cfg(not(feature = "diarize"))]
    {
        let _ = (f32_samples, i16_samples, sample_rate, seg_model_path, emb_model_path);
        return Err("[diarize] compiled without 'diarize' feature — diarization unavailable".to_string());
    }

    #[cfg(feature = "diarize")]
    diarize_impl(f32_samples, i16_samples, sample_rate, seg_model_path, emb_model_path)
}

#[cfg(feature = "diarize")]
fn diarize_impl(
    f32_samples: &[f32],
    i16_samples: &[i16],
    sample_rate: u32,
    seg_model_path: &Path,
    emb_model_path: &Path,
) -> Result<Vec<SpeakerSegment>, String> {
    const TARGET_SR: u32 = 16000;
    const WINDOW_SECS: f64 = 10.0;
    const STRIDE: usize = 270; // frames at 16kHz
    const SEG_THRESHOLD: f32 = 0.5;
    const MIN_SEG_SECS: f64 = 1.5;
    const COSINE_THRESHOLD: f32 = 0.4;

    eprintln!(
        "[diarize] start: samples={} src_rate={sample_rate}",
        f32_samples.len()
    );

    // --- 1. Resample to 16 kHz ---
    let samples_16k: Vec<f32> = resample_mono(f32_samples, sample_rate, TARGET_SR);
    let i16_16k: Vec<i16> = resample_mono_i16(f32_samples, sample_rate, TARGET_SR);

    let n_total = samples_16k.len();
    let window_size = (WINDOW_SECS * TARGET_SR as f64) as usize;

    if n_total == 0 {
        return Ok(Vec::new());
    }

    // --- 2. Segmentation via ONNX ---
    let session = ort::Session::builder()
        .map_err(|e| format!("[diarize] ort builder: {e}"))?
        .commit_from_file(seg_model_path)
        .map_err(|e| format!("[diarize] load seg model: {e}"))?;

    // Collect speech-active frame indices across all windows.
    let frame_count = n_total / STRIDE;
    let mut is_speech: Vec<bool> = vec![false; frame_count];

    let n_windows = if n_total <= window_size {
        1
    } else {
        (n_total - window_size) / (window_size / 2) + 2
    };

    eprintln!(
        "[diarize] segmentation: n_total={n_total} window_size={window_size} n_windows={n_windows}"
    );

    for win_idx in 0..n_windows {
        let start_sample = win_idx * (window_size / 2);
        if start_sample >= n_total {
            break;
        }
        let end_sample = (start_sample + window_size).min(n_total);
        let win_len = end_sample - start_sample;

        // Pad to window_size with zeros.
        let mut window_data = vec![0.0f32; window_size];
        window_data[..win_len].copy_from_slice(&samples_16k[start_sample..end_sample]);

        // Build tensor [1, 1, window_size].
        let tensor_data =
            ndarray::Array3::from_shape_fn((1, 1, window_size), |(_, _, i)| window_data[i]);
        let tensor = ort::inputs![tensor_data.clone()]
            .map_err(|e| format!("[diarize] ort input: {e}"))?;

        let outputs = session
            .run(tensor)
            .map_err(|e| format!("[diarize] seg model run: {e}"))?;

        // Output: [1, n_frames, n_speakers] — any speaker column > threshold = speech.
        if let Some(output) = outputs.into_values().next() {
            if let Ok(arr) = output.try_extract_tensor::<f32>() {
                let arr = arr.view();
                let shape = arr.shape();
                if shape.len() >= 2 {
                    let n_frames_out = shape[shape.len() - 2];
                    let n_sp = shape[shape.len() - 1];
                    for frame_out in 0..n_frames_out {
                        // Map this output frame back to global frame index.
                        let global_sample = start_sample + frame_out * STRIDE;
                        let global_frame = global_sample / STRIDE;
                        if global_frame >= frame_count {
                            break;
                        }
                        // Speech if any speaker column exceeds threshold.
                        let speech = (0..n_sp).any(|sp| {
                            let idx = if shape.len() == 3 {
                                arr[[0, frame_out, sp]]
                            } else {
                                arr[[frame_out, sp]]
                            };
                            idx > SEG_THRESHOLD
                        });
                        // OR-combine across overlapping windows.
                        is_speech[global_frame] |= speech;
                    }
                }
            }
        }
    }

    // --- Group consecutive speech frames into candidate segments ---
    #[derive(Debug)]
    struct Candidate {
        start_frame: usize,
        end_frame: usize,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut in_speech = false;
    let mut seg_start = 0usize;

    for (i, &speech) in is_speech.iter().enumerate() {
        if speech && !in_speech {
            seg_start = i;
            in_speech = true;
        } else if !speech && in_speech {
            candidates.push(Candidate {
                start_frame: seg_start,
                end_frame: i,
            });
            in_speech = false;
        }
    }
    if in_speech {
        candidates.push(Candidate {
            start_frame: seg_start,
            end_frame: frame_count,
        });
    }

    eprintln!("[diarize] segmentation found {} candidates", candidates.len());

    // Filter by minimum duration.
    let frame_dur = STRIDE as f64 / TARGET_SR as f64;
    let min_frames = (MIN_SEG_SECS / frame_dur).ceil() as usize;

    let candidates: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| c.end_frame.saturating_sub(c.start_frame) >= min_frames)
        .collect();

    eprintln!(
        "[diarize] {} segments >= {MIN_SEG_SECS}s after filter",
        candidates.len()
    );

    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // --- 3. Speaker embeddings ---
    let extractor = pyannote_rs::EmbeddingExtractor::new(emb_model_path)
        .map_err(|e| format!("[diarize] embedding extractor init: {e}"))?;

    struct SegmentEmb {
        start_s: f64,
        end_s: f64,
        embedding: Vec<f32>,
    }

    let mut seg_embs: Vec<SegmentEmb> = Vec::new();

    for cand in &candidates {
        let start_s = cand.start_frame as f64 * frame_dur;
        let end_s = cand.end_frame as f64 * frame_dur;

        let start_sample = (start_s * TARGET_SR as f64) as usize;
        let end_sample = ((end_s * TARGET_SR as f64) as usize).min(i16_16k.len());

        if start_sample >= end_sample {
            continue;
        }

        let chunk = &i16_16k[start_sample..end_sample];

        // Wrap in catch_unwind to handle panics on short/malformed segments.
        let result = std::panic::catch_unwind(|| extractor.extract(chunk));

        match result {
            Ok(Ok(emb)) => {
                eprintln!(
                    "[diarize] embedding extracted start={start_s:.2}s end={end_s:.2}s dims={}",
                    emb.len()
                );
                seg_embs.push(SegmentEmb {
                    start_s,
                    end_s,
                    embedding: emb,
                });
            }
            Ok(Err(e)) => {
                eprintln!(
                    "[diarize] embedding failed for segment [{start_s:.2}, {end_s:.2}]: {e}"
                );
            }
            Err(_) => {
                eprintln!(
                    "[diarize] embedding panicked for segment [{start_s:.2}, {end_s:.2}] — skipping"
                );
            }
        }
    }

    eprintln!("[diarize] {} segments with embeddings", seg_embs.len());

    if seg_embs.is_empty() {
        return Ok(Vec::new());
    }

    // --- 4. Cosine similarity clustering ---
    struct Cluster {
        template: Vec<f32>,
        count: usize,
        first_seen: f64,
    }

    let mut clusters: Vec<Cluster> = Vec::new();
    let mut assignments: Vec<usize> = Vec::new();

    for seg in &seg_embs {
        let emb = &seg.embedding;

        let best = clusters
            .iter()
            .enumerate()
            .map(|(ci, cl)| (ci, cosine_similarity(emb, &cl.template)))
            .filter(|(_, sim)| *sim > COSINE_THRESHOLD)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((best_ci, _)) = best {
            let cl = &mut clusters[best_ci];
            let n = cl.count as f32;
            for (t, &e) in cl.template.iter_mut().zip(emb.iter()) {
                *t = (*t * n + e) / (n + 1.0);
            }
            cl.count += 1;
            assignments.push(best_ci);
        } else {
            let idx = clusters.len();
            clusters.push(Cluster {
                template: emb.clone(),
                count: 1,
                first_seen: seg.start_s,
            });
            assignments.push(idx);
        }
    }

    eprintln!("[diarize] {} speaker clusters found", clusters.len());

    // Sort clusters by first appearance → SPEAKER_0, SPEAKER_1, …
    let mut order: Vec<usize> = (0..clusters.len()).collect();
    order.sort_by(|&a, &b| {
        clusters[a]
            .first_seen
            .partial_cmp(&clusters[b].first_seen)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut cluster_to_speaker = vec![String::new(); clusters.len()];
    for (speaker_n, &ci) in order.iter().enumerate() {
        cluster_to_speaker[ci] = format!("SPEAKER_{speaker_n}");
    }

    let mut result: Vec<SpeakerSegment> = seg_embs
        .iter()
        .zip(assignments.iter())
        .map(|(seg, &ci)| SpeakerSegment {
            speaker: cluster_to_speaker[ci].clone(),
            start: seg.start_s,
            end: seg.end_s,
        })
        .collect();

    result.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

    eprintln!("[diarize] diarization complete: {} segments", result.len());
    Ok(result)
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "diarize")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|&x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
