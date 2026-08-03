// rpc/knowledge_handlers.rs — Knowledge base, vault sync, and memory handlers.

use crate::{db, knowledge, rpc};
use serde_json::{json, Value};

pub fn handle_sync(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.knowledge_chunks_graph" => {
            let per_topic = params.get("per_topic").and_then(|v| v.as_u64()).unwrap_or(40) as usize;
            eprintln!("[rpc::knowledge] knowledge_chunks_graph per_topic={per_topic}");
            db::knowledge_chunks_graph(per_topic)
        }

        "openhuman.knowledge_chunk_get" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or("[rpc::knowledge] knowledge_chunk_get: missing id")?;
            db::knowledge_chunk_get(id).and_then(|c| rpc::ok(json!({ "result": c })))
        }

        "openhuman.knowledge_chunks_list" => {
            let topic = params.get("topic").and_then(|v| v.as_str());
            let limit  = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            eprintln!("[rpc::knowledge] knowledge_chunks_list topic={topic:?} limit={limit} offset={offset}");
            db::knowledge_chunks_list(topic, limit, offset)
        }

        "openhuman.knowledge_tree_nodes" => {
            eprintln!("[rpc::knowledge] knowledge_tree_nodes");
            db::knowledge_tree_nodes_list()
        }

        "openhuman.knowledge_vault_sync" => {
            let path_res = db::settings_get("knowledge_vault_path")?;
            let custom = path_res.get("value").and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "null");
            let vault_dir: String = if let Some(p) = custom {
                p.to_string()
            } else {
                format!("{}/obsidian-vault", crate::data_dir())
            };
            eprintln!("[rpc::knowledge] knowledge_vault_sync vault_dir={vault_dir}");
            let count = knowledge::sync_vault(&vault_dir)?;
            rpc::ok(json!({"ok": true, "files_written": count, "vault_dir": vault_dir}))
        }

        "openhuman.knowledge_vault_path_get" => {
            let path_res = db::settings_get("knowledge_vault_path")?;
            let stored = path_res.get("value").and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "null");
            let path = stored.map(String::from)
                .unwrap_or_else(|| format!("{}/obsidian-vault", crate::data_dir()));
            eprintln!("[rpc::knowledge] knowledge_vault_path_get → {path}");
            rpc::ok(json!({"path": path}))
        }

        "openhuman.knowledge_vault_path_set" => {
            let path = params.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::knowledge] knowledge_vault_path_set: missing path".to_string())?;
            db::settings_set("knowledge_vault_path", path)?;
            eprintln!("[rpc::knowledge] knowledge_vault_path_set → {path}");
            rpc::ok(json!({"ok": true}))
        }

        "openhuman.knowledge_ingest_text" => {
            let title = params.get("title").and_then(|v| v.as_str())
                .ok_or("[rpc::knowledge] knowledge_ingest_text: missing title")?;
            let content = params.get("content").and_then(|v| v.as_str())
                .ok_or("[rpc::knowledge] knowledge_ingest_text: missing content")?;
            let tags = params.get("tags").and_then(|v| v.as_str()).unwrap_or("");
            let source_id = params.get("source_id").and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let topic_hint = if tags.is_empty() { Some(title) } else { None };
            let chunks = knowledge::ingest_document(title, content, tags, &source_id, topic_hint)?;
            eprintln!("[rpc::knowledge] knowledge_ingest_text: {chunks} chunks for '{title}'");
            rpc::ok(json!({ "chunks": chunks, "source_id": source_id }))
        }

        "openhuman.knowledge_sources_list" => db::knowledge_manual_sources(),

        "openhuman.knowledge_source_delete" => {
            let source_id = params.get("source_id").and_then(|v| v.as_str())
                .ok_or("[rpc::knowledge] knowledge_source_delete: missing source_id")?;
            db::knowledge_source_delete(source_id)
        }

        "openhuman.knowledge_chunk_delete" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::knowledge] knowledge_chunk_delete: missing id".to_string())?;
            eprintln!("[rpc::knowledge] knowledge_chunk_delete id={id}");
            db::knowledge_chunk_delete(id)
        }

        _ => Err(format!("[rpc::knowledge] unknown sync method: {method}")),
    }
}

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.knowledge_rebuild" => {
            eprintln!("[rpc::knowledge] knowledge_rebuild — spawning background task");
            tauri::async_runtime::spawn(async {
                knowledge::ingest_all().await;
                knowledge::build_tree().await;
                eprintln!("[rpc::knowledge] knowledge_rebuild background task done");
            });
            rpc::ok(json!({"ok": true, "message": "Knowledge rebuild started in background."}))
        }

        "openhuman.knowledge_ingest_file" => {
            let path = params.get("path").and_then(|v| v.as_str())
                .ok_or("[rpc::knowledge] knowledge_ingest_file: missing path")?;
            let tags = params.get("tags").and_then(|v| v.as_str()).unwrap_or("");
            let chunks = knowledge::ingest_file(path, tags)?;
            eprintln!("[rpc::knowledge] knowledge_ingest_file: {chunks} chunks for path={path}");
            let path_owned = path.to_string();
            tokio::spawn(async move {
                if std::path::Path::new(&path_owned).is_dir() {
                    knowledge::embed_all_pending().await;
                } else {
                    knowledge::embed_source_chunks(&path_owned).await;
                }
            });
            rpc::ok(json!({ "chunks": chunks, "source_id": path }))
        }

        // Delegate sync methods
        other => handle_sync(other, params),
    }
}
