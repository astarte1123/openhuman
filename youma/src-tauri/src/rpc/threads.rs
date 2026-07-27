// rpc/threads.rs — Thread lifecycle handlers (create, list, delete, rename).

use crate::{db, rpc};
use serde_json::{json, Value};

pub fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.threads_list" => {
            db::threads_list().and_then(|v| rpc::ok(v))
        }

        "openhuman.threads_create_new" => {
            let labels: Vec<String> = params
                .get("labels")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            eprintln!("[rpc::threads] threads_create_new labels={labels:?}");
            db::threads_create(labels).and_then(|v| rpc::ok(v))
        }

        "openhuman.threads_delete" => {
            let thread_id = match params.get("thread_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return rpc::err_invalid("[rpc::threads] threads_delete: missing thread_id"),
            };
            let deleted_at = params
                .get("deleted_at")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| "now");
            let ts_owned;
            let ts = if deleted_at == "now" {
                ts_owned = chrono::Utc::now().to_rfc3339();
                ts_owned.as_str()
            } else {
                deleted_at
            };
            eprintln!("[rpc::threads] threads_delete thread_id={thread_id}");
            db::threads_delete(thread_id, ts)
        }

        "openhuman.threads_rename" => {
            let thread_id = match params.get("thread_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return rpc::err_invalid("[rpc::threads] threads_rename: missing thread_id"),
            };
            let title = match params.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return rpc::err_invalid("[rpc::threads] threads_rename: missing title"),
            };
            eprintln!("[rpc::threads] threads_rename thread_id={thread_id}");
            db::thread_set_title(thread_id, title)?;
            rpc::ok(json!({ "ok": true }))
        }

        _ => Err(format!("[rpc::threads] unknown method: {method}")),
    }
}
