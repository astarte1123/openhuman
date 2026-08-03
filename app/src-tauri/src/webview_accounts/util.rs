//! Small stand-alone helpers: URL log-redaction, the load-finished
//! reveal/emit routine, account-id sanitation, label/data-dir naming,
//! and the CEF `initialization_script` builder.
//! Split out of `mod.rs` — no behavior changes.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Runtime, Url};

use super::*;

const RUNTIME_JS: &str = include_str!("runtime.js");

/// Strip query string and fragment from a URL before emitting to the log.
/// Provider URLs occasionally embed auth material (Telegram WebApp data,
/// OAuth callback codes, sometimes session tokens) and we don't want those
/// to land in the long-lived shell log file. Returns the original input on
/// parse failure so we still surface *something* useful for debugging.
pub(crate) fn redact_url_for_log(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut u) => {
            u.set_query(None);
            u.set_fragment(None);
            u.to_string()
        }
        Err(_) => {
            // Fallback: drop everything from the first '?' or '#'.
            raw.split(['?', '#']).next().unwrap_or(raw).to_string()
        }
    }
}

/// Grow the first-cold-open webview back to its full requested bounds and
/// notify the frontend once the page is actually loaded. Called from three
/// signals (native `WebviewBuilder::on_page_load`, CDP `Page.loadEventFired`,
/// and the 15 s watchdog).
///
/// Timeout is a non-terminal state: we emit `webview-account:load{state:
/// "timeout"}` so the frontend can show retry/help UI, but we deliberately do
/// NOT reveal or mark the account as loaded yet. If a later `finished` signal
/// arrives, that call still reveals and emits `state:"finished"`.
///
/// Resetting the terminal loaded marker happens in `webview_account_close` /
/// `webview_account_purge` so a reopen fires again.
///
/// Doing the `set_size` server-side (instead of waiting for the frontend to
/// invoke `webview_account_reveal`) avoids an extra IPC round-trip and the
/// brief blank frame that would otherwise sit between the load event and
/// the frontend's reveal call.
pub(crate) fn emit_load_finished<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
    state: &str,
    url: &str,
    trigger: RevealTrigger,
) {
    let Some(app_state) = app.try_state::<WebviewAccountsState>() else {
        // No state => emit anyway so the frontend doesn't hang; best-effort.
        log::warn!(
            "[webview-accounts][{}] WebviewAccountsState missing — emitting without reveal",
            account_id
        );
        let _ = app.emit(
            "webview-account:load",
            serde_json::json!({
                "account_id": account_id,
                "state": state,
                "trigger": trigger.as_str(),
                "url": url,
            }),
        );
        return;
    };

    // Issue #1233 — accounts in prewarm mode have no React UI listening
    // for their load events; the user hasn't clicked the rail icon yet.
    // Suppress emit + reveal so the prewarm cycle finishes silently. The
    // page is still painted in the off-screen 1×1 webview so the eventual
    // user click hits the warm-reopen branch and emits `state:"reused"`.
    if app_state
        .prewarm_accounts
        .lock()
        .unwrap()
        .contains(account_id)
    {
        log::info!(
            "[webview-accounts][{}] prewarm load suppressed state={} url={}",
            account_id,
            state,
            redact_url_for_log(url)
        );
        // Mark the account as loaded so any later signals from the same
        // cold-load (native on_page_load + CDP Page.loadEventFired both
        // arriving) don't double-fire if the prewarm flag flips off in
        // between.
        if state != "timeout" {
            app_state
                .loaded_accounts
                .lock()
                .unwrap()
                .insert(account_id.to_string());
        }
        return;
    }

    if state == "timeout" {
        // If we've already observed a terminal load, ignore late watchdogs.
        let already_loaded = app_state
            .loaded_accounts
            .lock()
            .unwrap()
            .contains(account_id);
        if already_loaded {
            log::debug!(
                "[webview-accounts][{}] timeout deduped after terminal load url={}",
                account_id,
                url
            );
            return;
        }

        log::info!(
            "[webview-accounts][{}] load timeout event trigger={} url={}",
            account_id,
            trigger.as_str(),
            redact_url_for_log(url)
        );
        if let Err(err) = app.emit(
            "webview-account:load",
            serde_json::json!({
                "account_id": account_id,
                "state": state,
                "trigger": trigger.as_str(),
                "url": url,
            }),
        ) {
            log::warn!(
                "[webview-accounts][{}] emit webview-account:load(timeout) failed: {}",
                account_id,
                err
            );
        }
        return;
    }

    let is_first_terminal = app_state
        .loaded_accounts
        .lock()
        .unwrap()
        .insert(account_id.to_string());
    if !is_first_terminal {
        log::debug!(
            "[webview-accounts][{}] load event deduped state={} url={}",
            account_id,
            state,
            url
        );
        return;
    }

    // Restore the webview to its full requested size. The spawn path created
    // it at 1×1 so the React loading spinner wasn't covered; now that the page
    // is painted we can grow it into the placeholder rect.
    let label = app_state.inner.lock().unwrap().get(account_id).cloned();
    let bounds = app_state
        .requested_bounds
        .lock()
        .unwrap()
        .get(account_id)
        .copied();
    match (label, bounds) {
        (Some(label), Some(b)) => {
            if let Some(wv) = app.get_webview(&label) {
                if let Err(e) = wv.set_size(LogicalSize::new(b.width, b.height)) {
                    log::warn!(
                        "[webview-accounts][{}] reveal set_size failed: {}",
                        account_id,
                        e
                    );
                }
                if let Err(e) = wv.set_position(LogicalPosition::new(b.x, b.y)) {
                    log::warn!(
                        "[webview-accounts][{}] reveal set_position failed: {}",
                        account_id,
                        e
                    );
                }
                let _ = wv.show();
                log::info!(
                    "[webview-accounts][{}] revealed label={} bounds={:?} state={}",
                    account_id,
                    label,
                    b,
                    state
                );
            } else {
                log::warn!(
                    "[webview-accounts][{}] reveal: webview {} missing",
                    account_id,
                    label
                );
            }
        }
        _ => {
            log::info!(
                "[webview-accounts][{}] reveal skipped (account closed before load) state={}",
                account_id,
                state
            );
        }
    }

    // Redact the URL in the log: providers like Telegram (`#tgWebAppData=…`)
    // and OAuth callbacks embed auth material in the query/fragment. The full
    // URL still flows to the frontend listener over the Tauri event so any
    // consumer that needs it has access; we just don't persist it to the
    // shell's log file.
    log::info!(
        "[webview-accounts][{}] load event state={} trigger={} url={}",
        account_id,
        state,
        trigger.as_str(),
        redact_url_for_log(url)
    );
    if let Err(err) = app.emit(
        "webview-account:load",
        serde_json::json!({
            "account_id": account_id,
            "state": state,
            "trigger": trigger.as_str(),
            "url": url,
        }),
    ) {
        log::warn!(
            "[webview-accounts][{}] emit webview-account:load failed: {}",
            account_id,
            err
        );
    }
}

/// Reject any `account_id` that isn't strictly `[A-Za-z0-9_-]+`. The ID comes
/// from IPC (React shell, but also from injected recipe code running inside
/// third-party origins via `webview_recipe_event`), so treat it as untrusted.
/// Enforcing this early prevents `../` sequences from escaping the per-account
/// data directory in `data_directory_for` (which feeds `create_dir_all` and
/// `remove_dir_all`).
pub(crate) fn sanitize_account_id(account_id: &str) -> Result<&str, String> {
    if account_id.is_empty()
        || !account_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("invalid account_id: {account_id:?}"));
    }
    Ok(account_id)
}

pub(crate) fn label_for(account_id: &str) -> String {
    // Webview labels must be alphanumeric + `-` / `_`. Callers that reached
    // here without first going through `sanitize_account_id` still get a
    // defensively-scrubbed label so invalid characters never reach the
    // tauri webview-label parser.
    let safe: String = account_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("acct_{}", safe)
}

pub(crate) fn data_directory_for<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
) -> Result<PathBuf, String> {
    // Guard against path traversal — `account_id` is joined into a filesystem
    // path that is later passed to `create_dir_all` / `remove_dir_all`.
    let account_id = sanitize_account_id(account_id)?;
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("app_local_data_dir: {e}"))?;
    Ok(base.join("webview_accounts").join(account_id))
}

/// Produce the `initialization_script` payload for this webview.
///
/// Empty for the 6 zero-injection providers (whatsapp, wechat, telegram,
/// slack, discord, browserscan) — they load with ZERO injected JS. Some have
/// native/CDP scraper paths; WeChat is shell-only for now. The per-account
/// CDP session opener (`cdp::session`) still injects the notification-permission
/// shim via `Page.addScriptToEvaluateOnNewDocument` before the real provider
/// URL loads. The 2 deferred providers (linkedin, google-meet) still get the
/// JS recipe bridge.
pub(crate) fn build_init_script(account_id: &str, provider: &str) -> String {
    let Some(recipe_js) = provider_recipe_js(provider) else {
        return String::new();
    };
    let ctx = serde_json::json!({
        "accountId": account_id,
        "provider": provider,
    });
    format!(
        "window.__OPENHUMAN_RECIPE_CTX__ = {ctx};\n\n{runtime}\n\n{recipe}\n",
        ctx = ctx,
        runtime = RUNTIME_JS,
        recipe = recipe_js
    )
}
