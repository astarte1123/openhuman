use super::*;

// ── token_exchange_form ──────────────────────────────────────────────────────

#[test]
fn token_exchange_form_has_the_fixed_rfc8693_fields() {
    let form = token_exchange_form("actor-tok", "00000000-0000-0000-0000-000000000002");
    assert_eq!(
        form,
        [
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange"
            ),
            ("actor_token", "actor-tok"),
            ("subject_token", "00000000-0000-0000-0000-000000000002"),
            ("subject_token_type", "urn:youco:channel:youworkforce"),
            ("audience", "youworker-api"),
        ]
    );
}

// ── inbound_message_body ─────────────────────────────────────────────────────

#[test]
fn inbound_message_body_has_the_fixed_fields_only() {
    let body = inbound_message_body("conv-1", "operatore-centrale", "msg-1", "hello");
    assert_eq!(
        body,
        serde_json::json!({
            "conversation_ref": "conv-1",
            "persona": "operatore-centrale",
            "message_id": "msg-1",
            "text": "hello",
        })
    );
}

#[test]
fn inbound_message_body_preserves_text_verbatim() {
    // No trimming/mangling — the caller's already-assembled system+message
    // text rides through unchanged.
    let body = inbound_message_body("c", "p", "m", "line one\n\nline two  ");
    assert_eq!(body["text"], "line one\n\nline two  ");
}

// ── YouWorkerTransport::from_config_str ──────────────────────────────────────

#[test]
fn transport_from_config_str_parses_websocket() {
    assert_eq!(
        YouWorkerTransport::from_config_str("websocket"),
        YouWorkerTransport::Websocket
    );
}

#[test]
fn transport_from_config_str_defaults_to_webhook() {
    for s in ["", "webhook", "  ", "bogus", "WEBSOCKET"] {
        assert_eq!(
            YouWorkerTransport::from_config_str(s),
            YouWorkerTransport::Webhook,
            "input={s:?}"
        );
    }
}

// ── derive_ws_url ─────────────────────────────────────────────────────────────

#[test]
fn derive_ws_url_https_to_wss() {
    assert_eq!(
        derive_ws_url("https://dev.youworker.ai/api/v1").unwrap(),
        "wss://dev.youworker.ai/api/v1/ws"
    );
}

#[test]
fn derive_ws_url_http_to_ws() {
    assert_eq!(
        derive_ws_url("http://localhost:8070/api/v1").unwrap(),
        "ws://localhost:8070/api/v1/ws"
    );
}

#[test]
fn derive_ws_url_strips_trailing_slash() {
    assert_eq!(
        derive_ws_url("https://dev.youworker.ai/api/v1/").unwrap(),
        "wss://dev.youworker.ai/api/v1/ws"
    );
}

#[test]
fn derive_ws_url_rejects_missing_scheme() {
    assert!(derive_ws_url("dev.youworker.ai/api/v1").is_err());
}

// ── ws_fold_transcript_delta ─────────────────────────────────────────────────
//
// Confirmed shape: block.created(type=response) opens the block;
// block.updated.patch.markdown is the FULL cumulative text (not appended
// chunks); block.finalized on that block id is the terminal signal.

#[test]
fn ws_fold_ignores_non_transcript_frames() {
    let mut id = None;
    let mut text = String::new();
    let frame = serde_json::json!({ "type": "status.update", "payload": {} });
    assert_eq!(
        ws_fold_transcript_delta(&frame, &mut id, &mut text).unwrap(),
        None
    );
}

#[test]
fn ws_fold_bails_on_error_frame() {
    let mut id = None;
    let mut text = String::new();
    let frame = serde_json::json!({ "type": "error", "payload": { "message": "policy_denied" } });
    let err = ws_fold_transcript_delta(&frame, &mut id, &mut text).unwrap_err();
    assert!(err.to_string().contains("policy_denied"));
}

#[test]
fn ws_fold_ignores_non_response_block_created() {
    let mut id = None;
    let mut text = String::new();
    let frame = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.created", "block": { "block_id": "u1", "type": "user" } } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&frame, &mut id, &mut text).unwrap(),
        None
    );
    assert!(id.is_none());
}

#[test]
fn ws_fold_full_turn_accumulates_and_finalizes() {
    let mut id = None;
    let mut text = String::new();

    let created = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.created", "block": { "block_id": "b1", "type": "response" } } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&created, &mut id, &mut text).unwrap(),
        None
    );
    assert_eq!(id.as_deref(), Some("b1"));

    let updated = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.updated", "block_id": "b1", "patch": { "markdown": "pong" } } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&updated, &mut id, &mut text).unwrap(),
        None
    );
    assert_eq!(text, "pong");

    // A later block.updated replaces (not appends) — confirmed shape carries
    // the full cumulative text each time.
    let updated_again = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.updated", "block_id": "b1", "patch": { "markdown": "pong pong" } } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&updated_again, &mut id, &mut text).unwrap(),
        None
    );
    assert_eq!(text, "pong pong");

    let finalized = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.finalized", "block_id": "b1" } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&finalized, &mut id, &mut text).unwrap(),
        Some("pong pong".to_string())
    );
}

#[test]
fn ws_fold_ignores_finalize_for_other_blocks() {
    let mut id = Some("b1".to_string());
    let mut text = "answer".to_string();
    let frame = serde_json::json!({
        "type": "transcript.delta",
        "payload": { "delta": { "op": "block.finalized", "block_id": "reasoning-1" } }
    });
    assert_eq!(
        ws_fold_transcript_delta(&frame, &mut id, &mut text).unwrap(),
        None
    );
}

// ── Manual live smoke test ───────────────────────────────────────────────────
//
// Not part of `make test-unit` / CI — no network access there, and this test
// additionally needs the core's own HTTP server *running and reachable from
// youworker.ai* (e.g. via a local tunnel set as `ywf_base` on the DEV
// deployment) so the async outbound webhook can land on
// `POST /channels/youmessage/outbound/message` and resolve the pending turn.
// Without that, this call always times out after `TURN_TIMEOUT` even with
// perfectly valid credentials — the inbound POST alone only proves
// acceptance (`202`), never completion.
//
// Run by hand once per change to the wire format, with real credentials
// passed via env (never hardcoded/committed) and the core already running
// with the matching `cloud_providers` entry + both stored secrets
// (`client_secret` and the `outbound-hmac` secret) configured:
//
//   YOUWORKER_TEST_AUTENTIKA_URL=https://<workspace>.autentika.<host> \
//   YOUWORKER_TEST_YW_BASE=https://<host>/api/v1 \
//   YOUWORKER_TEST_CLIENT_ID=... \
//   YOUWORKER_TEST_CLIENT_SECRET=... \
//   YOUWORKER_TEST_PERSONA=... \
//   YOUWORKER_TEST_SUBJECT_UUID=... \
//   cargo test --manifest-path Cargo.toml --lib youworker_live_smoke -- --ignored --nocapture
#[tokio::test]
#[ignore = "hits a real youworker.ai deployment and needs our own webhook reachable — run manually"]
async fn youworker_live_smoke() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let provider = YouWorkerProvider::new(YouWorkerConfig {
        autentika_url: env("YOUWORKER_TEST_AUTENTIKA_URL"),
        yw_base: env("YOUWORKER_TEST_YW_BASE"),
        client_id: env("YOUWORKER_TEST_CLIENT_ID"),
        client_secret: env("YOUWORKER_TEST_CLIENT_SECRET"),
        persona: env("YOUWORKER_TEST_PERSONA"),
        subject_uuid: env("YOUWORKER_TEST_SUBJECT_UUID"),
        transport: YouWorkerTransport::Webhook,
    });
    let response = provider
        .chat_with_system(None, "Say the single word: pong", "", 0.0)
        .await
        .expect("live youworker turn failed");
    println!("[youworker_live_smoke] response: {response}");
    assert!(!response.trim().is_empty());
}

/// Same live smoke test but over the WS transport — no webhook/tunnel setup
/// needed, just the same Autentika/youworker credentials:
///
///   YOUWORKER_TEST_AUTENTIKA_URL=... YOUWORKER_TEST_YW_BASE=... \
///   YOUWORKER_TEST_CLIENT_ID=... YOUWORKER_TEST_CLIENT_SECRET=... \
///   YOUWORKER_TEST_PERSONA=... YOUWORKER_TEST_SUBJECT_UUID=... \
///   cargo test --manifest-path Cargo.toml --lib youworker_ws_live_smoke -- --ignored --nocapture
#[tokio::test]
#[ignore = "hits a real youworker.ai deployment — run manually with YOUWORKER_TEST_* env vars set"]
async fn youworker_ws_live_smoke() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let provider = YouWorkerProvider::new(YouWorkerConfig {
        autentika_url: env("YOUWORKER_TEST_AUTENTIKA_URL"),
        yw_base: env("YOUWORKER_TEST_YW_BASE"),
        client_id: env("YOUWORKER_TEST_CLIENT_ID"),
        client_secret: env("YOUWORKER_TEST_CLIENT_SECRET"),
        persona: env("YOUWORKER_TEST_PERSONA"),
        subject_uuid: env("YOUWORKER_TEST_SUBJECT_UUID"),
        transport: YouWorkerTransport::Websocket,
    });
    let response = provider
        .chat_with_system(None, "Say the single word: pong", "", 0.0)
        .await
        .expect("live youworker WS turn failed");
    println!("[youworker_ws_live_smoke] response: {response}");
    assert!(!response.trim().is_empty());
}
