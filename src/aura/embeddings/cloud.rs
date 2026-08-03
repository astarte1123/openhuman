//! Cloud embedding provider — routes through the Aura backend's
//! `POST /openai/v1/embeddings` surface (Voyage-backed) using the same
//! session JWT that the `AuraBackendProvider` chat path uses.
//!
//! This is the default embedder for a fresh install. The local Ollama path
//! stays available, but the user has to explicitly opt in (either by setting
//! `memory.embedding_provider = "ollama"` in `config.toml`, or by enabling
//! the local-AI runtime with `local_ai.usage.embeddings = true`).
//!
//! The JWT and API URL are resolved per call so a session refresh between
//! embed batches is picked up transparently — matching
//! [`crate::aura::inference::provider::aura_backend::AuraBackendProvider`].

use std::path::PathBuf;

use async_trait::async_trait;

use super::openai::OpenAiEmbedding;
use super::EmbeddingProvider;
use crate::api::config::effective_api_url;
use crate::aura::credentials::{AuthService, APP_SESSION_PROVIDER};

/// Default cloud embedding model — backed by `voyage-3.5` (1024 dims) on the
/// Aura backend. See `tinyhumansai/backend#746`.
pub const DEFAULT_CLOUD_EMBEDDING_MODEL: &str = "embedding-v1";

/// Default output dimensionality for [`DEFAULT_CLOUD_EMBEDDING_MODEL`].
pub const DEFAULT_CLOUD_EMBEDDING_DIMENSIONS: usize = 1024;

/// Aura-backend-backed embedding provider.
pub struct AuraCloudEmbedding {
    api_url: Option<String>,
    aura_dir: Option<PathBuf>,
    secrets_encrypt: bool,
    model: String,
    dims: usize,
}

impl AuraCloudEmbedding {
    /// Construct a cloud embedder. `api_url` and `aura_dir` are looked up
    /// per request; pass `None` to fall back to the runtime defaults
    /// ([`effective_api_url`] / `~/.aura`).
    pub fn new(
        api_url: Option<String>,
        aura_dir: Option<PathBuf>,
        secrets_encrypt: bool,
        model: impl Into<String>,
        dims: usize,
    ) -> Self {
        Self {
            api_url: api_url
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            aura_dir,
            secrets_encrypt,
            model: model.into(),
            dims,
        }
    }

    fn state_dir(&self) -> PathBuf {
        self.aura_dir.clone().unwrap_or_else(|| {
            directories::UserDirs::new()
                .map(|d| d.home_dir().join(".aura"))
                .unwrap_or_else(|| PathBuf::from(".aura"))
        })
    }

    fn resolve_bearer(&self) -> anyhow::Result<String> {
        let auth = AuthService::new(&self.state_dir(), self.secrets_encrypt);
        if let Some(t) = auth
            .get_provider_bearer_token(APP_SESSION_PROVIDER, None)?
            .filter(|s| !s.trim().is_empty())
        {
            return Ok(t);
        }
        anyhow::bail!(
            "No backend session for cloud embeddings: log in to Aura, or set \
             memory.embedding_provider to \"ollama\" / \"none\" in config.toml"
        )
    }

    fn base_url(&self) -> String {
        let u = effective_api_url(&self.api_url);
        format!("{}/openai/v1", u.trim_end_matches('/'))
    }
}

#[async_trait]
impl EmbeddingProvider for AuraCloudEmbedding {
    fn name(&self) -> &str {
        "cloud"
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let token = self.resolve_bearer()?;
        let inner = OpenAiEmbedding::new(&self.base_url(), &token, &self.model, self.dims);
        inner.embed(texts).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_dimensions() {
        let p = AuraCloudEmbedding::new(
            None,
            None,
            true,
            DEFAULT_CLOUD_EMBEDDING_MODEL,
            DEFAULT_CLOUD_EMBEDDING_DIMENSIONS,
        );
        assert_eq!(p.name(), "cloud");
        assert_eq!(p.model_id(), DEFAULT_CLOUD_EMBEDDING_MODEL);
        assert_eq!(p.dimensions(), DEFAULT_CLOUD_EMBEDDING_DIMENSIONS);
        assert_eq!(p.signature(), "provider=cloud;model=embedding-v1;dims=1024");
    }

    #[test]
    fn base_url_appends_openai_v1() {
        let p = AuraCloudEmbedding::new(
            Some("https://api.aura.example/".into()),
            None,
            true,
            DEFAULT_CLOUD_EMBEDDING_MODEL,
            DEFAULT_CLOUD_EMBEDDING_DIMENSIONS,
        );
        assert_eq!(p.base_url(), "https://api.aura.example/openai/v1");
    }

    #[tokio::test]
    async fn embed_empty_returns_empty_without_auth() {
        // Empty input should short-circuit *before* hitting the AuthService —
        // otherwise the no-op path would spuriously fail in unauthenticated
        // contexts (e.g. ingestion of an empty chunk batch).
        let p = AuraCloudEmbedding::new(
            None,
            None,
            false,
            DEFAULT_CLOUD_EMBEDDING_MODEL,
            DEFAULT_CLOUD_EMBEDDING_DIMENSIONS,
        );
        assert!(p.embed(&[]).await.unwrap().is_empty());
    }
}
