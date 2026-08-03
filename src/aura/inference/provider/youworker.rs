//! youworker.ai native WebSocket provider.
//!
//! Speaks youworker.ai's native transport (Autentika OAuth2 token-exchange +
//! `session.create` / `session.attach` / `message.send` over the
//! `youworker.v1` WebSocket subprotocol) rather than an OpenAI-compatible
//! HTTP API. One-shot per call, matching every other [`Provider`] impl in
//! this module: each `chat_with_system` invocation opens a fresh connection,
//! creates a new youworker session, sends one message, folds the streamed
//! `transcript.delta` frames into the terminal response text, and closes.
//! No reconnect/resume/heartbeat — that fidelity exists for a long-lived
//! multi-turn UI client (yw-web/yw-cli), not a stateless per-call provider.
//!
//! The wire envelope was reconstructed from prose docs (no formal
//! OpenAPI/AsyncAPI schema exists for the WS frames) — see
//! `docs/explanation/ws-transport.md` and
//! `docs/adr/0003-websocket-transport-and-transcript-delivery.md` in the
//! youworker.ai repo. Every inbound frame is logged raw at `debug` so a
//! mismatch between this guess and the live server is diagnosable rather
//! than a silent hang.

use super::traits::{Provider, ProviderCapabilities};
use crate::aura::credentials::profiles::TokenSet;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub const PROVIDER_LABEL: &str = "YouWorker";

/// Wait budget for a single Control-plane request/reply (`session.create`,
/// `session.attach`) and for the WS handshake itself.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
/// Wait budget for the agent to finish a turn end-to-end.
const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh the cached user JWT this far ahead of its `expires_at`.
const TOKEN_EXPIRY_SKEW: Duration = Duration::from_secs(60);

type WsSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Static connection config for one youworker.ai `AuthStyle::YouWorker`
/// `cloud_providers` entry. `client_secret` is resolved by the factory via
/// the same `provider:<slug>` `AuthService` lookup every other provider's
/// API key uses — never stored on [`CloudProviderCreds`].
#[derive(Debug, Clone)]
pub struct YouWorkerConfig {
    /// Autentika workspace issuer, e.g. `https://your-workspace.autentika.example`.
    pub autentika_url: String,
    /// Public inbound base, e.g. `https://dev.youworker.ai/api/v1` — the WS
    /// endpoint is derived from this (`{scheme}://{host}/api/v1/ws`).
    pub yw_base: String,
    pub client_id: String,
    pub client_secret: String,
    pub persona: String,
    /// The youworker end-user UUID presented as `subject_token` (MVP: one
    /// youworker user per broker grant).
    pub subject_uuid: String,
}

/// Outcome of folding one `transcript.delta` frame into the running response.
#[derive(Debug, PartialEq, Eq)]
enum FoldOutcome {
    /// Keep reading.
    Continue,
    /// The turn's terminal text block just finalized — this is the answer.
    Finalized(String),
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

    /// Derive the native WS URL from the configured HTTP `yw_base`, e.g.
    /// `https://dev.youworker.ai/api/v1` → `wss://dev.youworker.ai/api/v1/ws`.
    fn ws_url(&self) -> anyhow::Result<String> {
        derive_ws_url(&self.config.yw_base)
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

        let actor_resp = self
            .http
            .post(format!("{autentika}/oauth2/token"))
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
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

        let exch_resp = self
            .http
            .post(format!("{autentika}/v1/oauth/token_exchange"))
            .form(&token_exchange_form(
                &actor.access_token,
                &self.config.subject_uuid,
            ))
            .send()
            .await
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

    /// Connect, create + attach a session, send `message`, collect the
    /// terminal response, close.
    async fn run_turn(&self, message: &str) -> anyhow::Result<String> {
        let jwt = self.ensure_user_jwt().await?;
        let ws_url = self.ws_url()?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|e| anyhow::anyhow!("[youworker] invalid WS URL '{ws_url}': {e}"))?;
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {jwt}"))
                .map_err(|e| anyhow::anyhow!("[youworker] invalid bearer header: {e}"))?,
        );
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static("youworker.v1"),
        );

        // Needed for wss:// TLS; no-op if already installed elsewhere.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (mut socket, _) = timeout(CONTROL_TIMEOUT, tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| anyhow::anyhow!("[youworker] WS connect timed out"))?
            .map_err(|e| anyhow::anyhow!("[youworker] WS connect failed: {e}"))?;

        let create_result = send_control(
            &mut socket,
            "session.create",
            json!({ "persona": self.config.persona }),
        )
        .await;
        let create_result = match create_result {
            Ok(v) => v,
            Err(e) => {
                let _ = socket.close(None).await;
                return Err(e);
            }
        };
        let session_id = create_result
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("[youworker] session.create.result missing session_id"));
        let session_id = match session_id {
            Ok(id) => id.to_string(),
            Err(e) => {
                let _ = socket.close(None).await;
                return Err(e);
            }
        };

        if let Err(e) = send_control(
            &mut socket,
            "session.attach",
            json!({ "session_id": session_id, "channel": "yw-web" }),
        )
        .await
        {
            let _ = socket.close(None).await;
            return Err(e);
        }

        if let Err(e) = send_frame(
            &mut socket,
            "message.send",
            json!({ "session_id": session_id, "content": message }),
        )
        .await
        {
            let _ = socket.close(None).await;
            return Err(e);
        }

        let response = timeout(
            TURN_TIMEOUT,
            collect_terminal_response(&mut socket, &session_id),
        )
        .await
        .map_err(|_| anyhow::anyhow!("[youworker] timed out waiting for the agent's response"));

        let _ = socket.close(None).await;
        response?
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
        self.run_turn(&full_message).await
    }
}

// ── Pure helpers (unit-tested without a live socket) ────────────────────────

fn derive_ws_url(yw_base: &str) -> anyhow::Result<String> {
    let base = yw_base.trim().trim_end_matches('/');
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        anyhow::bail!("[youworker] yw_base '{base}' must start with http:// or https://");
    };
    Ok(format!("{ws}/ws"))
}

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

/// Fold one `transcript.delta` frame's effect into the accumulated response
/// text. Confirmed against a live `dev.youworker.ai` run (see git history /
/// PR description for the captured frame log) — the actually-observed
/// shapes, not a guess:
///
/// * `block.created` — `delta.block.block_id` (note: `block_id`, not `id`),
///   `delta.block.type` (`"user"` for the echoed input, `"response"` for the
///   agent's answer — this is the terminal-answer discriminator), initial
///   `delta.block.markdown` (often `""` while streaming).
/// * `block.appended` — `delta.block_id` (top-level here), `delta.field`
///   (which field is growing — only `"markdown"` matters), `delta.text`
///   (the chunk).
/// * `block.updated` — `delta.block_id` (top-level), `delta.patch.markdown`
///   (authoritative full replacement, not a delta).
/// * `block.finalized` — `delta.block_id` (top-level) only.
///
/// Tracks the response block's id once seen and treats ITS finalization as
/// the turn's terminal answer; a `"user"`-typed block (the input echoed back
/// as a transcript entry) and any other block kind (e.g. reasoning) are
/// never mistaken for the answer.
fn fold_transcript_delta(
    frame: &Value,
    session_id: &str,
    texts: &mut HashMap<String, String>,
    response_block_id: &mut Option<String>,
) -> anyhow::Result<FoldOutcome> {
    if frame.get("type").and_then(Value::as_str) == Some("error") {
        let message = frame
            .get("payload")
            .and_then(|p| p.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        anyhow::bail!("[youworker] error frame during turn: {message}");
    }
    if frame.get("type").and_then(Value::as_str) != Some("transcript.delta") {
        return Ok(FoldOutcome::Continue);
    }
    let Some(payload) = frame.get("payload") else {
        return Ok(FoldOutcome::Continue);
    };
    if payload.get("session_id").and_then(Value::as_str) != Some(session_id) {
        // A delegation subtree's child-addressed delta — out of scope for
        // this minimal one-shot client.
        return Ok(FoldOutcome::Continue);
    }
    let Some(delta) = payload.get("delta") else {
        return Ok(FoldOutcome::Continue);
    };
    let op = delta.get("op").and_then(Value::as_str).unwrap_or_default();

    match op {
        "block.created" => {
            let block = delta.get("block");
            let kind = block.and_then(|b| b.get("type")).and_then(Value::as_str);
            if kind != Some("response") {
                return Ok(FoldOutcome::Continue);
            }
            let Some(id) = block
                .and_then(|b| b.get("block_id"))
                .and_then(Value::as_str)
            else {
                return Ok(FoldOutcome::Continue);
            };
            let md = block
                .and_then(|b| b.get("markdown"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            texts.insert(id.to_string(), md.to_string());
            *response_block_id = Some(id.to_string());
        }
        "block.appended" => {
            if delta.get("field").and_then(Value::as_str) != Some("markdown") {
                return Ok(FoldOutcome::Continue);
            }
            let id = delta.get("block_id").and_then(Value::as_str);
            if id != response_block_id.as_deref() {
                return Ok(FoldOutcome::Continue);
            }
            if let (Some(id), Some(chunk)) = (id, delta.get("text").and_then(Value::as_str)) {
                texts.entry(id.to_string()).or_default().push_str(chunk);
            }
        }
        "block.updated" => {
            let id = delta.get("block_id").and_then(Value::as_str);
            if id != response_block_id.as_deref() {
                return Ok(FoldOutcome::Continue);
            }
            if let (Some(id), Some(md)) = (
                id,
                delta
                    .get("patch")
                    .and_then(|p| p.get("markdown"))
                    .and_then(Value::as_str),
            ) {
                texts.insert(id.to_string(), md.to_string());
            }
        }
        "block.finalized" => {
            let id = delta.get("block_id").and_then(Value::as_str);
            if id.is_some() && id == response_block_id.as_deref() {
                let id = id.unwrap();
                return Ok(FoldOutcome::Finalized(texts.remove(id).unwrap_or_default()));
            }
        }
        _ => {}
    }
    Ok(FoldOutcome::Continue)
}

// ── Wire I/O ─────────────────────────────────────────────────────────────────

/// The `ClientFrame` envelope: `{id, type, ts, payload}` — every field
/// content-bearing field rides under `payload`; `id`/`type`/`ts` are
/// protocol metadata. Confirmed against the live server's own `error`
/// frame shape (`{"id":"err_parse","type":"error","ts":"...","payload":
/// {"code":"invalid_frame","message":"..."}}`) after an initial guess that
/// flattened fields at the top level and omitted `ts` was rejected with
/// `pydantic` `ClientFrame.ts: Field required`.
async fn send_frame(
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
        .map_err(|e| anyhow::anyhow!("[youworker] WS send failed ({frame_type}): {e}"))?;
    Ok(id)
}

/// Send a Control-plane frame and wait for its `in_reply_to` reply
/// (`<frame_type>.result`), bailing loudly on a matching `error` frame.
/// Returns the reply's `payload` (not the whole envelope).
async fn send_control(
    socket: &mut WsSocket,
    frame_type: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    let id = send_frame(socket, frame_type, payload).await?;
    let expected_result_type = format!("{frame_type}.result");
    loop {
        let msg = timeout(CONTROL_TIMEOUT, socket.next())
            .await
            .map_err(|_| {
                anyhow::anyhow!("[youworker] timed out waiting for {expected_result_type}")
            })?
            .ok_or_else(|| {
                anyhow::anyhow!("[youworker] WS closed while waiting for {expected_result_type}")
            })?
            .map_err(|e| anyhow::anyhow!("[youworker] WS read error: {e}"))?;
        let WsMessage::Text(text) = msg else { continue };
        log::debug!("[youworker][ws] recv: {text}");
        let frame: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("[youworker] malformed frame: {e} — raw: {text}"))?;
        let in_reply_to = frame.get("in_reply_to").and_then(Value::as_str);
        let is_error = frame.get("type").and_then(Value::as_str) == Some("error");
        // A parse-level rejection (`id: "err_parse"`) never correlates via
        // `in_reply_to` — surface it immediately rather than waiting out
        // the full timeout for a reply that will never arrive.
        if in_reply_to != Some(id.as_str()) && !is_error {
            continue;
        }
        if is_error {
            let payload = frame.get("payload");
            let message = payload
                .and_then(|p| p.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            anyhow::bail!("[youworker] {expected_result_type} rejected: {message}");
        }
        return Ok(frame.get("payload").cloned().unwrap_or(Value::Null));
    }
}

/// Read `transcript.delta` frames until the response text's block finalizes.
async fn collect_terminal_response(
    socket: &mut WsSocket,
    session_id: &str,
) -> anyhow::Result<String> {
    let mut texts: HashMap<String, String> = HashMap::new();
    let mut response_block_id: Option<String> = None;
    loop {
        let msg = socket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("[youworker] WS closed before the turn completed"))?
            .map_err(|e| anyhow::anyhow!("[youworker] WS read error: {e}"))?;
        let WsMessage::Text(raw) = msg else { continue };
        log::debug!("[youworker][ws] recv: {raw}");
        let frame: Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("[youworker] malformed frame: {e} — raw: {raw}"))?;
        match fold_transcript_delta(&frame, session_id, &mut texts, &mut response_block_id)? {
            FoldOutcome::Continue => {}
            FoldOutcome::Finalized(text) => return Ok(text),
        }
    }
}

#[cfg(test)]
#[path = "youworker_tests.rs"]
mod tests;
