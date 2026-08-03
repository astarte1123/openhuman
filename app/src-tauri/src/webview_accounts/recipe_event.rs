//! `webview_recipe_event` — ingest bridge for the injected recipe JS
//! (linkedin, google-meet) and the zero-injection scanner paths.
//! Split out of `mod.rs` — no behavior changes.

use tauri::{AppHandle, Emitter, Runtime};

use super::*;

/// Called from the injected runtime each time the recipe emits an event.
/// We forward to React via a Tauri event so the UI can render and persist.
#[tauri::command]
pub async fn webview_recipe_event<R: Runtime>(
    app: AppHandle<R>,
    webview: tauri::Webview<R>,
    args: RecipeEventArgs,
) -> Result<(), String> {
    // The event can only be trusted if the invoking webview is the
    // `acct_<account_id>` webview for the account in the payload. A
    // compromised renderer or a sibling child webview must not be able to
    // forge events for another account.
    let caller_label = webview.label().to_string();
    let expected_label = label_for(&args.account_id);
    if caller_label != expected_label {
        log::warn!(
            "[webview-accounts] recipe_event rejected: caller_label={} expected={} account={}",
            caller_label,
            expected_label,
            args.account_id
        );
        return Err("webview label does not match account_id".to_string());
    }
    log::debug!(
        "[webview-accounts] recipe_event account={} provider={} kind={}",
        args.account_id,
        args.provider,
        args.kind
    );
    if args.provider == "google-meet" {
        match args.kind.as_str() {
            "meet_call_started" => {
                let code = args
                    .payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                log::info!("[gmeet][{}] call_started code={}", args.account_id, code);
            }
            "meet_captions" => {
                let code = args
                    .payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let n = args
                    .payload
                    .get("captions")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                log::info!(
                    "[gmeet][{}] captions code={} rows={}",
                    args.account_id,
                    code,
                    n
                );
            }
            "meet_call_ended" => {
                let code = args
                    .payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let reason = args
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                log::info!(
                    "[gmeet][{}] call_ended code={} reason={}",
                    args.account_id,
                    code,
                    reason
                );
            }
            _ => {}
        }
    }
    if args.kind == "ingest" {
        if let Some(messages) = args.payload.get("messages").and_then(|v| v.as_array()) {
            log::info!(
                "[webview-accounts] ingest from acct_{}: {} messages",
                args.account_id,
                messages.len()
            );
        }
    } else if args.kind == "ws_message" {
        let direction = args
            .payload
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let size = args
            .payload
            .get("size")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        log::trace!(
            "[webview-accounts][{}] ws {} {} bytes",
            args.account_id,
            direction,
            size
        );
    } else if args.kind == "log" {
        let level = args
            .payload
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("info");
        let msg = args
            .payload
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match level {
            "warn" => log::warn!("[webview-accounts][{}] {}", args.account_id, msg),
            "error" => log::error!("[webview-accounts][{}] {}", args.account_id, msg),
            _ => log::info!("[webview-accounts][{}] {}", args.account_id, msg),
        }
    }

    if let Err(err) = post_provider_surfaces_event(&args).await {
        log::warn!(
            "[webview-accounts] provider_surfaces ingest failed account={} provider={} kind={}: {}",
            args.account_id,
            args.provider,
            args.kind,
            err
        );
    }

    let event = WebviewEvent {
        account_id: args.account_id,
        provider: args.provider,
        kind: args.kind,
        payload: args.payload,
        ts: args.ts,
    };
    app.emit("webview:event", &event)
        .map_err(|e| format!("emit failed: {e}"))?;
    Ok(())
}
