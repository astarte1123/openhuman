//! CDP-driven call-join automation, dispatched per platform.
//!
//! Runs once per call, after the shell has successfully built the
//! dedicated CEF webview for the call URL. [`spawn`] picks a
//! [`CallJoinScanner`] implementation for the URL's platform and drives
//! it: connects to CEF's browser-level WebSocket, attaches to the new
//! call target, and walks through that platform's join page.
//!
//! Two implementations exist today: [`google_meet::GoogleMeetScanner`] and
//! [`teams::TeamsScanner`]. Meet's three phases (dismiss device-check,
//! type display name, click join) live entirely inside its impl; Teams
//! adds an extra phase 0 (dismiss the "continue on this browser"
//! interstitial) ahead of the same device-check/name/join shape — the
//! [`CallJoinScanner`] trait intentionally does not impose a fixed phase
//! shape, since each platform may need a different number/order of steps.
//!
//! All steps go through CDP from the scanner side — there is **no**
//! init-script JS injected into the webview. `Runtime.evaluate` is used
//! to find candidate elements by visible text / aria-label, and
//! `Input.insertText` to inject the display name as a synthetic IME
//! event so a React-controlled `<input>` actually picks it up.
//!
//! Every implementation's join sequence is best-effort: if any phase
//! times out, it logs and bails without crashing the window — the user
//! can finish joining manually. Future work: emit lifecycle events back
//! to the frontend so the UI can show "asking host…" / "joined" status.
//!
//! ## Cancellation
//!
//! [`spawn`] returns a [`tokio::task::AbortHandle`] that the caller must
//! store and abort when the associated call window is closing. Without
//! cancellation the scanner's CDP polling loops keep WebSocket
//! connections open to the CEF debugging endpoint. CEF waits for all
//! active CDP sessions to detach before completing renderer shutdown, so
//! an un-cancelled scanner delays the [`tauri::WindowEvent::Destroyed`]
//! event — and therefore the `meet-call:closed` frontend event — by up
//! to the scanner's full budget. See
//! [`crate::meet_call::meet_call_close_window`] for the abort site.

mod common;
mod google_meet;
mod teams;

use tauri::{AppHandle, Runtime};

/// Per-call context handed to a [`CallJoinScanner::run`] implementation.
///
/// Replaces the loose `(request_id, meet_url, display_name)` positional
/// args the pre-generalisation `run()` free function took.
pub struct JoinContext {
    pub request_id: String,
    pub call_url: String,
    pub display_name: String,
}

/// A platform-specific CDP-driven join automation.
///
/// Deliberately a single entry point (`run`) rather than fixed lifecycle
/// methods like `dismiss_device_check()` / `enter_name()` / `click_join()`
/// — that would impose Google Meet's exact 3-phase shape on every future
/// provider. Each implementation owns its own internal phase sequencing,
/// calling into the shared [`common`] CDP helpers as needed.
#[async_trait::async_trait]
pub trait CallJoinScanner: Send + Sync {
    /// Short identifier used in log lines, e.g. `"google-meet"`.
    fn provider_name(&self) -> &'static str;

    /// Drive the join automation for this platform. Best-effort: an
    /// `Err` is logged by the caller and does not crash the call window.
    async fn run(&self, ctx: &JoinContext) -> Result<(), String>;
}

/// Spawn the CDP-driven join automation for `meet_url`'s platform and
/// return an abort handle.
///
/// The caller **must** call [`tokio::task::AbortHandle::abort`] on the
/// returned handle when the call window is being torn down. Without
/// cancellation the scanner's polling loops hold CDP connections open and
/// delay CEF renderer shutdown. See the module-level doc for details.
///
/// `meet_url` is the exact normalised URL the window was navigated to;
/// implementations use it as a target-URL prefix so two concurrent calls
/// each attach to their own CEF target instead of cross-controlling.
///
/// Platform selection goes through
/// [`aura_core::aura::meet::ops::detect_platform`]. `meet_call`'s caller
/// rejects any URL that isn't `meet.google.com`, `teams.microsoft.com`,
/// or `teams.live.com` before calling `spawn`, so this resolves to either
/// [`google_meet::GoogleMeetScanner`] or [`teams::TeamsScanner`] in
/// practice — the `None` fallback exists so future blocks can add match
/// arms for new platforms without touching this function's shape.
pub fn spawn<R: Runtime>(
    _app: AppHandle<R>,
    request_id: String,
    meet_url: String,
    display_name: String,
) -> tokio::task::AbortHandle {
    // Use tokio::spawn (not tauri::async_runtime::spawn) so we get a
    // JoinHandle whose abort_handle() we can return to the caller.
    let handle = tokio::spawn(async move {
        let scanner = select_scanner(&meet_url);
        let provider = scanner.provider_name();
        let ctx = JoinContext {
            request_id: request_id.clone(),
            call_url: meet_url,
            display_name,
        };
        match scanner.run(&ctx).await {
            Ok(()) => log::info!(
                "[meet-scanner] join sequence completed provider={provider} request_id={request_id}"
            ),
            Err(err) => log::warn!(
                "[meet-scanner] join sequence aborted provider={provider} request_id={request_id} err={err}"
            ),
        }
    });
    handle.abort_handle()
}

/// Pick the [`CallJoinScanner`] for `call_url`'s platform.
///
/// Delegates host/scheme detection to the core (`src/aura/meet/ops.rs`)
/// so URL-shape knowledge lives in the domain, not the shell. Falls back
/// to [`google_meet::GoogleMeetScanner`] for an unrecognised URL rather
/// than panicking — `meet_call::meet_call_open_window` already rejects
/// non-`meet.google.com` URLs before `spawn` is ever called, so this
/// fallback is unreachable in practice today, but keeps `spawn` infallible
/// (no `Option`/`Result` in its signature) for future callers.
fn select_scanner(call_url: &str) -> Box<dyn CallJoinScanner> {
    let platform = url::Url::parse(call_url)
        .ok()
        .and_then(|url| aura_core::aura::meet::ops::detect_platform(&url));
    match platform {
        Some(aura_core::aura::meet::ops::CallPlatform::MicrosoftTeams) => {
            Box::new(teams::TeamsScanner)
        }
        Some(aura_core::aura::meet::ops::CallPlatform::GoogleMeet) | None => {
            Box::new(google_meet::GoogleMeetScanner)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_scanner_resolves_google_meet() {
        let scanner = select_scanner("https://meet.google.com/abc-defg-hij");
        assert_eq!(scanner.provider_name(), "google-meet");
    }

    #[test]
    fn select_scanner_resolves_teams_microsoft() {
        let scanner = select_scanner("https://teams.microsoft.com/l/meetup-join/abc");
        assert_eq!(scanner.provider_name(), "teams");
    }

    #[test]
    fn select_scanner_resolves_teams_live() {
        let scanner = select_scanner("https://teams.live.com/meet/abc");
        assert_eq!(scanner.provider_name(), "teams");
    }

    #[test]
    fn select_scanner_falls_back_to_google_meet_for_unrecognised_url() {
        // meet_call rejects non-meet.google.com URLs before spawn() is
        // ever called, so this path is unreachable today — but spawn()
        // must stay infallible, so the fallback must not panic.
        let scanner = select_scanner("https://example.com/whatever");
        assert_eq!(scanner.provider_name(), "google-meet");
    }

    #[tokio::test]
    async fn abort_handle_cancels_spawned_task() {
        // spawn a long-running task, abort it immediately, and assert it
        // was cancelled before completing normally.
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        use std::time::Duration;

        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            completed_clone.store(true, Ordering::SeqCst);
        });
        let abort = handle.abort_handle();

        abort.abort();

        // Give the runtime a tick to process the abort.
        tokio::task::yield_now().await;

        assert!(
            !completed.load(Ordering::SeqCst),
            "task must not have completed after abort"
        );
        assert!(
            handle.await.unwrap_err().is_cancelled(),
            "awaited task must report cancellation"
        );
    }
}
