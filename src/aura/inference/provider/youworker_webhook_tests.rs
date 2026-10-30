use super::*;
use base64::Engine;

/// Independently computes a valid Standard-Webhooks signature header for a
/// given `whsec_<base64>` secret + delivery, mirroring
/// `docs/reference/youworkforce-integration.md`'s algorithm by hand (not by
/// calling `verify_signature`) so the test actually exercises the function
/// under test against ground truth.
fn sign(secret: &str, webhook_id: &str, webhook_timestamp: &str, raw_body: &[u8]) -> String {
    let encoded_key = secret
        .strip_prefix("whsec_")
        .expect("test secret must have whsec_ prefix");
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_key)
        .expect("test secret must be valid base64");
    let mut signed_content = Vec::new();
    signed_content.extend_from_slice(webhook_id.as_bytes());
    signed_content.push(b'.');
    signed_content.extend_from_slice(webhook_timestamp.as_bytes());
    signed_content.push(b'.');
    signed_content.extend_from_slice(raw_body);
    let mut mac = HmacSha256::new_from_slice(&key_bytes).unwrap();
    mac.update(&signed_content);
    let sig = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    format!("v1,{sig}")
}

const TEST_SECRET: &str = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";

// ── verify_signature ─────────────────────────────────────────────────────────

#[test]
fn verify_signature_accepts_a_correctly_signed_delivery() {
    let body = br#"{"conversation_ref":"c1","text":"pong","idempotency_key":"k1"}"#;
    let sig = sign(TEST_SECRET, "wh_1", "1700000000", body);
    assert!(verify_signature(
        TEST_SECRET,
        "wh_1",
        "1700000000",
        body,
        &sig
    ));
}

#[test]
fn verify_signature_rejects_a_tampered_body() {
    let body = br#"{"conversation_ref":"c1","text":"pong","idempotency_key":"k1"}"#;
    let sig = sign(TEST_SECRET, "wh_1", "1700000000", body);
    let tampered = br#"{"conversation_ref":"c1","text":"PWNED","idempotency_key":"k1"}"#;
    assert!(!verify_signature(
        TEST_SECRET,
        "wh_1",
        "1700000000",
        tampered,
        &sig
    ));
}

#[test]
fn verify_signature_rejects_wrong_secret() {
    let body = br#"{"conversation_ref":"c1"}"#;
    let sig = sign(TEST_SECRET, "wh_1", "1700000000", body);
    let other_secret = "whsec_ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ";
    assert!(!verify_signature(
        other_secret,
        "wh_1",
        "1700000000",
        body,
        &sig
    ));
}

#[test]
fn verify_signature_rejects_missing_whsec_prefix() {
    let body = br#"{"conversation_ref":"c1"}"#;
    assert!(!verify_signature(
        "not-a-whsec-secret",
        "wh_1",
        "1700000000",
        body,
        "v1,anything"
    ));
}

#[test]
fn verify_signature_accepts_when_any_tuple_in_a_rotation_list_matches() {
    let body = br#"{"conversation_ref":"c1"}"#;
    let good = sign(TEST_SECRET, "wh_1", "1700000000", body);
    let header = format!("v1,bm9uc2Vuc2U= {good}");
    assert!(verify_signature(
        TEST_SECRET,
        "wh_1",
        "1700000000",
        body,
        &header
    ));
}

#[test]
fn verify_signature_rejects_empty_signature_header() {
    let body = br#"{}"#;
    assert!(!verify_signature(
        TEST_SECRET,
        "wh_1",
        "1700000000",
        body,
        ""
    ));
}

// ── within_replay_window ─────────────────────────────────────────────────────

#[test]
fn within_replay_window_accepts_current_timestamp() {
    let now = chrono::Utc::now().timestamp().to_string();
    assert!(within_replay_window(&now));
}

#[test]
fn within_replay_window_accepts_just_inside_the_window() {
    let ts = (chrono::Utc::now().timestamp() - (REPLAY_WINDOW_SECS - 5)).to_string();
    assert!(within_replay_window(&ts));
}

#[test]
fn within_replay_window_rejects_stale_timestamp() {
    let ts = (chrono::Utc::now().timestamp() - (REPLAY_WINDOW_SECS + 5)).to_string();
    assert!(!within_replay_window(&ts));
}

#[test]
fn within_replay_window_rejects_future_timestamp_beyond_window() {
    let ts = (chrono::Utc::now().timestamp() + (REPLAY_WINDOW_SECS + 5)).to_string();
    assert!(!within_replay_window(&ts));
}

#[test]
fn within_replay_window_rejects_non_numeric_timestamp() {
    assert!(!within_replay_window("not-a-number"));
}

// ── claim_idempotency_key ────────────────────────────────────────────────────

#[test]
fn claim_idempotency_key_first_claim_succeeds_then_dedups() {
    let key = format!("test-key-{}", uuid::Uuid::new_v4());
    assert!(claim_idempotency_key(&key));
    assert!(!claim_idempotency_key(&key));
    assert!(!claim_idempotency_key(&key));
}

// ── pending registry ─────────────────────────────────────────────────────────

#[tokio::test]
async fn register_and_resolve_pending_delivers_the_text() {
    let conversation_ref = format!("test-conv-{}", uuid::Uuid::new_v4());
    let rx = register_pending(&conversation_ref);
    assert!(resolve_pending(&conversation_ref, "hello".to_string()));
    assert_eq!(rx.await.unwrap(), "hello");
}

#[test]
fn resolve_pending_returns_false_when_nothing_is_registered() {
    let conversation_ref = format!("test-conv-{}", uuid::Uuid::new_v4());
    assert!(!resolve_pending(&conversation_ref, "orphan".to_string()));
}

#[test]
fn cancel_pending_removes_the_entry_without_resolving() {
    let conversation_ref = format!("test-conv-{}", uuid::Uuid::new_v4());
    let _rx = register_pending(&conversation_ref);
    cancel_pending(&conversation_ref);
    assert!(!resolve_pending(&conversation_ref, "too-late".to_string()));
}

#[test]
fn outbound_hmac_key_for_slug_is_distinct_from_the_bearer_key_namespace() {
    let key = outbound_hmac_key_for_slug("youworker");
    assert_eq!(key, "provider:youworker:outbound-hmac");
    assert_ne!(key, "provider:youworker");
}
