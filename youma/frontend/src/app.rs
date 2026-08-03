use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::{HashRouter, Link, Routable, Switch};

pub mod api;
pub mod state;
pub mod chat;
pub mod home;
pub mod channels;
pub mod settings;
pub mod notifications;

use state::{AppAction, AppCtx, AppState};

// ── Routes ──────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Welcome,
    #[at("/home")]
    Home,
    #[at("/chat")]
    Chat,
    #[at("/channels")]
    Channels,
    #[at("/notifications")]
    Notifications,
    #[at("/settings")]
    Settings,
    #[not_found]
    #[at("/*")]
    NotFound,
}

fn switch(route: Route) -> Html {
    match route {
        Route::Welcome  => html! { <WelcomePage /> },
        Route::Home     => html! { <home::HomePage /> },
        Route::Chat     => html! { <chat::ChatPage /> },
        Route::Channels => html! { <channels::ChannelsPage /> },
        Route::Notifications => html! { <notifications::NotificationsPage /> },
        Route::Settings => html! { <settings::SettingsPage /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}

// ── App root ─────────────────────────────────────────────────────────────────

#[function_component(App)]
pub fn app() -> Html {
    let state = use_reducer(AppState::default);

    // On mount: probe the core and load initial data.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                // 1. Ping the core (in-process, always ready after setup).
                match api::core_ping().await {
                    Ok(true) => state.dispatch(AppAction::SetCoreReady),
                    Ok(false) => state.dispatch(AppAction::SetCoreError("core ping returned false".into())),
                    Err(e) => state.dispatch(AppAction::SetCoreError(e)),
                }

                // 2. Load app snapshot.
                if let Ok(snap) = api::app_state_snapshot().await {
                    state.dispatch(AppAction::SetAppSnapshot(snap));
                }
            });
            || ()
        });
    }

    html! {
        <ContextProvider<AppCtx> context={state}>
            <HashRouter>
                <div class="app-shell">
                    <AppTopbar />
                    <main class="content">
                        <Switch<Route> render={switch} />
                    </main>
                    <AppFooter />
                </div>
            </HashRouter>
        </ContextProvider<AppCtx>>
    }
}

// ── Topbar ───────────────────────────────────────────────────────────────────

#[function_component(AppTopbar)]
fn app_topbar() -> Html {
    let ctx = use_context::<AppCtx>().expect("AppCtx missing");
    let dot_class = if ctx.core_ready { "status-dot status-dot--ok" } else { "status-dot status-dot--error" };

    html! {
        <header class="topbar">
            <div class="brand">
                <span class="brand__logo">{"Youma"}</span>
                <span class={dot_class} title={if ctx.core_ready { "Core online" } else { "Core offline" }} />
            </div>
            <nav class="topnav">
                <Link<Route> to={Route::Home}          classes="topnav__link">{"Home"}</Link<Route>>
                <Link<Route> to={Route::Chat}          classes="topnav__link">{"Chat"}</Link<Route>>
                <Link<Route> to={Route::Channels}      classes="topnav__link">{"Channels"}</Link<Route>>
                <Link<Route> to={Route::Notifications} classes="topnav__link">{"Notifications"}</Link<Route>>
                <Link<Route> to={Route::Settings}      classes="topnav__link">{"Settings"}</Link<Route>>
            </nav>
        </header>
    }
}

// ── Footer ───────────────────────────────────────────────────────────────────

#[function_component(AppFooter)]
fn app_footer() -> Html {
    let ctx = use_context::<AppCtx>().expect("AppCtx missing");
    let status = if ctx.core_ready {
        "Core online".to_string()
    } else if let Some(e) = &ctx.core_error {
        format!("Core offline — {}", e)
    } else {
        "Connecting…".to_string()
    };
    html! {
        <footer class="bottom-bar">
            <span>{"Youma — Rust + Tauri v2"}</span>
            <span class="footer-status">{status}</span>
        </footer>
    }
}

// ── Welcome ──────────────────────────────────────────────────────────────────

#[function_component(WelcomePage)]
fn welcome_page() -> Html {
    html! {
        <section class="page-card welcome-card">
            <h1>{"Welcome to Youma"}</h1>
            <p>{"OpenHuman reimplemented in Rust + Tauri v2. All core features proxied via JSON-RPC."}</p>
            <div class="hero-actions">
                <Link<Route> to={Route::Home} classes="button">{"Open Dashboard"}</Link<Route>>
                <Link<Route> to={Route::Chat} classes="button button--secondary">{"Start Chat"}</Link<Route>>
            </div>
        </section>
    }
}

// ── 404 ──────────────────────────────────────────────────────────────────────

#[function_component(NotFoundPage)]
fn not_found_page() -> Html {
    html! {
        <section class="page-card">
            <h2>{"404"}</h2>
            <p>{"Page not found."}</p>
            <Link<Route> to={Route::Welcome} classes="button">{"Go home"}</Link<Route>>
        </section>
    }
}
