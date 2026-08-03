// ai.rs — AI API client for Youma.
//
// Providers: Anthropic, OpenAI, DeepSeek, Custom (OpenAI-compatible), Ollama (local).
// Tool use: web_search (DuckDuckGo), web_fetch (HTTP scrape).
// Each provider runs an agentic loop: call → if tool_use → execute → continue.

use crate::provider::{ProviderConfig, ProviderKind};
use serde_json::{json, Value};
use std::time::Duration;

/// Runtime AI configuration derived from environment variables.
pub struct AiConfig {
    pub provider: String, // "anthropic" | "openai" | "deepseek" | "custom" | "ollama" | "none"
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

/// Resolve provider configuration from environment variables.
/// Provider selection is delegated to `ProviderConfig::from_env()` in provider.rs.
/// Only model selection (YOUMA_MODEL override, per-provider defaults) lives here.
pub fn load_config() -> AiConfig {
    let model_override = std::env::var("YOUMA_MODEL").ok();
    let pc = ProviderConfig::from_env();

    let model = model_override.unwrap_or_else(|| match pc.kind {
        ProviderKind::Custom => {
            std::env::var("CUSTOM_AI_MODEL").unwrap_or_else(|_| "default".to_string())
        }
        ProviderKind::Anthropic => "claude-sonnet-4-6".to_string(),
        ProviderKind::OpenAI => "gpt-4o".to_string(),
        ProviderKind::DeepSeek => "deepseek-chat".to_string(),
        ProviderKind::Ollama => {
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string())
        }
    });

    eprintln!("[ai] provider={} model={model}", pc.name());
    AiConfig {
        provider: pc.name().to_string(),
        api_key: pc.api_key,
        model,
        base_url: pc.base_url,
    }
}

/// Load AI configuration for a specific task type.
/// task: "default" | "reasoning" | "fast" | "vision"
/// Respects YOUMA_MODEL override — if set, always uses the override.
pub fn load_config_for_task(task: &str) -> AiConfig {
    let base = load_config();
    if std::env::var("YOUMA_MODEL").is_ok() {
        return base; // explicit override wins
    }
    let model = match (base.provider.as_str(), task) {
        ("anthropic", "reasoning") => "claude-opus-4-7".to_string(),
        ("anthropic", "fast")      => "claude-haiku-4-5-20251001".to_string(),
        ("openai",    "reasoning") => "o1-preview".to_string(),
        ("openai",    "fast")      => "gpt-4o-mini".to_string(),
        _ => return base,
    };
    eprintln!("[ai] task_routing task={task} model={model}");
    AiConfig { model, ..base }
}

/// Call the configured AI provider with a message list and return the reply text.
/// `system` is injected as the system prompt (memories, context, date).
/// The agentic loop: if the model calls a tool, execute it and continue.
/// If the primary provider is `custom` and it fails, automatically retries with
/// the next configured provider (anthropic → openai → deepseek → ollama).
pub async fn chat_completion(
    messages: Vec<Value>,
    system: &str,
    config: &AiConfig,
) -> Result<String, String> {
    eprintln!(
        "[ai] chat_completion provider={} model={} messages={} system_len={}",
        config.provider,
        config.model,
        messages.len(),
        system.len(),
    );

    let result = dispatch_chat(messages.clone(), system, config).await;

    if result.is_err() && config.provider == "custom" {
        let fb_pc = crate::provider::ProviderConfig::from_env_skip_custom();
        if fb_pc.is_configured() && fb_pc.kind != crate::provider::ProviderKind::Ollama
            || fb_pc.kind == crate::provider::ProviderKind::Ollama
        {
            let err_msg = result.as_ref().unwrap_err().clone();
            eprintln!(
                "[ai] custom provider failed ({err_msg}), falling back to {}",
                fb_pc.name()
            );
            let fb_model = match fb_pc.kind {
                crate::provider::ProviderKind::Anthropic => "claude-sonnet-4-6".to_string(),
                crate::provider::ProviderKind::OpenAI    => "gpt-4o".to_string(),
                crate::provider::ProviderKind::DeepSeek  => "deepseek-chat".to_string(),
                crate::provider::ProviderKind::Ollama    => {
                    std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string())
                }
                crate::provider::ProviderKind::Custom => unreachable!(),
            };
            let fb_config = AiConfig {
                provider: fb_pc.name().to_string(),
                api_key: fb_pc.api_key,
                model: fb_model,
                base_url: fb_pc.base_url,
            };
            return dispatch_chat(messages, system, &fb_config).await;
        }
    }

    result
}

async fn dispatch_chat(
    messages: Vec<Value>,
    system: &str,
    config: &AiConfig,
) -> Result<String, String> {
    match config.provider.as_str() {
        "anthropic" => chat_anthropic(messages, system, config).await,
        "openai"    => chat_openai(messages, system, config).await,
        "deepseek"  => chat_openai(messages, system, config).await,
        "custom"    => chat_openai(messages, system, config).await,
        "ollama"    => chat_ollama(messages, system, config).await,
        _ => Ok("[Youma] No AI provider configured. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, DEEPSEEK_API_KEY, CUSTOM_AI_URL, or start Ollama locally.".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn anthropic_tools() -> Value {
    let mut tools = vec![
        json!({
            "name": "web_search",
            "description": "Search the web using DuckDuckGo and return a list of results with titles, URLs and snippets.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The search query" }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "web_fetch",
            "description": "Fetch the content of a web page and return it as plain text (HTML stripped).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Full URL to fetch" }
                },
                "required": ["url"]
            }
        }),
    ];
    tools.extend(crate::tools::anthropic_tool_defs());
    json!(tools)
}

fn openai_tools() -> Value {
    let mut tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web using DuckDuckGo and return results.",
                "parameters": {
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "web_fetch",
                "description": "Fetch the content of a web page as plain text.",
                "parameters": {
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }
            }
        }),
    ];
    tools.extend(crate::tools::openai_tool_defs());
    json!(tools)
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

async fn execute_tool(name: &str, args: &Value) -> String {
    eprintln!("[ai] execute_tool name={name} args={args}");
    match name {
        "web_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            web_search(query).await.unwrap_or_else(|e| format!("Search error: {e}"))
        }
        "web_fetch" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            web_fetch(url).await.unwrap_or_else(|e| format!("Fetch error: {e}"))
        }
        other => crate::tools::execute(other, args).await,
    }
}

/// DuckDuckGo Lite search — no API key, returns top results as plain text.
pub async fn web_search(query: &str) -> Result<String, String> {
    eprintln!("[ai/search] query={query}");
    let client = build_client()?;
    let encoded = urlencoding_encode(query);
    let url = format!("https://lite.duckduckgo.com/lite/?q={encoded}");

    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; Youma/0.1)")
        .send()
        .await
        .map_err(|e| format!("search request: {e}"))?;

    let html = resp.text().await.map_err(|e| format!("search body: {e}"))?;

    // Parse <a class="result-link"> and sibling <td class="result-snippet">
    let results = extract_ddg_results(&html);
    if results.is_empty() {
        return Ok("No results found.".to_string());
    }
    Ok(results.join("\n\n"))
}

/// Fetch a URL and strip HTML to plain text (max 8 KB).
pub async fn web_fetch(url: &str) -> Result<String, String> {
    eprintln!("[ai/fetch] url={url}");
    let client = build_client()?;
    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; Youma/0.1)")
        .send()
        .await
        .map_err(|e| format!("fetch request: {e}"))?;

    let html = resp.text().await.map_err(|e| format!("fetch body: {e}"))?;
    let text = strip_html(&html);
    // Limit to ~8 000 chars to keep context manageable.
    let truncated: String = text.chars().take(8_000).collect();
    Ok(truncated)
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

async fn chat_anthropic(
    messages: Vec<Value>,
    system: &str,
    config: &AiConfig,
) -> Result<String, String> {
    let client = build_client()?;
    let mut msgs = messages;

    for _turn in 0..10 {
        let body = json!({
            "model": config.model,
            "max_tokens": 4096,
            "system": system,
            "messages": msgs,
            "tools": anthropic_tools(),
        });

        // Retry up to 3 times on 429 (rate limit) with backoff.
        let (status, text) = {
            let mut last_err = String::new();
            let mut result = None;
            for attempt in 0u32..3 {
                let resp = client
                    .post(format!("{}/v1/messages", config.base_url))
                    .header("x-api-key", &config.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("[ai] anthropic request: {e}"))?;

                let s = resp.status();
                if s.as_u16() == 429 {
                    // Respect retry-after header if present, else exponential backoff.
                    let wait_secs = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(5 * 2u64.pow(attempt)); // 5s, 10s, 20s
                    let wait_secs = wait_secs.min(60);
                    eprintln!("[ai] anthropic 429 — waiting {wait_secs}s (attempt {attempt})");
                    let t = resp.text().await.unwrap_or_default();
                    last_err = parse_api_error(429, &t, "Anthropic");
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    continue;
                }
                let t = resp.text().await.map_err(|e| format!("[ai] anthropic body: {e}"))?;
                result = Some((s, t));
                break;
            }
            result.ok_or(last_err)?
        };

        if !status.is_success() {
            return Err(parse_api_error(status.as_u16(), &text, "Anthropic"));
        }

        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("[ai] anthropic parse: {e} — {text}"))?;

        let stop_reason = v.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("");

        if stop_reason == "tool_use" {
            // Find tool_use blocks and execute them.
            let content_arr = v.get("content").and_then(|c| c.as_array()).cloned().unwrap_or_default();

            // Push the full assistant content turn.
            msgs.push(json!({ "role": "assistant", "content": content_arr }));

            let mut tool_results: Vec<Value> = Vec::new();
            for block in &content_arr {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let tool_id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let tool_input = block.get("input").cloned().unwrap_or(json!({}));
                    eprintln!("[ai] tool_use id={tool_id} name={tool_name}");
                    let result = execute_tool(tool_name, &tool_input).await;
                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result,
                    }));
                }
            }

            msgs.push(json!({ "role": "user", "content": tool_results }));
            continue; // next iteration
        }

        // end_turn or other — extract text.
        let reply = v
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .and_then(|b| b.get("text"))
                    .and_then(|t| t.as_str())
            })
            .ok_or_else(|| format!("[ai] no text block in anthropic response: {text}"))?
            .to_string();

        eprintln!("[ai] anthropic reply len={}", reply.len());
        return Ok(reply);
    }

    Err("[ai] anthropic: tool loop exceeded 10 iterations".to_string())
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Build the chat/completions URL tolerating base_url variants:
///   http://host:8080/v1/   → http://host:8080/v1/chat/completions
///   http://host:8080/v1    → http://host:8080/v1/chat/completions
///   http://host:8080       → http://host:8080/v1/chat/completions
fn openai_chat_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

async fn chat_openai(
    messages: Vec<Value>,
    system: &str,
    config: &AiConfig,
) -> Result<String, String> {
    let client = build_client()?;
    // Prepend system message; if first message is already system, replace it.
    let mut msgs = if messages.first().and_then(|m| m.get("role")).and_then(|r| r.as_str()) == Some("system") {
        messages
    } else {
        let mut v = vec![json!({"role": "system", "content": system})];
        v.extend(messages);
        v
    };

    // Modern llama.cpp-compatible servers (Qwen3, Llama-3.x, etc.) support tool calling in
    // OpenAI format. Enable for custom provider by default; set CUSTOM_AI_TOOLS=0 to disable
    // if your server rejects tool definitions (older LM Studio, etc.).
    let supports_tools = match config.provider.as_str() {
        "openai" | "deepseek" => true,
        "custom" => std::env::var("CUSTOM_AI_TOOLS").as_deref() != Ok("0"),
        _ => false,
    };

    // Qwen3 and other reasoning models on llama.cpp consume tokens for internal thinking
    // before producing a visible answer. A low token budget cuts off the response.
    // Custom provider gets a generous default; real OpenAI/DeepSeek don't need this.
    let custom_max_tokens: Option<u32> = if config.provider == "custom" {
        let v = std::env::var("CUSTOM_AI_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4096);
        Some(v)
    } else {
        None
    };

    if config.model == "default" {
        tracing::warn!("[ai] model is 'default' — set CUSTOM_AI_MODEL to the correct model name for your server");
    }

    // Qwen3 and other reasoning models emit <think>...</think> blocks before the answer.
    // Disable server-side when possible; always strip client-side as a safety net.
    let disable_thinking = config.provider == "custom"
        && std::env::var("CUSTOM_AI_THINKING").as_deref() != Ok("1");

    for _turn in 0..10 {
        let body = if supports_tools {
            // Include max_tokens for custom/reasoning providers even when tools are active.
            let mut b = json!({
                "model": config.model,
                "messages": msgs,
                "tools": openai_tools(),
                "tool_choice": "auto",
            });
            if let Some(max_tok) = custom_max_tokens {
                b["max_tokens"] = json!(max_tok);
            }
            if disable_thinking {
                b["enable_thinking"] = json!(false);
            }
            b
        } else if let Some(max_tok) = custom_max_tokens {
            let mut b = json!({
                "model": config.model,
                "messages": msgs,
                "max_tokens": max_tok,
            });
            if disable_thinking {
                b["enable_thinking"] = json!(false);
            }
            b
        } else {
            let mut b = json!({
                "model": config.model,
                "messages": msgs,
            });
            if disable_thinking {
                b["enable_thinking"] = json!(false);
            }
            b
        };

        let resp = client
            .post(openai_chat_url(&config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("[ai] openai request: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("[ai] openai body: {e}"))?;

        if !status.is_success() {
            let provider = match config.provider.as_str() {
                "deepseek" => "DeepSeek",
                "custom"   => "Custom provider",
                _           => "OpenAI",
            };
            return Err(parse_api_error(status.as_u16(), &text, provider));
        }

        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("[ai] openai parse: {e}"))?;

        let choice = v
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| format!("[ai] openai: no choices in {text}"))?;

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .unwrap_or("");

        if supports_tools && finish_reason == "tool_calls" {
            let message = choice.get("message").cloned().unwrap_or(json!({}));
            msgs.push(message.clone());

            let tool_calls = message
                .get("tool_calls")
                .and_then(|tc| tc.as_array())
                .cloned()
                .unwrap_or_default();

            for tc in &tool_calls {
                let tc_id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let fn_name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args_str = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                let result = execute_tool(fn_name, &args).await;
                msgs.push(json!({
                    "role": "tool",
                    "tool_call_id": tc_id,
                    "content": result,
                }));
            }
            continue;
        }

        let raw = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| format!("[ai] openai: no content in {text}"))?
            .to_string();

        let reply = if disable_thinking { strip_think_tags(&raw) } else { raw };
        eprintln!("[ai] openai reply len={}", reply.len());
        return Ok(reply);
    }

    Err("[ai] openai: tool loop exceeded 10 iterations".to_string())
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Strip Qwen3 / reasoning-model `<think>…</think>` blocks from a reply.
/// Some llama.cpp servers ignore `enable_thinking: false`; this catches any
/// thinking tokens that leak through regardless.
fn strip_think_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        match rest.find("<think>") {
            None => { out.push_str(rest); break; }
            Some(start) => {
                out.push_str(&rest[..start]);
                rest = &rest[start + 7..];
                match rest.find("</think>") {
                    None => break, // malformed — drop remainder
                    Some(end) => rest = &rest[end + 8..],
                }
            }
        }
    }
    out.trim_start_matches('\n').trim_start().to_string()
}

// ---------------------------------------------------------------------------
// Ollama (OpenAI-compatible /api/chat endpoint)
// ---------------------------------------------------------------------------

async fn chat_ollama(
    messages: Vec<Value>,
    system: &str,
    config: &AiConfig,
) -> Result<String, String> {
    let client = build_client()?;

    let msgs = if messages.first().and_then(|m| m.get("role")).and_then(|r| r.as_str()) == Some("system") {
        messages
    } else {
        let mut v = vec![json!({"role": "system", "content": system})];
        v.extend(messages);
        v
    };

    let body = json!({
        "model": config.model,
        "messages": msgs,
        "stream": false,
    });

    let resp = client
        .post(format!("{}/api/chat", config.base_url))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ai] ollama request: {e} — is Ollama running at {}?", config.base_url))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("[ai] ollama body: {e}"))?;

    if !status.is_success() {
        return Err(parse_api_error(status.as_u16(), &text, "Ollama"));
    }

    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("[ai] ollama parse: {e}"))?;

    let reply = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| format!("[ai] ollama: unexpected shape: {text}"))?
        .to_string();

    eprintln!("[ai] ollama reply len={}", reply.len());
    Ok(reply)
}

// ---------------------------------------------------------------------------
// OpenAI TTS
// ---------------------------------------------------------------------------

/// Synthesize speech via OpenAI TTS API.
/// Returns raw MP3 bytes. Voice: nova (natural female) by default.
pub async fn text_to_speech(text: &str, api_key: &str) -> Result<Vec<u8>, String> {
    eprintln!("[ai/tts] synthesizing {} chars", text.len());
    let client = build_client()?;

    let body = serde_json::json!({
        "model": "tts-1",
        "voice": "nova",
        "input": text,
        "response_format": "mp3",
    });

    let resp = client
        .post("https://api.openai.com/v1/audio/speech")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ai/tts] request: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let _text = resp.text().await.unwrap_or_default();
        return Err(match status.as_u16() {
            429 => "TTS rate limit — wait a moment.".to_string(),
            401 => "Invalid OpenAI API key.".to_string(),
            _ => format!("TTS error {status}"),
        });
    }

    let bytes = resp.bytes().await.map_err(|e| format!("[ai/tts] body: {e}"))?;
    eprintln!("[ai/tts] got {} bytes", bytes.len());
    Ok(bytes.to_vec())
}

// ---------------------------------------------------------------------------
// ElevenLabs TTS
// ---------------------------------------------------------------------------

/// Synthesize speech via ElevenLabs TTS API.
/// Returns raw MP3 bytes. Defaults to turbo v2.5 model for low latency.
pub async fn text_to_speech_elevenlabs(
    text: &str,
    voice_id: &str,
    api_key: &str,
) -> Result<Vec<u8>, String> {
    eprintln!("[ai/elevenlabs] synthesizing {} chars voice={voice_id}", text.len());
    let client = build_client()?;
    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}");
    let body = serde_json::json!({
        "text": text,
        "model_id": "eleven_turbo_v2_5",
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75
        }
    });
    let resp = client
        .post(&url)
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ai/elevenlabs] request: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let msg = resp.text().await.unwrap_or_default();
        return Err(match status.as_u16() {
            401 => "Invalid ElevenLabs API key.".to_string(),
            422 => format!("ElevenLabs invalid request: {msg}"),
            429 => "ElevenLabs rate limit — wait a moment.".to_string(),
            _ => format!("ElevenLabs error {status}: {msg}"),
        });
    }
    let bytes = resp.bytes().await.map_err(|e| format!("[ai/elevenlabs] body: {e}"))?;
    eprintln!("[ai/elevenlabs] got {} bytes", bytes.len());
    Ok(bytes.to_vec())
}

// ---------------------------------------------------------------------------
// Whisper STT
// ---------------------------------------------------------------------------

/// Transcribe audio bytes via OpenAI Whisper API.
/// Requires an OpenAI API key. `format` is the audio container (e.g. "webm", "mp4").
pub async fn transcribe_audio_whisper(
    audio_bytes: Vec<u8>,
    format: &str,
    api_key: &str,
) -> Result<String, String> {
    eprintln!("[ai/whisper] transcribing {} bytes format={format}", audio_bytes.len());

    let client = build_client()?;

    let mime = match format {
        "mp4" | "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        _ => "audio/webm",
    };

    let filename = format!("audio.{format}");

    let part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name(filename)
        .mime_str(mime)
        .map_err(|e| format!("[ai/whisper] mime: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1");

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("[ai/whisper] request: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("[ai/whisper] body: {e}"))?;

    if !status.is_success() {
        return Err(match status.as_u16() {
            429 => "Rate limit — wait a moment and try again.".to_string(),
            401 => "Invalid OpenAI API key. Check Settings.".to_string(),
            _ => format!("Transcription error {status}"),
        });
    }

    let v: Value = serde_json::from_str(&text).map_err(|e| format!("[ai/whisper] parse: {e}"))?;
    let transcript = v
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    eprintln!("[ai/whisper] transcript len={}", transcript.len());
    Ok(transcript)
}

// ---------------------------------------------------------------------------
// Whisper STT — verbose_json (timed segments)
// ---------------------------------------------------------------------------

/// Transcribe audio via OpenAI Whisper with `response_format=verbose_json`.
/// Returns a list of timed segments: each has `start`, `end`, and `text`.
pub async fn transcribe_audio_whisper_timed(
    audio_bytes: Vec<u8>,
    format: &str,
    api_key: &str,
) -> Result<Vec<crate::meetings::diarize::TimedTranscriptSegment>, String> {
    eprintln!(
        "[ai/whisper-timed] transcribing {} bytes format={format}",
        audio_bytes.len()
    );

    let client = build_client()?;

    let mime = match format {
        "mp4" | "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        _ => "audio/webm",
    };

    let filename = format!("audio.{format}");

    let part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name(filename)
        .mime_str(mime)
        .map_err(|e| format!("[ai/whisper-timed] mime: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("response_format", "verbose_json");

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("[ai/whisper-timed] request: {e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("[ai/whisper-timed] body: {e}"))?;

    if !status.is_success() {
        return Err(match status.as_u16() {
            429 => "Rate limit — wait a moment and try again.".to_string(),
            401 => "Invalid OpenAI API key. Check Settings.".to_string(),
            _ => format!("Transcription error {status}"),
        });
    }

    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("[ai/whisper-timed] parse: {e}"))?;

    // verbose_json response: { "segments": [ { "start", "end", "text" }, … ] }
    let segments = v
        .get("segments")
        .and_then(|s| s.as_array())
        .ok_or_else(|| {
            format!("[ai/whisper-timed] no segments in response: {}", &text[..text.len().min(200)])
        })?;

    let timed: Vec<crate::meetings::diarize::TimedTranscriptSegment> = segments
        .iter()
        .filter_map(|seg| {
            let start = seg.get("start").and_then(|v| v.as_f64())?;
            let end = seg.get("end").and_then(|v| v.as_f64())?;
            let text = seg.get("text").and_then(|v| v.as_str())?.to_string();
            Some(crate::meetings::diarize::TimedTranscriptSegment { start, end, text })
        })
        .collect();

    eprintln!("[ai/whisper-timed] parsed {} timed segments", timed.len());
    Ok(timed)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a failed API response into a clean, user-readable error string.
/// Extracts the human message from provider JSON; maps common HTTP codes.
fn parse_api_error(status: u16, body: &str, provider: &str) -> String {
    eprintln!("[ai] {provider} error {status}: {body}");

    // Try to extract a human-readable message from the JSON body.
    let api_msg: Option<String> = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        // Anthropic: {"error": {"message": "..."}}
        v.pointer("/error/message")
            // OpenAI / DeepSeek / custom: {"error": {"message": "..."}}
            .or_else(|| v.pointer("/error/message"))
            // Ollama: {"error": "..."}
            .or_else(|| v.get("error"))
            .and_then(|m| m.as_str())
            .map(|s| {
                // Truncate long provider messages — keep first sentence only.
                let first = s.split(". ").next().unwrap_or(s);
                first.chars().take(200).collect()
            })
    });

    match status {
        429 => {
            let detail = api_msg.unwrap_or_else(|| "too many requests".to_string());
            format!("Rate limit reached ({provider}): {detail}. Wait a moment and try again.")
        }
        401 => format!("Invalid API key for {provider}. Check Settings → API Keys."),
        403 => format!("Access denied by {provider}. Check your account or billing."),
        500 | 502 | 503 | 504 => {
            format!("{provider} server error ({status}). Try again in a moment.")
        }
        _ => {
            if let Some(msg) = api_msg {
                format!("{provider} error ({status}): {msg}")
            } else {
                format!("{provider} error ({status}).")
            }
        }
    }
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300)) // 5 min — local models on large contexts are slow
        .build()
        .map_err(|e| format!("[ai] build client: {e}"))
}

fn urlencoding_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => '+'.to_string(),
            other => {
                let mut buf = [0u8; 4];
                let bytes = other.encode_utf8(&mut buf);
                bytes.bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

/// Extract search results from DuckDuckGo Lite HTML.
fn extract_ddg_results(html: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    let bytes = html.as_bytes();

    while pos < bytes.len() && results.len() < 8 {
        // Look for result links
        if let Some(a_start) = find_str(html, "result-link", pos) {
            if let Some(href_start) = find_str(html, "href=\"", a_start) {
                let href_content_start = href_start + 6;
                if let Some(href_end) = find_str(html, "\"", href_content_start) {
                    let href = &html[href_content_start..href_end];
                    // Extract anchor text
                    if let Some(text_start) = find_str(html, ">", href_end) {
                        if let Some(text_end) = find_str(html, "</a>", text_start + 1) {
                            let title = strip_html(&html[text_start + 1..text_end]).trim().to_string();
                            // Look for snippet after this result
                            let snippet = if let Some(snip_start) = find_str(html, "result-snippet", href_end) {
                                if let Some(td_end) = find_str(html, ">", snip_start) {
                                    if let Some(td_close) = find_str(html, "</td>", td_end + 1) {
                                        strip_html(&html[td_end + 1..td_close]).trim().to_string()
                                    } else { String::new() }
                                } else { String::new() }
                            } else { String::new() };

                            if !title.is_empty() && href.starts_with("http") {
                                results.push(format!("**{title}**\n{href}\n{snippet}"));
                            }
                            pos = text_end + 4;
                            continue;
                        }
                    }
                    pos = href_end + 1;
                    continue;
                }
            }
            pos = a_start + 11;
        } else {
            break;
        }
    }
    results
}

fn find_str(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack[from..].find(needle).map(|i| from + i)
}

/// Strip HTML tags and decode basic entities.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode common entities
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Unit tests — ai.rs
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-var-mutating tests to avoid race conditions with parallel
    // test threads that also read/write the same env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_ai_env() {
        for key in &[
            "CUSTOM_AI_URL",
            "CUSTOM_AI_KEY",
            "CUSTOM_AI_MODEL",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "OLLAMA_URL",
            "OLLAMA_MODEL",
            "YOUMA_MODEL",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn custom_url_wins_over_all_keyed_providers() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("CUSTOM_AI_URL", "http://10.20.10.24:8080/v1/");
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test-key");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-test-key");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(
            cfg.provider, "custom",
            "CUSTOM_AI_URL should override all keyed providers, got provider={}",
            cfg.provider
        );
    }

    #[test]
    fn custom_url_without_key_still_wins() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("CUSTOM_AI_URL", "http://localhost:8080/v1/");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-key");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(
            cfg.provider, "custom",
            "CUSTOM_AI_URL without key should still win, got {}",
            cfg.provider
        );
    }

    #[test]
    fn custom_model_name_used_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("CUSTOM_AI_URL", "http://localhost:8080");
        std::env::set_var("CUSTOM_AI_MODEL", "Qwen3.6-35B-A3B-UD-Q4_K_XL");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(
            cfg.model, "Qwen3.6-35B-A3B-UD-Q4_K_XL",
            "custom model should be used, got model={}",
            cfg.model
        );
    }

    #[test]
    fn anthropic_selected_when_no_custom_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-real-key");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(cfg.provider, "anthropic", "got {}", cfg.provider);
    }

    #[test]
    fn ollama_is_fallback_when_nothing_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(
            cfg.provider, "ollama",
            "ollama should be fallback, got {}",
            cfg.provider
        );
    }

    #[test]
    fn youma_model_override_wins_over_custom_model() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("CUSTOM_AI_URL", "http://localhost:8080");
        std::env::set_var("CUSTOM_AI_MODEL", "model-a");
        std::env::set_var("YOUMA_MODEL", "model-override");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(
            cfg.model, "model-override",
            "YOUMA_MODEL should override CUSTOM_AI_MODEL, got {}",
            cfg.model
        );
    }

    #[test]
    fn openai_selected_over_deepseek() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("OPENAI_API_KEY", "sk-openai-abc");
        std::env::set_var("DEEPSEEK_API_KEY", "ds-key");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(cfg.provider, "openai", "openai should beat deepseek, got {}", cfg.provider);
    }

    #[test]
    fn deepseek_selected_when_only_deepseek_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("DEEPSEEK_API_KEY", "ds-key-xyz");
        let cfg = load_config();
        clear_ai_env();
        assert_eq!(cfg.provider, "deepseek", "got {}", cfg.provider);
    }

    #[test]
    fn anthropic_default_model_is_sonnet() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_ai_env();
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-key");
        let cfg = load_config();
        clear_ai_env();
        assert!(
            cfg.model.contains("claude"),
            "anthropic default model should contain 'claude', got {}",
            cfg.model
        );
    }

    #[test]
    fn strip_html_basic() {
        let result = strip_html("<b>Hello</b> &amp; world");
        assert_eq!(result, "Hello & world");
    }

    #[test]
    fn strip_html_empty() {
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn strip_html_no_tags() {
        assert_eq!(strip_html("plain text"), "plain text");
    }

    #[test]
    fn urlencoding_encode_space() {
        let encoded = urlencoding_encode("hello world");
        assert_eq!(encoded, "hello+world");
    }

    #[test]
    fn urlencoding_encode_special_chars() {
        let encoded = urlencoding_encode("a=b&c");
        // '=' and '&' should be percent-encoded
        assert!(!encoded.contains('='), "= should be percent-encoded");
        assert!(!encoded.contains('&'), "& should be percent-encoded");
    }

    #[test]
    fn urlencoding_encode_safe_chars_unchanged() {
        let encoded = urlencoding_encode("hello-world.test~ok_123");
        assert_eq!(encoded, "hello-world.test~ok_123");
    }

    #[test]
    fn openai_chat_url_with_trailing_v1_slash() {
        // The user's actual setup: CUSTOM_AI_URL = http://10.20.10.24:8080/v1/
        assert_eq!(
            openai_chat_url("http://10.20.10.24:8080/v1/"),
            "http://10.20.10.24:8080/v1/chat/completions"
        );
    }

    #[test]
    fn openai_chat_url_with_v1_no_slash() {
        assert_eq!(
            openai_chat_url("http://host:8080/v1"),
            "http://host:8080/v1/chat/completions"
        );
    }

    #[test]
    fn openai_chat_url_without_v1() {
        // Plain base URL — appends /v1/chat/completions
        assert_eq!(
            openai_chat_url("http://host:8080"),
            "http://host:8080/v1/chat/completions"
        );
    }

    #[test]
    fn openai_chat_url_no_double_v1() {
        // Must NOT produce /v1/v1/chat/completions
        let url = openai_chat_url("http://host:8080/v1/");
        assert!(!url.contains("/v1/v1"), "double /v1 must not appear: {url}");
    }
}
