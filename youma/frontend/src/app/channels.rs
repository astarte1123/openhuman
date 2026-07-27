/// Channels page.
///
/// Lists all connected channels returned by `openhuman.channels_list`.
/// Shows provider name, channel id, and a status indicator dot for each entry.
/// Displays a friendly empty-state message when no channels are configured.
use gloo_console::log;
use serde_json::Value;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::api;
use super::state::{AppAction, AppCtx};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse the channel list from a raw RPC response.
///
/// Supports `{ "result": [...] }` and bare-array shapes.
fn parse_channels(response: &Value) -> Vec<Value> {
    if let Some(arr) = response["result"].as_array() {
        return arr.clone();
    }
    if let Some(arr) = response.as_array() {
        return arr.clone();
    }
    vec![]
}

/// Extract a human-readable display name for a channel.
fn channel_display_name(ch: &Value) -> String {
    ch["name"]
        .as_str()
        .or_else(|| ch["provider"].as_str())
        .or_else(|| ch["id"].as_str())
        .unwrap_or("(unknown)")
        .to_string()
}

/// Extract the provider label.
fn channel_provider(ch: &Value) -> String {
    ch["provider"].as_str().unwrap_or("—").to_string()
}

/// Extract the status string; default to "unknown".
fn channel_status(ch: &Value) -> String {
    ch["status"].as_str().unwrap_or("unknown").to_string()
}

/// Map a status string to a CSS modifier class for the status dot.
fn status_dot_class(status: &str) -> &'static str {
    match status {
        "connected" | "active" | "ok" => "status-dot--ok",
        "error" | "disconnected" | "failed" => "status-dot--error",
        _ => "status-dot--unknown",
    }
}

// ---------------------------------------------------------------------------
// Channels page component
// ---------------------------------------------------------------------------

#[function_component(ChannelsPage)]
pub fn channels_page() -> Html {
    let ctx = use_context::<AppCtx>().expect("[channels] AppCtx missing from tree");
    let error_msg = use_state(|| Option::<String>::None);

    // ----- Load channel list on mount -----------------------------------------
    {
        let ctx = ctx.clone();
        let error_msg = error_msg.clone();
        use_effect_with((), move |_| {
            log!("[channels] mount — loading channel list");
            spawn_local(async move {
                match api::channels_list().await {
                    Ok(resp) => {
                        log!("[channels] channels_list ok");
                        let channels = parse_channels(&resp);
                        ctx.dispatch(AppAction::SetChannels(channels));
                    }
                    Err(e) => {
                        log!("[channels] channels_list error:", &e);
                        error_msg.set(Some(format!("Failed to load channels: {e}")));
                    }
                }
            });
            || ()
        });
    }

    // ----- Callbacks ----------------------------------------------------------

    // Manual reload button.
    let on_reload = {
        let ctx = ctx.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_: MouseEvent| {
            let ctx = ctx.clone();
            let error_msg = error_msg.clone();
            log!("[channels] reloading channel list");
            spawn_local(async move {
                match api::channels_list().await {
                    Ok(resp) => {
                        log!("[channels] channels_list reload ok");
                        ctx.dispatch(AppAction::SetChannels(parse_channels(&resp)));
                        error_msg.set(None);
                    }
                    Err(e) => {
                        log!("[channels] channels_list reload error:", &e);
                        error_msg.set(Some(format!("Reload failed: {e}")));
                    }
                }
            });
        })
    };

    // ----- Render -------------------------------------------------------------

    let channels = ctx.channels.clone();

    html! {
        <section class="page-card">
            <div class="channels-header">
                <h2>{"Channels"}</h2>
                <button class="button button--secondary channels-header__reload" onclick={on_reload}>
                    {"Reload"}
                </button>
            </div>

            // Error banner
            if let Some(ref err) = *error_msg {
                <div class="channels-error">
                    <span>{ err }</span>
                    <button
                        class="chat-error__dismiss"
                        onclick={{
                            let error_msg = error_msg.clone();
                            Callback::from(move |_: MouseEvent| error_msg.set(None))
                        }}
                    >
                        {"\u{00d7}"}
                    </button>
                </div>
            }

            if channels.is_empty() {
                <div class="channels-empty">
                    <p>{"No channels connected yet."}</p>
                    <p class="channels-empty__hint">
                        {"Connect a channel in the OpenHuman core to see it here."}
                    </p>
                </div>
            } else {
                <ul class="channel-list">
                    { for channels.iter().map(|ch| {
                        let name = channel_display_name(ch);
                        let provider = channel_provider(ch);
                        let status = channel_status(ch);
                        let dot_mod = status_dot_class(&status);
                        let key = ch["id"].as_str().unwrap_or(&name).to_string();

                        html! {
                            <li class="channel-item" key={key}>
                                <div class="channel-item__info">
                                    <span class="channel-item__name">{ &name }</span>
                                    <span class="channel-item__provider">{ format!("via {}", provider) }</span>
                                </div>
                                <div class="channel-item__status">
                                    <span class={classes!("status-dot", dot_mod)}></span>
                                    <span class="channel-item__status-label">{ &status }</span>
                                </div>
                            </li>
                        }
                    }) }
                </ul>
            }
        </section>
    }
}
