//! Cloud provider credential schema.
//!
//! Each entry in `Config::cloud_providers` represents one configured LLM
//! backend. Providers are keyed by a user-chosen `slug` (e.g. `"openai"`,
//! `"my-deepseek"`). The factory in `crate::aura::inference::provider::factory`
//! resolves workload-to-provider strings against this list at runtime using
//! the grammar `"<slug>:<model>"`.
//!
//! Legacy configs that use `type`/`default_model` are migrated in-memory on
//! load via `migrate_legacy_fields()`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Authentication header style for a cloud provider.
///
/// Wire format is lowercase (e.g. `"bearer"`). Determines which HTTP headers
/// are attached when calling the provider's API.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthStyle {
    /// OpenAI-compatible: `Authorization: Bearer <key>`
    #[default]
    Bearer,
    /// Anthropic: `x-api-key: <key>` + `anthropic-version: 2023-06-01`
    Anthropic,
    /// Aura session JWT (injected by the backend provider, not stored here).
    AuraJwt,
    /// No auth header — e.g. local Ollama.
    None,
    /// youworker.ai YouWorkForce REST/webhook channel (Autentika
    /// token-exchange + `POST {yw_base}/channels/youmessage/inbound/message`,
    /// with the terminal reply delivered async to our own HMAC-verified
    /// outbound webhook — see `inference::provider::youworker`). Needs
    /// [`CloudProviderCreds::autentika_url`], [`CloudProviderCreds::persona`],
    /// [`CloudProviderCreds::client_id`] in addition to `endpoint` (used as
    /// `yw_base`) and the usual `provider:<slug>` stored secret (here,
    /// `client_secret` rather than a bearer API key). The outbound HMAC
    /// secret is a second stored secret, keyed `provider:<slug>:outbound-hmac`
    /// (see `youworker_webhook::outbound_hmac_key_for_slug`).
    YouWorker,
}

impl AuthStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bearer => "bearer",
            Self::Anthropic => "anthropic",
            Self::AuraJwt => "aura_jwt",
            Self::None => "none",
            Self::YouWorker => "youworker",
        }
    }
}

/// Endpoint config for one cloud LLM provider.
///
/// **Note on secrets**: API keys are NOT stored on this struct. They live in
/// `auth-profiles.json` via [`crate::aura::credentials::AuthService`],
/// keyed by `provider:<slug>` (falling back to bare `<slug>` for legacy
/// entries). The factory looks up the token at call time via
/// [`crate::aura::inference::provider::factory::auth_key_for_slug`].
///
/// ## Back-compat
///
/// Old configs may have `type` and `default_model` fields. These are
/// tolerated on read (via `legacy_type` / `default_model`) but never written.
/// Call `migrate_legacy_fields()` after deserialising.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct CloudProviderCreds {
    /// Opaque stable id, e.g. `"p_openai_a8c3f"`. Never shown in the UI.
    /// Generated once by [`generate_provider_id`] and never changes.
    pub id: String,
    /// Routing key chosen by the user or seeded from the legacy type.
    /// Lower-case alphanumeric + `-`. Must be unique per config and not in the
    /// reserved list (see [`is_slug_reserved`]). The factory resolves
    /// `"<slug>:<model>"` strings against this field.
    pub slug: String,
    /// Human-readable display label, supplied by the frontend. Not used in routing.
    pub label: String,
    /// OpenAI-compatible base URL (`/models`, `/chat/completions` etc. are appended).
    pub endpoint: String,
    /// Authentication header style.
    pub auth_style: AuthStyle,

    // ── Back-compat: old `type` field ───────────────────────────────────────
    /// Legacy discriminator written by older builds. Read-only; never emitted.
    #[serde(rename = "type", default, skip_serializing)]
    pub legacy_type: Option<String>,

    // ── Back-compat: old `default_model` field ──────────────────────────────
    /// Legacy default model written by older builds. Read-only; never emitted.
    #[serde(default, skip_serializing)]
    pub default_model: Option<String>,

    // ── youworker.ai (AuthStyle::YouWorker only) ─────────────────────────────
    /// Autentika workspace issuer base URL, e.g.
    /// `https://your-workspace.autentika.example`. Hosts the two token-exchange
    /// endpoints (`/oauth2/token`, `/v1/oauth/token_exchange`).
    #[serde(default)]
    pub autentika_url: Option<String>,
    /// Persona slug sent on `session.create` (deployment-specific).
    #[serde(default)]
    pub persona: Option<String>,
    /// Autentika service-principal client_id for the `client_credentials` step.
    /// Not a secret by itself (paired with `client_secret`, which lives in
    /// `auth-profiles.json` like every other provider's key).
    #[serde(default)]
    pub client_id: Option<String>,
    /// The youworker end-user UUID presented as `subject_token` in the RFC
    /// 8693 token-exchange (MVP: every Aura end-user maps to this one
    /// youworker user). Not a secret.
    #[serde(default)]
    pub subject_uuid: Option<String>,
}

impl Default for CloudProviderCreds {
    fn default() -> Self {
        Self {
            id: String::new(),
            slug: String::new(),
            label: String::new(),
            endpoint: String::new(),
            auth_style: AuthStyle::Bearer,
            legacy_type: None,
            default_model: None,
            autentika_url: None,
            persona: None,
            client_id: None,
            subject_uuid: None,
        }
    }
}

/// Reserved slugs that may not be used for user-configured providers.
/// These are sentinels in the factory's routing grammar.
pub fn is_slug_reserved(s: &str) -> bool {
    matches!(s.trim(), "" | "cloud" | "aura" | "ollama" | "pid")
}

/// Apply legacy field migration in-place.
///
/// Idempotent: only fills in empty fields from the legacy `type`/`default_model`
/// values. Safe to call on already-migrated entries.
pub fn migrate_legacy_fields(entry: &mut CloudProviderCreds) {
    let legacy_type = entry.legacy_type.clone().unwrap_or_default();
    let lt = legacy_type.trim();

    // Slug from legacy type when missing.
    if entry.slug.is_empty() && !lt.is_empty() {
        entry.slug = lt.to_string();
        log::debug!(
            "[config][cloud_providers] migrated slug from legacy type='{}' id={}",
            lt,
            entry.id
        );
    }

    // Label from static map when missing.
    if entry.label.is_empty() {
        entry.label = legacy_label_for(if entry.slug.is_empty() {
            lt
        } else {
            &entry.slug
        })
        .to_string();
        log::debug!(
            "[config][cloud_providers] migrated label='{}' for slug='{}' id={}",
            entry.label,
            entry.slug,
            entry.id
        );
    }

    // Endpoint from legacy defaults when missing.
    if entry.endpoint.is_empty() {
        let ep = legacy_default_endpoint(lt);
        if !ep.is_empty() {
            entry.endpoint = ep.to_string();
        }
    }

    // Auth style from legacy type when still at default Bearer.
    if entry.auth_style == AuthStyle::Bearer {
        match lt {
            "anthropic" => {
                entry.auth_style = AuthStyle::Anthropic;
            }
            "aura" => {
                entry.auth_style = AuthStyle::AuraJwt;
            }
            _ => {}
        }
    }
}

/// Map a legacy type string (or slug) to a human-readable label.
fn legacy_label_for(type_str: &str) -> &'static str {
    match type_str {
        "aura" => "Aura",
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "openrouter" => "OpenRouter",
        "custom" => "Custom",
        _ => "Custom",
    }
}

/// Map a legacy type string to its well-known default endpoint.
fn legacy_default_endpoint(type_str: &str) -> &'static str {
    match type_str {
        "aura" => "https://api.aura.ai/v1",
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        _ => "",
    }
}

/// Generate a short opaque id for a new provider entry.
///
/// Format: `"p_<slug>_<5 random alphanumerics>"`, e.g. `"p_openai_a8c3f"`.
/// The random suffix is not cryptographically strong — it only needs to be
/// unique within a single user's config file.
pub fn generate_provider_id(slug: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Cheap pseudo-random from timestamp nanoseconds — adequate for local
    // config uniqueness without pulling in a PRNG crate.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut suffix = String::with_capacity(5);
    let mut seed = nanos as usize;
    for _ in 0..5 {
        suffix.push(chars[seed % chars.len()] as char);
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        seed = (seed >> 33) ^ seed;
    }
    // Sanitise slug to only alphanumeric + '-' for the id prefix.
    let safe_slug: String = slug
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(20)
        .collect();
    format!("p_{}_{}", safe_slug, suffix)
}

// ── Back-compat type alias ──────────────────────────────────────────────────
// Kept so existing code that imports `CloudProviderType` compiles without
// sweeping changes. New code should use `AuthStyle` directly.

/// Legacy discriminator enum. **Deprecated**: use `AuthStyle` on new entries.
/// Retained only to satisfy callers that still pattern-match on
/// `CloudProviderType` (e.g. the migration module). Will be removed once all
/// call sites are updated to slug-keyed lookups.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CloudProviderType {
    Aura,
    Openai,
    Anthropic,
    Openrouter,
    Custom,
}

impl CloudProviderType {
    /// Well-known default base URL for each provider type.
    pub fn default_endpoint(&self) -> &'static str {
        match self {
            Self::Aura => "https://api.aura.ai/v1",
            Self::Openai => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Openrouter => "https://openrouter.ai/api/v1",
            Self::Custom => "",
        }
    }

    /// Human-readable label used in logs and error messages.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aura => "Aura",
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Openrouter => "OpenRouter",
            Self::Custom => "Custom",
        }
    }

    /// Lowercase wire-format string (matches JSON serialisation).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aura => "aura",
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::Custom => "custom",
        }
    }

    /// Corresponding `AuthStyle`.
    pub fn auth_style(&self) -> AuthStyle {
        match self {
            Self::Aura => AuthStyle::AuraJwt,
            Self::Anthropic => AuthStyle::Anthropic,
            _ => AuthStyle::Bearer,
        }
    }
}
