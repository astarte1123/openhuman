//! `webview_account_open` — the CEF child-webview spawn/wire command.
//! High-risk zone: wires `on_navigation`, `on_new_window`, and
//! `LoadHandler::OnLoadStart` for the embedded provider webviews.
//! Moved out of `mod.rs` unchanged — no behavior changes.

use std::time::Instant;

use cef::ImplBrowser;
use tauri::{
    webview::NewWindowResponse, AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Runtime,
    Url, WebviewBuilder, WebviewUrl,
};

use crate::cdp;

use super::*;

/// Spawn (or focus) the embedded webview for an account.
#[tauri::command]
pub async fn webview_account_open<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, WebviewAccountsState>,
    args: OpenArgs,
) -> Result<String, String> {
    let label = label_for(&args.account_id);
    log::info!(
        "[webview-accounts] open account_id={} provider={} label={}",
        args.account_id,
        args.provider,
        label
    );

    // Reject unknown providers early. `provider_url` already errors when
    // no URL override is supplied; the `provider_is_supported` check
    // additionally gates custom-URL overrides so an arbitrary provider
    // string can't ride in via the debug `url` field.
    if !provider_is_supported(&args.provider) {
        return Err(format!("unknown provider: {}", args.provider));
    }
    let real_url_str = args
        .url
        .as_deref()
        .or_else(|| provider_url(&args.provider))
        .ok_or_else(|| format!("no url for provider: {}", args.provider))?
        .to_string();
    // Validate the real URL up front — otherwise a malformed debug
    // `args.url` would only fail later inside the async CDP session
    // loop, which is much harder to surface to the caller. The parsed
    // Url also feeds `scanner_url_prefix` so scanners match on the
    // actual origin the user navigated to (honoring debug overrides).
    let real_url: Url = real_url_str
        .parse()
        .map_err(|e| format!("invalid provider url {real_url_str}: {e}"))?;
    // Scanner target-match uses `url.starts_with(prefix)`, so the
    // prefix needs to be the ORIGIN (scheme + host), not the full URL
    // — same-host intra-app navigations must keep matching after the
    // initial load.
    let scanner_url_prefix = format!("{}/", real_url.origin().ascii_serialization());
    let skip_cdp_for_debug = args.provider == "slack" && !slack_scanner_enabled();
    // We normally open the webview at a tiny placeholder URL so the CDP
    // session opener can attach and inject the notification-permission
    // shim (see `cdp/session.rs`) BEFORE the real provider URL loads;
    // without it Slack surfaces in-app "enable notifications"
    // banners. For Slack debug sessions we allow opting out via
    // `AURA_DISABLE_SLACK_SCANNER=1`, which also skips the long-lived
    // CDP session so external DevTools can attach cleanly.
    let initial_url_str = if skip_cdp_for_debug {
        real_url_str.clone()
    } else {
        cdp::placeholder_url(&args.account_id)
    };
    let initial_url: Url = initial_url_str
        .parse()
        .map_err(|e| format!("invalid initial url {initial_url_str}: {e}"))?;

    // If a webview for this account already exists, just reposition / show.
    {
        let map = state.inner.lock().unwrap();
        if let Some(existing_label) = map.get(&args.account_id).cloned() {
            drop(map);
            if let Some(existing) = app.get_webview(&existing_label) {
                // Issue #1233 — when this is a prewarm call landing on an
                // already-prewarmed account, do nothing: the webview is
                // already off-screen, the CDP session is already attached,
                // and the prewarm flag should stay set so the eventual
                // user-initiated open can promote it. Just return the label.
                if args.prewarm {
                    log::debug!(
                        "[webview-accounts] prewarm idempotent skip: account={} already warm label={}",
                        args.account_id,
                        existing_label
                    );
                    return Ok(existing_label);
                }
                // Issue #1233 — a prewarmed webview is reaching its first
                // user-initiated open. Clear the prewarm flag BEFORE we
                // resize/reveal so any in-flight CDP load event still
                // racing toward `emit_load_finished` flows through the
                // normal path instead of being silently suppressed.
                let was_prewarmed = state
                    .prewarm_accounts
                    .lock()
                    .unwrap()
                    .remove(&args.account_id);
                if was_prewarmed {
                    log::info!(
                        "[webview-accounts] prewarm hit account={} label={} — promoting to live",
                        args.account_id,
                        existing_label
                    );
                }
                if let Some(b) = args.bounds {
                    let _ = existing.set_position(LogicalPosition::new(b.x, b.y));
                    let _ = existing.set_size(LogicalSize::new(b.width, b.height));
                    state
                        .requested_bounds
                        .lock()
                        .unwrap()
                        .insert(args.account_id.clone(), b);
                }
                let _ = existing.show();
                log::info!(
                    "[webview-accounts] reused existing label={} for account={}",
                    existing_label,
                    args.account_id
                );
                // Warm re-open: the page is already painted, so skip the
                // loading overlay cycle and tell the frontend to go straight
                // to `open`. We bypass `emit_load_finished` because the
                // `loaded_accounts` dedup set would swallow the emit after
                // the first cold open of this account.
                let reuse_url = existing.url().map(|u| u.to_string()).unwrap_or_default();
                if let Err(err) = app.emit(
                    "webview-account:load",
                    serde_json::json!({
                        "account_id": args.account_id,
                        "state": "reused",
                        "trigger": RevealTrigger::Load.as_str(),
                        "url": reuse_url,
                    }),
                ) {
                    log::warn!(
                        "[webview-accounts][{}] emit reused event failed: {}",
                        args.account_id,
                        err
                    );
                }
                return Ok(existing_label);
            }
            // Stale entry — fall through and rebuild
            log::warn!(
                "[webview-accounts] stale label {} found for account {}, rebuilding",
                existing_label,
                args.account_id
            );
        }
    }

    // Grab the raw Window (not WebviewWindow) so `add_child` works even
    // after we've attached sibling webviews — `get_webview_window` checks
    // `is_webview_window()` which flips to false once a window has more
    // than one webview.
    let parent_window = app
        .get_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let data_dir = data_directory_for(&app, &args.account_id)?;
    if let Err(err) = std::fs::create_dir_all(&data_dir) {
        log::warn!(
            "[webview-accounts] failed to create data dir {}: {}",
            data_dir.display(),
            err
        );
    }

    let init_script = build_init_script(&args.account_id, &args.provider);

    let mut builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(initial_url))
        .data_directory(data_dir);
    if !init_script.is_empty() {
        builder = builder.initialization_script(&init_script);
    }

    // Keep link clicks that leave the provider's host set in the OS
    // browser, not the embedded webview. Same-host navigations (including
    // OAuth hops to accounts.google.com etc., which we pre-declare per
    // provider) stay in-app. Provider-specific native-app deep links
    // (`zoomus://`, `zoommtg://`, …) are rewritten to the web-client URL
    // and re-navigated in-app so meetings don't bounce out.
    let nav_provider = args.provider.clone();
    let nav_app = app.clone();
    let nav_label = label.clone();
    let nav_account_id = args.account_id.clone();
    builder = builder.on_navigation(move |url| {
        // Notify the frontend on every committed navigation. The
        // `webview-account:load` event is dedup'd per cold open, so it
        // can't be used to spot post-login redirects (e.g. Google
        // Meet's accounts.google.com → meet.google.com hop). Frontends
        // that
        // care about live URL transitions — onboarding's auto-detect
        // for "user finished signing in", for instance — listen here.
        if let Err(err) = nav_app.emit(
            "webview-account:navigate",
            serde_json::json!({
                "account_id": nav_account_id,
                "provider": nav_provider,
                "url": redact_navigation_url(url),
            }),
        ) {
            log::debug!(
                "[webview-accounts] emit webview-account:navigate failed: {}",
                err
            );
        }
        // Google Meet: when Google's edge SSR-redirects the post-account-
        // picker URL to `workspace.google.com/products/meet/...` (the
        // marketing landing page), `workspace.google.com` matches the
        // bare `google.com` suffix in `provider_allowed_hosts` so
        // `url_is_internal` would commit the navigation and the user
        // would land on the Workspace marketing page instead of Meet.
        // Catch this here and replace the parent URL with the canonical
        // Meet entry point so the embedded view stays on the app.
        //
        // BUT: unauthenticated users get bounced right back to
        // `workspace.google.com/products/meet/` by Google's edge, so an
        // unguarded rewrite ping-pongs forever (`navigate` → Google
        // redirect → `on_navigation` → `navigate` → …). We track per-label
        // attempts in `WebviewAccountsState::gmeet_marketing_rewrites` and
        // bail to a Google sign-in URL after a small threshold so the user
        // can break out of the loop. See #1213 (downstream watchdog
        // symptom) and `track_gmeet_marketing_rewrite` for the policy.
        if nav_provider == "google-meet" {
            if let Some(host) = url.host_str() {
                // Post-auth handoff: when the bail's `ServiceLogin?continue=`
                // chain completes, Google sometimes drops the continue param
                // (URL ends in `?utm_source=sign_in_no_continue`) and dumps
                // the user on `myaccount.google.com` instead of Meet. The
                // session cookie is now valid, so a direct navigation to
                // `meet.google.com` will paint Meet without bouncing back
                // through the workspace marketing redirect (which was the
                // unauthenticated branch). Force the hop here so the user
                // doesn't have to click through the apps grid manually.
                //
                // Gated on the per-label `gmeet_awaiting_handoff` flag —
                // set by the Bail branch right before navigating to
                // `ServiceLogin?continue=` — so legitimate user-initiated
                // visits to `myaccount.google.com` (e.g. "Manage your
                // Google Account" from the avatar menu) pass through and
                // remain reachable in-app.
                if host == "myaccount.google.com" {
                    let consumed = nav_app
                        .try_state::<WebviewAccountsState>()
                        .map(|s| s.take_awaiting_gmeet_handoff(&nav_label))
                        .unwrap_or(false);
                    if !consumed {
                        log::debug!(
                            "[webview-accounts] gmeet myaccount.google.com nav (label={}) not in handoff window; passing through",
                            nav_label
                        );
                    } else {
                        log::info!(
                            "[webview-accounts] gmeet post-auth handoff detected on myaccount.google.com; navigating parent to https://meet.google.com/"
                        );
                        let app = nav_app.clone();
                        let label = nav_label.clone();
                        // Reset the marketing-rewrite counter so the next
                        // workspace bounce (if any) gets a fresh window —
                        // the user is now authenticated and shouldn't
                        // loop again.
                        if let Some(s) = nav_app.try_state::<WebviewAccountsState>() {
                            s.clear_gmeet_marketing_rewrite(&nav_label);
                        }
                        tauri::async_runtime::spawn(async move {
                            if let Some(wv) = app.get_webview(&label) {
                                if let Ok(target) = Url::parse("https://meet.google.com/") {
                                    if let Err(e) = wv.navigate(target) {
                                        log::warn!(
                                            "[webview-accounts] gmeet post-auth navigate failed label={} err={}",
                                            label,
                                            e
                                        );
                                    }
                                }
                            }
                        });
                        return false;
                    }
                }
                if is_gmeet_marketing_redirect(host, url.path()) {
                    let action = nav_app
                        .try_state::<WebviewAccountsState>()
                        .map(|s| s.track_gmeet_marketing_rewrite(&nav_label, Instant::now()))
                        .unwrap_or(GmeetRewriteAction::Rewrite);
                    let app = nav_app.clone();
                    let label = nav_label.clone();
                    let target_url = match action {
                        GmeetRewriteAction::Rewrite => {
                            log::info!(
                                "[webview-accounts] gmeet workspace marketing redirect intercepted ({}); rewriting parent to https://meet.google.com/",
                                url
                            );
                            "https://meet.google.com/"
                        }
                        GmeetRewriteAction::Bail => {
                            log::warn!(
                                "[webview-accounts] gmeet workspace rewrite loop detected on label={} (>{} attempts in {}s); falling through to Google sign-in",
                                nav_label,
                                GMEET_REWRITE_MAX_ATTEMPTS,
                                GMEET_REWRITE_WINDOW.as_secs()
                            );
                            // Arm the post-auth handoff flag so the next
                            // `myaccount.google.com` commit on this label
                            // (the `?utm_source=sign_in_no_continue` dump
                            // page Google sometimes drops users on after
                            // the ServiceLogin chain) gets force-redirected
                            // back to Meet. Without this gate, ANY
                            // `myaccount.google.com` visit was hijacked,
                            // breaking legitimate "Manage your Google
                            // Account" flows.
                            if let Some(s) = nav_app.try_state::<WebviewAccountsState>() {
                                s.mark_awaiting_gmeet_handoff(&nav_label);
                            }
                            // `service=meet` is rejected by Google (`400
                            // malformed`); the continue param must be
                            // URL-encoded since `&` would split it. Drop
                            // `service=` and encode `continue=` so the
                            // sign-in landing actually loads.
                            "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmeet.google.com%2F"
                        }
                    };
                    tauri::async_runtime::spawn(async move {
                        if let Some(wv) = app.get_webview(&label) {
                            if let Ok(target) = Url::parse(target_url) {
                                if let Err(e) = wv.navigate(target) {
                                    log::warn!(
                                        "[webview-accounts] gmeet workspace rewrite navigate failed label={} target={} err={}",
                                        label,
                                        target_url,
                                        e
                                    );
                                }
                            }
                        }
                    });
                    return false;
                }
            }
        }
        if let Some(rewritten) = rewrite_provider_deep_link(&nav_provider, url) {
            log::info!(
                "[webview-accounts] deep-link rewrite {} → {} (provider={})",
                url,
                rewritten,
                nav_provider
            );
            let app = nav_app.clone();
            let label = nav_label.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(wv) = app.get_webview(&label) {
                    if let Err(e) = wv.navigate(rewritten) {
                        log::warn!(
                            "[webview-accounts] post-rewrite navigate failed label={} err={}",
                            label,
                            e
                        );
                    }
                }
            });
            return false;
        }
        if url_is_internal(&nav_provider, url) {
            true
        } else {
            // Suppress provider native-desktop-app deep-link schemes that
            // we don't know how to rewrite. macOS would otherwise hand
            // these to the native provider app — `slack://magic-login/…`
            // signs the native Slack app into the workspace, breaking
            // embedded-webview isolation (#1074). The web flow's HTTPS
            // fallback handles sign-in without the deep link.
            if is_provider_native_deep_link_scheme(url.scheme()) {
                log::warn!(
                    "[webview-accounts] suppressing native-app deep-link scheme={} url={} (would breach workspace isolation)",
                    url.scheme(),
                    redact_native_deep_link_url(url)
                );
                return false;
            }
            let target = unwrap_provider_redirect(url)
                .map(|u| u.to_string())
                .unwrap_or_else(|| url.to_string());
            if target != url.as_str() {
                log::info!(
                    "[webview-accounts] external navigation {} → (unwrapped) {} → system browser",
                    url,
                    target
                );
            } else {
                log::info!(
                    "[webview-accounts] external navigation {} → system browser",
                    url
                );
            }
            open_in_system_browser(&target);
            false
        }
    });

    // Cmd/Ctrl-click and `target="_blank"` / `window.open(...)` trigger a
    // new-window request. Default policy: deny and hand the URL to the
    // system browser — matches user intent of "open in new tab outside
    // the app".
    //
    // Exception: some providers (Slack Huddles) spawn popups via
    // `window.open()` and abort the flow if the return value is falsey.
    // For those URLs we allow CEF's default popup handling so an in-app
    // child window opens and the caller gets a real window handle.
    let popup_provider = args.provider.clone();
    let popup_app = app.clone();
    let popup_label = label.clone();
    builder = builder.on_new_window(move |url, _features| {
        if let Some(rewritten) = rewrite_provider_deep_link(&popup_provider, &url) {
            log::info!(
                "[webview-accounts] new-window deep-link rewrite {} → {} (provider={})",
                url,
                rewritten,
                popup_provider
            );
            let app = popup_app.clone();
            let label = popup_label.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(wv) = app.get_webview(&label) {
                    if let Err(e) = wv.navigate(rewritten) {
                        log::warn!(
                            "[webview-accounts] post-rewrite navigate (popup) failed label={} err={}",
                            label,
                            e
                        );
                    }
                }
            });
            return NewWindowResponse::Deny;
        }
        if let Some(target) = popup_should_navigate_parent(&popup_provider, &url) {
            log::info!(
                "[webview-accounts] new-window {} → navigate parent (provider={})",
                redact_navigation_url(&url),
                popup_provider
            );
            let app = popup_app.clone();
            let label = popup_label.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(wv) = app.get_webview(&label) {
                    if let Err(e) = wv.navigate(target) {
                        log::warn!(
                            "[webview-accounts] popup→parent navigate failed label={} err={}",
                            label,
                            e
                        );
                    }
                }
            });
            return NewWindowResponse::Deny;
        }
        if popup_should_stay_in_app(&popup_provider, &url) {
            log::info!(
                "[webview-accounts] new-window request {} → in-app popup (provider={})",
                url,
                popup_provider
            );
            NewWindowResponse::Allow
        } else {
            // Suppress provider native-desktop-app deep-link schemes that
            // we don't know how to rewrite (matches the on_navigation
            // fallback). Without this, a `slack://...` popup would land
            // in the native Slack app via macOS's URL handler and
            // breach embedded-webview workspace isolation (#1074).
            if is_provider_native_deep_link_scheme(url.scheme()) {
                log::warn!(
                    "[webview-accounts] suppressing native-app deep-link scheme={} url={} (would breach workspace isolation)",
                    url.scheme(),
                    redact_native_deep_link_url(&url)
                );
                return NewWindowResponse::Deny;
            }
            let target = unwrap_provider_redirect(&url)
                .map(|u| u.to_string())
                .unwrap_or_else(|| url.to_string());
            if target != url.as_str() {
                log::info!(
                    "[webview-accounts] new-window request {} → (unwrapped) {} → system browser",
                    url,
                    target
                );
            } else {
                log::info!(
                    "[webview-accounts] new-window request {} → system browser",
                    url
                );
            }
            open_in_system_browser(&target);
            NewWindowResponse::Deny
        }
    });

    // Enable devtools on child webviews in debug builds only so recipe
    // diagnostics and IndexedDB state can be inspected. Access on macOS is via
    //   Safari → Develop → <App name> → <webview label>
    // (the parent Tauri window's right-click "Inspect" does not propagate
    // into child webviews on WKWebView). In release builds we leave CDP off
    // so third-party-site webviews are not remotely inspectable.
    if cfg!(debug_assertions) {
        builder = builder.devtools(true);
    }

    // Wire the native page-load signal and forward only *usable* load
    // completions to `emit_load_finished`:
    //   - skip placeholder `about:blank#aura-acct-*` commits (otherwise
    //     we reveal a blank viewport before real content arrives),
    //   - treat Chromium network error pages (`chrome-error://…`) as timeout
    //     signals so frontend shows retry/help UI instead of the dino page.
    //
    // Real provider commits still emit `finished`. Dedup against CDP
    // `Page.loadEventFired` + watchdog happens in `emit_load_finished`.
    let page_load_app = app.clone();
    let page_load_account_id = args.account_id.clone();
    let page_load_placeholder_fragment = format!("#{}", cdp::placeholder_marker(&args.account_id));
    let page_load_real_url = real_url_str.clone();
    builder = builder.on_page_load(move |_webview, payload| {
        if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
            return;
        }
        let url = payload.url();
        if url.scheme() == "data" {
            return;
        }
        if !skip_cdp_for_debug && url.as_str().ends_with(&page_load_placeholder_fragment) {
            log::debug!(
                "[webview-accounts][{}] skipping placeholder native-finished url={}",
                page_load_account_id,
                redact_url_for_log(url.as_str())
            );
            return;
        }
        if url.scheme() == "chrome-error" {
            emit_load_finished(
                &page_load_app,
                &page_load_account_id,
                "timeout",
                &page_load_real_url,
                RevealTrigger::Load,
            );
            return;
        }
        emit_load_finished(
            &page_load_app,
            &page_load_account_id,
            "finished",
            url.as_str(),
            RevealTrigger::Load,
        );
    });

    let bounds = args.bounds.unwrap_or(Bounds {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });

    // Park the webview off-screen during its first page load so the React
    // placeholder's loading spinner is not covered by the native CEF subview.
    // `webview_account_reveal` (invoked from the frontend after the load event
    // arrives, or by the 15 s watchdog) moves it back to `bounds` + shows it.
    //
    // Warm-open reuse (when a webview already exists for this account) earlier
    // in this function returns before we get here, so existing webviews keep
    // their current position — we only off-screen the first cold spawn.
    // Spawn strategy: keep the webview at the caller's requested position
    // but shrink the initial size to 1×1 under CEF so the native subview
    // doesn't paint over the React loading spinner. `webview_account_reveal`
    // grows it back to `bounds.width × bounds.height` once the page-loaded
    // signal arrives.
    //
    // Why not move off-screen: moving the NSView after a cold CEF spawn on
    // macOS sometimes leaves the page painted but not repainted at the new
    // origin, leaving the user looking at a blank viewport until they
    // reload. Keeping the position stable and only toggling size sidesteps
    // that repaint edge case while still keeping the webview visually
    // hidden (1 px under the overlay) during load.
    //
    // Issue #1233 — when `args.prewarm == true`, the frontend has not asked
    // for a visible rect (the user hasn't clicked the rail icon yet). Spawn
    // the webview at a fixed off-screen position with size 1×1 so it never
    // paints anywhere on screen until the eventual user-initiated open
    // promotes it via the warm-reopen branch above.
    let (initial_position, initial_size) = if args.prewarm {
        (
            LogicalPosition::new(PREWARM_OFFSCREEN_X, PREWARM_OFFSCREEN_Y),
            LogicalSize::new(1.0, 1.0),
        )
    } else if skip_cdp_for_debug {
        (
            LogicalPosition::new(bounds.x, bounds.y),
            LogicalSize::new(bounds.width, bounds.height),
        )
    } else {
        (
            LogicalPosition::new(bounds.x, bounds.y),
            LogicalSize::new(1.0, 1.0),
        )
    };

    // Issue #1233 — only remember `requested_bounds` for non-prewarm opens.
    // Prewarm doesn't have a visible rect to restore to; the user-initiated
    // open later supplies the bounds via the warm-reopen branch.
    if !args.prewarm {
        state
            .requested_bounds
            .lock()
            .unwrap()
            .insert(args.account_id.clone(), bounds);
    }
    // Issue #1233 — mark the account as prewarmed BEFORE add_child so the
    // load-event suppression in `emit_load_finished` is in place by the time
    // the CDP session or native on_page_load fires.
    if args.prewarm {
        state
            .prewarm_accounts
            .lock()
            .unwrap()
            .insert(args.account_id.clone());
    }
    // Defensive reset: if a prior close/purge was raced by a stale emit we
    // could still have the account marked as "already loaded". Clear here so
    // the fresh spawn is allowed to fire the first event again.
    state
        .loaded_accounts
        .lock()
        .unwrap()
        .remove(&args.account_id);

    let webview = parent_window
        .add_child(builder, initial_position, initial_size)
        .map_err(|e| format!("add_child failed: {e}"))?;

    // Capture the cold-spawn timestamp so the reveal-time log can compute
    // spawn -> frontend reveal latency for the Slack first-load investigation.
    state
        .spawn_started_at
        .lock()
        .unwrap()
        .insert(args.account_id.clone(), Instant::now());
    state
        .account_providers
        .lock()
        .unwrap()
        .insert(args.account_id.clone(), args.provider.clone());

    log::info!(
        "[webview-accounts] spawned label={} requested_bounds={:?} initial_size={:?}",
        webview.label(),
        bounds,
        initial_size
    );

    state
        .inner
        .lock()
        .unwrap()
        .insert(args.account_id.clone(), label.clone());

    // Spawn the per-account CDP session opener: holds an attached session
    // for the lifetime of the webview so `Emulation.setUserAgentOverride`
    // (which reverts on detach) keeps applying, and drives the initial
    // Page.navigate from our placeholder URL to the real provider URL.
    // Also installs the `#aura-account-{id}` fragment the scanners
    // match on for multi-account disambiguation.
    // Spawn the per-account CDP session opener, replacing any prior
    // handle for this account (the old one would still be trying to
    // attach to a target that's been torn down).
    {
        if skip_cdp_for_debug {
            log::info!(
                "[webview-accounts] skipping CDP session via AURA_DISABLE_SLACK_SCANNER for account={}",
                args.account_id
            );
        } else {
            let cdp::SpawnedSession { session, watchdog } =
                cdp::spawn_session(app.clone(), args.account_id.clone(), real_url_str.clone());
            if let Some(old) = state
                .cdp_sessions
                .lock()
                .unwrap()
                .insert(args.account_id.clone(), session)
            {
                old.abort();
            }
            if let Some(old) = state
                .load_watchdogs
                .lock()
                .unwrap()
                .insert(args.account_id.clone(), watchdog)
            {
                old.abort();
            }
        }
    }

    // For providers we know how to scrape via CDP, kick off the IndexedDB
    // scanner. CDP requires the CEF runtime's remote-debugging port.
    {
        // Prefix is derived from the validated real URL's origin above
        // so debug `args.url` overrides (alt hosts, localhost mirrors)
        // resolve correctly — previously we always used the static
        // `provider_url(...)` default even when the webview had
        // navigated elsewhere.
        if args.provider == "whatsapp" {
            let registry = app
                .try_state::<std::sync::Arc<crate::whatsapp_scanner::ScannerRegistry>>()
                .map(|s| s.inner().clone());
            if let Some(registry) = registry {
                registry.ensure_scanner(
                    app.clone(),
                    args.account_id.clone(),
                    scanner_url_prefix.clone(),
                );
            } else {
                log::warn!("[webview-accounts] CDP ScannerRegistry not in app state");
            }
        } else if args.provider == "slack" {
            if slack_scanner_enabled() {
                let registry = app
                    .try_state::<std::sync::Arc<crate::slack_scanner::ScannerRegistry>>()
                    .map(|s| s.inner().clone());
                if let Some(registry) = registry {
                    registry.ensure_scanner(
                        app.clone(),
                        args.account_id.clone(),
                        scanner_url_prefix.clone(),
                    );
                } else {
                    log::warn!("[webview-accounts] slack ScannerRegistry not in app state");
                }
            } else {
                log::info!(
                    "[webview-accounts] slack scanner disabled via AURA_DISABLE_SLACK_SCANNER for account={}",
                    args.account_id
                );
            }
        } else if args.provider == "telegram" {
            let registry = app
                .try_state::<std::sync::Arc<crate::telegram_scanner::ScannerRegistry>>()
                .map(|s| s.inner().clone());
            if let Some(registry) = registry {
                registry.ensure_scanner(
                    app.clone(),
                    args.account_id.clone(),
                    scanner_url_prefix.clone(),
                );
            } else {
                log::warn!("[webview-accounts] telegram ScannerRegistry not in app state");
            }
        } else if args.provider == "discord" {
            // Discord MITM uses CDP `Network.*` to capture HTTP API calls
            // and gateway WebSocket frames — see `discord_scanner/mod.rs`.
            let registry = app
                .try_state::<std::sync::Arc<crate::discord_scanner::ScannerRegistry>>()
                .map(|s| s.inner().clone());
            if let Some(registry) = registry {
                registry.ensure_scanner(
                    app.clone(),
                    args.account_id.clone(),
                    scanner_url_prefix.clone(),
                );
            } else {
                log::warn!("[webview-accounts] discord ScannerRegistry not in app state");
            }
        }

        // Browser Notification interception, native CEF path. The renderer
        // subprocess (cef-helper) has already replaced `window.Notification`
        // and `ServiceWorkerRegistration.prototype.showNotification` with
        // V8 native bindings that send a `"aura.notify"` ProcessMessage
        // to the browser process. `tauri-runtime-cef::notification::register`
        // installs a per-browser callback that the runtime invokes when that
        // IPC arrives. We need the CEF browser id to key the registration —
        // hence the `with_webview` downcast hop. The callback is dispatched
        // from a CEF thread, so keep work inside it short / non-blocking.
        let app_for_register = app.clone();
        let acct_for_register = args.account_id.clone();
        let provider_for_register = args.provider.clone();
        if let Err(err) = webview.with_webview(move |raw| {
            let Some(browser) = raw.downcast_ref::<cef::Browser>() else {
                log::warn!(
                    "[notify-cef] with_webview returned non-cef::Browser handle for account={} — skipping notification registration",
                    acct_for_register
                );
                return;
            };
            let browser_id = browser.identifier();
            if let Some(state) = app_for_register.try_state::<WebviewAccountsState>() {
                state
                    .browser_ids
                    .lock()
                    .unwrap()
                    .insert(acct_for_register.clone(), browser_id);
            }
            let acct_in_handler = acct_for_register.clone();
            let provider_in_handler = provider_for_register.clone();
            let app_in_handler = app_for_register.clone();
            tauri_runtime_cef::notification::register(browser_id, move |payload| {
                forward_native_notification(
                    &app_in_handler,
                    &acct_in_handler,
                    &provider_in_handler,
                    &payload,
                );
            });
            log::info!(
                "[notify-cef] registered handler account={} provider={} browser_id={}",
                acct_for_register,
                provider_for_register,
                browser_id
            );
        }) {
            log::warn!(
                "[notify-cef] with_webview dispatch failed for account={}: {}",
                args.account_id,
                err
            );
        }
    }

    Ok(label)
}

/// Off-screen position used for the prewarmed webview. Same magnitude as
/// the [`crate::lib::CEF_PREWARM_LABEL`] warmup placeholder so the native
/// view is well outside any plausible monitor layout. Issue #1233.
pub(crate) const PREWARM_OFFSCREEN_X: f64 = -20_000.0;
pub(crate) const PREWARM_OFFSCREEN_Y: f64 = -20_000.0;
