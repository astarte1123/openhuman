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

use super::traits::{Provider, ProviderCapabilities};
use super::youworker_webhook::{cancel_pending, register_pending};
use crate::aura::credentials::profiles::TokenSet;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

pub const PROVIDER_LABEL: &str = "YouWorker";

/// The only inbound channel a text [`Provider`] speaks — youworker's other
/// webhook channel, `youtalkie`, is voice-only (see module docs).
const CHANNEL: &str = "youmessage";

/// Wait budget for the Autentika token-exchange HTTP calls and the inbound
/// `POST .../inbound/message` itself (all fast, synchronous HTTP round-trips).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Wait budget for youworker's async webhook reply to arrive after the
/// inbound POST is accepted (an agent turn can run long).
const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh the cached user JWT this far ahead of its `expires_at`.
const TOKEN_EXPIRY_SKEW: Duration = Duration::from_secs(60);

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
    async fn run_turn(&self, message: &str) -> anyhow::Result<String> {
        let jwt = self.ensure_user_jwt().await?;
        let conversation_ref = uuid::Uuid::new_v4().to_string();
        let message_id = uuid::Uuid::new_v4().to_string();

        let receiver = register_pending(&conversation_ref);

        let url = format!(
            "{}/channels/{CHANNEL}/inbound/message",
            self.config.yw_base.trim_end_matches('/')
        );
        let body = inbound_message_body(&conversation_ref, &self.config.persona, &message_id, message);

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

#[cfg(test)]
#[path = "youworker_tests.rs"]
mod tests;
