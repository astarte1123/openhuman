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
    });
    let response = provider
        .chat_with_system(None, "Say the single word: pong", "", 0.0)
        .await
        .expect("live youworker turn failed");
    println!("[youworker_live_smoke] response: {response}");
    assert!(!response.trim().is_empty());
}
