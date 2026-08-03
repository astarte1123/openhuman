/// Home / Intelligence page.
///
/// Shows:
/// - Core connectivity status (green/red dot + label)
/// - Health snapshot card (version, uptime from `openhuman.health_snapshot`)
/// - App snapshot info (user email/name from `openhuman.app_state_snapshot`)
/// - Quick-action buttons: "New Chat" and "View Channels"
use gloo_console::log;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::Link;

use super::api;
use super::state::{AppAction, AppCtx};
use crate::app::Route;

// ---------------------------------------------------------------------------
// Home page component
// ---------------------------------------------------------------------------

#[function_component(HomePage)]
pub fn home_page() -> Html {
    let ctx = use_context::<AppCtx>().expect("[home] AppCtx missing from tree");

    // ----- Load health snapshot on mount --------------------------------------
    {
        let ctx = ctx.clone();
        use_effect_with((), move |_| {
            log!("[home] mount — loading health snapshot");
            let ctx_health = ctx.clone();
            let ctx_snapshot = ctx.clone();

            spawn_local(async move {
                match api::health_snapshot().await {
                    Ok(resp) => {
                        log!("[home] health_snapshot ok");
                        // Surface connectivity as well.
                        ctx_health.dispatch(AppAction::SetCoreReady);
                        ctx_health.dispatch(AppAction::SetHealth(resp));
                    }
                    Err(e) => {
                        log!("[home] health_snapshot error:", &e);
                        ctx_health.dispatch(AppAction::SetCoreError(e.clone()));
                    }
                }
            });

            spawn_local(async move {
                match api::app_state_snapshot().await {
                    Ok(resp) => {
                        log!("[home] app_state_snapshot ok");
                        ctx_snapshot.dispatch(AppAction::SetAppSnapshot(resp));
                    }
                    Err(e) => {
                        log!("[home] app_state_snapshot error:", &e);
                        // Non-fatal: the snapshot is supplementary info only.
                    }
                }
            });

            || ()
        });
    }

    // ----- Derived values for the template ------------------------------------

    let core_ready = ctx.core_ready;
    let core_error = ctx.core_error.clone();
    let health = ctx.health.clone();
    let app_snapshot = ctx.app_snapshot.clone();

    // Status dot CSS classes
    let dot_class = if core_ready {
        classes!("status-dot", "status-dot--ok")
    } else {
        classes!("status-dot", "status-dot--error")
    };
    let status_label = if core_ready {
        "Core connected"
    } else {
        "Core unreachable"
    };

    // Health fields — extract with graceful fallbacks.
    let version = health
        .as_ref()
        .and_then(|h| {
            h["version"]
                .as_str()
                .or_else(|| h["result"]["version"].as_str())
        })
        .unwrap_or("—")
        .to_string();

    let uptime = health
        .as_ref()
        .map(|h| {
            if let Some(s) = h["uptime"].as_str() {
                return s.to_string();
            }
            if let Some(s) = h["result"]["uptime"].as_str() {
                return s.to_string();
            }
            if let Some(secs) = h["uptime_seconds"].as_u64() {
                return format!("{}s", secs);
            }
            "—".to_string()
        })
        .unwrap_or_else(|| "—".to_string());

    // App snapshot fields.
    let user_email = app_snapshot
        .as_ref()
        .and_then(|s| {
            s["user"]["email"]
                .as_str()
                .or_else(|| s["email"].as_str())
                .or_else(|| s["result"]["email"].as_str())
        })
        .unwrap_or("—")
        .to_string();

    let user_name = app_snapshot
        .as_ref()
        .and_then(|s| {
            s["user"]["name"]
                .as_str()
                .or_else(|| s["name"].as_str())
                .or_else(|| s["result"]["name"].as_str())
        })
        .unwrap_or("—")
        .to_string();

    // ----- Render -------------------------------------------------------------

    html! {
        <section class="page-card">
            <h2>{"Intelligence Dashboard"}</h2>

            // Core status indicator
            <div class="home-status-row">
                <span class={dot_class}></span>
                <span class="home-status-label">{ status_label }</span>
            </div>

            if let Some(ref err) = core_error {
                <p class="home-error">{ format!("Error: {}", err) }</p>
            }

            // Health card
            <div class="home-grid">
                <div class="home-info-card">
                    <h3 class="home-info-card__title">{"Core Health"}</h3>
                    <dl class="home-info-card__dl">
                        <dt>{"Version"}</dt>
                        <dd>{ &version }</dd>
                        <dt>{"Uptime"}</dt>
                        <dd>{ &uptime }</dd>
                    </dl>
                    if health.is_none() {
                        <p class="home-info-card__empty">{"Loading\u{2026}"}</p>
                    }
                </div>

                // User info card
                <div class="home-info-card">
                    <h3 class="home-info-card__title">{"Session"}</h3>
                    <dl class="home-info-card__dl">
                        <dt>{"Name"}</dt>
                        <dd>{ &user_name }</dd>
                        <dt>{"Email"}</dt>
                        <dd>{ &user_email }</dd>
                    </dl>
                    if app_snapshot.is_none() {
                        <p class="home-info-card__empty">{"Loading\u{2026}"}</p>
                    }
                </div>
            </div>

            // Quick actions
            <div class="hero-actions" style="margin-top: 24px;">
                <Link<Route> to={Route::Chat} classes="button">
                    {"\u{1f4ac} New Chat"}
                </Link<Route>>
                <Link<Route> to={Route::Channels} classes="button button--secondary">
                    {"\u{1f4e1} View Channels"}
                </Link<Route>>
            </div>
        </section>
    }
}
