//! youworker.ai YouWorkForce (YWF) webhook-channel provider.
//!
//! Speaks the REST/webhook contract YWF was actually provisioned for — NOT
//! the native WebSocket transport used by youworker's own `yw-web`/`yw-cli`
//! clients. Confirmed against `docs/howto/provision-youworkforce-channel.md`
//! and `docs/reference/youworkforce-integration.md`: the YWF broker grant
//! only trusts the `youmessage`/`youtalkie` webhook-adapter routes; the
//! native `session.attach` RPC plane lives behind Session Manager's
//! `/internal/sessions` API (`BearerJWT`/`CookieAuth` service auth), which
//! YWF's broker-exchanged user JWT does not carry.
//!
//! Flow per [`Provider::chat_with_system`] call (one-shot: every call opens
//! a fresh `conversation_ref`, matching every other [`Provider`] impl in
//! this module — no multi-turn session reuse):
//!
//! 1. Mint/refresh the youworker user JWT via Autentika token-exchange
//!    (`ensure_user_jwt` — two-step `client_credentials` + RFC 8693
//!    exchange, unchanged from the original design).
//! 2. Register a pending correlation slot for a fresh `conversation_ref`
//!    (see [`super::youworker_webhook`]).
//! 3. `POST {yw_base}/channels/youmessage/inbound/message` with
//!    `{conversation_ref, persona, message_id, text}` and
//!    `Authorization: Bearer <user JWT>`. Expect `202 accepted`.
//! 4. Await the webhook callback: youworker's Channel Gateway delivers the
//!    terminal response asynchronously to our own outbound endpoint
//!    (`POST /channels/youmessage/outbound/message`, HMAC-verified), which
//!    resolves the pending slot registered in step 2.
//!
//! Voice (`youtalkie`) is not implemented — [`Provider`] is text-only.
//!
//! ## Alternate transport: native WebSocket
//!
//! [`YouWorkerTransport::Websocket`] skips the webhook leg entirely: the
//! SAME broker-exchanged user JWT from step 1 is also accepted by
//! youworker's native `youworker.v1` WS upgrade (empirically confirmed
//! against the real `dev.youworker.ai` DEV deployment — no separate
//! credential set, despite the public REST API reference documenting no
//! WS/session route at all). Mints a fresh `session.create` +
//! `session.attach`, then `message.send`, and reads the reply off the SAME
//! open connection — no public inbound endpoint, no HMAC secret, no
//! `YW_CG_YWF_OUTBOUND_BASE_URL` coordination needed. Use this transport
//! whenever nobody has registered our outbound base URL on youworker.ai's
//! side yet; switch to `Webhook` once they have (it scales better — one
//! connection per call here vs. one long-lived socket).

use super::traits::{Provider, ProviderCapabilities};
use super::youworker_webhook::{cancel_pending, register_pending};
use crate::aura::credentials::profiles::TokenSet;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub const PROVIDER_LABEL: &str = "YouWorker";

/// The only inbound channel a text [`Provider`] speaks — youworker's other
/// webhook channel, `youtalkie`, is voice-only (see module docs).
const CHANNEL: &str = "youmessage";
/// `origin` on `session.create` / `channel` on `session.attach` for the WS
/// transport. Matches the confirmed-working default — the
/// `operatore-centrale` persona's manifest allows `yw-web` (and
/// `youmessage`/`youtalkie`) but NOT `yw-cli`.
const WS_ORIGIN: &str = "yw-web";
const WS_SUBPROTOCOL: &str = "youworker.v1";

/// Wait budget for the Autentika token-exchange HTTP calls and the inbound
/// `POST .../inbound/message` itself (all fast, synchronous HTTP round-trips).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Wait budget for youworker's async webhook reply to arrive after the
/// inbound POST is accepted (an agent turn can run long).
const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Wait budget for a single WS control round-trip (`session.create`,
/// `session.attach`, the WS handshake itself, and `message.send`'s ack).
const WS_CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
/// Wait budget for the agent to finish streaming a turn over WS.
const WS_TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh the cached user JWT this far ahead of its `expires_at`.
const TOKEN_EXPIRY_SKEW: Duration = Duration::from_secs(60);

/// Which leg carries youworker's reply. See the module docs' "Alternate
/// transport" section for why both exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YouWorkerTransport {
    /// `POST inbound/message` + await our own HMAC-verified outbound webhook.
    #[default]
    Webhook,
    /// Native `youworker.v1` WebSocket — reply arrives on the same connection.
    Websocket,
}

impl YouWorkerTransport {
    /// Parses the `cloud_providers` entry's `youworker_transport` string
    /// (`"webhook"` | `"websocket"`, defaulting to `Webhook` on anything
    /// else, including empty/absent).
    pub fn from_config_str(s: &str) -> Self {
        match s.trim() {
            "websocket" => Self::Websocket,
            _ => Self::Webhook,
        }
    }
}

/// Static connection config for one youworker.ai `AuthStyle::YouWorker`
/// `cloud_providers` entry. `client_secret` is resolved by the factory via
/// the same `provider:<slug>` `AuthService` lookup every other provider's
/// API key uses — never stored on [`CloudProviderCreds`].
#[derive(Debug, Clone)]
pub struct YouWorkerConfig {
    /// Autentika workspace issuer, e.g. `https://your-workspace.autentika.example`.
    pub autentika_url: String,
    /// Public inbound base, e.g. `https://dev.youworker.ai/api/v1` — routes
    /// are `{yw_base}/channels/{channel}/inbound/message` etc.
    pub yw_base: String,
    pub client_id: String,
    pub client_secret: String,
    pub persona: String,
    /// The youworker end-user UUID presented as `subject_token` (MVP: one
    /// youworker user per broker grant).
    pub subject_uuid: String,
    /// Which leg carries the reply — see [`YouWorkerTransport`].
    pub transport: YouWorkerTransport,
}

pub struct YouWorkerProvider {
    config: YouWorkerConfig,
    http: reqwest::Client,
    user_jwt: Mutex<Option<TokenSet>>,
}

impl YouWorkerProvider {
    pub fn new(config: YouWorkerConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            user_jwt: Mutex::new(None),
        }
    }

    async fn ensure_user_jwt(&self) -> anyhow::Result<String> {
        {
            let cached = self.user_jwt.lock().await;
            if let Some(t) = cached.as_ref() {
                if !t.is_expiring_within(TOKEN_EXPIRY_SKEW) {
                    return Ok(t.access_token.clone());
                }
            }
        }
        let token = self.exchange_user_jwt().await?;
        let access_token = token.access_token.clone();
        *self.user_jwt.lock().await = Some(token);
        Ok(access_token)
    }

    /// Two-step Autentika exchange: `client_credentials` for an actor token,
    /// then RFC 8693 token-exchange for the youworker user JWT.
    async fn exchange_user_jwt(&self) -> anyhow::Result<TokenSet> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            #[serde(default)]
            expires_in: Option<i64>,
        }

        let autentika = self.config.autentika_url.trim().trim_end_matches('/');

        let actor_resp = timeout(
            HTTP_TIMEOUT,
            self.http
                .post(format!("{autentika}/oauth2/token"))
                .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
                .form(&[("grant_type", "client_credentials")])
                .send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("[youworker] actor-token request timed out"))?
        .map_err(|e| anyhow::anyhow!("[youworker] actor-token request failed: {e}"))?;
        if !actor_resp.status().is_success() {
            let status = actor_resp.status();
            let body = actor_resp.text().await.unwrap_or_default();
            anyhow::bail!("[youworker] actor-token exchange failed: {status} {body}");
        }
        let actor: TokenResponse = actor_resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("[youworker] actor-token response parse failed: {e}"))?;

        let exch_resp = timeout(
            HTTP_TIMEOUT,
            self.http
                .post(format!("{autentika}/v1/oauth/token_exchange"))
                .form(&token_exchange_form(
                    &actor.access_token,
                    &self.config.subject_uuid,
                ))
                .send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("[youworker] token-exchange request timed out"))?
        .map_err(|e| anyhow::anyhow!("[youworker] token-exchange request failed: {e}"))?;
        if !exch_resp.status().is_success() {
            let status = exch_resp.status();
            let body = exch_resp.text().await.unwrap_or_default();
            anyhow::bail!("[youworker] token-exchange failed: {status} {body}");
        }
        let exch: TokenResponse = exch_resp.json().await.map_err(|e| {
            anyhow::anyhow!("[youworker] token-exchange response parse failed: {e}")
        })?;

        let expires_at = exch
            .expires_in
            .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs));

        Ok(TokenSet {
            access_token: exch.access_token,
            refresh_token: None,
            id_token: None,
            expires_at,
            token_type: Some("Bearer".to_string()),
            scope: None,
        })
    }

    /// POST one inbound message, then await the async webhook reply that
    /// [`super::youworker_webhook`] resolves when youworker's outbound
    /// callback lands.
    async fn run_turn_webhook(&self, message: &str) -> anyhow::Result<String> {
        let jwt = self.ensure_user_jwt().await?;
        let conversation_ref = uuid::Uuid::new_v4().to_string();
        let message_id = uuid::Uuid::new_v4().to_string();

        let receiver = register_pending(&conversation_ref);

        let url = format!(
            "{}/channels/{CHANNEL}/inbound/message",
            self.config.yw_base.trim_end_matches('/')
        );
        let body = inbound_message_body(
            &conversation_ref,
            &self.config.persona,
            &message_id,
            message,
        );

        let send_result = timeout(
            HTTP_TIMEOUT,
            self.http.post(&url).bearer_auth(&jwt).json(&body).send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("[youworker] inbound message POST timed out"));

        let resp = match send_result {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                cancel_pending(&conversation_ref);
                anyhow::bail!("[youworker] inbound message POST failed: {e}");
            }
            Err(e) => {
                cancel_pending(&conversation_ref);
                return Err(e);
            }
        };

        if !resp.status().is_success() {
            cancel_pending(&conversation_ref);
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("[youworker] inbound message rejected: {status} {body}");
        }

        log::debug!(
            "[youworker] inbound message accepted conversation_ref={conversation_ref} — \
             awaiting outbound webhook reply"
        );

        match timeout(TURN_TIMEOUT, receiver).await {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(_)) => {
                anyhow::bail!(
                    "[youworker] correlation channel dropped for conversation_ref={conversation_ref} \
                     — the core may be shutting down"
                )
            }
            Err(_) => {
                cancel_pending(&conversation_ref);
                anyhow::bail!(
                    "[youworker] timed out after {}s waiting for youworker's webhook reply \
                     (conversation_ref={conversation_ref}) — is the outbound webhook route \
                     reachable from youworker.ai (ywf_base/tunnel), and is the outbound HMAC \
                     secret configured?",
                    TURN_TIMEOUT.as_secs()
                )
            }
        }
    }

    /// Connect, `session.create` + `session.attach`, `message.send`, collect
    /// the streamed terminal response, close. See the module docs' "Alternate
    /// transport" section — wire shapes below are empirically confirmed, not
    /// guessed.
    async fn run_turn_ws(&self, message: &str) -> anyhow::Result<String> {
        let jwt = self.ensure_user_jwt().await?;
        let ws_url = derive_ws_url(&self.config.yw_base)?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|e| anyhow::anyhow!("[youworker][ws] invalid WS URL '{ws_url}': {e}"))?;
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {jwt}"))
                .map_err(|e| anyhow::anyhow!("[youworker][ws] invalid bearer header: {e}"))?,
        );
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static(WS_SUBPROTOCOL),
        );

        // Needed for wss:// TLS; no-op if already installed elsewhere.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (mut socket, _) = timeout(
            WS_CONTROL_TIMEOUT,
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| anyhow::anyhow!("[youworker][ws] connect timed out"))?
        .map_err(|e| anyhow::anyhow!("[youworker][ws] connect failed: {e}"))?;

        let result = self.run_turn_ws_on_socket(&mut socket, message).await;
        let _ = socket.close(None).await;
        result
    }

    async fn run_turn_ws_on_socket(
        &self,
        socket: &mut WsSocket,
        message: &str,
    ) -> anyhow::Result<String> {
        let create_result = ws_send_control(
            socket,
            "session.create",
            json!({ "persona": self.config.persona, "origin": WS_ORIGIN }),
        )
        .await?;
        let session_id = create_result
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("[youworker][ws] session.create.result missing session_id")
            })?
            .to_string();

        ws_send_control(
            socket,
            "session.attach",
            json!({ "session_id": session_id, "channel": WS_ORIGIN }),
        )
        .await?;

        ws_send_control(
            socket,
            "message.send",
            json!({ "session_id": session_id, "content": message }),
        )
        .await?;

        timeout(
            WS_TURN_TIMEOUT,
            ws_collect_terminal_response(socket, &session_id),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!("[youworker][ws] timed out waiting for the agent's response")
        })?
    }
}

#[async_trait]
impl Provider for YouWorkerProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        // youworker's own agent handles tool use server-side; this
        // abstraction never sees it — no native tool calling to declare.
        ProviderCapabilities::default()
    }

    async fn chat_with_system(
        &self,
        system_prompt: Option<&str>,
        message: &str,
        _model: &str,
        _temperature: f64,
    ) -> anyhow::Result<String> {
        // youworker's agent runs its own persona-driven system prompt
        // server-side (selected via `persona`, not a caller-supplied
        // string) — there is no channel to override it. The caller's
        // system_prompt, if any, rides as context ahead of the message
        // instead of replacing anything.
        let full_message = match system_prompt {
            Some(sp) if !sp.trim().is_empty() => format!("{sp}\n\n{message}"),
            _ => message.to_string(),
        };
        match self.config.transport {
            YouWorkerTransport::Webhook => self.run_turn_webhook(&full_message).await,
            YouWorkerTransport::Websocket => self.run_turn_ws(&full_message).await,
        }
    }
}

// ── Pure helpers (unit-tested without network I/O) ──────────────────────────

fn token_exchange_form<'a>(
    actor_token: &'a str,
    subject_uuid: &'a str,
) -> [(&'static str, &'a str); 5] {
    [
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        ),
        ("actor_token", actor_token),
        ("subject_token", subject_uuid),
        ("subject_token_type", "urn:youco:channel:youworkforce"),
        ("audience", "youworker-api"),
    ]
}

/// Build the `POST {yw_base}/channels/youmessage/inbound/message` JSON body.
/// Only the FISSO fields per `docs/reference/youworkforce-integration.md` —
/// `title` (FLESSIBILE) is omitted, defaulted server-side on first create.
fn inbound_message_body(
    conversation_ref: &str,
    persona: &str,
    message_id: &str,
    text: &str,
) -> serde_json::Value {
    json!({
        "conversation_ref": conversation_ref,
        "persona": persona,
        "message_id": message_id,
        "text": text,
    })
}

/// Derives the native WS URL from the configured HTTP `yw_base`, e.g.
/// `https://dev.youworker.ai/api/v1` → `wss://dev.youworker.ai/api/v1/ws`.
/// Matches the confirmed-working default (`wss://dev.youworker.ai/api/v1/ws`).
fn derive_ws_url(yw_base: &str) -> anyhow::Result<String> {
    let base = yw_base.trim().trim_end_matches('/');
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        anyhow::bail!("[youworker][ws] yw_base '{base}' must start with http:// or https://");
    };
    Ok(format!("{ws}/ws"))
}

/// Fold one `transcript.delta` frame into the accumulated response text.
/// Confirmed shape: `block.created` (type="response") opens the block,
/// `block.updated`'s `patch.markdown` carries the FULL cumulative text so
/// far (not a delta — no need to also track `block.appended`), and
/// `block.finalized` on that same block id is the terminal signal.
fn ws_fold_transcript_delta(
    frame: &Value,
    response_block_id: &mut Option<String>,
    response_text: &mut String,
) -> anyhow::Result<Option<String>> {
    if frame.get("type").and_then(Value::as_str) == Some("error") {
        let message = frame
            .get("payload")
            .and_then(|p| p.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        anyhow::bail!("[youworker][ws] error frame during turn: {message}");
    }
    if frame.get("type").and_then(Value::as_str) != Some("transcript.delta") {
        return Ok(None);
    }
    let Some(delta) = frame.get("payload").and_then(|p| p.get("delta")) else {
        return Ok(None);
    };
    let op = delta.get("op").and_then(Value::as_str).unwrap_or_default();

    match op {
        "block.created" => {
            let block = delta.get("block");
            if block.and_then(|b| b.get("type")).and_then(Value::as_str) == Some("response") {
                *response_block_id = block
                    .and_then(|b| b.get("block_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
        }
        "block.updated" => {
            let block_id = delta.get("block_id").and_then(Value::as_str);
            if block_id.is_some() && block_id == response_block_id.as_deref() {
                if let Some(md) = delta
                    .get("patch")
                    .and_then(|p| p.get("markdown"))
                    .and_then(Value::as_str)
                {
                    *response_text = md.to_string();
                }
            }
        }
        "block.finalized" => {
            let block_id = delta.get("block_id").and_then(Value::as_str);
            if block_id.is_some() && block_id == response_block_id.as_deref() {
                return Ok(Some(response_text.clone()));
            }
        }
        _ => {}
    }
    Ok(None)
}

// ── WS wire I/O ──────────────────────────────────────────────────────────────

type WsSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// The `ClientFrame` envelope: `{id, type, ts, payload}`.
async fn ws_send_frame(
    socket: &mut WsSocket,
    frame_type: &str,
    payload: Value,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let envelope = json!({
        "id": id,
        "type": frame_type,
        "ts": chrono::Utc::now().to_rfc3339(),
        "payload": payload,
    });
    let text = envelope.to_string();
    log::debug!("[youworker][ws] send: {text}");
    socket
        .send(WsMessage::Text(text))
        .await
        .map_err(|e| anyhow::anyhow!("[youworker][ws] send failed ({frame_type}): {e}"))?;
    Ok(id)
}

/// Send a control frame and wait for its `in_reply_to` reply
/// (`<frame_type>.result`), bailing loudly on a matching `error` frame.
/// Returns the reply's `payload`.
async fn ws_send_control(
    socket: &mut WsSocket,
    frame_type: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    let id = ws_send_frame(socket, frame_type, payload).await?;
    let expected_result_type = format!("{frame_type}.result");
    loop {
        let msg = timeout(WS_CONTROL_TIMEOUT, socket.next())
            .await
            .map_err(|_| {
                anyhow::anyhow!("[youworker][ws] timed out waiting for {expected_result_type}")
            })?
            .ok_or_else(|| {
                anyhow::anyhow!("[youworker][ws] closed while waiting for {expected_result_type}")
            })?
            .map_err(|e| anyhow::anyhow!("[youworker][ws] read error: {e}"))?;
        let WsMessage::Text(text) = msg else { continue };
        log::debug!("[youworker][ws] recv: {text}");
        let frame: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("[youworker][ws] malformed frame: {e} — raw: {text}"))?;
        let in_reply_to = frame.get("in_reply_to").and_then(Value::as_str);
        let is_error = frame.get("type").and_then(Value::as_str) == Some("error");
        if in_reply_to != Some(id.as_str()) && !is_error {
            continue;
        }
        if is_error {
            let message = frame
                .get("payload")
                .and_then(|p| p.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            anyhow::bail!("[youworker][ws] {expected_result_type} rejected: {message}");
        }
        return Ok(frame.get("payload").cloned().unwrap_or(Value::Null));
    }
}

/// Read `transcript.delta` frames until the response block finalizes.
async fn ws_collect_terminal_response(
    socket: &mut WsSocket,
    _session_id: &str,
) -> anyhow::Result<String> {
    let mut response_block_id: Option<String> = None;
    let mut response_text = String::new();
    loop {
        let msg = socket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("[youworker][ws] closed before the turn completed"))?
            .map_err(|e| anyhow::anyhow!("[youworker][ws] read error: {e}"))?;
        let WsMessage::Text(raw) = msg else { continue };
        log::debug!("[youworker][ws] recv: {raw}");
        let frame: Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("[youworker][ws] malformed frame: {e} — raw: {raw}"))?;
        if let Some(text) =
            ws_fold_transcript_delta(&frame, &mut response_block_id, &mut response_text)?
        {
            return Ok(text);
        }
    }
}

#[cfg(test)]
#[path = "youworker_tests.rs"]
mod tests;
