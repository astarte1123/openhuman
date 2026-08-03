// rpc/system.rs — Health, config, auth, memories, tasks, skills, settings,
//                  invites, rewards, privacy, and embedding handlers.

use crate::{ai, db, knowledge, rpc};
use serde_json::{json, Value};

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        // ------------------------------------------------------------------
        // Health
        // ------------------------------------------------------------------
        "openhuman.health_snapshot" => rpc::ok(json!({
            "status": "ok",
            "uptime_secs": 0,
            "version": "0.1.0-youma",
        })),

        // ------------------------------------------------------------------
        // Config
        // ------------------------------------------------------------------
        "openhuman.config_get" => {
            let cfg = ai::load_config();
            rpc::ok(json!({
                "model": cfg.model,
                "provider": cfg.provider,
                "version": "0.1.0-youma",
            }))
        }

        // ------------------------------------------------------------------
        // Auth
        // ------------------------------------------------------------------
        "openhuman.auth_get_me" => rpc::ok(json!({
            "user": null,
            "authenticated": false,
        })),

        // ------------------------------------------------------------------
        // Memories
        // ------------------------------------------------------------------
        "openhuman.memories_list" => db::memories_list(),

        "openhuman.memories_create" => {
            let content = params.get("content").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] memories_create: missing content".to_string())?;
            let tags = params.get("tags").and_then(|v| v.as_str()).unwrap_or("");
            db::memories_create(content, tags)
        }

        "openhuman.memories_delete" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] memories_delete: missing id".to_string())?;
            db::memories_delete(id)
        }

        // ------------------------------------------------------------------
        // Tasks
        // ------------------------------------------------------------------
        "openhuman.tasks_list" => db::tasks_list(),

        "openhuman.tasks_create" => {
            let title = params.get("title").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] tasks_create: missing title".to_string())?;
            db::tasks_create(title)
        }

        "openhuman.tasks_update" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] tasks_update: missing id".to_string())?;
            let status = params.get("status").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] tasks_update: missing status".to_string())?;
            db::tasks_update(id, status)
        }

        "openhuman.tasks_delete" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] tasks_delete: missing id".to_string())?;
            db::tasks_delete(id)
        }

        // ------------------------------------------------------------------
        // Skills
        // ------------------------------------------------------------------
        "openhuman.skills_list" => db::skills_list(),

        "openhuman.skills_toggle" => {
            let id = params.get("id").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] skills_toggle: missing id".to_string())?;
            let enabled = params.get("enabled").and_then(|v| v.as_bool())
                .ok_or_else(|| "[rpc::system] skills_toggle: missing enabled".to_string())?;
            db::skills_toggle(id, enabled)
        }

        // ------------------------------------------------------------------
        // Env var helpers — read directly from process env (source of truth = youma.env)
        // ------------------------------------------------------------------
        "openhuman.env_get" => {
            // Returns the current string value of an env var. Non-sensitive vars only.
            let key = params.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] env_get: missing key".to_string())?;
            let value = std::env::var(key).unwrap_or_default();
            rpc::ok(json!({ "value": value }))
        }

        "openhuman.env_is_set" => {
            // Returns whether an env var is non-empty — safe for sensitive keys (no value exposed).
            let key = params.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] env_is_set: missing key".to_string())?;
            let set = !std::env::var(key).unwrap_or_default().is_empty();
            rpc::ok(json!({ "set": set }))
        }

        // ------------------------------------------------------------------
        // App settings
        // ------------------------------------------------------------------
        "openhuman.settings_get" => {
            let key = params.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] settings_get: missing key".to_string())?;
            db::settings_get(key)
        }

        "openhuman.settings_set" => {
            let key = params.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] settings_set: missing key".to_string())?;
            let value = params.get("value").and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::system] settings_set: missing value".to_string())?;
            db::settings_set(key, value)
        }

        // ------------------------------------------------------------------
        // Invites
        // ------------------------------------------------------------------
        "openhuman.invites_list" => {
            let res = db::settings_get("invite_codes")?;
            let codes_str = res.get("value").and_then(|v| v.as_str()).unwrap_or("[]");
            let codes: Value = serde_json::from_str(codes_str).unwrap_or(json!([]));
            rpc::ok(json!({"result": codes}))
        }

        "openhuman.invites_generate" => {
            let id_str = uuid::Uuid::new_v4().to_string().to_uppercase();
            let code = format!("YOUMA-{}", &id_str[..8]);
            let res = db::settings_get("invite_codes")?;
            let codes_str = res.get("value").and_then(|v| v.as_str()).unwrap_or("[]");
            let mut codes: Vec<Value> = serde_json::from_str(codes_str).unwrap_or_default();
            let created_at = chrono::Utc::now().to_rfc3339();
            codes.push(json!({"code": code, "created_at": created_at, "used": false}));
            let new_str = serde_json::to_string(&codes).unwrap_or_default();
            db::settings_set("invite_codes", &new_str)?;
            eprintln!("[rpc::system] invites_generate code={code}");
            rpc::ok(json!({"code": code, "created_at": created_at, "used": false}))
        }

        // ------------------------------------------------------------------
        // Rewards
        // ------------------------------------------------------------------
        "openhuman.rewards_get" => {
            let res = db::settings_get("referral_code")?;
            let referral_code = match res.get("value").and_then(|v| v.as_str()) {
                Some(v) if !v.is_empty() && v != "null" => v.to_string(),
                _ => {
                    let id_str = uuid::Uuid::new_v4().to_string().to_uppercase();
                    let code = format!("REF-{}", &id_str[..6]);
                    db::settings_set("referral_code", &code)?;
                    code
                }
            };
            let points_res = db::settings_get("reward_points")?;
            let points: i64 = points_res.get("value").and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()).unwrap_or(0);
            rpc::ok(json!({
                "referral_code": referral_code,
                "points": points,
                "referrals": 0,
                "tier": "Explorer",
            }))
        }

        // ------------------------------------------------------------------
        // Privacy — data reset
        // ------------------------------------------------------------------
        "openhuman.data_reset" => {
            eprintln!("[rpc::system] data_reset requested");
            db::data_reset()
        }

        // ------------------------------------------------------------------
        // Embedding configuration
        // ------------------------------------------------------------------
        "openhuman.embedding_config_get" => {
            eprintln!("[rpc::system] embedding_config_get");
            let get = |key: &str| -> String {
                db::settings_get(key).ok()
                    .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_default()
            };
            let provider = get("embedding_provider");
            let raw_model = get("embedding_model");
            let model = if raw_model.is_empty() {
                crate::embeddings::default_model(&provider)
            } else { raw_model };
            rpc::ok(json!({
                "provider": provider,
                "model": model,
                "base_url": get("embedding_base_url"),
            }))
        }

        "openhuman.embedding_config_set" => {
            eprintln!("[rpc::system] embedding_config_set");
            if let Some(p) = params.get("provider").and_then(|v| v.as_str()) {
                let _ = db::settings_set("embedding_provider", p);
            }
            if let Some(m) = params.get("model").and_then(|v| v.as_str()) {
                let _ = db::settings_set("embedding_model", m);
            }
            if let Some(u) = params.get("base_url").and_then(|v| v.as_str()) {
                let _ = db::settings_set("embedding_base_url", u);
            }
            if let Some(k) = params.get("api_key").and_then(|v| v.as_str()) {
                let _ = db::settings_set("embedding_api_key", k);
            }
            tauri::async_runtime::spawn(async { knowledge::embed_all_pending().await; });
            rpc::ok(json!({ "ok": true }))
        }

        "openhuman.embedding_test" => {
            eprintln!("[rpc::system] embedding_test");
            let config = crate::embeddings::load_embedding_config();
            if !config.is_configured() {
                return rpc::ok(json!({ "ok": false, "error": "no embedding provider configured" }));
            }
            match crate::embeddings::generate_embedding("embedding test", &config).await {
                Ok(emb) => rpc::ok(json!({
                    "ok": true,
                    "dims": emb.len(),
                    "provider": config.provider,
                    "model": config.model,
                    "hnsw_size": crate::vector_index::size(),
                })),
                Err(e) => rpc::ok(json!({ "ok": false, "error": e })),
            }
        }

        "openhuman.vector_index_status" => {
            eprintln!("[rpc::system] vector_index_status");
            rpc::ok(json!({
                "size": crate::vector_index::size(),
                "ready": crate::vector_index::is_ready(),
            }))
        }

        other => {
            eprintln!("[rpc::system] unknown method: {other}");
            Err(format!("unknown method: {other}"))
        }
    }
}
