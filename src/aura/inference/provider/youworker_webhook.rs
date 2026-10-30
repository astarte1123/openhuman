//! Receives youworker.ai's outbound webhook (the async reply leg of the
//! YouWorkForce REST channel — see [`super::youworker`]) and resolves the
//! matching pending turn.
//!
//! Two pieces, per `docs/reference/youworkforce-integration.md` §Outbound:
//!
//! - **Correlation registry** — a process-wide map from `conversation_ref`
//!   to the `oneshot::Sender` a [`super::youworker::YouWorkerProvider`] call
//!   is awaiting. One entry per in-flight turn (this provider is one-shot
//!   per call, so at most one sender per `conversation_ref`).
//! - **HTTP handler** — `POST /channels/{channel}/outbound/message`,
//!   Standard-Webhooks HMAC-verified against the configured youworker
//!   provider's outbound secret before the body is even JSON-parsed.
//!
//! Only the primary outbound secret is supported (no rotation-overlap
//! `_ADDITIONAL` secret list) — sufficient for a single desktop deployment;
//! add multi-secret support if/when secret rotation is actually needed.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

use crate::aura::config::schema::cloud_providers::AuthStyle;
use crate::aura::credentials::AuthService;

type HmacSha256 = Hmac<Sha256>;

/// How long a delivered `idempotency_key` is remembered to swallow
/// at-least-once retries that arrive after we've already resolved (or given
/// up on) the matching `conversation_ref`.
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(10 * 60);
/// Standard-Webhooks anti-replay window (spec: ±300s).
const REPLAY_WINDOW_SECS: i64 = 300;

fn pending() -> &'static Mutex<HashMap<String, oneshot::Sender<String>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<String>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn seen_idempotency_keys() -> &'static Mutex<HashMap<String, Instant>> {
    static SEEN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a pending turn, returning the receiver half the caller awaits.
/// Overwrites (and drops) any stale entry already registered under the same
/// `conversation_ref` — cannot happen in practice since `conversation_ref` is
/// a fresh UUID per call, but overwriting rather than panicking keeps this
/// infallible.
pub fn register_pending(conversation_ref: &str) -> oneshot::Receiver<String> {
    let (tx, rx) = oneshot::channel();
    if let Ok(mut map) = pending().lock() {
        map.insert(conversation_ref.to_string(), tx);
    }
    rx
}

/// Deregister a pending turn without resolving it — call on any error path
/// (POST failed, timed out) so a late webhook delivery for a
/// `conversation_ref` nobody is waiting on any more is a harmless no-op
/// (dropped sender → the receiver, if still held, sees a closed channel).
pub fn cancel_pending(conversation_ref: &str) {
    if let Ok(mut map) = pending().lock() {
        map.remove(conversation_ref);
    }
}

/// Resolve a pending turn with the terminal response text. Returns `false`
/// if nothing is (or is no longer) waiting on this `conversation_ref` —
/// callers treat that as "harmless, ack anyway" per the webhook's
/// at-least-once delivery semantics.
fn resolve_pending(conversation_ref: &str, text: String) -> bool {
    let sender = match pending().lock() {
        Ok(mut map) => map.remove(conversation_ref),
        Err(_) => None,
    };
    match sender {
        Some(tx) => tx.send(text).is_ok(),
        None => false,
    }
}

/// `true` if this is the first time we've seen `idempotency_key` within the
/// TTL window. Sweeps expired entries opportunistically on every call — no
/// background task needed for this low a write rate.
fn claim_idempotency_key(idempotency_key: &str) -> bool {
    let Ok(mut seen) = seen_idempotency_keys().lock() else {
        return true;
    };
    let now = Instant::now();
    seen.retain(|_, seen_at| now.duration_since(*seen_at) < IDEMPOTENCY_TTL);
    if seen.contains_key(idempotency_key) {
        false
    } else {
        seen.insert(idempotency_key.to_string(), now);
        true
    }
}

/// `provider:<slug>:outbound-hmac` — the [`AuthService`] key the outbound
/// HMAC secret is stored under, parallel to `provider:<slug>` for the
/// client_secret (see `factory::auth_key_for_slug`). Deliberately distinct
/// from the client_secret key: they are different secrets with different
/// rotation lifecycles.
fn outbound_hmac_key_for_slug(slug: &str) -> String {
    format!("provider:{slug}:outbound-hmac")
}

/// Find the configured youworker provider's outbound HMAC secret, if any
/// youworker `cloud_providers` entry (and stored secret) exists.
async fn configured_outbound_secret() -> Option<String> {
    let config = crate::aura::config::Config::load_or_init().await.ok()?;
    let slug = config
        .cloud_providers
        .iter()
        .find(|p| p.auth_style == AuthStyle::YouWorker)
        .map(|p| p.slug.clone())?;
    let auth = AuthService::from_config(&config);
    auth.get_provider_bearer_token(&outbound_hmac_key_for_slug(&slug), None)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
}

/// Verify a Standard-Webhooks HMAC-SHA256 signature.
///
/// `secret` must be a `whsec_<base64>` string (per
/// `docs/reference/youworkforce-integration.md`): the HMAC key is the
/// base64-decoded payload AFTER the `whsec_` prefix, never the literal
/// secret string. A secret without that prefix fails closed (matches CG's
/// own fail-closed behaviour on the signing side).
fn verify_signature(
    secret: &str,
    webhook_id: &str,
    webhook_timestamp: &str,
    raw_body: &[u8],
    signature_header: &str,
) -> bool {
    let Some(encoded_key) = secret.strip_prefix("whsec_") else {
        log::warn!("[youworker][webhook] outbound secret missing 'whsec_' prefix — rejecting");
        return false;
    };
    let Ok(key_bytes) =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_key)
    else {
        log::warn!("[youworker][webhook] outbound secret is not valid base64 — rejecting");
        return false;
    };

    let mut signed_content =
        Vec::with_capacity(webhook_id.len() + 1 + webhook_timestamp.len() + 1 + raw_body.len());
    signed_content.extend_from_slice(webhook_id.as_bytes());
    signed_content.push(b'.');
    signed_content.extend_from_slice(webhook_timestamp.as_bytes());
    signed_content.push(b'.');
    signed_content.extend_from_slice(raw_body);

    signature_header.split_whitespace().any(|tuple| {
        let Some(b64) = tuple.strip_prefix("v1,") else {
            return false;
        };
        let Ok(given) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        else {
            return false;
        };
        // Fresh MAC per tuple: `Mac::verify_slice` consumes `self` and does
        // the tag comparison in constant time internally — the whole point
        // of using the `hmac` crate's own verify path instead of finalizing
        // once and comparing bytes ourselves.
        let Ok(mac) = HmacSha256::new_from_slice(&key_bytes) else {
            return false;
        };
        mac.chain_update(&signed_content)
            .verify_slice(&given)
            .is_ok()
    })
}

/// `true` if `webhook_timestamp` (Unix seconds) is within the anti-replay
/// window of now.
fn within_replay_window(webhook_timestamp: &str) -> bool {
    let Ok(ts) = webhook_timestamp.parse::<i64>() else {
        return false;
    };
    let now = chrono::Utc::now().timestamp();
    (now - ts).abs() <= REPLAY_WINDOW_SECS
}

/// `POST /channels/{channel}/outbound/message` — youworker's async reply to
/// an earlier `inbound/message` POST. Handles `youmessage` (text) only;
/// `youtalkie` (voice) is acked but ignored since [`Provider`] is text-only.
///
/// [`Provider`]: super::traits::Provider
pub async fn youworker_outbound_message_handler(
    Path(channel): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    };
    let webhook_id = header("webhook-id");
    let webhook_timestamp = header("webhook-timestamp");
    let webhook_signature = header("webhook-signature");

    if webhook_id.is_empty() || webhook_timestamp.is_empty() || webhook_signature.is_empty() {
        log::warn!("[youworker][webhook] missing Standard-Webhooks headers — rejecting");
        return StatusCode::UNAUTHORIZED;
    }
    if !within_replay_window(webhook_timestamp) {
        log::warn!("[youworker][webhook] webhook-timestamp outside the {REPLAY_WINDOW_SECS}s replay window");
        return StatusCode::UNAUTHORIZED;
    }

    let Some(secret) = configured_outbound_secret().await else {
        log::error!(
            "[youworker][webhook] no youworker provider configured (or no outbound secret \
             stored) — rejecting delivery"
        );
        return StatusCode::UNAUTHORIZED;
    };

    if !verify_signature(
        &secret,
        webhook_id,
        webhook_timestamp,
        &body,
        webhook_signature,
    ) {
        log::warn!("[youworker][webhook] signature verification failed — rejecting delivery");
        return StatusCode::UNAUTHORIZED;
    }

    if channel != "youmessage" {
        // youtalkie (voice) or an unknown channel: ack so youworker doesn't
        // retry forever, but there is nothing to resolve — this provider
        // never sends a youtalkie inbound message, so it can't be waiting
        // on one either.
        log::debug!("[youworker][webhook] ignoring outbound delivery for channel='{channel}'");
        return StatusCode::OK;
    }

    #[derive(serde::Deserialize)]
    struct OutboundMessage {
        conversation_ref: String,
        text: String,
        idempotency_key: String,
    }
    let parsed: OutboundMessage = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[youworker][webhook] malformed outbound/message body: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    if !claim_idempotency_key(&parsed.idempotency_key) {
        log::debug!(
            "[youworker][webhook] duplicate delivery idempotency_key={} — no-op ack",
            parsed.idempotency_key
        );
        return StatusCode::OK;
    }

    let resolved = resolve_pending(&parsed.conversation_ref, parsed.text);
    if !resolved {
        log::warn!(
            "[youworker][webhook] no pending turn for conversation_ref={} (already timed out, \
             or delivered twice) — acking anyway",
            parsed.conversation_ref
        );
    }

    StatusCode::OK
}

#[cfg(test)]
#[path = "youworker_webhook_tests.rs"]
mod tests;
