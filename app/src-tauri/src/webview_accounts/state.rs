//! Per-app `WebviewAccountsState` (account/label/CDP/watchdog bookkeeping,
//! gmeet marketing-rewrite loop breaker) and the shutdown/teardown helpers
//! that drain it. Split out of `mod.rs` — no behavior changes.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};

use super::*;

#[derive(Default)]
pub struct WebviewAccountsState {
    /// account_id -> webview label (we use `acct_<id>` as the label).
    pub(crate) inner: Mutex<HashMap<String, String>>,
    /// account_id -> provider id. Kept so late reveal/close paths can log
    /// provider-scoped diagnostics without trusting frontend echo fields.
    pub(crate) account_providers: Mutex<HashMap<String, String>>,
    /// account_id -> CEF `Browser::identifier()`. Populated asynchronously
    /// inside the `with_webview` callback once the renderer hands us the
    /// browser handle, and consumed at close/purge time so we can call
    /// `tauri_runtime_cef::notification::unregister` without leaking
    /// per-browser handler entries across account churn.
    pub(crate) browser_ids: Mutex<HashMap<String, i32>>,
    /// account_id -> CDP session task. One long-lived task per account
    /// keeps the UA override resident (see `cdp::session`); aborted on
    /// close/purge so reopen cycles don't stack multiple live loops.
    pub(crate) cdp_sessions: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    /// account_id -> 15s `webview-account:load{state:"timeout"}` watchdog.
    /// Aborted in close/purge so a watchdog spawned for a now-closed
    /// account can't fire a stale timeout against a freshly-reused id.
    pub(crate) load_watchdogs: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    /// account_id of webviews that have already emitted their first
    /// `webview-account:load{state:"finished"}` event. Used to dedup
    /// triple-signal fires (native on_page_load, CDP `Page.loadEventFired`,
    /// 15 s watchdog) so the frontend only reveals once per cold open.
    pub(crate) loaded_accounts: Mutex<HashSet<String>>,
    /// Last bounds requested by the frontend for a given account, captured at
    /// `webview_account_open` time so the off-screen-spawned webview can be
    /// revealed at the right rect without the frontend having to round-trip
    /// them again.
    pub(crate) requested_bounds: Mutex<HashMap<String, Bounds>>,
    /// account_id -> `Instant` captured at the moment the cold spawn returns
    /// from `add_child`. Consumed by `webview_account_reveal` to compute
    /// `elapsed_ms` (spawn -> frontend reveal call) for the diagnostic log
    /// instrumented for the Slack first-load investigation (#1036). Cleared
    /// alongside `loaded_accounts` on close/purge so a subsequent reopen
    /// starts fresh.
    pub(crate) spawn_started_at: Mutex<HashMap<String, Instant>>,
    /// Runtime notification-bypass controls used by the settings UI.
    pub(crate) notification_bypass: Mutex<NotificationBypassPrefs>,
    /// Per-label rewrite counter for the gmeet `workspace.google.com`
    /// marketing intercept. Google's edge SSR-redirects unauthenticated
    /// `meet.google.com` GETs back to `workspace.google.com/products/meet/`,
    /// so an unguarded rewrite (see `:1534-1559`) ping-pongs forever and
    /// the page never lands. We track `(last_attempt, attempts_in_window)`
    /// per webview label and bail to the Google sign-in flow after a
    /// threshold so the user breaks out of the loop.
    pub(crate) gmeet_marketing_rewrites: Mutex<HashMap<String, (Instant, u32)>>,
    /// Per-label "awaiting post-auth handoff" flag. Set when the gmeet
    /// rewrite-loop bail navigates to `accounts.google.com/ServiceLogin`,
    /// consumed (single-shot) by the `myaccount.google.com` intercept so
    /// only the immediate post-auth bounce gets rewritten back to Meet —
    /// legitimate user-initiated `myaccount.google.com` navigations (e.g.
    /// "Manage your Google Account" from the avatar menu) are passed
    /// through unchanged.
    pub(crate) gmeet_awaiting_handoff: Mutex<HashSet<String>>,
    /// account_ids spawned via `webview_account_prewarm` that have not yet
    /// been opened by the user. Issue #1233 — emit_load_finished suppresses
    /// `webview-account:load` events for these so the React UI never sees
    /// load/timeout signals for an account it didn't ask to open. The flag
    /// is cleared on the first user-initiated `webview_account_open`
    /// (warm-reopen branch) and on close/purge so subsequent reopens flow
    /// through the normal cold-load lifecycle.
    pub(crate) prewarm_accounts: Mutex<HashSet<String>>,
}

/// Threshold and window for the gmeet workspace-marketing rewrite loop
/// breaker. After `GMEET_REWRITE_MAX_ATTEMPTS` rewrites within
/// `GMEET_REWRITE_WINDOW`, we bail to the Google sign-in URL instead of
/// rewriting again.
pub(crate) const GMEET_REWRITE_MAX_ATTEMPTS: u32 = 3;
pub(crate) const GMEET_REWRITE_WINDOW: Duration = Duration::from_secs(5);

/// Result of consulting the gmeet marketing-redirect counter.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GmeetRewriteAction {
    /// Allow the rewrite — fewer than `GMEET_REWRITE_MAX_ATTEMPTS` attempts in
    /// the current window.
    Rewrite,
    /// Loop detected — caller should navigate to the Google sign-in URL
    /// instead of rewriting back to `meet.google.com`.
    Bail,
}

impl WebviewAccountsState {
    /// Drop the gmeet marketing-rewrite counter for `label`. Called after
    /// the post-auth handoff so a future workspace-marketing bounce (which
    /// shouldn't occur post-auth, but could on session expiry) gets a fresh
    /// counter window instead of inheriting half-saturated state from the
    /// pre-auth loop.
    pub(crate) fn clear_gmeet_marketing_rewrite(&self, label: &str) {
        if let Ok(mut g) = self.gmeet_marketing_rewrites.lock() {
            g.remove(label);
        }
    }

    /// Mark `label` as awaiting the post-auth `myaccount.google.com` →
    /// `meet.google.com` handoff. Set by the rewrite-loop bail right
    /// before navigating to `accounts.google.com/ServiceLogin?continue=`,
    /// so the next `myaccount.google.com` commit on this label is treated
    /// as the auth chain's terminal hop (the `?utm_source=sign_in_no_continue`
    /// dump-page) and gets force-redirected to Meet.
    pub(crate) fn mark_awaiting_gmeet_handoff(&self, label: &str) {
        if let Ok(mut g) = self.gmeet_awaiting_handoff.lock() {
            g.insert(label.to_string());
        }
    }

    /// Single-shot consume of the post-auth handoff flag for `label`.
    /// Returns `true` exactly once after `mark_awaiting_gmeet_handoff`,
    /// then resets so subsequent `myaccount.google.com` navigations
    /// (e.g. user-initiated profile/settings visits) pass through as
    /// normal and aren't hijacked back to Meet.
    pub(crate) fn take_awaiting_gmeet_handoff(&self, label: &str) -> bool {
        match self.gmeet_awaiting_handoff.lock() {
            Ok(mut g) => g.remove(label),
            Err(_) => false,
        }
    }

    /// Increment the per-label gmeet marketing-rewrite counter for `now` and
    /// decide whether to rewrite or bail. Resets the counter when the last
    /// attempt was outside `GMEET_REWRITE_WINDOW` so a future genuine
    /// `workspace.google.com` navigation (e.g. user clicks a link inside Meet
    /// after sign-in) gets intercepted normally.
    pub(crate) fn track_gmeet_marketing_rewrite(
        &self,
        label: &str,
        now: Instant,
    ) -> GmeetRewriteAction {
        let mut map = match self.gmeet_marketing_rewrites.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let entry = map.entry(label.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) > GMEET_REWRITE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.0 = now;
        if entry.1 > GMEET_REWRITE_MAX_ATTEMPTS {
            GmeetRewriteAction::Bail
        } else {
            GmeetRewriteAction::Rewrite
        }
    }

    /// Drain every per-account resource owned by this state and abort the
    /// associated background tasks. Returns the `(account_id, label)`
    /// pairs of webviews that still need closing — the caller does the
    /// actual `wv.close()` because that needs an `AppHandle`. Splitting
    /// it out keeps the rest of the teardown unit-testable without
    /// constructing a Tauri runtime.
    ///
    /// Aborts CDP session tasks and load watchdogs, unregisters CEF
    /// notification handlers, and clears the loaded-accounts /
    /// requested-bounds bookkeeping. All collections are drained — a
    /// repeat call returns an empty `Vec` and is a safe no-op.
    pub(crate) fn drain_for_shutdown(&self) -> Vec<(String, String)> {
        let cdp_tasks: Vec<_> = self
            .cdp_sessions
            .lock()
            .ok()
            .map(|mut g| g.drain().collect())
            .unwrap_or_default();
        for (acct, task) in cdp_tasks {
            task.abort();
            log::debug!("[webview-accounts] shutdown abort cdp account={}", acct);
        }
        let watchdogs: Vec<_> = self
            .load_watchdogs
            .lock()
            .ok()
            .map(|mut g| g.drain().collect())
            .unwrap_or_default();
        for (acct, task) in watchdogs {
            task.abort();
            log::debug!(
                "[webview-accounts] shutdown abort watchdog account={}",
                acct
            );
        }
        let browser_ids: Vec<_> = self
            .browser_ids
            .lock()
            .ok()
            .map(|mut g| g.drain().collect())
            .unwrap_or_default();
        for (acct, browser_id) in browser_ids {
            tauri_runtime_cef::notification::unregister(browser_id);
            log::debug!(
                "[notify-cef] shutdown unregistered handler account={} browser_id={}",
                acct,
                browser_id
            );
        }
        if let Ok(mut g) = self.loaded_accounts.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.requested_bounds.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.spawn_started_at.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.account_providers.lock() {
            g.clear();
        }
        // Per-label gmeet rewrite counter must clear too — `label_for()`
        // reuses the same label on reopen, so a stale saturated entry
        // would jump a fresh open straight to the bail URL.
        if let Ok(mut g) = self.gmeet_marketing_rewrites.lock() {
            g.clear();
        }
        // Drop the post-auth handoff flag too — a stale flag would
        // hijack the first user-initiated `myaccount.google.com` visit
        // after a relaunch back to Meet.
        if let Ok(mut g) = self.gmeet_awaiting_handoff.lock() {
            g.clear();
        }
        // Issue #1233 — clear prewarm flags so a relaunch can't suppress
        // load events for accounts that were prewarmed in the previous
        // session.
        if let Ok(mut g) = self.prewarm_accounts.lock() {
            g.clear();
        }
        self.inner
            .lock()
            .ok()
            .map(|mut g| g.drain().collect())
            .unwrap_or_default()
    }

    /// Tear down every per-account resource owned by this state — used by
    /// the app's `RunEvent::ExitRequested` path so nothing outlives the
    /// tokio runtime / `AppHandle` (issue #920).
    ///
    /// On top of [`drain_for_shutdown`], this closes every `acct_*` child
    /// webview so CEF browsers tear down before `cef::shutdown()` runs,
    /// and tells the per-account scanner registries to forget the
    /// account so a future open of the same id starts from a clean slate.
    /// All collections are drained — repeat calls are cheap no-ops.
    pub fn shutdown_all<R: Runtime>(&self, app: &AppHandle<R>) -> Vec<String> {
        teardown_all_account_scanners(app);
        let labels = self.drain_for_shutdown();
        let mut closed_labels = Vec::with_capacity(labels.len());
        for (acct, label) in labels {
            teardown_account_scanners(app, &acct);
            if let Some(wv) = app.get_webview(&label) {
                // Track the label as soon as the webview exists so a failed
                // `close()` still participates in the post-close drain poll
                // (issue #1120 / CodeRabbit).
                closed_labels.push(label.clone());
                if let Err(e) = wv.close() {
                    log::warn!(
                        "[webview-accounts] shutdown close({label}) failed account={acct}: {e}"
                    );
                }
            } else {
                log::debug!(
                    "[webview-accounts] shutdown label already gone account={} label={}",
                    acct,
                    label
                );
            }
        }
        log::info!(
            "[webview-accounts] shutdown_all complete closed_labels={:?}",
            closed_labels
        );
        closed_labels
    }
}

/// Abort every provider scanner task tracked by the per-provider
/// registries. Used by full-app shutdown before the per-account state is
/// drained so CDP loops stop even if an account label was already removed
/// from `WebviewAccountsState`.
pub(crate) fn teardown_all_account_scanners<R: Runtime>(app: &AppHandle<R>) {
    let mut total = 0usize;
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::whatsapp_scanner::ScannerRegistry>>()
    {
        total += registry.inner().forget_all();
    }
    if let Some(registry) = app.try_state::<std::sync::Arc<crate::slack_scanner::ScannerRegistry>>()
    {
        total += registry.inner().forget_all();
    }
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::discord_scanner::ScannerRegistry>>()
    {
        total += registry.inner().forget_all();
    }
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::telegram_scanner::ScannerRegistry>>()
    {
        total += registry.inner().forget_all();
    }
    if total > 0 {
        log::info!(
            "[webview-accounts] aborted {} provider scanner task(s) for shutdown",
            total
        );
    }
}

/// Tell the per-account scanner registries (whatsapp / slack / discord /
/// telegram) to forget `account_id`. Shared by `webview_account_close`,
/// `webview_account_purge`, and `WebviewAccountsState::shutdown_all` so
/// every exit path goes through the same teardown.
pub(crate) fn teardown_account_scanners<R: Runtime>(app: &AppHandle<R>, account_id: &str) {
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::whatsapp_scanner::ScannerRegistry>>()
    {
        registry.inner().forget(account_id);
    }
    if let Some(registry) = app.try_state::<std::sync::Arc<crate::slack_scanner::ScannerRegistry>>()
    {
        registry.inner().forget(account_id);
    }
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::discord_scanner::ScannerRegistry>>()
    {
        registry.inner().forget(account_id);
    }
    if let Some(registry) =
        app.try_state::<std::sync::Arc<crate::telegram_scanner::ScannerRegistry>>()
    {
        registry.inner().forget(account_id);
    }
}
