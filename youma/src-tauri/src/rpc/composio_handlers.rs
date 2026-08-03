// rpc/composio_handlers.rs — Composio integrations and channels handlers.

use crate::{composio, db, rpc};
use serde_json::{json, Value};

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.composio_apps_list" => {
            let cached = db::composio_apps_cache_get()?;
            let items = cached.get("result").and_then(|r| r.as_array()).cloned().unwrap_or_default();
            if !items.is_empty() {
                eprintln!("[rpc::composio] composio_apps_list → {} cached", items.len());
                return rpc::ok(json!({"result": items}));
            }
            match composio::load_client() {
                None => rpc::ok(json!({"result": [], "error": "no_api_key"})),
                Some(client) => {
                    let apps = client.list_apps().await.unwrap_or_default();
                    let _ = db::composio_apps_cache_upsert(&apps);
                    eprintln!("[rpc::composio] composio_apps_list live → {}", apps.len());
                    rpc::ok(json!({"result": apps}))
                }
            }
        }

        "openhuman.composio_connections_list" => db::composio_connections_list(),

        "openhuman.composio_connect" => {
            let app_name = params.get("app_name").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::composio] composio_connect: missing app_name".to_string())?;
            let entity_id = params.get("entity_id").and_then(|v| v.as_str()).unwrap_or("default");

            let client = composio::load_client()
                .ok_or_else(|| "composio_connect: no COMPOSIO_API_KEY configured".to_string())?;

            let result = client.initiate_connection(app_name, entity_id, None).await?;

            let conn_id = result.get("connectedAccountId")
                .or_else(|| result.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let redirect_url = result.get("redirectUrl")
                .or_else(|| result.get("redirect_url"))
                .and_then(|v| v.as_str());

            let app_display = app_name.to_string();
            db::composio_connection_upsert(
                conn_id, app_name, &app_display, entity_id,
                "INITIATED", redirect_url,
            )?;

            if let Some(url) = redirect_url {
                eprintln!("[rpc::composio] composio_connect: opening browser → {url}");
                let _ = open::that(url);
            }

            eprintln!("[rpc::composio] composio_connect app={app_name} conn_id={conn_id}");
            rpc::ok(result)
        }

        "openhuman.composio_poll" => {
            let connection_id = params.get("connection_id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::composio] composio_poll: missing connection_id".to_string())?;

            let client = composio::load_client()
                .ok_or_else(|| "composio_poll: no COMPOSIO_API_KEY configured".to_string())?;

            let result = client.poll_connection_status(connection_id).await?;
            let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
            db::composio_connection_set_status(connection_id, status)?;

            eprintln!("[rpc::composio] composio_poll connection_id={connection_id} status={status}");
            rpc::ok(json!({"connection_id": connection_id, "status": status}))
        }

        "openhuman.composio_disconnect" => {
            let connection_id = params.get("connection_id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::composio] composio_disconnect: missing connection_id".to_string())?;

            if let Some(client) = composio::load_client() {
                if let Err(e) = client.delete_connection(connection_id).await {
                    eprintln!("[rpc::composio] composio_disconnect remote delete error: {e}");
                }
            }
            db::composio_connection_delete(connection_id)?;
            eprintln!("[rpc::composio] composio_disconnect connection_id={connection_id}");
            rpc::ok(json!({"ok": true}))
        }

        "openhuman.composio_execute" => {
            let action_id = params.get("action_id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::composio] composio_execute: missing action_id".to_string())?;
            let entity_id = params.get("entity_id").and_then(|v| v.as_str()).unwrap_or("default");
            let input = params.get("input").cloned().unwrap_or(json!({}));

            let client = composio::load_client()
                .ok_or_else(|| "composio_execute: no COMPOSIO_API_KEY configured".to_string())?;

            let result = client.execute_action(action_id, entity_id, input).await?;
            eprintln!("[rpc::composio] composio_execute action={action_id}");
            rpc::ok(result)
        }

        "openhuman.channels_list" => {
            let active = db::composio_active_connections().unwrap_or_default();
            let channels: Vec<serde_json::Value> = active.into_iter().filter(|c| {
                let name = c.get("app_name").and_then(|v| v.as_str()).unwrap_or("");
                matches!(name, "gmail" | "slack" | "discord" | "hubspot")
            }).collect();
            rpc::ok(json!({ "result": channels }))
        }

        _ => Err(format!("[rpc::composio] unknown method: {method}")),
    }
}
