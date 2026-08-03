//! Prewarm / close / purge / bounds / reveal / hide / show commands for
//! embedded webview accounts. Split out of `mod.rs` — no behavior changes.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime};

use super::*;

/// Issue #1233 — spawn a hidden 1×1 webview for `account_id` so its CEF
/// profile and provider page are warm before the user clicks the rail icon.
/// On the user's first click, the existing `webview_account_open` warm-reopen
/// branch reuses the prewarmed webview and emits `state:"reused"` so the React
/// loading overlay never has to wait for a cold load.
///
/// Implemented as a thin delegate to `webview_account_open` with
/// `prewarm: true`. Sharing the cold-open code path means the prewarmed
/// webview gets the full handler suite (`on_navigation`, `on_new_window`,
/// `on_page_load`), the per-provider scanner bootstrap, and the CEF
/// notification registration — none of which can be retroactively wired
/// when the warm-reopen branch later returns early.
///
/// Idempotent — calling for an already-warm account is a no-op. Best-effort —
/// the frontend can safely fire-and-forget; on failure the worst case is a
/// normal cold open later.
#[tauri::command]
pub async fn webview_account_prewarm<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: PrewarmArgs,
) -> Result<(), String> {
    log::info!(
        "[webview-accounts] prewarm account_id={} provider={}",
        args.account_id,
        args.provider
    );
    let open_args = OpenArgs {
        account_id: args.account_id,
        provider: args.provider,
        url: args.url,
        bounds: None,
        prewarm: true,
    };
    webview_account_open(app, state, open_args)
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn webview_account_close<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: AccountIdArgs,
) -> Result<(), String> {
    let label_opt = state.inner.lock().unwrap().remove(&args.account_id);
    let Some(label) = label_opt else {
        log::debug!(
            "[webview-accounts] close: no webview for account {}",
            args.account_id
        );
        return Ok(());
    };
    if let Some(wv) = app.get_webview(&label) {
        if let Err(e) = wv.close() {
            log::warn!("[webview-accounts] close({label}) failed: {e}");
        }
    }
    teardown_account_scanners(&app, &args.account_id);
    if let Some(browser_id) = state.browser_ids.lock().unwrap().remove(&args.account_id) {
        tauri_runtime_cef::notification::unregister(browser_id);
        log::debug!(
            "[notify-cef] unregistered handler account={} browser_id={}",
            args.account_id,
            browser_id
        );
    }
    if let Some(task) = state.cdp_sessions.lock().unwrap().remove(&args.account_id) {
        task.abort();
        log::debug!(
            "[cdp-session] aborted session task for account={}",
            args.account_id
        );
    }
    if let Some(task) = state
        .load_watchdogs
        .lock()
        .unwrap()
        .remove(&args.account_id)
    {
        task.abort();
        log::debug!(
            "[webview-accounts] aborted load watchdog for account={}",
            args.account_id
        );
    }
    // Reset load-overlay bookkeeping so the next open of this account starts
    // with a fresh "not yet loaded" state.
    state
        .loaded_accounts
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .requested_bounds
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .spawn_started_at
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .account_providers
        .lock()
        .unwrap()
        .remove(&args.account_id);
    // Issue #1233 — drop the prewarm flag too so a future prewarm dispatch
    // for the same id can re-attempt cleanly.
    state
        .prewarm_accounts
        .lock()
        .unwrap()
        .remove(&args.account_id);
    // Drop any gmeet workspace-rewrite counter for this label — labels are
    // reused on reopen, so a stale entry from a closed-mid-loop session
    // would saturate the next fresh open's window.
    state.clear_gmeet_marketing_rewrite(&label);
    log::info!("[webview-accounts] closed label={}", label);
    Ok(())
}

/// Close the webview AND wipe its on-disk `data_directory` so cookies,
/// storage and cached credentials are forgotten. Use this for the
/// user-initiated "logout" action — `webview_account_close` keeps the
/// data dir intact so the next open restores the session.
#[tauri::command]
pub async fn webview_account_purge<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: AccountIdArgs,
) -> Result<(), String> {
    // Close first so the native webview releases its file handles before we
    // try to delete the data directory.
    let label_opt = state.inner.lock().unwrap().remove(&args.account_id);
    if let Some(label) = label_opt.as_ref() {
        if let Some(wv) = app.get_webview(label) {
            if let Err(e) = wv.close() {
                log::warn!("[webview-accounts] purge close({label}) failed: {e}");
            }
        }
    }

    teardown_account_scanners(&app, &args.account_id);
    if let Some(browser_id) = state.browser_ids.lock().unwrap().remove(&args.account_id) {
        tauri_runtime_cef::notification::unregister(browser_id);
        log::debug!(
            "[notify-cef] purge unregistered handler account={} browser_id={}",
            args.account_id,
            browser_id
        );
    }
    if let Some(task) = state.cdp_sessions.lock().unwrap().remove(&args.account_id) {
        task.abort();
        log::debug!(
            "[cdp-session] purge aborted session task for account={}",
            args.account_id
        );
    }
    if let Some(task) = state
        .load_watchdogs
        .lock()
        .unwrap()
        .remove(&args.account_id)
    {
        task.abort();
        log::debug!(
            "[webview-accounts] purge aborted load watchdog for account={}",
            args.account_id
        );
    }
    state
        .loaded_accounts
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .requested_bounds
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .spawn_started_at
        .lock()
        .unwrap()
        .remove(&args.account_id);
    state
        .account_providers
        .lock()
        .unwrap()
        .remove(&args.account_id);
    // Issue #1233 — drop the prewarm flag too on purge.
    state
        .prewarm_accounts
        .lock()
        .unwrap()
        .remove(&args.account_id);
    if let Some(label) = label_opt.as_ref() {
        state.clear_gmeet_marketing_rewrite(label);
        // Drop any pending handoff flag for this label so a stale entry
        // can't hijack the next genuine `myaccount.google.com` visit on
        // a webview that re-uses the same label.
        state.take_awaiting_gmeet_handoff(label);
    }

    let data_dir = data_directory_for(&app, &args.account_id)?;
    purge_data_dir_with_retry(&data_dir)
        .await
        .map_err(|e| format!("purge data dir {}: {e}", data_dir.display()))?;

    log::info!(
        "[webview-accounts] purged account={} label={:?}",
        args.account_id,
        label_opt
    );
    Ok(())
}

/// CEF / WKWebView holds file handles briefly after `wv.close()` returns,
/// so a single `remove_dir_all` racing the close call routinely fails on
/// macOS and leaves the per-account cookie jar on disk. Re-adding the same
/// account after a logout then lands the user already signed in (#1076).
///
/// Retry the deletion a handful of times with exponential backoff so the
/// subprocess has a chance to drop its handles. Logs every attempt so a
/// stuck handle is diagnosable from the audit log.
pub(crate) async fn purge_data_dir_with_retry(data_dir: &std::path::Path) -> std::io::Result<()> {
    if !data_dir.exists() {
        return Ok(());
    }
    const MAX_ATTEMPTS: u32 = 5;
    const INITIAL_BACKOFF_MS: u64 = 100;
    let mut backoff = INITIAL_BACKOFF_MS;
    for attempt in 1..=MAX_ATTEMPTS {
        match std::fs::remove_dir_all(data_dir) {
            Ok(()) => {
                log::info!(
                    "[webview-accounts] purged data dir {} (attempt {}/{})",
                    data_dir.display(),
                    attempt,
                    MAX_ATTEMPTS
                );
                return Ok(());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                log::info!(
                    "[webview-accounts] purge data dir {} already removed before attempt {}/{}",
                    data_dir.display(),
                    attempt,
                    MAX_ATTEMPTS
                );
                return Ok(());
            }
            Err(err) if attempt < MAX_ATTEMPTS => {
                log::debug!(
                    "[webview-accounts] purge remove_dir_all {} attempt {}/{} failed: {} — retrying in {}ms",
                    data_dir.display(),
                    attempt,
                    MAX_ATTEMPTS,
                    err,
                    backoff
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                backoff *= 2;
            }
            Err(err) => {
                log::warn!(
                    "[webview-accounts] purge remove_dir_all {} failed after {} attempts: {} — cookies may persist; cross-launch fallback handled by schedule_cef_profile_purge",
                    data_dir.display(),
                    MAX_ATTEMPTS,
                    err
                );
                return Err(err);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn webview_account_bounds<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: BoundsArgs,
) -> Result<(), String> {
    let label_opt = state.inner.lock().unwrap().get(&args.account_id).cloned();
    let Some(label) = label_opt else {
        return Err(format!("no webview for account {}", args.account_id));
    };
    let wv = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} missing"))?;
    wv.set_position(LogicalPosition::new(args.bounds.x, args.bounds.y))
        .map_err(|e| format!("set_position: {e}"))?;
    wv.set_size(LogicalSize::new(args.bounds.width, args.bounds.height))
        .map_err(|e| format!("set_size: {e}"))?;
    log::trace!(
        "[webview-accounts] bounds label={} -> {:?}",
        label,
        args.bounds
    );
    // Keep the in-state bounds synced so `webview_account_reveal` has the
    // latest rect even if the frontend's own cache is cleared between the
    // `webview_account_open` call and the `webview-account:load` signal.
    state
        .requested_bounds
        .lock()
        .unwrap()
        .insert(args.account_id.clone(), args.bounds);
    Ok(())
}

/// Move an off-screen-spawned webview back to the frontend's desired rect and
/// show it. Invoked by the frontend when it receives the `webview-account:load`
/// event so the loading spinner is uncovered only after the page has painted.
///
/// Called as the final step of the first-open flow:
///   1. `webview_account_open` — CEF subview spawned off-screen
///   2. native `on_page_load` OR CDP `Page.loadEventFired` OR 15 s watchdog
///   3. frontend listener → `webview_account_reveal`
#[tauri::command]
pub async fn webview_account_reveal<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: RevealArgs,
) -> Result<(), String> {
    let label_opt = state.inner.lock().unwrap().get(&args.account_id).cloned();
    let Some(label) = label_opt else {
        // Reveal race: the webview was closed before the load event arrived.
        // Return Ok so the frontend doesn't surface an error.
        log::debug!(
            "[webview-accounts] reveal: no webview for account {}",
            args.account_id
        );
        return Ok(());
    };
    let wv = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} missing"))?;
    wv.set_position(LogicalPosition::new(args.bounds.x, args.bounds.y))
        .map_err(|e| format!("set_position: {e}"))?;
    wv.set_size(LogicalSize::new(args.bounds.width, args.bounds.height))
        .map_err(|e| format!("set_size: {e}"))?;
    wv.show().map_err(|e| format!("show: {e}"))?;
    state
        .requested_bounds
        .lock()
        .unwrap()
        .insert(args.account_id.clone(), args.bounds);
    let provider = state
        .account_providers
        .lock()
        .unwrap()
        .get(&args.account_id)
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let elapsed_ms = state
        .spawn_started_at
        .lock()
        .unwrap()
        .remove(&args.account_id)
        .map(|started| started.elapsed().as_millis())
        .map(|ms| ms.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let trigger = RevealTrigger::from_ipc(args.trigger.as_deref()).as_str();
    log::info!(
        "[webview-accounts][{}][{}] reveal trigger={} elapsed_ms={} bounds={:?}",
        provider,
        args.account_id,
        trigger,
        elapsed_ms,
        args.bounds
    );
    Ok(())
}

#[tauri::command]
pub async fn webview_account_hide<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: AccountIdArgs,
) -> Result<(), String> {
    let label_opt = state.inner.lock().unwrap().get(&args.account_id).cloned();
    let Some(label) = label_opt else {
        return Ok(());
    };
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.hide();
        log::debug!("[webview-accounts] hide label={}", label);
    }
    Ok(())
}

#[tauri::command]
pub async fn webview_account_show<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: AccountIdArgs,
) -> Result<(), String> {
    let label_opt = state.inner.lock().unwrap().get(&args.account_id).cloned();
    let Some(label) = label_opt else {
        return Ok(());
    };
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.show();
        log::debug!("[webview-accounts] show label={}", label);
    }
    Ok(())
}
