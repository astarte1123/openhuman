//! Locale resolution — the single source of truth for which locales Aura
//! supports and how an arbitrary tag maps onto one of them, on the Rust
//! side. Mirrors `app/src/lib/i18n/locales.ts`; keep the two in sync.
//!
//! Other domains (AI prompts, voice/meeting locale defaults, localized
//! notifications) should build a [`LocaleContext`] via
//! [`LocaleContext::resolve`] rather than re-implementing fallback logic.

mod resolve;
mod schemas;
mod types;

pub use resolve::{
    is_supported_locale, locale_definition, resolve_first_supported_locale,
    resolve_supported_locale, LocaleDefinition, DEFAULT_LOCALE, SUPPORTED_LOCALES,
};
pub use schemas::{
    all_controller_schemas as all_locale_controller_schemas,
    all_registered_controllers as all_locale_registered_controllers,
};
pub use types::LocaleContext;
