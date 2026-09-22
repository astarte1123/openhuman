//! Provider-agnostic CDP building blocks shared by every `CallJoinScanner`
//! implementation.
//!
//! These are pure CDP helpers — no init-script JS is ever injected into
//! the webview, per the project's CEF child-webview rule. Everything here
//! is byte-for-byte the same logic Meet used before this module was
//! generalised, just parameterised so Teams/Zoom scanners can reuse it
//! without duplicating the CDP boilerplate or being forced onto Meet's
//! exact timeouts.

use std::time::Duration;

use serde_json::{json, Value};

use crate::cdp::{self, CdpConn};

/// Poll CEF's target list until a page whose URL starts with `url_prefix`
/// shows up, then attach a CDP session to it. Filtering by the full
/// per-call URL prefix (rather than just the host) keeps two concurrent
/// calls from cross-controlling each other when both are open.
///
/// `budget` / `poll_interval` are caller-supplied so each provider can
/// tune target-discovery timing independently instead of being coupled to
/// Meet's original constants.
pub async fn wait_for_target(
    url_prefix: &str,
    budget: Duration,
    poll_interval: Duration,
) -> Result<(CdpConn, String), String> {
    let deadline = tokio::time::Instant::now() + budget;
    let mut last_err = String::new();
    while tokio::time::Instant::now() < deadline {
        match cdp::connect_and_attach_matching(|t| t.url.starts_with(url_prefix)).await {
            Ok(pair) => return Ok(pair),
            Err(err) => {
                last_err = err;
                tokio::time::sleep(poll_interval).await;
            }
        }
    }
    Err(format!(
        "timeout waiting for target matching `{url_prefix}`: {last_err}"
    ))
}

/// Repeatedly evaluate a click-by-text helper in the page until either a
/// click lands or `budget` elapses.
pub async fn wait_and_click_text(
    cdp: &mut CdpConn,
    session: &str,
    labels: &[&str],
    budget: Duration,
) -> Result<(), String> {
    let labels_js = serde_json::to_string(labels).map_err(|e| format!("labels json: {e}"))?;
    let expression = format!(
        r#"
        (() => {{
          const labels = {labels_js};
          const want = labels.map(l => l.toLowerCase());
          const candidates = document.querySelectorAll(
            'button, [role="button"], a[role="button"]'
          );
          for (const el of candidates) {{
            if (el.disabled || el.getAttribute('aria-disabled') === 'true') continue;
            const text = ((el.innerText || el.textContent) || '').trim().toLowerCase();
            if (!text) continue;
            if (!want.some(w => text.includes(w))) continue;
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) continue;
            el.scrollIntoView({{ block: 'center', inline: 'center' }});
            el.click();
            return text;
          }}
          return null;
        }})()
        "#
    );

    let deadline = tokio::time::Instant::now() + budget;
    let mut last_value = Value::Null;
    while tokio::time::Instant::now() < deadline {
        let res = cdp
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": false,
                }),
                Some(session),
            )
            .await?;
        let value = res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null);
        if value.is_string() {
            log::info!(
                "[meet-scanner] clicked element matching {labels:?} text={}",
                value.as_str().unwrap_or("")
            );
            return Ok(());
        }
        last_value = value;
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Err(format!(
        "timeout waiting for clickable element matching {labels:?} (last={last_value})"
    ))
}

/// Focus an `<input>` whose `aria-label` or `placeholder` contains `hint`,
/// then dispatch the supplied text via `Input.insertText` so a
/// React-controlled `<input>` actually picks it up as a real keystroke.
pub async fn type_into_named_input(
    cdp: &mut CdpConn,
    session: &str,
    hint: &str,
    text: &str,
    budget: Duration,
) -> Result<(), String> {
    let hint_js = serde_json::to_string(hint).map_err(|e| format!("hint json: {e}"))?;
    let focus_expr = format!(
        r#"
        (() => {{
          const hint = {hint_js}.toLowerCase();
          const inputs = document.querySelectorAll('input');
          for (const inp of inputs) {{
            const t = (inp.getAttribute('type') || 'text').toLowerCase();
            if (t !== 'text' && t !== 'search') continue;
            const aria = (inp.getAttribute('aria-label') || '').toLowerCase();
            const ph = (inp.placeholder || '').toLowerCase();
            if (!aria.includes(hint) && !ph.includes(hint)) continue;
            inp.focus();
            inp.click();
            // Clear any value already there so we don't append to a
            // half-typed name from a previous attempt.
            try {{ inp.select(); }} catch (_) {{}}
            return true;
          }}
          return false;
        }})()
        "#
    );

    let deadline = tokio::time::Instant::now() + budget;
    while tokio::time::Instant::now() < deadline {
        let res = cdp
            .call(
                "Runtime.evaluate",
                json!({ "expression": focus_expr, "returnByValue": true }),
                Some(session),
            )
            .await?;
        let focused = res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if focused {
            cdp.call("Input.insertText", json!({ "text": text }), Some(session))
                .await?;
            log::info!(
                "[meet-scanner] inserted display name (hint={hint} chars={})",
                text.chars().count()
            );
            return Ok(());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Err(format!("timeout waiting for input matching hint={hint}"))
}

/// Shared poll cadence for [`wait_and_click_text`] / [`type_into_named_input`]'s
/// internal retry loops. Not a per-provider knob (unlike the outer
/// budgets) since it's purely an implementation-detail poll rate, not
/// something that should vary provider to provider.
const POLL_INTERVAL: Duration = Duration::from_millis(500);
