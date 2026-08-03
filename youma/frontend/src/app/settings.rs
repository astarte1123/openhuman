use gloo_console::log;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use super::api;
use super::state::{AppAction, AppCtx};

#[function_component(SettingsPage)]
pub fn settings_page() -> Html {
    let ctx = use_context::<AppCtx>().expect("[settings] AppCtx missing");
    let loading = use_state(|| false);
    let error_msg = use_state(|| Option::<String>::None);
    let save_ok = use_state(|| false);

    // Input fields for API keys.
    let anthropic_key = use_state(String::new);
    let openai_key = use_state(String::new);
    let youma_model = use_state(String::new);

    // Load config on mount.
    {
        let ctx = ctx.clone();
        let loading = loading.clone();
        let error_msg = error_msg.clone();
        use_effect_with((), move |_| {
            log!("[settings] mount");
            loading.set(true);
            spawn_local(async move {
                match api::config_get().await {
                    Ok(resp) => {
                        ctx.dispatch(AppAction::SetConfig(resp));
                        error_msg.set(None);
                    }
                    Err(e) => {
                        error_msg.set(Some(format!("Failed to load config: {e}")));
                    }
                }
                loading.set(false);
            });
            || ()
        });
    }

    // Save API keys.
    let on_save = {
        let anthropic_key = anthropic_key.clone();
        let openai_key = openai_key.clone();
        let youma_model = youma_model.clone();
        let error_msg = error_msg.clone();
        let save_ok = save_ok.clone();
        let ctx = ctx.clone();
        Callback::from(move |_: MouseEvent| {
            let ak = (*anthropic_key).clone();
            let ok = (*openai_key).clone();
            let model = (*youma_model).clone();
            let error_msg = error_msg.clone();
            let save_ok = save_ok.clone();
            let ctx = ctx.clone();
            spawn_local(async move {
                let mut errs = Vec::new();
                if !ak.is_empty() {
                    if let Err(e) = api::set_env_var("ANTHROPIC_API_KEY", &ak).await {
                        errs.push(e);
                    }
                }
                if !ok.is_empty() {
                    if let Err(e) = api::set_env_var("OPENAI_API_KEY", &ok).await {
                        errs.push(e);
                    }
                }
                if !model.is_empty() {
                    if let Err(e) = api::set_env_var("youma_model", &model).await {
                        errs.push(e);
                    }
                }
                if errs.is_empty() {
                    // Reload config to show updated provider.
                    if let Ok(resp) = api::config_get().await {
                        ctx.dispatch(AppAction::SetConfig(resp));
                    }
                    save_ok.set(true);
                    error_msg.set(None);
                } else {
                    error_msg.set(Some(errs.join("; ")));
                }
            });
        })
    };

    let config = ctx.config.clone();
    let provider = config
        .as_ref()
        .and_then(|c| c["provider"].as_str())
        .unwrap_or("none")
        .to_string();
    let model = config
        .as_ref()
        .and_then(|c| c["model"].as_str())
        .unwrap_or("—")
        .to_string();

    let provider_class = if provider == "none" { "provider-badge provider-badge--none" } else { "provider-badge provider-badge--ok" };

    html! {
        <section class="page-card">
            <h2>{"Settings"}</h2>

            // Current AI status
            <div class="settings-status">
                <span class={provider_class}>{ format!("AI: {}", provider) }</span>
                <span class="settings-model">{ format!("Model: {}", model) }</span>
            </div>

            // API key form
            <div class="settings-form">
                <h3>{"Configure AI Provider"}</h3>
                <p class="settings-hint">{"Enter an API key to enable AI chat. Keys are saved locally and never sent to any server other than the provider."}</p>

                <label class="settings-label">{"Anthropic API Key (claude-sonnet-4-6)"}</label>
                <input
                    class="settings-input"
                    type="password"
                    placeholder="sk-ant-..."
                    value={(*anthropic_key).clone()}
                    oninput={{
                        let anthropic_key = anthropic_key.clone();
                        Callback::from(move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            anthropic_key.set(input.value());
                        })
                    }}
                />

                <label class="settings-label">{"OpenAI API Key (gpt-4o)"}</label>
                <input
                    class="settings-input"
                    type="password"
                    placeholder="sk-..."
                    value={(*openai_key).clone()}
                    oninput={{
                        let openai_key = openai_key.clone();
                        Callback::from(move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            openai_key.set(input.value());
                        })
                    }}
                />

                <label class="settings-label">{"Model Override (optional, e.g. claude-opus-4-7)"}</label>
                <input
                    class="settings-input"
                    type="text"
                    placeholder="leave blank for default"
                    value={(*youma_model).clone()}
                    oninput={{
                        let youma_model = youma_model.clone();
                        Callback::from(move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            youma_model.set(input.value());
                        })
                    }}
                />

                <button class="button" onclick={on_save} disabled={*loading}>
                    {"Save"}
                </button>

                if *save_ok {
                    <span class="settings-save-ok">{"Saved. Restart a chat to use the new key."}</span>
                }
            </div>

            // Error banner
            if let Some(ref err) = *error_msg {
                <div class="settings-error">
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

            // Stack info
            <div class="settings-stack">
                <h3>{"Stack"}</h3>
                <ul class="settings-stack__list">
                    <li>{"Runtime: Rust + Tauri v2"}</li>
                    <li>{"UI: Yew 0.21 + WebAssembly"}</li>
                    <li>{"Core: in-process (SQLite + AI)"}</li>
                    <li>{"Transport: Tauri IPC (core_invoke)"}</li>
                </ul>
            </div>
        </section>
    }
}
