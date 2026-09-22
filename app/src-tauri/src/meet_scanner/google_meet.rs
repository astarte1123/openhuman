//! Google Meet's [`CallJoinScanner`] implementation.
//!
//! This is today's join automation, unchanged in behavior, reshaped as a
//! trait impl: attach to the Meet CDP target, then walk through the join
//! page in three phases:
//!
//!  1. Dismiss the device-check ("Continue without microphone and camera").
//!  2. Type the supplied guest display name into the "Your name" input.
//!  3. Click "Ask to join".
//!
//! All steps go through CDP — there is **no** init-script JS injected
//! into the webview. `Runtime.evaluate` finds candidate elements by
//! visible text / aria-label, and `Input.insertText` injects the display
//! name as a synthetic IME event so Meet's React-controlled `<input>`
//! actually picks it up.
//!
//! The whole sequence is best-effort: if any phase times out we log and
//! bail without crashing the window — the user can finish joining
//! manually.

use std::time::Duration;

use super::common::{type_into_named_input, wait_and_click_text, wait_for_target};
use super::{CallJoinScanner, JoinContext};

/// Wait at most this long for CEF to surface the new Meet page target
/// after `WebviewWindowBuilder::build()` returns. CEF lazy-creates the
/// renderer-side target a few hundred ms after the host-side window is
/// ready.
const TARGET_DISCOVERY_BUDGET: Duration = Duration::from_secs(20);
const TARGET_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);

/// Per-phase polling budgets. With the mascot fake-camera flag set
/// process-wide in `lib.rs`, Meet sees a "real" webcam and does NOT
/// show the "Continue without microphone and camera" screen at all,
/// so the device-check phase becomes a quick best-effort probe rather
/// than a meaningful wait. We still keep the phase in case a future
/// build runs without the fake-camera flag (or the Y4M failed to
/// rasterize), but cap it tight so the join flow doesn't stall.
const DEVICE_CHECK_BUDGET: Duration = Duration::from_secs(6);
const NAME_INPUT_BUDGET: Duration = Duration::from_secs(30);
const JOIN_BUTTON_BUDGET: Duration = Duration::from_secs(30);

/// CDP-driven join automation for `meet.google.com`.
pub struct GoogleMeetScanner;

#[async_trait::async_trait]
impl CallJoinScanner for GoogleMeetScanner {
    fn provider_name(&self) -> &'static str {
        "google-meet"
    }

    async fn run(&self, ctx: &JoinContext) -> Result<(), String> {
        let (mut cdp, session) = wait_for_target(
            &ctx.call_url,
            TARGET_DISCOVERY_BUDGET,
            TARGET_DISCOVERY_INTERVAL,
        )
        .await?;
        log::info!(
            "[meet-scanner] attached to meet target request_id={} session={session}",
            ctx.request_id
        );

        // `Runtime.enable` is required before `Runtime.evaluate` returns
        // structured results in some CEF builds. `Page.enable` is harmless
        // and gives us frame-lifecycle events for free if a future PR wants
        // them. Both are best-effort — if they fail we still try to
        // evaluate.
        let _ = cdp.call("Page.enable", serde_json::json!({}), Some(&session)).await;
        let _ = cdp
            .call("Runtime.enable", serde_json::json!({}), Some(&session))
            .await;

        // Phase 1 — dismiss the device-check screen.
        //
        // Meet's exact copy varies by region/A-B test; we try the canonical
        // English variants. The button is usually `[role="button"]` not
        // `<button>`, so `wait_and_click_text` looks at both.
        if let Err(err) = wait_and_click_text(
            &mut cdp,
            &session,
            &[
                "Continue without microphone and camera",
                "Continue without microphone",
                "Continue without camera",
            ],
            DEVICE_CHECK_BUDGET,
        )
        .await
        {
            log::info!("[meet-scanner] device-check dismissal not needed or unavailable: {err}");
        }

        // Phase 2 — type the display name.
        type_into_named_input(
            &mut cdp,
            &session,
            "Your name",
            &ctx.display_name,
            NAME_INPUT_BUDGET,
        )
        .await?;

        // Phase 3 — request to join.
        wait_and_click_text(
            &mut cdp,
            &session,
            &["Ask to join", "Join now"],
            JOIN_BUTTON_BUDGET,
        )
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_constants_are_sane() {
        // The total scanner budget (discovery + phases) should stay well
        // under 120 s so it never outlasts a full meet session or a build
        // timeout. This assertion catches accidental inflation.
        let total =
            TARGET_DISCOVERY_BUDGET + DEVICE_CHECK_BUDGET + NAME_INPUT_BUDGET + JOIN_BUTTON_BUDGET;
        assert!(
            total <= Duration::from_secs(120),
            "total scanner budget {total:?} exceeds 120 s — check if constants were accidentally inflated"
        );
    }

    #[test]
    fn provider_name_is_google_meet() {
        assert_eq!(GoogleMeetScanner.provider_name(), "google-meet");
    }
}
