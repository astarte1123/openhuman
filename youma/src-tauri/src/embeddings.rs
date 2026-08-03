// embeddings.rs — Vector embedding generation for semantic RAG search.
//
// Supported providers:
//   ollama   — local Ollama server (nomic-embed-text, mxbai-embed-large, etc.)
//   openai   — OpenAI Embeddings API (text-embedding-3-small, ada-002, etc.)
//   deepseek — DeepSeek Embeddings API (OpenAI-compatible)
//   <any>    — generic OpenAI-compatible endpoint via custom base_url

use reqwest::Client;
use serde_json::json;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

pub struct EmbeddingConfig {
    pub provider: String,
    pub model:    String,
    pub base_url: String,
    pub api_key:  String,
}

impl EmbeddingConfig {
    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && self.provider != "none"
    }
}

pub fn load_embedding_config() -> EmbeddingConfig {
    let get = |key: &str| -> String {
        crate::db::settings_get(key)
            .ok()
            .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
            .unwrap_or_default()
    };
    let provider = get("embedding_provider");
    let raw_model = get("embedding_model");
    let model = if raw_model.is_empty() { default_model(&provider) } else { raw_model };
    EmbeddingConfig {
        provider,
        model,
        base_url: get("embedding_base_url"),
        api_key:  get("embedding_api_key"),
    }
}

pub fn default_model(provider: &str) -> String {
    match provider {
        "ollama"   => "nomic-embed-text".to_string(),
        "openai"   => "text-embedding-3-small".to_string(),
        "deepseek" => "deepseek-embedding".to_string(),
        _          => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Generate an embedding vector for `text` using the configured provider.
/// Text is truncated to 8 000 chars to stay within typical token limits.
pub async fn generate_embedding(text: &str, config: &EmbeddingConfig) -> Result<Vec<f32>, String> {
    if !config.is_configured() {
        return Err("embedding provider not configured".to_string());
    }
    let text: String = text.chars().take(2_000).collect(); // chunks are ≤1k — 2k is a safety cap
    match config.provider.as_str() {
        "ollama" => embed_ollama(&text, config).await,
        _        => embed_openai_compat(&text, config).await,
    }
}

async fn embed_ollama(text: &str, config: &EmbeddingConfig) -> Result<Vec<f32>, String> {
    let base = if config.base_url.is_empty() {
        "http://localhost:11434".to_string()
    } else {
        config.base_url.trim_end_matches('/').to_string()
    };
    let resp = Client::new()
        .post(format!("{base}/api/embeddings"))
        .json(&json!({ "model": config.model, "prompt": text }))
        .send()
        .await
        .map_err(|e| format!("[embed] ollama request: {e}"))?;
    if !resp.status().is_success() {
        let s = resp.status();
        return Err(format!("[embed] ollama {s}: {}", resp.text().await.unwrap_or_default()));
    }
    let v: serde_json::Value = resp.json().await
        .map_err(|e| format!("[embed] ollama parse: {e}"))?;
    parse_array(v.get("embedding").ok_or("[embed] missing 'embedding' key in Ollama response")?)
}

async fn embed_openai_compat(text: &str, config: &EmbeddingConfig) -> Result<Vec<f32>, String> {
    let base = if !config.base_url.is_empty() {
        config.base_url.trim_end_matches('/').to_string()
    } else {
        match config.provider.as_str() {
            "deepseek" => "https://api.deepseek.com/v1".to_string(),
            _           => "https://api.openai.com/v1".to_string(),
        }
    };
    // Resolve API key: embedding-specific → env var for provider → active AI provider key.
    let key = if !config.api_key.is_empty() {
        config.api_key.clone()
    } else {
        let env_key = match config.provider.as_str() {
            "openai"   => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            "deepseek" => std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
            _          => String::new(),
        };
        if env_key.is_empty() {
            crate::provider::ProviderConfig::from_env().api_key
        } else {
            env_key
        }
    };
    let resp = Client::new()
        .post(format!("{base}/embeddings"))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {key}"))
        .json(&json!({ "model": config.model, "input": text, "encoding_format": "float" }))
        .send()
        .await
        .map_err(|e| format!("[embed] openai-compat request: {e}"))?;
    if !resp.status().is_success() {
        let s = resp.status();
        return Err(format!("[embed] openai-compat {s}: {}", resp.text().await.unwrap_or_default()));
    }
    let v: serde_json::Value = resp.json().await
        .map_err(|e| format!("[embed] openai parse: {e}"))?;
    let arr = v.get("data").and_then(|d| d.get(0)).and_then(|d| d.get("embedding"))
        .ok_or("[embed] missing data[0].embedding in OpenAI response")?;
    parse_array(arr)
}

fn parse_array(v: &serde_json::Value) -> Result<Vec<f32>, String> {
    v.as_array()
        .ok_or_else(|| "[embed] embedding field is not an array".to_string())?
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32)
            .ok_or_else(|| "[embed] non-numeric value in embedding array".to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32   = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { (dot / (mag_a * mag_b)).clamp(-1.0, 1.0) }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

pub fn serialize(v: &[f32]) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

pub fn deserialize(s: &str) -> Option<Vec<f32>> {
    serde_json::from_str(s).ok()
}
