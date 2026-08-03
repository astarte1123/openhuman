use super::*;
use serde_json::json;

// ── derive_ws_url ────────────────────────────────────────────────────────────

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
            (
                "subject_token",
                "00000000-0000-0000-0000-000000000002"
            ),
            ("subject_token_type", "urn:youco:channel:youworkforce"),
            ("audience", "youworker-api"),
        ]
    );
}

// ── fold_transcript_delta ────────────────────────────────────────────────────

// Every fixture uses the confirmed `ClientFrame`/`ServerFrame` envelope
// (`{id, type, ts, payload}`) with content-bearing fields nested under
// `payload`, and the confirmed block-delta shapes captured from a live
// `dev.youworker.ai` run (see `fold_transcript_delta`'s doc comment):
// `block.created` nests the new block's `block_id`/`type`/`markdown` under
// `delta.block`; `block.appended`/`block.updated`/`block.finalized` carry
// `block_id` directly on `delta`, and `block.appended`'s chunk rides
// `delta.text` gated by `delta.field == "markdown"`, while `block.updated`'s
// full replacement rides `delta.patch.markdown`.

#[test]
fn fold_transcript_delta_ignores_other_frame_types() {
    let mut texts = HashMap::new();
    let mut response_block_id = None;
    let frame = json!({ "type": "status.update", "payload": { "session_id": "s1" } });
    let outcome = fold_transcript_delta(&frame, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Continue);
    assert!(texts.is_empty());
}

#[test]
fn fold_transcript_delta_ignores_other_sessions() {
    let mut texts = HashMap::new();
    let mut response_block_id = None;
    let frame = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "other-session",
            "delta": { "op": "block.created", "block": { "block_id": "b1", "type": "response", "markdown": "" } }
        }
    });
    let outcome = fold_transcript_delta(&frame, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Continue);
    assert!(texts.is_empty());
}

#[test]
fn fold_transcript_delta_ignores_the_echoed_user_block() {
    // The user's own message rides back as a `type: "user"` block.created —
    // it must never be mistaken for the response.
    let mut texts = HashMap::new();
    let mut response_block_id = None;
    let frame = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.created", "block": { "block_id": "u1", "type": "user", "markdown": "hi" } }
        }
    });
    let outcome = fold_transcript_delta(&frame, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Continue);
    assert!(response_block_id.is_none());
    assert!(texts.is_empty());
}

#[test]
fn fold_transcript_delta_accumulates_and_finalizes() {
    let mut texts = HashMap::new();
    let mut response_block_id = None;

    let created = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.created", "block": { "block_id": "b1", "type": "response", "markdown": "" } }
        }
    });
    assert_eq!(
        fold_transcript_delta(&created, "s1", &mut texts, &mut response_block_id).unwrap(),
        FoldOutcome::Continue
    );
    assert_eq!(response_block_id.as_deref(), Some("b1"));

    let appended = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.appended", "block_id": "b1", "field": "markdown", "text": "pong" }
        }
    });
    assert_eq!(
        fold_transcript_delta(&appended, "s1", &mut texts, &mut response_block_id).unwrap(),
        FoldOutcome::Continue
    );

    let updated = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.updated", "block_id": "b1", "patch": { "markdown": "pong" } }
        }
    });
    assert_eq!(
        fold_transcript_delta(&updated, "s1", &mut texts, &mut response_block_id).unwrap(),
        FoldOutcome::Continue
    );

    let finalized = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.finalized", "block_id": "b1" }
        }
    });
    let outcome = fold_transcript_delta(&finalized, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Finalized("pong".to_string()));
}

#[test]
fn fold_transcript_delta_ignores_appended_and_finalized_for_other_blocks() {
    // A reasoning block (or any non-response block) streaming/finalizing
    // alongside the response must never affect the tracked response text.
    let mut texts = HashMap::new();
    let mut response_block_id = None;

    let response_created = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.created", "block": { "block_id": "response-1", "type": "response", "markdown": "" } }
        }
    });
    fold_transcript_delta(&response_created, "s1", &mut texts, &mut response_block_id).unwrap();

    let reasoning_appended = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.appended", "block_id": "reasoning-1", "field": "markdown", "text": "thinking…" }
        }
    });
    let outcome = fold_transcript_delta(&reasoning_appended, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Continue);

    let reasoning_finalized = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.finalized", "block_id": "reasoning-1" }
        }
    });
    let outcome = fold_transcript_delta(&reasoning_finalized, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Continue);

    let response_appended = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.appended", "block_id": "response-1", "field": "markdown", "text": "Answer" }
        }
    });
    fold_transcript_delta(&response_appended, "s1", &mut texts, &mut response_block_id).unwrap();

    let response_finalized = json!({
        "type": "transcript.delta",
        "payload": {
            "session_id": "s1",
            "delta": { "op": "block.finalized", "block_id": "response-1" }
        }
    });
    let outcome = fold_transcript_delta(&response_finalized, "s1", &mut texts, &mut response_block_id).unwrap();
    assert_eq!(outcome, FoldOutcome::Finalized("Answer".to_string()));
}

#[test]
fn fold_transcript_delta_bails_on_error_frame() {
    let mut texts = HashMap::new();
    let mut response_block_id = None;
    let frame = json!({ "type": "error", "payload": { "message": "policy_denied" } });
    let err = fold_transcript_delta(&frame, "s1", &mut texts, &mut response_block_id).unwrap_err();
    assert!(err.to_string().contains("policy_denied"));
}

// ── Manual live smoke test ───────────────────────────────────────────────────
//
// Not part of `make test-unit` / CI — no network access and no credentials
// there, matching the mock-backend-only automated suite. Run by hand once per
// change to the wire format, with real credentials passed via env (never
// hardcoded/committed):
//
//   YOUWORKER_TEST_AUTENTIKA_URL=https://<workspace>.autentika.<host> \
//   YOUWORKER_TEST_YW_BASE=https://<host>/api/v1 \
//   YOUWORKER_TEST_CLIENT_ID=... \
//   YOUWORKER_TEST_CLIENT_SECRET=... \
//   YOUWORKER_TEST_PERSONA=... \
//   YOUWORKER_TEST_SUBJECT_UUID=... \
//   cargo test --manifest-path Cargo.toml --lib youworker_live_smoke -- --ignored --nocapture
#[tokio::test]
#[ignore = "hits a real youworker.ai deployment — run manually with YOUWORKER_TEST_* env vars set"]
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
