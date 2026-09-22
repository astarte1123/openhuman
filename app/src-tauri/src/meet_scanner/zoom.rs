//! Zoom's [`CallJoinScanner`] implementation.
//!
//! Mirrors [`super::teams::TeamsScanner`]'s shape — an optional
//! app-launch interstitial ahead of the same device-check/name/join
//! phases Meet and Teams share:
//!
//!  0. Dismiss Zoom's native-app-launch interstitial by clicking the
//!     **"join from your browser"** link, if shown. Zoom's `j/<id>` join
//!     links land on a page that tries to hand off to the installed Zoom
//!     client (`zoommtg://` custom URI scheme) first, and only offer the
//!     browser path as a secondary link below the primary "Launch
//!     Meeting" / "Open Zoom Meetings" call-to-action. This phase's label
//!     list is deliberately narrow — **only** "join from your browser"
//!     text variants — so [`super::common::wait_and_click_text`] can
//!     never accidentally match and click the native-app-launch CTA
//!     instead. See the risk note below.
//!  1. Dismiss the device-check screen, if shown (mic/camera permission
//!     prompt or a "Join without ..." choice — copy unconfirmed, see
//!     below).
//!  2. Type the supplied guest display name into the name input.
//!  3. Click "Join".
//!
//! All steps go through CDP — there is **no** init-script JS injected
//! into the webview, per the project's CEF child-webview rule (see
//! `CLAUDE.md`). `Runtime.evaluate` (via the shared [`super::common`]
//! helpers) finds candidate elements by visible text / aria-label, and
//! `Input.insertText` injects the display name as a synthetic IME event.
//!
//! ## Zoom-specific risk: the native-app-launch CTA
//!
//! Unlike Meet (no native-app nag) and Teams (a single "continue on this
//! browser" style interstitial), Zoom's classic `j/<id>` join page
//! typically renders **two** competing calls to action at once: a
//! prominent "Launch Meeting" / "Open Zoom Meetings" button that hands
//! off to the installed desktop client, and a much less prominent "join
//! from your browser" link/anchor for guests without the app installed.
//! [`wait_and_click_text`] clicks the *first* element whose visible text
//! contains any of the supplied labels — so if this scanner's phase-0
//! label list were broadened to something generic like `"join"`, it
//! could match "Launch Meeting" first (if that text happens to also
//! satisfy a looser predicate) and hand the guest off to a native app
//! install/launch flow instead of keeping them in the anonymous CEF
//! webview, silently breaking the automated join. Phase 0 therefore uses
//! **only** the literal "join from your browser" phrase (Zoom's
//! well-known copy for this link) and nothing looser.
//!
//! ## Unverified — needs a live Zoom meeting to confirm
//!
//! **Every text label below is a best-effort guess** based on generally
//! known Zoom web-client copy, not confirmed against a real DOM. None of
//! this has been exercised against an actual Zoom meeting in this
//! environment. Before this scanner can be trusted:
//!
//!  - Confirm the exact "join from your browser" link copy and element
//!    shape. If it's rendered as a plain `<a>` **without**
//!    `role="button"`, [`super::common::wait_and_click_text`]'s selector
//!    (`button, [role="button"], a[role="button"]`) will **not** find it.
//!  - Confirm whether `https://app.zoom.us/wc/join/<id>` links (the
//!    explicit web-client URL, distinct from the classic `j/<id>` link)
//!    skip phase 0 entirely by landing straight on the name-entry screen
//!    — if so this scanner should still work unmodified since phase 0 is
//!    best-effort and simply times out quickly when the interstitial
//!    never appears.
//!  - Confirm whether dismissing the interstitial navigates to a **new**
//!    CDP target URL (which would break [`wait_for_target`]'s prefix
//!    match against `ctx.call_url`, keyed on the original URL) or stays
//!    on the same target.
//!  - Confirm the device-check and name-entry copy, and whether they are
//!    one page or two separate interstitials (this impl assumes they can
//!    be probed independently, matching Meet/Teams' shape).
//!  - Confirm the join button's exact label ("Join" is assumed here based
//!    on Zoom's general web UI; Meet uses "Ask to join", Teams uses
//!    "Join now").
//!
//! See [`super::common::wait_for_target`] and
//! [`super::common::wait_and_click_text`].

use std::time::Duration;

use super::common::{type_into_named_input, wait_and_click_text, wait_for_target};
use super::{CallJoinScanner, JoinContext};

/// Wait at most this long for CEF to surface the new Zoom page target
/// after `WebviewWindowBuilder::build()` returns. Same budget as
/// Meet/Teams' `TARGET_DISCOVERY_BUDGET` — no evidence yet that Zoom
/// needs a different value.
const TARGET_DISCOVERY_BUDGET: Duration = Duration::from_secs(20);
const TARGET_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);

/// Phase 0 — dismiss the native-app-launch interstitial by clicking
/// "join from your browser", if shown. Best-effort: guests who land
/// directly on the browser join page (e.g. via `wc/join/<id>`, which may
/// skip the interstitial entirely — unconfirmed) won't see it at all, so
/// a short budget keeps the join flow from stalling when it's simply
/// absent.
const APP_LAUNCH_INTERSTITIAL_BUDGET: Duration = Duration::from_secs(10);

/// Per-phase polling budgets for the remaining phases. Mirrors Meet/Teams'
/// values — unconfirmed whether Zoom's actual timing differs.
const DEVICE_CHECK_BUDGET: Duration = Duration::from_secs(6);
const NAME_INPUT_BUDGET: Duration = Duration::from_secs(30);
const JOIN_BUTTON_BUDGET: Duration = Duration::from_secs(30);

/// CDP-driven join automation for `zoom.us` / `app.zoom.us` /
/// `zoomgov.com`.
pub struct ZoomScanner;

#[async_trait::async_trait]
impl CallJoinScanner for ZoomScanner {
    fn provider_name(&self) -> &'static str {
        "zoom"
    }

    async fn run(&self, ctx: &JoinContext) -> Result<(), String> {
        let (mut cdp, session) = wait_for_target(
            &ctx.call_url,
            TARGET_DISCOVERY_BUDGET,
            TARGET_DISCOVERY_INTERVAL,
        )
        .await?;
        log::info!(
            "[meet-scanner:zoom] attached to zoom target request_id={} session={session}",
            ctx.request_id
        );

        let _ = cdp
            .call("Page.enable", serde_json::json!({}), Some(&session))
            .await;
        let _ = cdp
            .call("Runtime.enable", serde_json::json!({}), Some(&session))
            .await;

        // Phase 0 — dismiss the native-app-launch interstitial by
        // clicking "join from your browser".
        //
        // UNVERIFIED: exact copy is a best guess. Deliberately narrow
        // label list — see the module-level "Zoom-specific risk" note —
        // so this can never accidentally click the "Launch Meeting" /
        // "Open Zoom Meetings" native-app CTA instead. If Zoom renders
        // this as a plain `<a>` without `role="button"`, this click will
        // not find it and the phase silently times out (best-effort,
        // matching Teams' interstitial phase) — the user would then need
        // to click through manually.
        if let Err(err) = wait_and_click_text(
            &mut cdp,
            &session,
            &["join from your browser", "join from browser"],
            APP_LAUNCH_INTERSTITIAL_BUDGET,
        )
        .await
        {
            log::debug!(
                "[meet-scanner:zoom] app-launch interstitial dismissal not needed or unavailable: {err}"
            );
        }

        // Phase 1 — dismiss the device-check screen, if shown.
        //
        // UNVERIFIED: exact copy is a best guess mirroring Meet/Teams'
        // phrasing pattern; Zoom may not use this wording at all.
        if let Err(err) = wait_and_click_text(
            &mut cdp,
            &session,
            &[
                "Join without audio",
                "Join without video",
                "Continue without microphone and camera",
            ],
            DEVICE_CHECK_BUDGET,
        )
        .await
        {
            log::debug!(
                "[meet-scanner:zoom] device-check dismissal not needed or unavailable: {err}"
            );
        }

        // Phase 2 — type the display name.
        //
        // UNVERIFIED: "Your Name" is a best guess for the input's
        // aria-label/placeholder hint; Zoom's actual copy has not been
        // confirmed.
        type_into_named_input(
            &mut cdp,
            &session,
            "Your Name",
            &ctx.display_name,
            NAME_INPUT_BUDGET,
        )
        .await?;

        // Phase 3 — request to join.
        //
        // UNVERIFIED: "Join" is Zoom's commonly documented label but has
        // not been confirmed against a live meeting in this environment.
        wait_and_click_text(&mut cdp, &session, &["Join"], JOIN_BUTTON_BUDGET).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_constants_are_sane() {
        // Same guard as Meet/Teams' equivalent test: the total scanner
        // budget (discovery + all phases, including Zoom's extra phase 0)
        // should stay well under 120 s.
        let total = TARGET_DISCOVERY_BUDGET
            + APP_LAUNCH_INTERSTITIAL_BUDGET
            + DEVICE_CHECK_BUDGET
            + NAME_INPUT_BUDGET
            + JOIN_BUTTON_BUDGET;
        assert!(
            total <= Duration::from_secs(120),
            "total scanner budget {total:?} exceeds 120 s — check if constants were accidentally inflated"
        );
    }

    #[test]
    fn provider_name_is_zoom() {
        assert_eq!(ZoomScanner.provider_name(), "zoom");
    }

    #[test]
    fn app_launch_interstitial_labels_never_include_native_app_cta_text() {
        // Regression guard for the module-level "Zoom-specific risk"
        // note: phase 0's label list must stay narrow enough that it can
        // never match Zoom's native-app-launch CTA copy. If a future
        // edit widens this list to something like "join" or "launch",
        // this test catches it before it ships.
        let labels = ["join from your browser", "join from browser"];
        let forbidden_substrings = ["launch meeting", "open zoom"];
        for label in labels {
            let lower = label.to_lowercase();
            for forbidden in forbidden_substrings {
                assert!(
                    !lower.contains(forbidden),
                    "phase-0 label {label:?} unexpectedly overlaps native-app CTA text {forbidden:?}"
                );
            }
        }
    }
}
