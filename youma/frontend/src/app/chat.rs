/// Chat page — split view: thread list on the left, message area + compose on
/// the right.
///
/// ```text
/// ┌──────────────┬────────────────────────────────┐
/// │ Thread List  │  Message View                  │
/// │              │  ┌──────────────────────────┐  │
/// │ [+ New]      │  │ [user] Hello             │  │
/// │              │  │ [ai]   Response...       │  │
/// │ Thread 1     │  └──────────────────────────┘  │
/// │ Thread 2     │  ┌──────────────────────────┐  │
/// │              │  │ Input...     [Send]      │  │
/// │              │  └──────────────────────────┘  │
/// └──────────────┴────────────────────────────────┘
/// ```
use gloo_console::log;
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use super::api;
use super::state::{AppAction, AppCtx};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a displayable title for a thread. Falls back to the thread id, then
/// to a static placeholder when neither field is present.
fn thread_title(thread: &Value) -> String {
    thread["title"]
        .as_str()
        .or_else(|| thread["id"].as_str())
        .unwrap_or("(untitled)")
        .to_string()
}

/// Extract the thread id string from a thread value.
fn thread_id_str(thread: &Value) -> Option<String> {
    thread["id"].as_str().map(|s| s.to_string())
}

/// Extract the role label from a message value.
fn message_role(msg: &Value) -> String {
    msg["role"].as_str().unwrap_or("unknown").to_string()
}

/// Extract the text content from a message value.
///
/// Handles two shapes returned by the core:
/// - Flat: `{ "content": "hello" }`
/// - Nested: `{ "content": [{ "text": "hello" }] }`
fn message_content(msg: &Value) -> String {
    if let Some(s) = msg["content"].as_str() {
        return s.to_string();
    }
    if let Some(arr) = msg["content"].as_array() {
        if let Some(first) = arr.first() {
            if let Some(s) = first["text"].as_str() {
                return s.to_string();
            }
        }
    }
    "(empty)".to_string()
}

/// Parse the thread list from a raw RPC response.
///
/// The core may return `{ "result": [...] }` or the array directly.
fn parse_threads(response: &Value) -> Vec<Value> {
    if let Some(arr) = response["result"].as_array() {
        return arr.clone();
    }
    if let Some(arr) = response.as_array() {
        return arr.clone();
    }
    vec![]
}

/// Parse the message list from a raw RPC response.
fn parse_messages(response: &Value) -> Vec<Value> {
    if let Some(arr) = response["result"].as_array() {
        return arr.clone();
    }
    if let Some(arr) = response.as_array() {
        return arr.clone();
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Chat page root component
// ---------------------------------------------------------------------------

#[function_component(ChatPage)]
pub fn chat_page() -> Html {
    let ctx = use_context::<AppCtx>().expect("[chat] AppCtx missing from tree");

    // Local UI state
    let compose_text = use_state(|| String::new());
    let agent_loading = use_state(|| false);
    let error_msg = use_state(|| Option::<String>::None);

    // ----- Load thread list on mount ------------------------------------------
    {
        let ctx = ctx.clone();
        let error_msg = error_msg.clone();
        use_effect_with((), move |_| {
            log!("[chat] mount — loading thread list");
            spawn_local(async move {
                match api::threads_list().await {
                    Ok(resp) => {
                        log!("[chat] threads_list ok");
                        let threads = parse_threads(&resp);
                        ctx.dispatch(AppAction::SetThreads(threads));
                    }
                    Err(e) => {
                        log!("[chat] threads_list error:", &e);
                        error_msg.set(Some(format!("Failed to load threads: {e}")));
                    }
                }
            });
            || ()
        });
    }

    // ----- Callbacks ----------------------------------------------------------

    // Create a new thread then select it.
    let on_new_thread = {
        let ctx = ctx.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_: MouseEvent| {
            let ctx = ctx.clone();
            let error_msg = error_msg.clone();
            log!("[chat] creating new thread");
            spawn_local(async move {
                match api::threads_create_new(None).await {
                    Ok(resp) => {
                        log!("[chat] threads_create_new ok");
                        // Reload the full list, then pick the new thread id.
                        match api::threads_list().await {
                            Ok(list_resp) => {
                                let threads = parse_threads(&list_resp);
                                // Try to get the id from the create-response first,
                                // fall back to the last element of the refreshed list.
                                let new_id: Option<String> = resp["id"]
                                    .as_str()
                                    .or_else(|| resp["result"]["id"].as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        threads
                                            .last()
                                            .and_then(|t| t["id"].as_str())
                                            .map(|s| s.to_string())
                                    });
                                ctx.dispatch(AppAction::SetThreads(threads));
                                if let Some(id) = new_id {
                                    log!("[chat] selecting new thread:", &id);
                                    ctx.dispatch(AppAction::SelectThread(id.clone()));
                                    // Load messages for the new (empty) thread.
                                    match api::threads_messages_list(&id).await {
                                        Ok(m) => {
                                            ctx.dispatch(AppAction::SetMessages(
                                                parse_messages(&m),
                                            ));
                                        }
                                        Err(e) => {
                                            log!("[chat] messages load after create error:", &e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log!("[chat] threads_list after create failed:", &e);
                                error_msg.set(Some(format!(
                                    "Thread created but list reload failed: {e}"
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        log!("[chat] threads_create_new error:", &e);
                        error_msg.set(Some(format!("Failed to create thread: {e}")));
                    }
                }
            });
        })
    };

    // Select an existing thread and load its messages.
    let on_select_thread = {
        let ctx = ctx.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |tid: String| {
            let ctx = ctx.clone();
            let error_msg = error_msg.clone();
            log!("[chat] selecting thread:", &tid);
            ctx.dispatch(AppAction::SelectThread(tid.clone()));
            spawn_local(async move {
                match api::threads_messages_list(&tid).await {
                    Ok(resp) => {
                        log!("[chat] threads_messages_list ok");
                        ctx.dispatch(AppAction::SetMessages(parse_messages(&resp)));
                    }
                    Err(e) => {
                        log!("[chat] threads_messages_list error:", &e);
                        error_msg.set(Some(format!("Failed to load messages: {e}")));
                    }
                }
            });
        })
    };

    // Update the compose input value.
    let on_compose_input = {
        let compose_text = compose_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            compose_text.set(input.value());
        })
    };

    // Send the composed message to the agent.
    let on_send = {
        let ctx = ctx.clone();
        let compose_text = compose_text.clone();
        let agent_loading = agent_loading.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_: MouseEvent| {
            let content = (*compose_text).trim().to_string();
            if content.is_empty() {
                return;
            }
            let thread_id = match ctx.current_thread_id.clone() {
                Some(id) => id,
                None => {
                    error_msg.set(Some("Select or create a thread first.".to_string()));
                    return;
                }
            };

            let ctx = ctx.clone();
            let compose_text = compose_text.clone();
            let agent_loading = agent_loading.clone();
            let error_msg = error_msg.clone();

            log!("[chat] sending message to thread:", &thread_id);
            compose_text.set(String::new());
            agent_loading.set(true);

            spawn_local(async move {
                // Optimistic user bubble.
                ctx.dispatch(AppAction::PushMessage(serde_json::json!({
                    "role": "user",
                    "content": content.clone(),
                })));

                // Append user message — backend also calls AI and appends assistant reply.
                match api::threads_message_append(&thread_id, "user", &content).await {
                    Ok(resp) => {
                        log!("[chat] threads_message_append ok");
                        // Backend returns {user_message, assistant_message}.
                        let assistant_text = resp["assistant_message"]["content"]
                            .as_str()
                            .unwrap_or("(no response)")
                            .to_string();
                        ctx.dispatch(AppAction::PushMessage(serde_json::json!({
                            "role": "assistant",
                            "content": assistant_text,
                        })));
                    }
                    Err(e) => {
                        log!("[chat] threads_message_append error:", &e);
                        error_msg.set(Some(format!("Error: {e}")));
                    }
                }

                agent_loading.set(false);
            });
        })
    };

    // Submit on Enter (without Shift).
    let on_compose_keydown = {
        let on_send = on_send.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() {
                e.prevent_default();
                // Synthesise a click event to reuse the send callback.
                if let Ok(click) = MouseEvent::new("click") {
                    on_send.emit(click);
                }
            }
        })
    };

    // ----- Derived values for the template ------------------------------------

    let threads = ctx.threads.clone();
    let current_thread_id = ctx.current_thread_id.clone();
    let messages = ctx.messages.clone();
    let is_thread_selected = current_thread_id.is_some();
    let send_disabled = *agent_loading || !is_thread_selected;

    // ----- Render -------------------------------------------------------------

    html! {
        <div class="chat-layout">

            // ---- Thread sidebar ------------------------------------------------
            <aside class="thread-list">
                <div class="thread-list__header">
                    <span class="thread-list__title">{"Threads"}</span>
                    <button class="button thread-list__new" onclick={on_new_thread}>
                        {"+ New"}
                    </button>
                </div>

                if threads.is_empty() {
                    <p class="thread-list__empty">{"No threads yet. Create one to start."}</p>
                }

                { for threads.iter().map(|thread| {
                    let tid = thread_id_str(thread);
                    let title = thread_title(thread);
                    let is_active = tid.as_ref() == current_thread_id.as_ref();

                    let item_class = if is_active {
                        classes!("thread-item", "thread-item--active")
                    } else {
                        classes!("thread-item")
                    };

                    let on_select = on_select_thread.clone();
                    let tid_click = tid.clone();

                    html! {
                        <button
                            key={tid.clone().unwrap_or_default()}
                            class={item_class}
                            onclick={Callback::from(move |_| {
                                if let Some(ref id) = tid_click {
                                    on_select.emit(id.clone());
                                }
                            })}
                        >
                            { title }
                        </button>
                    }
                }) }
            </aside>

            // ---- Message pane --------------------------------------------------
            <section class="message-pane">

                // Thread header / empty-selection placeholder
                if let Some(ref tid) = current_thread_id {
                    <div class="message-pane__header">
                        <span class="message-pane__thread-label">
                            { format!("Thread: {}", tid) }
                        </span>
                    </div>
                } else {
                    <div class="message-pane__placeholder">
                        {"Select a thread or create a new one to start chatting."}
                    </div>
                }

                // Message list
                <div class="message-area">
                    { for messages.iter().map(|msg| {
                        let role = message_role(msg);
                        let content = message_content(msg);
                        let key = format!("{}-{}", role, content.len());
                        let msg_class = if role == "user" {
                            classes!("message", "message--user")
                        } else {
                            classes!("message", "message--assistant")
                        };
                        html! {
                            <div class={msg_class} key={key}>
                                <span class="message__role">{ format!("[{}]", role) }</span>
                                <span class="message__content">{ content }</span>
                            </div>
                        }
                    }) }

                    // Thinking indicator while waiting for the agent
                    if *agent_loading {
                        <div class={classes!("message", "message--assistant", "message--loading")}>
                            <span class="message__role">{"[assistant]"}</span>
                            <span class={classes!("message__content", "message__thinking")}>
                                {"Thinking\u{2026}"}
                            </span>
                        </div>
                    }
                </div>

                // Error banner
                if let Some(ref err) = *error_msg {
                    <div class="chat-error">
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

                // Compose bar
                <div class="compose-area">
                    <input
                        class="compose-area__input"
                        type="text"
                        placeholder="Type a message\u{2026} (Enter to send)"
                        value={(*compose_text).clone()}
                        oninput={on_compose_input}
                        onkeydown={on_compose_keydown}
                        disabled={send_disabled}
                    />
                    <button
                        class="button compose-area__send"
                        onclick={on_send}
                        disabled={send_disabled}
                    >
                        if *agent_loading { {"\u{2026}"} } else { {"Send"} }
                    </button>
                </div>

            </section>
        </div>
    }
}
