//! Franz-style embedded webview accounts.
//!
//! Hosts third-party web apps (WhatsApp Web, Slack, …) as a child Tauri
//! `Webview` positioned inside the main React window at a rect chosen by the
//! UI. A small per-provider "recipe" JS file is injected via
//! `initialization_script` to scrape the DOM and pipe state back to Rust as
//! `webview_recipe_event` invocations. Rust forwards each event up to the
//! React UI as a `webview:event` Tauri event; React is responsible for
//! persisting interesting payloads to memory via the existing core RPC.
//!
//! Architecture:
//!   React → invoke('webview_account_open',  …)  → spawn child Webview
//!   React → invoke('webview_account_bounds', …) → reposition / resize
//!   recipe → invoke('webview_recipe_event',  …) → emit('webview:event', …)
//!
//! Per-account session isolation: each account gets its own
//! `data_directory` under `{app_local_data_dir}/webview_accounts/{id}` so
//! cookies and storage don't bleed between accounts (best-effort on
//! WKWebView — see Tauri docs on `data_store_identifier` for the macOS path).
//!
//! Split across sibling files to stay under the project's ~500-line-per-file
//! guideline; this file only declares the submodules and re-exports the
//! symbols other crate modules (`cdp::session`, `lib.rs`, the scanner
//! modules) and the CLI/UI-facing Tauri commands reach via
//! `crate::webview_accounts::*`. See each submodule's header comment for
//! what it owns:
//!   - `provider_policy` — pure provider URL/host/popup/deep-link policy.
//!   - `payload`         — recipe-event payload parsing + surfaces ingest.
//!   - `state`           — `WebviewAccountsState` + shutdown/teardown.
//!   - `notifications`   — CEF -> OS-toast bridge + bypass-prefs commands.
//!   - `types`           — IPC argument/event DTOs + `RevealTrigger`.
//!   - `util`            — id/label/data-dir helpers + init-script builder.
//!   - `open`            — `webview_account_open` (the CEF wiring command).
//!   - `lifecycle`        — prewarm/close/purge/bounds/reveal/hide/show.
//!   - `recipe_event`    — `webview_recipe_event` ingest bridge.

mod lifecycle;
mod notifications;
mod open;
mod payload;
mod provider_policy;
mod recipe_event;
mod state;
#[cfg(test)]
mod tests;
mod types;
mod util;

pub(crate) use lifecycle::*;
pub(crate) use notifications::*;
pub(crate) use open::*;
pub(crate) use payload::*;
pub(crate) use provider_policy::*;
pub(crate) use recipe_event::*;
pub(crate) use state::*;
pub(crate) use types::*;
pub(crate) use util::*;
