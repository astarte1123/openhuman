//! Unified chat-provider factory.
//!
//! Resolves workload names (e.g. `"reasoning"`, `"heartbeat"`) to a
//! `(Box<dyn Provider>, String)` tuple where the second element is the model
//! id to pass into `chat_with_history` / `simple_chat`.
//!
//! ## Provider-string grammar
//!
//! ```text
//! "aura"        → AuraBackendProvider; model = config.default_model
//! "ollama:<model>"   → local Ollama at config.local_ai.base_url
//! "<slug>:<model>"   → cloud_providers entry keyed by slug;
//!                      builds OpenAiCompatibleProvider (Bearer) or Anthropic
//!                      flavour depending on auth_style.
//! ""  / missing      → falls back to "aura"
//! ```
//!
//! Unknown slugs and missing-creds configurations produce actionable errors.

use crate::aura::config::schema::cloud_providers::AuthStyle;
use crate::aura::config::Config;
use crate::aura::credentials::AuthService;
use crate::aura::inference::provider::compatible::{
    AuthStyle as CompatAuthStyle, OpenAiCompatibleProvider,
};
use crate::aura::inference::provider::aura_backend::AuraBackendProvider;
use crate::aura::inference::provider::traits::Provider;
use crate::aura::inference::provider::youworker::{YouWorkerConfig, YouWorkerProvider};
use crate::aura::inference::provider::ProviderRuntimeOptions;

/// Sentinel meaning "use the Aura backend session JWT".
pub const PROVIDER_AURA: &str = "aura";
/// Prefix for Ollama-local providers: `"ollama:<model>"`.
pub const OLLAMA_PROVIDER_PREFIX: &str = "ollama:";

/// Auth-profile storage key for a slug-keyed provider.
///
/// New writes use `"provider:<slug>"`. Lookups also try the bare `<slug>`
/// as a legacy fallback (old configs stored keys as e.g. `"openai:default"`).
pub fn auth_key_for_slug(slug: &str) -> String {
    format!("provider:{slug}")
}

/// Return the configured provider string for a named workload role.
///
/// Returns `"aura"` when the workload has no explicit override.
pub fn provider_for_role(role: &str, config: &Config) -> String {
    let opt = match role {
        "reasoning" => config.reasoning_provider.as_deref(),
        "agentic" => config.agentic_provider.as_deref(),
        "coding" => config.coding_provider.as_deref(),
        // `memory_provider` covers both the memory-tree extract path and
        // the summarizer sub-agent (whose definition declares
        // `hint = "summarization"`). Both are "produce a condensed
        // representation of input text" — same model class, no reason
        // for a separate config knob.
        "memory" | "summarization" => config.memory_provider.as_deref(),
        "embeddings" => config.embeddings_provider.as_deref(),
        "heartbeat" => config.heartbeat_provider.as_deref(),
        "learning" => config.learning_provider.as_deref(),
        "subconscious" => config.subconscious_provider.as_deref(),
        _ => None,
    };
    let s = opt.unwrap_or("").trim();
    if s.is_empty() || s == "cloud" {
        PROVIDER_AURA.to_string()
    } else {
        s.to_string()
    }
}

/// Build a `(Provider, model)` for the given workload role.
pub fn create_chat_provider(
    role: &str,
    config: &Config,
) -> anyhow::Result<(Box<dyn Provider>, String)> {
    let s = provider_for_role(role, config);
    log::debug!(
        "[providers][chat-factory] create_chat_provider role={} resolved_string={}",
        role,
        s
    );
    create_chat_provider_from_string(role, &s, config)
}

/// Build a `(Provider, model)` from an explicit provider string and config.
///
/// See module-level grammar documentation for valid formats.
pub fn create_chat_provider_from_string(
    role: &str,
    provider: &str,
    config: &Config,
) -> anyhow::Result<(Box<dyn Provider>, String)> {
    let p = provider.trim();
    log::debug!(
        "[providers][chat-factory] create_chat_provider_from_string role={} provider={}",
        role,
        p
    );

    // Empty / legacy "cloud" sentinel → Aura backend.
    if p.is_empty() || p == "cloud" {
        return make_aura_backend(config);
    }

    if p == PROVIDER_AURA {
        return make_aura_backend(config);
    }

    if let Some(model) = p.strip_prefix(OLLAMA_PROVIDER_PREFIX) {
        if model.trim().is_empty() {
            anyhow::bail!(
                "[chat-factory] provider string '{}' for role '{}' has an empty model — \
                 use 'ollama:<model-id>'",
                p,
                role
            );
        }
        return make_ollama_provider(model.trim(), config);
    }

    // New grammar: "<slug>:<model>"
    if let Some(colon_pos) = p.find(':') {
        let slug = p[..colon_pos].trim();
        let model = p[colon_pos + 1..].trim();

        if slug.is_empty() {
            anyhow::bail!(
                "[chat-factory] provider string '{}' for role '{}' has an empty slug",
                p,
                role
            );
        }

        return make_cloud_provider_by_slug(role, slug, model, config);
    }

    // No colon: might be a bare legacy type string (e.g. "openai"). Try as
    // slug lookup with empty model — gives a clear "no entry" error rather
    // than an opaque parse failure.
    anyhow::bail!(
        "[chat-factory] unrecognised provider string '{}' for role '{}'. \
         Valid forms: aura, ollama:<model>, <slug>:<model>. \
         Configured slugs: [{}]",
        p,
        role,
        config
            .cloud_providers
            .iter()
            .map(|e| e.slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build the Aura backend provider (session-JWT auth).
fn make_aura_backend(config: &Config) -> anyhow::Result<(Box<dyn Provider>, String)> {
    let model = config
        .default_model
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "reasoning-v1".to_string());
    // Critical: pass the *config's* workspace directory through so the
    // provider's `AuthService` reads `auth-profiles.json` from the
    // same dir login wrote to. Without this, `ProviderRuntimeOptions::default()`
    // leaves `aura_dir = None`, the provider falls back to
    // `~/.aura`, and reads an unrelated (or empty)
    // profile store — surfacing as "No backend session: store a JWT
    // via auth (app-session)" even though login just succeeded in the
    // user's actual workspace (e.g. test workspaces under AURA_WORKSPACE).
    let options = ProviderRuntimeOptions {
        aura_dir: config.config_path.parent().map(std::path::PathBuf::from),
        secrets_encrypt: config.secrets.encrypt,
        ..ProviderRuntimeOptions::default()
    };
    log::debug!(
        "[providers][chat-factory] building aura backend provider model={} state_dir={:?} secrets_encrypt={}",
        model,
        options.aura_dir,
        options.secrets_encrypt
    );
    // Translate `hint:<tier>` model strings into the Aura backend's
    // canonical tier names.
    let model = match model.strip_prefix("hint:") {
        Some("reasoning") => crate::aura::config::MODEL_REASONING_V1.to_string(),
        Some("chat") => crate::aura::config::MODEL_REASONING_QUICK_V1.to_string(),
        Some("agentic") => crate::aura::config::MODEL_AGENTIC_V1.to_string(),
        Some("coding") => crate::aura::config::MODEL_CODING_V1.to_string(),
        _ => model,
    };
    let p = Box::new(AuraBackendProvider::new(
        config.api_url.as_deref(),
        &options,
    ));
    Ok((p, model))
}

/// Build an Ollama local provider.
fn make_ollama_provider(
    model: &str,
    config: &Config,
) -> anyhow::Result<(Box<dyn Provider>, String)> {
    let base_url = config
        .local_ai
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434");
    // Ollama exposes an OpenAI-compatible endpoint at /v1.
    let endpoint = format!("{}/v1", base_url.trim_end_matches('/'));
    log::info!(
        "[providers][chat-factory] building ollama provider model={} endpoint_host={}",
        model,
        redact_endpoint(&endpoint)
    );
    let p = make_openai_compatible_provider_with_config(
        &endpoint,
        "",
        CompatAuthStyle::None,
        &config.temperature_unsupported_models,
    )?;
    Ok((p, model.to_string()))
}

/// Look up a `cloud_providers` entry by slug and build the provider.
fn make_cloud_provider_by_slug(
    role: &str,
    slug: &str,
    model: &str,
    config: &Config,
) -> anyhow::Result<(Box<dyn Provider>, String)> {
    let entry = config.cloud_providers.iter().find(|e| e.slug == slug);

    let entry = entry.ok_or_else(|| {
        let known: Vec<&str> = config
            .cloud_providers
            .iter()
            .map(|e| e.slug.as_str())
            .collect();
        anyhow::anyhow!(
            "[chat-factory] no cloud provider configured for slug '{}' (role '{}') — \
             add an entry with that slug to cloud_providers in config.toml. \
             Configured slugs: [{}]",
            slug,
            role,
            known.join(", ")
        )
    })?;

    // Resolve effective model: use provided model if non-empty, else fall back
    // to the entry's legacy default_model (if any), else empty → error.
    let effective_model = if model.trim().is_empty() {
        entry.default_model.clone().unwrap_or_default()
    } else {
        model.to_string()
    };

    log::info!(
        "[providers][chat-factory] role={} slug={} model={} endpoint_host={}",
        role,
        slug,
        effective_model,
        redact_endpoint(&entry.endpoint)
    );

    let key = lookup_key_for_slug(slug, config)?;

    let unsupported = &config.temperature_unsupported_models;
    match entry.auth_style {
        AuthStyle::Anthropic => {
            let p = make_openai_compatible_provider_with_config(
                &entry.endpoint,
                &key,
                CompatAuthStyle::Anthropic,
                unsupported,
            )?;
            Ok((p, effective_model))
        }
        AuthStyle::AuraJwt => {
            // Route to the Aura backend — ignore the entry's endpoint
            // and model; use the backend provider with the configured default.
            log::debug!(
                "[providers][chat-factory] slug='{}' has auth_style=AuraJwt → routing to aura backend",
                slug
            );
            make_aura_backend(config)
        }
        AuthStyle::None => {
            let p = make_openai_compatible_provider_with_config(
                &entry.endpoint,
                "",
                CompatAuthStyle::None,
                unsupported,
            )?;
            Ok((p, effective_model))
        }
        AuthStyle::Bearer => {
            let p = make_openai_compatible_provider_with_config(
                &entry.endpoint,
                &key,
                CompatAuthStyle::Bearer,
                unsupported,
            )?;
            Ok((p, effective_model))
        }
        AuthStyle::YouWorker => {
            let p = make_youworker_provider(slug, entry, &key)?;
            Ok((p, effective_model))
        }
    }
}

/// Build a `YouWorkerProvider` from a `cloud_providers` entry. `client_secret`
/// is the slug's resolved `provider:<slug>` credential (same lookup as every
/// other provider's API key — `key` here is that secret, not a bearer token).
fn make_youworker_provider(
    slug: &str,
    entry: &crate::aura::config::schema::cloud_providers::CloudProviderCreds,
    client_secret: &str,
) -> anyhow::Result<Box<dyn Provider>> {
    let require = |field: Option<&String>, name: &str| -> anyhow::Result<String> {
        field
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "[chat-factory] youworker provider '{}' is missing '{}' — set it on the \
                     cloud_providers entry",
                    slug,
                    name
                )
            })
    };
    let autentika_url = require(entry.autentika_url.as_ref(), "autentika_url")?;
    let persona = require(entry.persona.as_ref(), "persona")?;
    let client_id = require(entry.client_id.as_ref(), "client_id")?;
    let subject_uuid = require(entry.subject_uuid.as_ref(), "subject_uuid")?;
    if client_secret.trim().is_empty() {
        anyhow::bail!(
            "[chat-factory] youworker provider '{}' has no client_secret stored — \
             set it via auth (provider:{})",
            slug,
            slug
        );
    }
    let yw_base = entry.endpoint.trim().trim_end_matches('/').to_string();
    if yw_base.is_empty() {
        anyhow::bail!(
            "[chat-factory] youworker provider '{}' is missing 'endpoint' (used as yw_base)",
            slug
        );
    }

    let transport = crate::aura::inference::provider::youworker::YouWorkerTransport::from_config_str(
        entry.youworker_transport.as_deref().unwrap_or(""),
    );

    Ok(Box::new(YouWorkerProvider::new(YouWorkerConfig {
        autentika_url,
        yw_base,
        client_id,
        client_secret: client_secret.to_string(),
        persona,
        subject_uuid,
        transport,
    })))
}

/// Fetch the bearer token for a slug from the workspace `auth-profiles.json`.
///
/// Tries `provider:<slug>` first (new key format), then the bare `<slug>`
/// (legacy format where keys were stored as `"openai"`, `"anthropic"`, etc.).
/// Missing or empty keys return `Ok(String::new())` — callers treat that as
/// "no auth", which surfaces an authentication error at first call rather than
/// at factory build time.
pub fn lookup_key_for_slug(slug: &str, config: &Config) -> anyhow::Result<String> {
    let auth = AuthService::from_config(config);
    // Try new-style key first.
    let new_key = auth_key_for_slug(slug);
    if let Ok(Some(k)) = auth.get_provider_bearer_token(&new_key, None) {
        if !k.is_empty() {
            log::debug!(
                "[providers][chat-factory] auth lookup slug={} key_present=true (new-style)",
                slug
            );
            return Ok(k);
        }
    }
    // Fall back to legacy bare slug.
    let key = auth
        .get_provider_bearer_token(slug, None)
        .map_err(|e| {
            anyhow::anyhow!(
                "[chat-factory] failed to read API key for slug '{}': {}",
                slug,
                e
            )
        })?
        .unwrap_or_default();
    log::debug!(
        "[providers][chat-factory] auth lookup slug={} key_present={}",
        slug,
        !key.is_empty()
    );
    Ok(key)
}

/// Build an `OpenAiCompatibleProvider` with the given auth style.
fn make_openai_compatible_provider(
    endpoint: &str,
    api_key: &str,
    auth_style: CompatAuthStyle,
) -> anyhow::Result<Box<dyn Provider>> {
    make_openai_compatible_provider_with_config(endpoint, api_key, auth_style, &[])
}

/// Build an `OpenAiCompatibleProvider` with auth style and temperature
/// suppression list from config.
fn make_openai_compatible_provider_with_config(
    endpoint: &str,
    api_key: &str,
    auth_style: CompatAuthStyle,
    temperature_unsupported_models: &[String],
) -> anyhow::Result<Box<dyn Provider>> {
    let key = if api_key.trim().is_empty() {
        None
    } else {
        Some(api_key)
    };
    Ok(Box::new(
        OpenAiCompatibleProvider::new("cloud", endpoint, key, auth_style)
            .with_temperature_unsupported_models(temperature_unsupported_models.to_vec()),
    ))
}

/// Return a safe-to-log representation of a URL endpoint: `scheme://host` only.
fn redact_endpoint(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(rest) = trimmed.split_once("://") {
        let scheme = rest.0;
        let authority = rest.1.split('/').next().unwrap_or("");
        let host = authority.split('@').last().unwrap_or(authority);
        let host_no_query = host.split('?').next().unwrap_or(host);
        return format!("{}://{}", scheme, host_no_query);
    }
    "<endpoint>".to_string()
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "factory_test.rs"]
mod factory_test;
