use std::path::PathBuf;
use std::time::Instant;

use log::{debug, warn};

use crate::aura::config::Config;
use crate::aura::inference::model_ids;
use crate::aura::inference::paths::{
    config_root_dir, resolve_piper_binary, resolve_stt_model_path, resolve_tts_voice_path,
    resolve_whisper_binary,
};
use crate::aura::inference::types::{LocalAiSpeechResult, LocalAiTtsResult};

use super::whisper_engine;
use super::LocalAiService;

const LOG_PREFIX: &str = "[speech]";

impl LocalAiService {
    pub async fn transcribe(
        &self,
        config: &Config,
        audio_path: &str,
    ) -> Result<LocalAiSpeechResult, String> {
        self.transcribe_with_prompt(config, audio_path, None).await
    }

    /// Ensures the in-process whisper engine is loaded, loading it lazily
    /// (serialized via `whisper_load_lock` so concurrent callers don't spawn
    /// duplicate heavy contexts) if it isn't already. Returns `Ok(())` once
    /// `whisper_engine::is_loaded` is true, or an error describing why
    /// loading didn't happen (STT model path unresolved, or the load itself
    /// failing).
    ///
    /// Unlike [`transcribe_with_prompt`](Self::transcribe_with_prompt), this
    /// does **not** gate on `config.local_ai.whisper_in_process` — that flag
    /// only controls whether *dictation* prefers the in-process engine over
    /// its CLI-subprocess fallback. Meeting-capture transcription
    /// (`meetings::capture::pipeline::flush_and_transcribe`) has no such
    /// fallback: it calls `whisper_engine::transcribe_pcm_f32` directly on
    /// raw PCM, so it needs the engine loaded unconditionally before a
    /// capture session starts, or every chunk fails transcription silently
    /// for the whole meeting (see `meetings::ops::meetings_start_local_capture`,
    /// which calls this before starting capture; the dictation call site in
    /// [`transcribe_with_prompt`](Self::transcribe_with_prompt) keeps its own
    /// `whisper_in_process` gate around the call).
    pub async fn ensure_whisper_loaded(&self, config: &Config) -> Result<(), String> {
        if whisper_engine::is_loaded(&self.whisper) {
            return Ok(());
        }
        let lazy_load_started = Instant::now();
        let _load_guard = self.whisper_load_lock.lock().await;
        if whisper_engine::is_loaded(&self.whisper) {
            return Ok(());
        }

        let model_path = resolve_stt_model_path(config)
            .map_err(|e| format!("STT model path not resolved: {e}"))?;
        let handle = self.whisper.clone();
        let model = PathBuf::from(model_path);
        debug!("{LOG_PREFIX} whisper in-process unloaded; loading model lazily");
        // Detect GPU at lazy-load time so whisper can use acceleration.
        let device = crate::aura::inference::device::detect_device_profile();
        let gpu = device.has_gpu;
        let gpu_desc = device.gpu_description.clone();
        tokio::task::spawn_blocking(move || {
            whisper_engine::load_engine(&handle, &model, gpu, gpu_desc.as_deref())
        })
        .await
        .map_err(|e| format!("whisper load task join error: {e}"))?
        .map_err(|e| format!("whisper load failed: {e}"))?;

        debug!(
            "{LOG_PREFIX} lazy in-process whisper load complete (elapsed_ms={})",
            lazy_load_started.elapsed().as_millis()
        );
        Ok(())
    }

    /// Transcribe audio with an optional initial_prompt for vocabulary bias.
    ///
    /// The `initial_prompt` is passed to whisper.cpp's `initial_prompt` parameter,
    /// biasing the decoder toward the supplied words/phrases. Used for custom
    /// dictionary support and conversational continuity.
    pub async fn transcribe_with_prompt(
        &self,
        config: &Config,
        audio_path: &str,
        initial_prompt: Option<&str>,
    ) -> Result<LocalAiSpeechResult, String> {
        let started = Instant::now();
        if !config.local_ai.runtime_enabled {
            return Err("local ai is disabled".to_string());
        }

        // Lazily load in-process whisper engine when enabled. `ensure_whisper_loaded`
        // serializes concurrent load attempts (via `whisper_load_lock`) so
        // concurrent requests do not spawn duplicate heavy contexts. A load
        // failure here is non-fatal — this path falls back to the CLI
        // subprocess below — so it's only logged, not propagated.
        if config.local_ai.whisper_in_process {
            if let Err(e) = self.ensure_whisper_loaded(config).await {
                warn!("{LOG_PREFIX} lazy in-process whisper load failed: {e}");
            }
        }

        // Try in-process whisper engine first (offloaded to a blocking thread).
        if whisper_engine::is_loaded(&self.whisper) {
            debug!("{LOG_PREFIX} using in-process whisper engine for {audio_path}");
            let handle = self.whisper.clone();
            let path = audio_path.to_string();
            let prompt_owned = initial_prompt.map(String::from);
            let lang_hint = whisper_language_hint(config);
            let in_process_started = Instant::now();
            let result = tokio::task::spawn_blocking(move || {
                Self::transcribe_in_process_inner(
                    &handle,
                    &path,
                    prompt_owned.as_deref(),
                    lang_hint.as_deref(),
                )
            })
            .await
            .map_err(|e| format!("whisper task join error: {e}"))?;
            match result {
                Ok(text) => {
                    debug!(
                        "{LOG_PREFIX} in-process transcription complete (elapsed_ms={}, total_elapsed_ms={})",
                        in_process_started.elapsed().as_millis(),
                        started.elapsed().as_millis()
                    );
                    self.status.lock().stt_state = "ready".to_string();
                    return Ok(LocalAiSpeechResult {
                        text,
                        model_id: model_ids::effective_stt_model_id(config),
                    });
                }
                Err(e) => {
                    warn!("{LOG_PREFIX} in-process transcription failed, falling back to CLI: {e}");
                }
            }
        }

        // Fallback: subprocess per call (original behavior).
        debug!("{LOG_PREFIX} using whisper-cli subprocess for {audio_path}");
        let subprocess_started = Instant::now();
        let result = self.transcribe_subprocess(config, audio_path).await;
        debug!(
            "{LOG_PREFIX} subprocess transcription finished (elapsed_ms={}, total_elapsed_ms={})",
            subprocess_started.elapsed().as_millis(),
            started.elapsed().as_millis()
        );
        result
    }

    /// Transcribe using the in-process whisper-rs engine. Runs on a blocking
    /// thread — takes the engine handle directly so it can be `Send`.
    fn transcribe_in_process_inner(
        handle: &whisper_engine::WhisperEngineHandle,
        audio_path: &str,
        initial_prompt: Option<&str>,
        language: Option<&str>,
    ) -> Result<String, String> {
        let path = std::path::Path::new(audio_path);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let result = if ext == "wav" {
            whisper_engine::transcribe_wav_file(handle, path, language, initial_prompt)?
        } else {
            warn!(
                "{LOG_PREFIX} non-WAV input ({ext}), attempting WAV decode anyway \
                 (may fail — use ffmpeg conversion for best results)"
            );
            whisper_engine::transcribe_wav_file(handle, path, language, initial_prompt)?
        };

        debug!(
            "{LOG_PREFIX} in-process result: avg_logprob={:.3}, segments={}/{}",
            result.avg_logprob.unwrap_or(0.0),
            result.segments_accepted,
            result.segments_total
        );
        Ok(result.text)
    }

    /// Original subprocess-based transcription via whisper-cli.
    async fn transcribe_subprocess(
        &self,
        config: &Config,
        audio_path: &str,
    ) -> Result<LocalAiSpeechResult, String> {
        let whisper_bin = resolve_whisper_binary().ok_or_else(|| {
            "whisper.cpp binary not found. Set WHISPER_BIN or install whisper-cli.".to_string()
        })?;
        let model_path = resolve_stt_model_path(config)?;
        let mut args = vec!["-m".to_string(), model_path, "-f".to_string(), audio_path.to_string()];
        if let Some(lang) = whisper_language_hint(config) {
            args.push("-l".to_string());
            args.push(lang);
        }
        let output = tokio::process::Command::new(whisper_bin)
            .args(&args)
            .output()
            .await
            .map_err(|e| format!("failed to run whisper.cpp: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "whisper.cpp failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return Err("whisper.cpp returned empty transcript".to_string());
        }
        self.status.lock().stt_state = "ready".to_string();
        Ok(LocalAiSpeechResult {
            text,
            model_id: model_ids::effective_stt_model_id(config),
        })
    }

    pub async fn tts(
        &self,
        config: &Config,
        text: &str,
        output_path: Option<&str>,
    ) -> Result<LocalAiTtsResult, String> {
        if !config.local_ai.runtime_enabled {
            return Err("local ai is disabled".to_string());
        }
        let piper_bin = resolve_piper_binary()
            .ok_or_else(|| "piper binary not found. Set PIPER_BIN or install piper.".to_string())?;
        let model_path = resolve_tts_voice_path(config)?;
        let out_path = output_path
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| {
                config_root_dir(config)
                    .join("models")
                    .join("local-ai")
                    .join("tts-output.wav")
                    .display()
                    .to_string()
            });
        let parent = PathBuf::from(&out_path)
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "invalid output_path".to_string())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create TTS output directory: {e}"))?;

        let mut child = tokio::process::Command::new(piper_bin)
            .args(["--model", &model_path, "--output_file", &out_path])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to launch piper: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(text.as_bytes())
                .await
                .map_err(|e| format!("failed to write text to piper stdin: {e}"))?;
        }
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("failed to wait for piper: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "piper failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        self.status.lock().tts_state = "ready".to_string();
        Ok(LocalAiTtsResult {
            output_path: out_path,
            voice_id: model_ids::effective_tts_voice_id(config),
        })
    }
}

/// Derive a whisper.cpp language hint (ISO-639-1, e.g. `"it"`) from the
/// app's configured locale. Returns `None` when no locale is configured,
/// letting whisper auto-detect instead of silently defaulting to English —
/// see `whisper_engine::transcribe_pcm_f32`.
fn whisper_language_hint(config: &Config) -> Option<String> {
    let locale = config.locale.as_deref()?;
    let resolved = crate::aura::locale::resolve_supported_locale(Some(locale));
    resolved.split(['-', '_']).next().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aura::config::TEST_ENV_LOCK;
    use tempfile::TempDir;

    // `ensure_whisper_loaded`'s happy path (an actual whisper.cpp model
    // loading successfully) needs real model files and can't be
    // meaningfully unit tested here — see `whisper_engine::load_engine`'s
    // own doc comment. These tests cover the error path (no model
    // resolvable) and that the `whisper_load_lock` serialization doesn't
    // deadlock concurrent callers.
    //
    // `resolve_stt_model_path` falls back to the *shared* `~/.aura` root
    // (`inference::paths::shared_root_dir`) whenever `AURA_WORKSPACE` isn't
    // set, not `config.workspace_dir` — so on a dev machine that already
    // has a whisper model installed there, a bare `Config::default()`
    // would actually resolve it. Set `AURA_WORKSPACE` to an empty temp dir
    // (guarded by `TEST_ENV_LOCK`, since env mutation isn't test-isolated)
    // so these tests see "no model installed" regardless of the host.
    fn isolated_config(tmp: &TempDir) -> Config {
        let mut config = Config::default();
        config.workspace_dir = tmp.path().join("workspace");
        config.config_path = tmp.path().join("config.toml");
        config
    }

    #[tokio::test]
    async fn ensure_whisper_loaded_errors_when_model_path_unresolved() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        // SAFETY: guarded by TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("AURA_WORKSPACE", tmp.path());
        }

        let config = isolated_config(&tmp);
        let svc = LocalAiService::new(&config);

        let err = svc.ensure_whisper_loaded(&config).await.unwrap_err();
        assert!(
            err.contains("STT model path not resolved"),
            "unexpected error: {err}"
        );
        assert!(!whisper_engine::is_loaded(&svc.whisper));

        // SAFETY: guarded by TEST_ENV_LOCK.
        unsafe {
            std::env::remove_var("AURA_WORKSPACE");
        }
    }

    #[tokio::test]
    async fn ensure_whisper_loaded_serializes_concurrent_callers_without_deadlock() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        // SAFETY: guarded by TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("AURA_WORKSPACE", tmp.path());
        }

        let config = isolated_config(&tmp);
        let svc = std::sync::Arc::new(LocalAiService::new(&config));

        let (r1, r2) = tokio::join!(
            svc.ensure_whisper_loaded(&config),
            svc.ensure_whisper_loaded(&config),
        );
        assert!(r1.is_err());
        assert!(r2.is_err());

        // SAFETY: guarded by TEST_ENV_LOCK.
        unsafe {
            std::env::remove_var("AURA_WORKSPACE");
        }
    }
}
