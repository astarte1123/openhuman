// inbox.rs — Unified message inbox via Composio.
//
// Fetches inbound messages from connected services (Gmail, Slack, Discord …),
// stores them locally with AES-256-GCM-encrypted body, and supports outbound
// send via the matching Composio action.

use crate::{composio, db, encryption};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Service capability maps
// ---------------------------------------------------------------------------

fn fetch_action(service: &str) -> Option<&'static str> {
    match service.to_lowercase().as_str() {
        "gmail"                              => Some("GMAIL_FETCH_EMAILS"),
        "slack"                              => Some("SLACK_GET_MESSAGES"),
        "discord"                            => Some("DISCORD_FETCH_MESSAGES"),
        "hubspot"                            => Some("HUBSPOT_LIST_EMAILS"),
        "googlegmail" | "google_gmail"       => Some("GMAIL_FETCH_EMAILS"),
        _                                    => None,
    }
}

fn send_action(service: &str) -> Option<&'static str> {
    match service.to_lowercase().as_str() {
        "gmail"                        => Some("GMAIL_SEND_EMAIL"),
        "slack"                        => Some("SLACK_SENDS_A_MESSAGE_TO_A_SLACK_CHANNEL"),
        "discord"                      => Some("DISCORD_SEND_MESSAGE"),
        "hubspot"                      => Some("HUBSPOT_SEND_EMAIL"),
        _                              => None,
    }
}

/// Build the `input` JSON payload for a Composio send action.
fn build_send_input(service: &str, to: &str, subject: &str, body: &str) -> Value {
    match service.to_lowercase().as_str() {
        "gmail" => json!({
            "recipient_email": to,
            "subject": subject,
            "body": body,
        }),
        "slack" => json!({
            "channel": to,
            "text": body,
        }),
        "discord" => json!({
            "channel_id": to,
            "content": body,
        }),
        "hubspot" => json!({
            "to_email": to,
            "subject": subject,
            "html_body": body,
        }),
        _ => json!({ "to": to, "subject": subject, "body": body }),
    }
}

// ---------------------------------------------------------------------------
// Response normalisation
// ---------------------------------------------------------------------------

struct Msg {
    external_id: String,
    sender: String,
    subject: String,
    body: String,
    received_at: String,
    metadata: String,
}

fn str_field<'a>(obj: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(v) = obj.get(k).and_then(|v| v.as_str()) {
            if !v.is_empty() { return Some(v); }
        }
    }
    None
}

fn gmail_sender(msg: &Value) -> String {
    msg.get("payload")
        .and_then(|p| p.get("headers"))
        .and_then(|h| h.as_array())
        .and_then(|arr| {
            arr.iter().find(|h| h.get("name").and_then(|v| v.as_str()) == Some("From"))
               .and_then(|h| h.get("value").and_then(|v| v.as_str()))
               .map(String::from)
        })
        .unwrap_or_default()
}

fn gmail_subject(msg: &Value) -> String {
    msg.get("payload")
        .and_then(|p| p.get("headers"))
        .and_then(|h| h.as_array())
        .and_then(|arr| {
            arr.iter().find(|h| h.get("name").and_then(|v| v.as_str()) == Some("Subject"))
               .and_then(|h| h.get("value").and_then(|v| v.as_str()))
               .map(String::from)
        })
        .unwrap_or_default()
}

fn parse_messages(service: &str, result: &Value) -> Vec<Msg> {
    // Composio wraps results: { "data": { ... }, "successful": true }
    let data = result.get("data").unwrap_or(result);

    // Find the message list under common field names
    let arr = data.get("messages")
        .or_else(|| data.get("emails"))
        .or_else(|| data.get("items"))
        .or_else(|| data.get("results"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| {
            // Some services return the array at root level
            data.as_array().cloned().unwrap_or_default()
        });

    arr.iter().map(|msg| {
        let external_id = str_field(msg, &["id", "messageId", "message_id", "ts", "timestamp"])
            .map(String::from)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let (sender, subject, body) = match service.to_lowercase().as_str() {
            "gmail" => (
                gmail_sender(msg),
                gmail_subject(msg),
                str_field(msg, &["snippet", "body", "bodyText"])
                    .map(String::from)
                    .unwrap_or_default(),
            ),
            "slack" => (
                str_field(msg, &["user", "username", "user_profile.real_name"])
                    .map(String::from)
                    .unwrap_or_default(),
                str_field(msg, &["channel", "channel_name"]).map(String::from).unwrap_or_default(),
                str_field(msg, &["text"]).map(String::from).unwrap_or_default(),
            ),
            "discord" => (
                msg.get("author")
                    .and_then(|a| a.get("username"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default(),
                str_field(msg, &["channel_id"]).map(String::from).unwrap_or_default(),
                str_field(msg, &["content"]).map(String::from).unwrap_or_default(),
            ),
            _ => (
                str_field(msg, &["from", "sender", "user", "author"]).map(String::from).unwrap_or_default(),
                str_field(msg, &["subject", "title", "channel"]).map(String::from).unwrap_or_default(),
                str_field(msg, &["body", "content", "text", "snippet"]).map(String::from).unwrap_or_default(),
            ),
        };

        let received_at = str_field(msg, &["date", "internalDate", "ts", "timestamp", "created_at", "receivedAt"])
            .map(String::from)
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let metadata = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());
        let metadata_trunc: String = metadata.chars().take(1024).collect();

        Msg { external_id, sender, subject, body, received_at, metadata: metadata_trunc }
    }).collect()
}

// ---------------------------------------------------------------------------
// Fetch — pull inbound messages from a single connection
// ---------------------------------------------------------------------------

pub async fn fetch_for_connection(conn_id: &str, app_name: &str, entity_id: &str) {
    let client = match composio::load_client() {
        Some(c) => c,
        None => {
            eprintln!("[inbox] fetch_for_connection: no COMPOSIO_API_KEY");
            return;
        }
    };

    let action = match fetch_action(app_name) {
        Some(a) => a,
        None => {
            eprintln!("[inbox] fetch_for_connection: no fetch action for {app_name}");
            return;
        }
    };

    eprintln!("[inbox] fetch_for_connection conn={conn_id} service={app_name} action={action}");

    let result = match client.execute_action(action, entity_id, json!({})).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[inbox] fetch_for_connection error for {app_name}: {e}");
            return;
        }
    };

    let msgs = parse_messages(app_name, &result);
    eprintln!("[inbox] fetch_for_connection {app_name}: parsed {} messages", msgs.len());

    let key = encryption::key();
    let mut new_count = 0usize;

    for msg in &msgs {
        let id = uuid::Uuid::new_v4().to_string();
        let body_enc = encryption::encrypt_str(key, &msg.body);
        match db::inbox_message_upsert(
            &id,
            app_name,
            &msg.external_id,
            &msg.sender,
            &msg.subject,
            &body_enc,
            &msg.received_at,
            &msg.metadata,
        ) {
            Ok(true)  => new_count += 1,
            Ok(false) => {}
            Err(e)    => eprintln!("[inbox] upsert error: {e}"),
        }
    }

    eprintln!("[inbox] fetch_for_connection {app_name}: {new_count} new messages stored");
    let _ = db::composio_connection_set_fetched(conn_id);
}

/// Iterate all ACTIVE connections that have a matching fetch action and pull messages.
pub async fn fetch_all() {
    eprintln!("[inbox] fetch_all started");

    let active = match db::composio_active_connections() {
        Ok(v)  => v,
        Err(e) => { eprintln!("[inbox] fetch_all db error: {e}"); return; }
    };

    for conn in &active {
        let app_name  = match conn.get("app_name").and_then(|v| v.as_str()) { Some(s) => s, None => continue };
        let entity_id = conn.get("entity_id").and_then(|v| v.as_str()).unwrap_or("default");
        let conn_id   = conn.get("id").and_then(|v| v.as_str()).unwrap_or("");

        if fetch_action(app_name).is_some() {
            fetch_for_connection(conn_id, app_name, entity_id).await;
        }
    }

    eprintln!("[inbox] fetch_all done");
}

// ---------------------------------------------------------------------------
// Send outbound message
// ---------------------------------------------------------------------------

pub async fn send_message(
    service: &str,
    entity_id: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<Value, String> {
    let client = composio::load_client()
        .ok_or_else(|| "No COMPOSIO_API_KEY configured".to_string())?;

    let action = send_action(service)
        .ok_or_else(|| format!("[inbox] no send action for service: {service}"))?;

    eprintln!("[inbox] send_message service={service} action={action} to={to}");

    let input = build_send_input(service, to, subject, body);
    let result = client.execute_action(action, entity_id, input).await?;

    eprintln!("[inbox] send_message {service}: ok");
    Ok(result)
}

// ---------------------------------------------------------------------------
// RPC helpers
// ---------------------------------------------------------------------------

pub async fn rpc_list(params: &Value) -> Result<Value, String> {
    let service = params.get("service").and_then(|v| v.as_str());
    let limit  = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    let raw = db::inbox_messages_list(service, limit, offset)?;
    let key = encryption::key();

    // Decrypt body_enc → body in each message before returning to UI
    let result = raw.get("result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().map(|msg| {
                let mut m = msg.clone();
                if let Some(b64) = msg.get("body_enc").and_then(|v| v.as_str()) {
                    let body = encryption::decrypt_str(key, b64).unwrap_or_default();
                    m["body"] = json!(body);
                }
                m.as_object_mut().map(|o| o.remove("body_enc"));
                m
            }).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let unread = db::inbox_unread_count(service).unwrap_or(0);
    eprintln!("[inbox] rpc_list → {} messages, {} unread", result.len(), unread);
    Ok(json!({ "result": result, "unread": unread }))
}

pub async fn rpc_send(params: &Value) -> Result<Value, String> {
    let service   = params.get("service").and_then(|v| v.as_str())
        .ok_or("[inbox] rpc_send: missing service")?;
    let entity_id = params.get("entity_id").and_then(|v| v.as_str()).unwrap_or("default");
    let to        = params.get("to").and_then(|v| v.as_str())
        .ok_or("[inbox] rpc_send: missing to")?;
    let subject   = params.get("subject").and_then(|v| v.as_str()).unwrap_or("");
    let body      = params.get("body").and_then(|v| v.as_str())
        .ok_or("[inbox] rpc_send: missing body")?;

    send_message(service, entity_id, to, subject, body).await
        .map(|r| json!({ "ok": true, "result": r }))
}

pub fn rpc_mark_read(params: &Value) -> Result<Value, String> {
    let id = params.get("id").and_then(|v| v.as_str())
        .ok_or("[inbox] rpc_mark_read: missing id")?;
    db::inbox_message_mark_read(id)
}

pub async fn rpc_refresh() -> Result<Value, String> {
    fetch_all().await;
    let unread = db::inbox_unread_count(None).unwrap_or(0);
    Ok(json!({ "ok": true, "unread": unread }))
}
