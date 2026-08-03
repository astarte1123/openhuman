//! Recipe-event payload parsing and the provider-surfaces ingest bridge.
//! Split out of `mod.rs` — no behavior changes.

use std::time::Duration;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::*;

pub(crate) fn payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn payload_bool(payload: &serde_json::Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(|v| v.as_bool())
}

pub(crate) fn payload_i64(payload: &serde_json::Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|v| v.as_i64())
}

pub(crate) fn first_message_field(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get("messages")
        .and_then(|v| v.as_array())
        .and_then(|messages| messages.first())
        .and_then(|message| message.get(key))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn event_timestamp_rfc3339(ts_ms: Option<i64>) -> String {
    ts_ms
        .and_then(|ts| Utc.timestamp_millis_opt(ts).single())
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

pub(crate) fn normalize_provider_surfaces_event(
    args: &RecipeEventArgs,
) -> Option<serde_json::Value> {
    if args.kind != "ingest" {
        return None;
    }

    let entity_id = payload_string(&args.payload, "entity_id")
        .or_else(|| payload_string(&args.payload, "threadId"))
        .or_else(|| payload_string(&args.payload, "chatId"))
        .or_else(|| payload_string(&args.payload, "snapshotKey"))
        .unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                args.provider,
                args.account_id,
                args.ts.unwrap_or_else(|| Utc::now().timestamp_millis())
            )
        });

    let thread_id = payload_string(&args.payload, "threadId")
        .or_else(|| payload_string(&args.payload, "chatId"))
        .or_else(|| payload_string(&args.payload, "conversationId"));
    let title = payload_string(&args.payload, "title")
        .or_else(|| payload_string(&args.payload, "chatName"))
        .or_else(|| payload_string(&args.payload, "channelName"));
    let snippet = payload_string(&args.payload, "snippet")
        .or_else(|| first_message_field(&args.payload, "body"));
    let sender_name = payload_string(&args.payload, "senderName")
        .or_else(|| first_message_field(&args.payload, "from"));
    let sender_handle = payload_string(&args.payload, "senderHandle");
    let deep_link = payload_string(&args.payload, "deepLink");
    let unread = payload_i64(&args.payload, "unread").unwrap_or(0);
    let requires_attention = payload_bool(&args.payload, "requires_attention")
        .unwrap_or(unread > 0 || sender_name.is_some() || snippet.is_some());

    Some(json!({
        "provider": args.provider,
        "account_id": args.account_id,
        "event_kind": args.kind,
        "entity_id": entity_id,
        "thread_id": thread_id,
        "title": title,
        "snippet": snippet,
        "sender_name": sender_name,
        "sender_handle": sender_handle,
        "timestamp": event_timestamp_rfc3339(args.ts),
        "deep_link": deep_link,
        "requires_attention": requires_attention,
        "raw_payload": args.payload,
    }))
}

pub(crate) async fn post_provider_surfaces_event(args: &RecipeEventArgs) -> Result<(), String> {
    let Some(params) = normalize_provider_surfaces_event(args) else {
        return Ok(());
    };

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "openhuman.provider_surfaces_ingest_event",
        "params": params,
    });

    let url = std::env::var("OPENHUMAN_CORE_RPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7788/rpc".to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("{status}: {body}"));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    if let Some(err) = v.get("error") {
        return Err(format!("rpc error: {err}"));
    }
    Ok(())
}
