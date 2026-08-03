// rpc/license_handlers.rs — License activation, trial, and validation handlers.

use crate::{db, license, rpc};
use serde_json::{json, Value};

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "license.status" => {
            let status = license::local_status();
            tokio::spawn(async { license::validate_remote().await; });
            rpc::ok(status)
        }

        "license.activate" => {
            let key = params.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::license] license.activate: missing key".to_string())?;
            let server_url = params.get("server_url").and_then(|v| v.as_str());
            eprintln!("[rpc::license] license.activate key_len={}", key.len());
            rpc::ok(license::activate(key, server_url).await)
        }

        "license.start_trial" => {
            eprintln!("[rpc::license] license.start_trial");
            rpc::ok(license::start_trial())
        }

        "license.deactivate" => {
            eprintln!("[rpc::license] license.deactivate");
            rpc::ok(license::deactivate().await)
        }

        "license.server_url_set" => {
            let url = params.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::license] license.server_url_set: missing url".to_string())?;
            db::settings_set("license.server_url", url)?;
            eprintln!("[rpc::license] license.server_url_set url={url}");
            rpc::ok(json!({ "ok": true }))
        }

        "license.machine_id" => {
            rpc::ok(json!({ "machine_id": license::get_machine_id() }))
        }

        _ => Err(format!("[rpc::license] unknown method: {method}")),
    }
}
