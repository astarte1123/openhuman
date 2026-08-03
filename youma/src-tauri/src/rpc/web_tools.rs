// rpc/web_tools.rs — Web search and fetch handlers.

use crate::{ai, rpc};
use serde_json::{json, Value};

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.web_search" => {
            let query = params
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::web_tools] web_search: missing query".to_string())?;
            let results = ai::web_search(query).await?;
            rpc::ok(json!({ "result": results }))
        }

        "openhuman.web_fetch" => {
            let url = params
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::web_tools] web_fetch: missing url".to_string())?;
            let content = ai::web_fetch(url).await?;
            rpc::ok(json!({ "result": content }))
        }

        _ => rpc::err_invalid(format!("[rpc::web_tools] unknown method: {method}")),
    }
}
