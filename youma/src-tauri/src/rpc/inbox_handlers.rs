// rpc/inbox_handlers.rs — Unified inbox handlers across Composio-connected services.

use crate::inbox;
use serde_json::Value;

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.inbox_list"      => inbox::rpc_list(params).await,
        "openhuman.inbox_send"      => inbox::rpc_send(params).await,
        "openhuman.inbox_mark_read" => inbox::rpc_mark_read(params),
        "openhuman.inbox_refresh"   => inbox::rpc_refresh().await,
        _ => Err(format!("[rpc::inbox] unknown method: {method}")),
    }
}
