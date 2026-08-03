use std::rc::Rc;
use yew::prelude::*;

/// Global application state. All core data is stored as `serde_json::Value` so
/// we avoid mirroring the full openhuman domain model on the frontend while still
/// being able to display and pass it around.
#[derive(Clone, PartialEq, Default)]
pub struct AppState {
    /// True once `core.ping` succeeds, meaning the embedded core is reachable.
    pub core_ready: bool,
    /// Non-empty when the core is unreachable or returned an error on startup.
    pub core_error: Option<String>,
    /// Snapshot from `openhuman.app_state_snapshot`.
    pub app_snapshot: Option<serde_json::Value>,
    /// List returned by `openhuman.threads_list`.
    pub threads: Vec<serde_json::Value>,
    /// ID of the thread currently open in the chat view.
    pub current_thread_id: Option<String>,
    /// Messages in the currently selected thread (`openhuman.threads_messages_list`).
    pub messages: Vec<serde_json::Value>,
    /// Channel list from `openhuman.channels_list`.
    pub channels: Vec<serde_json::Value>,
    /// Latest health snapshot from `openhuman.health_snapshot`.
    pub health: Option<serde_json::Value>,
    /// Core configuration from `openhuman.config_get`.
    pub config: Option<serde_json::Value>,
}

/// All state mutations expressed as discriminated actions so components never
/// mutate the state struct directly — they dispatch an `AppAction` instead.
pub enum AppAction {
    /// Mark the embedded core as reachable and clear any prior error.
    SetCoreReady,
    /// Record that the core could not be reached (stores the error message).
    SetCoreError(String),
    /// Replace the full app snapshot value.
    SetAppSnapshot(serde_json::Value),
    /// Replace the full thread list.
    SetThreads(Vec<serde_json::Value>),
    /// Set the currently selected thread (also clears the message list so
    /// the chat view starts from a clean slate before messages are loaded).
    SelectThread(String),
    /// Deselect the current thread and clear its messages.
    ClearThread,
    /// Replace the message list for the current thread.
    SetMessages(Vec<serde_json::Value>),
    /// Append a single optimistically-rendered message to the current list.
    PushMessage(serde_json::Value),
    /// Replace the channel list.
    SetChannels(Vec<serde_json::Value>),
    /// Replace the latest health snapshot.
    SetHealth(serde_json::Value),
    /// Replace the configuration snapshot.
    SetConfig(serde_json::Value),
}

impl Reducible for AppState {
    type Action = AppAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let next = match action {
            AppAction::SetCoreReady => AppState {
                core_ready: true,
                core_error: None,
                ..(*self).clone()
            },
            AppAction::SetCoreError(msg) => AppState {
                core_ready: false,
                core_error: Some(msg),
                ..(*self).clone()
            },
            AppAction::SetAppSnapshot(snapshot) => AppState {
                app_snapshot: Some(snapshot),
                ..(*self).clone()
            },
            AppAction::SetThreads(threads) => AppState {
                threads,
                ..(*self).clone()
            },
            AppAction::SelectThread(id) => AppState {
                current_thread_id: Some(id),
                // Clear stale messages so the chat view doesn't flash old content.
                messages: Vec::new(),
                ..(*self).clone()
            },
            AppAction::ClearThread => AppState {
                current_thread_id: None,
                messages: Vec::new(),
                ..(*self).clone()
            },
            AppAction::SetMessages(messages) => AppState {
                messages,
                ..(*self).clone()
            },
            AppAction::PushMessage(msg) => {
                let mut messages = (*self).messages.clone();
                messages.push(msg);
                AppState {
                    messages,
                    ..(*self).clone()
                }
            }
            AppAction::SetChannels(channels) => AppState {
                channels,
                ..(*self).clone()
            },
            AppAction::SetHealth(health) => AppState {
                health: Some(health),
                ..(*self).clone()
            },
            AppAction::SetConfig(config) => AppState {
                config: Some(config),
                ..(*self).clone()
            },
        };
        Rc::new(next)
    }
}

/// Convenience type alias used throughout the component tree.
pub type AppCtx = UseReducerHandle<AppState>;
