// rpc/messages.rs — Message append, list, and agent_chat handlers.
// Relies on shared helpers in rpc/mod.rs (get_ai_reply_for_thread, get_history_window,
// build_system_prompt, build_rag_query) exposed as pub(super).

use crate::{ai, db, rpc};
use serde_json::{json, Value};

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "openhuman.threads_messages_list" => {
            let thread_id = params
                .get("thread_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::messages] threads_messages_list: missing thread_id".to_string())?;
            eprintln!("[rpc::messages] threads_messages_list thread_id={thread_id}");
            db::messages_list(thread_id).and_then(|v| rpc::ok(v))
        }

        "openhuman.threads_message_append" => {
            let msg = params
                .get("message")
                .ok_or_else(|| "[rpc::messages] threads_message_append: missing message".to_string())?;
            let thread_id = msg
                .get("thread_id")
                .and_then(|v| v.as_str())
                .or_else(|| params.get("thread_id").and_then(|v| v.as_str()))
                .ok_or_else(|| "[rpc::messages] threads_message_append: missing thread_id".to_string())?;
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::messages] threads_message_append: missing message.role".to_string())?;
            let content = msg
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::messages] threads_message_append: missing message.content".to_string())?;

            eprintln!("[rpc::messages] threads_message_append thread_id={thread_id} role={role}");

            let appended = db::message_append(thread_id, role, content)?;

            if role == "user" {
                if let Ok(1) = db::messages_count(thread_id) {
                    let title: String = content.chars().take(48).collect();
                    let _ = db::thread_set_title(thread_id, &title);
                }
            }

            if role == "user" {
                eprintln!("[rpc::messages] user message — triggering AI completion");
                let task_type = params.get("task_type").and_then(|v| v.as_str()).unwrap_or("default");
                let ai_reply = super::get_ai_reply_for_thread(thread_id, task_type).await;
                match ai_reply {
                    Ok(reply_text) => {
                        let assistant_msg = db::message_append(thread_id, "assistant", &reply_text)?;
                        let tid = thread_id.to_string();
                        tokio::spawn(async move { super::maybe_reflect_thread(&tid).await; });
                        return rpc::ok(json!({
                            "user_message": appended,
                            "assistant_message": assistant_msg,
                        }));
                    }
                    Err(e) => {
                        eprintln!("[rpc::messages] AI completion error: {e}");
                        let error_msg = db::message_append(
                            thread_id,
                            "assistant",
                            &format!("[Youma error] {e}"),
                        )?;
                        return rpc::ok(json!({
                            "user_message": appended,
                            "assistant_message": error_msg,
                        }));
                    }
                }
            }

            rpc::ok(appended)
        }

        "openhuman.agent_chat" => {
            let user_message = params
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "[rpc::messages] agent_chat: missing message".to_string())?;

            eprintln!("[rpc::messages] agent_chat message_len={}", user_message.len());

            let thread_id_opt = params.get("thread_id").and_then(|v| v.as_str());

            let (mut messages, ctx_raw) = if let Some(tid) = thread_id_opt {
                super::get_history_window(tid).await
            } else {
                (Vec::new(), Vec::new())
            };

            messages.push(json!({ "role": "user", "content": user_message }));

            let task_type = params.get("task_type").and_then(|v| v.as_str()).unwrap_or("default");
            let rag_query = super::build_rag_query(&ctx_raw, user_message);
            let system = super::build_system_prompt(&rag_query, &ctx_raw).await;
            let config = ai::load_config_for_task(task_type);
            let reply = ai::chat_completion(messages, &system, &config).await?;

            let effective_thread_id: Option<String> = if let Some(tid) = thread_id_opt {
                db::message_append(tid, "user", user_message)?;
                db::message_append(tid, "assistant", &reply)?;
                Some(tid.to_string())
            } else {
                None
            };

            rpc::ok(json!({
                "reply": reply,
                "thread_id": effective_thread_id,
            }))
        }

        _ => rpc::err_invalid(format!("[rpc::messages] unknown method: {method}")),
    }
}
