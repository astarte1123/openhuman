//! Microsoft Teams' [`CallJoinScanner`] implementation.
//!
//! Mirrors [`super::google_meet::GoogleMeetScanner`]'s structure — attach
//! to the Teams CDP target, then walk the join page — but with one extra
//! phase ahead of Meet's three:
//!
//!  0. Dismiss the "Continue on this browser" interstitial. Teams' web
//!     client nudges users toward the native app; guests who stay in the
//!     browser must click through this screen before the join page (device
//!     check / name entry) even renders.
//!  1. Dismiss the device-check screen, if shown (mic/camera permission
//!     prompt or a "Continue without ..." choice — copy unconfirmed, see
//!     below).
//!  2. Type the supplied guest display name into the name input.
//!  3. Click "Join now".
//!
//! All steps go through CDP — there is **no** init-script JS injected
//! into the webview, per the project's CEF child-webview rule (see
//! `CLAUDE.md`). `Runtime.evaluate` (via the shared [`super::common`]
//! helpers) finds candidate elements by visible text / aria-label, and
//! `Input.insertText` injects the display name as a synthetic IME event.
//!
//! ## Unverified — needs a live Teams meeting to confirm
//!
//! **Every text label below is a best-effort guess** based on generally
//! known Teams web-client copy, not confirmed against a real DOM. None of
//! this has been exercised against an actual Teams meeting in this
//! environment. Before this scanner can be trusted:
//!
//!  - Confirm the exact "continue on this browser" interstitial copy and
//!    element shape. If it's rendered as a plain `<a>` **without**
//!    `role="button"`, [`super::common::wait_and_click_text`]'s selector
//!    (`button, [role="button"], a[role="button"]`) will **not** find it —
//!    that helper is shared with the working Meet scanner, so widening it
//!    needs to be a careful, low-risk change made only after confirming
//!    Teams actually needs it.
//!  - Confirm whether dismissing the interstitial navigates to a **new**
//!    CDP target URL (which would break [`wait_for_target`]'s prefix match
//!    against `ctx.call_url`, keyed on the original URL) or stays on the
//!    same target.
//!  - Confirm the device-check and name-entry copy, and whether they are
//!    one page or two separate interstitials (this impl assumes they can
//!    be probed independently, matching Meet's shape).
//!  - Confirm the join button's exact label ("Join now" is assumed here
//!    based on Teams' general web UI; Meet uses "Ask to join").
//!
//! See [`super::common::wait_for_target`] and
//! [`super::common::wait_and_click_text`].

use std::time::Duration;

use super::common::{type_into_named_input, wait_and_click_text, wait_for_target};
use super::{CallJoinScanner, JoinContext};

/// Wait at most this long for CEF to surface the new Teams page target
/// after `WebviewWindowBuilder::build()` returns. Same budget as Meet's
/// `TARGET_DISCOVERY_BUDGET` — no evidence yet that Teams needs a
/// different value.
const TARGET_DISCOVERY_BUDGET: Duration = Duration::from_secs(20);
const TARGET_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);

/// Phase 0 — dismiss the "Continue on this browser" interstitial, if
/// shown. Best-effort: guests who land directly on the join page (e.g. a
/// browser Teams already nudged past this screen for) won't see it at
/// all, so a short budget keeps the join flow from stalling when it's
/// simply absent.
const CONTINUE_IN_BROWSER_BUDGET: Duration = Duration::from_secs(10);

/// Per-phase polling budgets for the remaining phases. Mirrors Meet's
/// values — unconfirmed whether Teams' actual timing differs.
const DEVICE_CHECK_BUDGET: Duration = Duration::from_secs(6);
const NAME_INPUT_BUDGET: Duration = Duration::from_secs(30);
const JOIN_BUTTON_BUDGET: Duration = Duration::from_secs(30);

/// CDP-driven join automation for `teams.microsoft.com` /
/// `teams.live.com`.
pub struct TeamsScanner;

#[async_trait::async_trait]
impl CallJoinScanner for TeamsScanner {
    fn provider_name(&self) -> &'static str {
        "teams"
    }

    async fn run(&self, ctx: &JoinContext) -> Result<(), String> {
        let (mut cdp, session) = wait_for_target(
            &ctx.call_url,
            TARGET_DISCOVERY_BUDGET,
            TARGET_DISCOVERY_INTERVAL,
        )
        .await?;
        log::info!(
            "[meet-scanner:teams] attached to teams target request_id={} session={session}",
            ctx.request_id
        );

        let _ = cdp
            .call("Page.enable", serde_json::json!({}), Some(&session))
            .await;
        let _ = cdp
            .call("Runtime.enable", serde_json::json!({}), Some(&session))
            .await;

        // Phase 0 — dismiss the "continue on this browser" interstitial.
        //
        // UNVERIFIED: exact copy is a best guess. If Teams renders this as
        // a plain `<a>` without `role="button"`, this click will not find
        // it and the phase silently times out (best-effort, matching
        // Meet's device-check phase) — the user would then need to click
        // through manually. See module-level doc.
        if let Err(err) = wait_and_click_text(
            &mut cdp,
            &session,
            &[
                "Continue on this browser",
                "Use the web app instead",
                "Continue without the app",
            ],
            CONTINUE_IN_BROWSER_BUDGET,
        )
        .await
        {
            log::debug!(
                "[meet-scanner:teams] continue-in-browser dismissal not needed or unavailable: {err}"
            );
        }

        // Phase 1 — dismiss the device-check screen, if shown.
        //
        // UNVERIFIED: exact copy is a best guess mirroring Meet's phrasing
        // pattern; Teams may not use this wording at all.
        if let Err(err) = wait_and_click_text(
            &mut cdp,
            &session,
            &[
                "Continue without audio or video",
                "Continue without microphone and camera",
                "Join without audio",
            ],
            DEVICE_CHECK_BUDGET,
        )
        .await
        {
            log::debug!(
                "[meet-scanner:teams] device-check dismissal not needed or unavailable: {err}"
            );
        }

        // Phase 2 — type the display name.
        //
        // UNVERIFIED: "Type your name" is a best guess for the input's
        // aria-label/placeholder hint; Teams' actual copy has not been
        // confirmed.
        type_into_named_input(
            &mut cdp,
            &session,
            "Type your name",
            &ctx.display_name,
            NAME_INPUT_BUDGET,
        )
        .await?;

        // Phase 3 — request to join.
        //
        // UNVERIFIED: "Join now" is Teams' commonly documented label but
        // has not been confirmed against a live meeting in this
        // environment.
        wait_and_click_text(&mut cdp, &session, &["Join now"], JOIN_BUTTON_BUDGET).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_constants_are_sane() {
        // Same guard as Meet's equivalent test: the total scanner budget
        // (discovery + all phases, including Teams' extra phase 0) should
        // stay well under 120 s.
        let total = TARGET_DISCOVERY_BUDGET
            + CONTINUE_IN_BROWSER_BUDGET
            + DEVICE_CHECK_BUDGET
            + NAME_INPUT_BUDGET
            + JOIN_BUTTON_BUDGET;
        assert!(
            total <= Duration::from_secs(120),
            "total scanner budget {total:?} exceeds 120 s — check if constants were accidentally inflated"
        );
    }

    #[test]
    fn provider_name_is_teams() {
        assert_eq!(TeamsScanner.provider_name(), "teams");
    }
}
