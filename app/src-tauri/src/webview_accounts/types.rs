//! IPC argument/event DTOs for the webview-accounts commands, plus the
//! `RevealTrigger` reveal-source enum. Split out of `mod.rs` — no
//! behavior changes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
pub struct OpenArgs {
    pub account_id: String,
    pub provider: String,
    /// Optional URL override (debug tooling) — falls back to `provider_url`.
    pub url: Option<String>,
    pub bounds: Option<Bounds>,
    /// Issue #1233 — when true, spawn the webview off-screen and route the
    /// load through the prewarm-suppression path. The full handler/scanner/
    /// notification setup is identical to a normal cold open; only the
    /// initial position and the load-event emit are different. Defaults
    /// to false so the field is forwards-compatible with frontends that
    /// don't pass it.
    #[serde(default)]
    pub prewarm: bool,
}

/// Issue #1233 — args for the background `webview_account_prewarm` command.
/// No bounds — prewarm always spawns at a fixed off-screen 1×1 rect; the
/// user-initiated open later supplies the visible rect via the warm-reopen
/// branch in `webview_account_open`.
#[derive(Debug, Deserialize)]
pub struct PrewarmArgs {
    pub account_id: String,
    pub provider: String,
    /// Optional URL override (debug tooling) — falls back to `provider_url`.
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BoundsArgs {
    pub account_id: String,
    pub bounds: Bounds,
}

#[derive(Debug, Deserialize)]
pub struct RevealArgs {
    pub account_id: String,
    pub bounds: Bounds,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RevealTrigger {
    Load,
    Watchdog,
}

impl RevealTrigger {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Watchdog => "watchdog",
        }
    }

    pub(crate) fn from_ipc(raw: Option<&str>) -> Self {
        match raw {
            Some("load") | None => Self::Load,
            Some("watchdog") => Self::Watchdog,
            Some(other) => {
                log::warn!(
                    "[webview-accounts] unknown reveal trigger {:?}; defaulting to load",
                    other
                );
                Self::Load
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AccountIdArgs {
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RecipeEventArgs {
    pub account_id: String,
    pub provider: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebviewEvent {
    pub account_id: String,
    pub provider: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub ts: Option<i64>,
}
