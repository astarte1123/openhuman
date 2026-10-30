//! Live meeting transcription driven by the Minutes CLI (`minutes live`).
//!
//! The in-process cpal -> VAD -> whisper pipeline in `recorder.rs` never
//! produced a single transcript chunk on real hardware, while the Minutes
//! engine (MIT, github.com/silverstein/minutes) transcribes Italian and
//! English correctly on the same machine. So recording is delegated to it:
//!
//! 1. `minutes live [-l <lang>] [-D <mic>]` runs as a child process and
//!    appends one JSON line per finalized utterance to its live JSONL.
//! 2. A poll task runs `minutes transcript --since <N> --format json` every
//!    [`POLL_INTERVAL`] and pushes each new line into the core `meetings`
//!    domain via `aura.meetings_append_transcript_chunk`, so the existing
//!    MeetingDetail live view, summary, and exports work unchanged.
//! 3. `minutes stop` finalizes the session; a last drain picks up any lines
//!    written during finalization.
//!
//! Binary lookup: `YOUMA_MINUTES_BIN`, else `%USERPROFILE%\.minutes\bin\minutes.exe`.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::{Child, Command};

const LOG_PREFIX: &str = "[meeting-recorder:minutes]";
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// One line of `minutes transcript --format json` output.
#[derive(Debug, Deserialize)]
struct LiveLine {
    line: u64,
    offset_ms: u64,
    duration_ms: u64,
    text: String,
    speaker: Option<String>,
}

pub fn minutes_binary() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("YOUMA_MINUTES_BIN") {
        let p = PathBuf::from(p.trim());
        if p.is_file() {
            return Ok(p);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "cannot resolve home directory to locate minutes.exe".to_string())?;
    let exe = if cfg!(windows) {
        "minutes.exe"
    } else {
        "minutes"
    };
    let p = PathBuf::from(home).join(".minutes").join("bin").join(exe);
    if p.is_file() {
        Ok(p)
    } else {
        Err(format!(
            "Minutes engine not found at {} (set YOUMA_MINUTES_BIN to override)",
            p.display()
        ))
    }
}

fn command(bin: &PathBuf) -> Command {
    let mut cmd = Command::new(bin);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console flashing up for every poll.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub struct MinutesSession {
    bin: PathBuf,
    child: Child,
    stop_flag: Arc<AtomicBool>,
    poll_task: tauri::async_runtime::JoinHandle<u64>,
    meeting_id: String,
}

/// Spawns `minutes live` and the transcript poll task for `meeting_id`.
pub async fn start(
    meeting_id: String,
    language: Option<String>,
    mic_device_name: Option<String>,
) -> Result<MinutesSession, String> {
    let bin = minutes_binary()?;

    // A stale session from a crashed run would make `live` refuse to start.
    let _ = command(&bin)
        .arg("stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    let mut cmd = command(&bin);
    cmd.arg("live");
    if let Some(lang) = language.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        cmd.args(["-l", lang]);
    }
    if let Some(dev) = mic_device_name
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        cmd.args(["-D", dev]);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    log::info!(
        "{LOG_PREFIX} spawning {} live language={language:?} mic={mic_device_name:?} meeting_id={meeting_id}",
        bin.display()
    );
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to start Minutes engine: {e}"))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let poll_task = tauri::async_runtime::spawn(poll_loop(
        bin.clone(),
        meeting_id.clone(),
        stop_flag.clone(),
    ));

    Ok(MinutesSession {
        bin,
        child,
        stop_flag,
        poll_task,
        meeting_id,
    })
}

/// Stops the Minutes session and drains any remaining transcript lines.
pub async fn stop(mut session: MinutesSession) {
    session.stop_flag.store(true, Ordering::SeqCst);
    let last_line = session.poll_task.await.unwrap_or(0);

    log::info!(
        "{LOG_PREFIX} stopping live session meeting_id={}",
        session.meeting_id
    );
    match command(&session.bin)
        .arg("stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(status) => log::info!("{LOG_PREFIX} minutes stop exited status={status}"),
        Err(err) => log::warn!("{LOG_PREFIX} minutes stop failed: {err}"),
    }

    if tokio::time::timeout(Duration::from_secs(10), session.child.wait())
        .await
        .is_err()
    {
        log::warn!("{LOG_PREFIX} live process did not exit after stop; killing");
        let _ = session.child.kill().await;
    }

    let pushed = drain_once(&session.bin, &session.meeting_id, last_line).await;
    log::info!(
        "{LOG_PREFIX} final drain meeting_id={} last_line={pushed}",
        session.meeting_id
    );
}

async fn poll_loop(bin: PathBuf, meeting_id: String, stop_flag: Arc<AtomicBool>) -> u64 {
    let mut last_line = 0u64;
    while !stop_flag.load(Ordering::SeqCst) {
        tokio::time::sleep(POLL_INTERVAL).await;
        last_line = drain_once(&bin, &meeting_id, last_line).await;
    }
    last_line
}

/// Reads lines after `since` and pushes them; returns the highest line seen.
async fn drain_once(bin: &PathBuf, meeting_id: &str, since: u64) -> u64 {
    let output = match command(bin)
        .args([
            "transcript",
            "--since",
            &since.to_string(),
            "--format",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    {
        Ok(o) => o,
        Err(err) => {
            log::warn!("{LOG_PREFIX} transcript read failed: {err}");
            return since;
        }
    };

    let mut last = since;
    for raw in String::from_utf8_lossy(&output.stdout).lines() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Ok(line) = serde_json::from_str::<LiveLine>(raw) else {
            continue;
        };
        if line.line <= last {
            continue;
        }
        last = line.line;
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        let params = serde_json::json!({
            "meeting_id": meeting_id,
            "start_ms": line.offset_ms as i64,
            "end_ms": (line.offset_ms + line.duration_ms) as i64,
            "text": text,
            "speaker": line.speaker,
        });
        match super::recorder::rpc_call("aura.meetings_append_transcript_chunk", params).await {
            Ok(_) => log::info!(
                "{LOG_PREFIX} pushed line={} chars={} meeting_id={meeting_id}",
                line.line,
                text.len()
            ),
            Err(err) => log::warn!(
                "{LOG_PREFIX} append_transcript_chunk failed line={}: {err}",
                line.line
            ),
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes_transcript_json_line() {
        let raw = r#"{"line":1,"ts":"2026-09-23T17:09:32.926672800+02:00","offset_ms":21318,"duration_ms":1200,"text":"Buongiorno a tutti.","speaker":null}"#;
        let line: LiveLine = serde_json::from_str(raw).unwrap();
        assert_eq!(line.line, 1);
        assert_eq!(line.offset_ms, 21318);
        assert_eq!(line.duration_ms, 1200);
        assert_eq!(line.text, "Buongiorno a tutti.");
        assert!(line.speaker.is_none());
    }
}
