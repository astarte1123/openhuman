//! Native-OS notification bridging for embedded webview accounts:
//! bypass/DND prefs, the CEF -> OS-toast forwarder, and the
//! `webview_notification_*` / `webview_set_focused_account` commands.
//! Split out of `mod.rs` — no behavior changes.

use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::sync::{mpsc::sync_channel, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
#[cfg(windows)]
use tauri_plugin_notification::NotificationExt;

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct NotificationBypassPrefs {
    global_dnd: bool,
    muted_accounts: HashSet<String>,
    bypass_when_focused: bool,
    focused_account: Option<String>,
}

impl Default for NotificationBypassPrefs {
    fn default() -> Self {
        Self {
            global_dnd: false,
            muted_accounts: HashSet::new(),
            // Match the existing UI copy: focused account may suppress toast.
            bypass_when_focused: true,
            focused_account: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationBypassPrefsPayload {
    pub global_dnd: bool,
    pub muted_accounts: Vec<String>,
    pub bypass_when_focused: bool,
}

impl From<&NotificationBypassPrefs> for NotificationBypassPrefsPayload {
    fn from(value: &NotificationBypassPrefs) -> Self {
        let mut muted_accounts = value.muted_accounts.iter().cloned().collect::<Vec<_>>();
        muted_accounts.sort();
        Self {
            global_dnd: value.global_dnd,
            muted_accounts,
            bypass_when_focused: value.bypass_when_focused,
        }
    }
}

/// Title prefix applied to every OS toast fired from an embedded webview.
/// Matches `openhuman_core::webview_notifications::OPENHUMAN_TITLE_PREFIX`
/// — kept inline here so the shell crate doesn't take a build-time dep on
/// the core library. Disambiguates from natively-installed apps (Slack,
/// Discord, Telegram desktop) firing the same message twice.
const OPENHUMAN_TITLE_PREFIX: &str = "OpenHuman: ";

pub(crate) fn slack_scanner_enabled() -> bool {
    std::env::var("OPENHUMAN_DISABLE_SLACK_SCANNER")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !(v == "1" || v == "true" || v == "yes" || v == "on")
        })
        .unwrap_or(true)
}

/// Serialised fire-event payload shipped to the frontend over the
/// `webview-notification:fired` Tauri event. Carries `account_id` +
/// `provider` so the React side can route a subsequent click back to
/// the originating webview via Redux.
#[derive(Debug, Clone, Serialize)]
struct WebviewNotificationFired {
    account_id: String,
    provider: String,
    title: String,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

/// Linux: one worker thread + bounded queue so a burst of toasts does not
/// spawn unbounded `std::thread` handles (each would block in `wait_for_action`).
#[cfg(target_os = "linux")]
const LINUX_NOTIFY_QUEUE_CAP: usize = 16;

#[cfg(target_os = "linux")]
static LINUX_NOTIFY_TX: OnceLock<std::sync::mpsc::SyncSender<Box<dyn FnOnce() + Send>>> =
    OnceLock::new();

#[cfg(target_os = "linux")]
pub(crate) fn enqueue_linux_notification(job: Box<dyn FnOnce() + Send>) {
    let tx = LINUX_NOTIFY_TX.get_or_init(|| {
        let (tx, rx) = sync_channel::<Box<dyn FnOnce() + Send>>(LINUX_NOTIFY_QUEUE_CAP);
        std::thread::Builder::new()
            .name("openhuman-linux-notify".to_string())
            .spawn(move || {
                while let Ok(j) = rx.recv() {
                    j();
                }
            })
            .expect("spawn openhuman-linux-notify");
        tx
    });
    if let Err(e) = tx.try_send(job) {
        log::warn!(
            "[notify-cef] linux notification queue full (cap={}), dropping toast: {}",
            LINUX_NOTIFY_QUEUE_CAP,
            e
        );
    }
}

/// Translate a `tauri-runtime-cef` notification payload into a native OS
/// toast via `tauri-plugin-notification`, and mirror the fire to the
/// React frontend so it can drive click-to-focus routing.
///
/// Gated on the runtime `NotificationSettings` flag (OFF by default) so
/// v1 ships the plumbing without surprising users with a toast storm the
/// first time they open a busy Slack tab.
pub(crate) fn forward_native_notification<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
    provider: &str,
    payload: &tauri_runtime_cef::notification::NotificationPayload,
) {
    if let Some(state) = app.try_state::<WebviewAccountsState>() {
        let prefs = state.notification_bypass.lock().unwrap().clone();
        if prefs.global_dnd {
            log::debug!(
                "[notify-bypass][{}] suppressed global_dnd provider={}",
                account_id,
                provider
            );
            return;
        }
        if prefs.muted_accounts.contains(account_id) {
            log::debug!(
                "[notify-bypass][{}] suppressed muted_account provider={}",
                account_id,
                provider
            );
            return;
        }
        if prefs.bypass_when_focused && prefs.focused_account.as_deref() == Some(account_id) {
            log::debug!(
                "[notify-bypass][{}] suppressed focused_account provider={}",
                account_id,
                provider
            );
            return;
        }
    }

    // Feature flag — bail early when the user hasn't opted in.
    if let Some(settings) =
        app.try_state::<crate::notification_settings::NotificationSettingsState>()
    {
        if !settings.enabled() {
            log::debug!(
                "[notify-cef][{}] suppressed (feature flag off) provider={}",
                account_id,
                provider
            );
            return;
        }
    }

    let provider_label = provider_display_name(provider);
    let raw_title = payload.title.as_str().trim();
    let notify_title = if raw_title.is_empty() {
        format!("{OPENHUMAN_TITLE_PREFIX}{provider_label}")
    } else {
        format!("{OPENHUMAN_TITLE_PREFIX}{provider_label} — {raw_title}")
    };
    let body = payload.body.as_deref().unwrap_or("");
    log::info!(
        "[notify-cef][{}] source={:?} tag={:?} silent={} title_chars={} body_chars={}",
        account_id,
        payload.source,
        payload.tag,
        payload.silent,
        raw_title.chars().count(),
        body.chars().count()
    );
    log::debug!("[notify-cef][{}] raw_title={:?}", account_id, raw_title);

    // Mirror to the frontend BEFORE firing the OS toast so the Redux
    // store has the routing context ready by the time the user clicks.
    let fired = WebviewNotificationFired {
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        title: notify_title.clone(),
        body: body.to_string(),
        tag: payload.tag.clone(),
    };
    if let Err(err) = app.emit("webview-notification:fired", &fired) {
        log::warn!(
            "[notify-cef][{}] emit webview-notification:fired failed: {}",
            account_id,
            err
        );
    }

    // Respect the Web Notification `silent` flag — the mirror event above
    // still updates the in-app notification center, but the OS toast is
    // suppressed so the user is not audibly/visually interrupted for
    // notifications the page explicitly marked as silent.
    if payload.silent {
        log::debug!(
            "[notify-cef][{}] silent=true, suppressing OS toast",
            account_id
        );
        return;
    }

    // Fire the OS toast and wire a click callback that emits `notification:click`
    // so the frontend can bring the originating account into focus.
    //
    // macOS: mac-notification-sys blocks in wait_for_click mode — run on a
    //        blocking thread so the async executor is not stalled.
    // Linux: notify_rust's wait_for_action hooks D-Bus action delivery.
    // Windows: no click callback available; fall back to fire-and-forget.
    let acct_for_click = account_id.to_string();
    let prov_for_click = provider.to_string();
    let app_for_click = app.clone();

    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Each `wait_for_click` thread blocks at ~100% CPU until the user
        // clicks or the toast auto-dismisses. Under notification bursts this
        // can pin many cores; cap concurrent click-wait threads and fall back
        // to fire-and-forget (no click callback) once the budget is reached.
        const MAX_CLICK_WAIT_THREADS: usize = 8;
        static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

        let title_c = notify_title.clone();
        let body_c = body.to_string();
        let app_id = app.config().identifier.clone();
        let prev = IN_FLIGHT.fetch_add(1, Ordering::AcqRel);
        if prev >= MAX_CLICK_WAIT_THREADS {
            IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            log::debug!(
                "[notify-cef][{}] click-wait budget exhausted ({}), firing without click callback",
                account_id,
                prev
            );
            std::thread::spawn(move || {
                let _ = mac_notification_sys::set_application(if tauri::is_dev() {
                    "com.apple.Terminal"
                } else {
                    &app_id
                });
                use mac_notification_sys::Notification as MacNotif;
                let mut n = MacNotif::new();
                n.title(&title_c).message(&body_c);
                let _ = n.send();
            });
            return;
        }

        std::thread::spawn(move || {
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let _guard = Guard;

            let _ = mac_notification_sys::set_application(if tauri::is_dev() {
                "com.apple.Terminal"
            } else {
                &app_id
            });
            use mac_notification_sys::{Notification as MacNotif, NotificationResponse};
            let t = title_c;
            let b = body_c;
            let mut n = MacNotif::new();
            n.title(&t).message(&b).wait_for_click(true);
            match n.send() {
                Ok(NotificationResponse::Click) | Ok(NotificationResponse::ActionButton(_)) => {
                    log::info!(
                        "[notify-click][{}] clicked provider={}",
                        acct_for_click,
                        prov_for_click
                    );
                    if let Err(e) = app_for_click.emit(
                        "notification:click",
                        serde_json::json!({
                            "account_id": acct_for_click,
                            "provider": prov_for_click,
                        }),
                    ) {
                        log::warn!(
                            "[notify-click][{}] emit notification:click failed: {}",
                            acct_for_click,
                            e
                        );
                    }
                }
                Ok(other) => {
                    log::info!("[notify-click][{}] response={:?}", acct_for_click, other);
                }
                Err(e) => {
                    log::warn!("[notify-click][{}] send error: {}", acct_for_click, e);
                }
            }
        });
    }

    #[cfg(target_os = "linux")]
    {
        let title_c = notify_title.clone();
        let body_c = body.to_string();
        enqueue_linux_notification(Box::new(move || {
            let t = title_c;
            let b = body_c;
            let mut n = notify_rust::Notification::new();
            n.summary(&t).body(&b);
            match n.show() {
                Ok(handle) => {
                    handle.wait_for_action(|action| {
                        // "__closed" is the synthetic dismiss action; skip it.
                        if action != "__closed" && !action.is_empty() {
                            log::info!(
                                "[notify-click][{}] action={} provider={}",
                                acct_for_click,
                                action,
                                prov_for_click
                            );
                            if let Err(e) = app_for_click.emit(
                                "notification:click",
                                serde_json::json!({
                                    "account_id": acct_for_click,
                                    "provider": prov_for_click,
                                }),
                            ) {
                                log::warn!(
                                    "[notify-click][{}] emit notification:click failed: {}",
                                    acct_for_click,
                                    e
                                );
                            }
                        }
                    });
                }
                Err(e) => {
                    log::warn!("[notify-click][{}] show failed: {}", acct_for_click, e);
                }
            }
        }));
    }

    #[cfg(windows)]
    {
        let mut builder = app.notification().builder().title(&notify_title);
        if !body.is_empty() {
            builder = builder.body(body);
        }
        if let Err(e) = builder.show() {
            log::warn!(
                "[notify-cef][{}] notification show failed: {}",
                account_id,
                e
            );
        }
    }
}

pub(crate) fn forward_synthetic_notification<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
    provider: &str,
    title: impl Into<String>,
    body: impl Into<String>,
) {
    let payload = tauri_runtime_cef::notification::NotificationPayload {
        source: tauri_runtime_cef::notification::NotificationSource::Window,
        title: title.into(),
        body: Some(body.into()),
        icon: None,
        tag: None,
        silent: false,
        origin: format!("synthetic://{}", provider),
    };
    forward_native_notification(app, account_id, provider, &payload);
}

/// Web-shape notification permission state used by frontend parity code.
/// Effectively granted because interception is handled in-app via CEF.
#[tauri::command]
pub fn webview_notification_permission_state() -> String {
    "granted".to_string()
}

/// Request notification permission and return web-shape state.
#[tauri::command]
pub fn webview_notification_permission_request() -> String {
    webview_notification_permission_state()
}

/// Enable/disable global DND for embedded webview OS toasts.
#[tauri::command]
pub fn webview_notification_set_dnd(
    state: tauri::State<'_, WebviewAccountsState>,
    enabled: bool,
) -> Result<(), String> {
    let mut prefs = state.notification_bypass.lock().unwrap();
    prefs.global_dnd = enabled;
    log::debug!("[notify-bypass] set global_dnd={enabled}");
    Ok(())
}

/// Mute/unmute a specific embedded account for OS toasts.
#[tauri::command]
pub fn webview_notification_mute_account(
    state: tauri::State<'_, WebviewAccountsState>,
    account_id: String,
    muted: bool,
) -> Result<(), String> {
    let account_id = sanitize_account_id(&account_id)?.to_string();
    let mut prefs = state.notification_bypass.lock().unwrap();
    if muted {
        prefs.muted_accounts.insert(account_id.clone());
    } else {
        prefs.muted_accounts.remove(&account_id);
    }
    log::debug!(
        "[notify-bypass] set muted account_id={} muted={}",
        account_id,
        muted
    );
    Ok(())
}

/// Return current bypass preferences for the settings UI.
#[tauri::command]
pub fn webview_notification_get_bypass_prefs(
    state: tauri::State<'_, WebviewAccountsState>,
) -> NotificationBypassPrefsPayload {
    let prefs = state.notification_bypass.lock().unwrap();
    NotificationBypassPrefsPayload::from(&*prefs)
}

/// Track which account is currently focused in the shell UI.
#[tauri::command]
pub fn webview_set_focused_account(
    state: tauri::State<'_, WebviewAccountsState>,
    account_id: Option<String>,
) -> Result<(), String> {
    let mut prefs = state.notification_bypass.lock().unwrap();
    prefs.focused_account = match account_id {
        Some(id) => Some(sanitize_account_id(&id)?.to_string()),
        None => None,
    };
    log::debug!(
        "[notify-bypass] set focused_account={}",
        prefs.focused_account.as_deref().unwrap_or("<none>")
    );
    Ok(())
}
