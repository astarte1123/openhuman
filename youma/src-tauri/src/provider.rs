//! Shared provider resolution for AI completions and embeddings.
//! Both backends use the same priority: CUSTOM_AI_URL > ANTHROPIC > OPENAI > DEEPSEEK > OLLAMA.

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderKind {
    Custom,
    Anthropic,
    OpenAI,
    DeepSeek,
    Ollama,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub api_key: String,
    pub base_url: String,
}

impl ProviderConfig {
    /// Resolve the active AI provider from environment variables.
    /// Priority: CUSTOM_AI_URL (if set) > ANTHROPIC_API_KEY > OPENAI_API_KEY > DEEPSEEK_API_KEY > OLLAMA_URL
    pub fn from_env() -> Self {
        let custom_url = std::env::var("CUSTOM_AI_URL").unwrap_or_default();
        let custom_key = std::env::var("CUSTOM_AI_KEY").unwrap_or_default();

        if !custom_url.is_empty() {
            tracing::debug!("[provider] resolved=custom url={custom_url}");
            return Self {
                kind: ProviderKind::Custom,
                api_key: custom_key,
                base_url: custom_url,
            };
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] resolved=anthropic");
                return Self {
                    kind: ProviderKind::Anthropic,
                    api_key: key,
                    base_url: "https://api.anthropic.com".into(),
                };
            }
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] resolved=openai");
                return Self {
                    kind: ProviderKind::OpenAI,
                    api_key: key,
                    base_url: "https://api.openai.com".into(),
                };
            }
        }
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] resolved=deepseek");
                return Self {
                    kind: ProviderKind::DeepSeek,
                    api_key: key,
                    base_url: "https://api.deepseek.com".into(),
                };
            }
        }
        let ollama_url = std::env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".into());
        let ollama_model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "llama3.2".into());
        tracing::debug!("[provider] resolved=ollama url={ollama_url} model={ollama_model}");
        Self {
            kind: ProviderKind::Ollama,
            api_key: String::new(),
            base_url: ollama_url,
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            ProviderKind::Custom => "custom",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenAI => "openai",
            ProviderKind::DeepSeek => "deepseek",
            ProviderKind::Ollama => "ollama",
        }
    }

    /// Resolve the next available provider, skipping Custom.
    /// Used for fallback when the custom endpoint is unreachable or errors.
    pub fn from_env_skip_custom() -> Self {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] fallback resolved=anthropic");
                return Self {
                    kind: ProviderKind::Anthropic,
                    api_key: key,
                    base_url: "https://api.anthropic.com".into(),
                };
            }
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] fallback resolved=openai");
                return Self {
                    kind: ProviderKind::OpenAI,
                    api_key: key,
                    base_url: "https://api.openai.com".into(),
                };
            }
        }
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                tracing::debug!("[provider] fallback resolved=deepseek");
                return Self {
                    kind: ProviderKind::DeepSeek,
                    api_key: key,
                    base_url: "https://api.deepseek.com".into(),
                };
            }
        }
        let ollama_url = std::env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".into());
        let ollama_model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "llama3.2".into());
        tracing::debug!("[provider] fallback resolved=ollama url={ollama_url} model={ollama_model}");
        Self {
            kind: ProviderKind::Ollama,
            api_key: String::new(),
            base_url: ollama_url,
        }
    }

    /// Returns true if this provider has a real backend (key set or url reachable).
    /// For Ollama the best we can do is check the URL is non-empty.
    pub fn is_configured(&self) -> bool {
        match self.kind {
            ProviderKind::Custom    => !self.base_url.is_empty(),
            ProviderKind::Anthropic |
            ProviderKind::OpenAI    |
            ProviderKind::DeepSeek  => !self.api_key.is_empty(),
            ProviderKind::Ollama    => true,
        }
    }
}
