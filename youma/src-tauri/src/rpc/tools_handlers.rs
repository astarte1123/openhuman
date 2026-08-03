// rpc/tools_handlers.rs — Filesystem and shell tool handlers.

use crate::tools;
use serde_json::Value;

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.tools_fs_read" => {
            eprintln!("[rpc::tools] tools_fs_read");
            tools::rpc_fs_read(params)
        }

        "openhuman.tools_fs_write" => {
            eprintln!("[rpc::tools] tools_fs_write");
            tools::rpc_fs_write(params)
        }

        "openhuman.tools_fs_list" => {
            eprintln!("[rpc::tools] tools_fs_list");
            tools::rpc_fs_list(params)
        }

        "openhuman.tools_fs_grep" => {
            eprintln!("[rpc::tools] tools_fs_grep");
            tools::rpc_fs_grep(params)
        }

        "openhuman.tools_shell_exec" => {
            eprintln!("[rpc::tools] tools_shell_exec");
            tools::rpc_shell_exec(params).await
        }

        _ => Err(format!("[rpc::tools] unknown method: {method}")),
    }
}
