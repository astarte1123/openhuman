use serde::{Deserialize, Serialize};

use super::resolve::{locale_definition, resolve_supported_locale, DEFAULT_LOCALE};

/// The resolved locale context threaded through a single request — the
/// structured payload every AI call, error response, and localized
/// notification should carry, instead of a free-text hint appended to a
/// prompt. Built once via [`LocaleContext::resolve`] and passed down.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocaleContext {
    /// BCP 47 locale code, e.g. "it-IT". Always one of
    /// [`super::resolve::SUPPORTED_LOCALES`].
    pub locale: String,
    /// English name of the language, e.g. "Italian" — for prompts that read
    /// more naturally with a language name than a bare code.
    pub language: String,
    /// The language a response should be produced in. Identical to `locale`
    /// today; kept as a separate field so a future "translate this specific
    /// reply into X" override doesn't need to change the shape.
    pub response_language: String,
}

impl LocaleContext {
    /// Resolve a raw, untrusted locale hint (from a client-supplied RPC
    /// field, a header, anything) into a [`LocaleContext`]. Never fails —
    /// unsupported or missing input resolves to the default locale.
    pub fn resolve(raw: Option<&str>) -> Self {
        let code = resolve_supported_locale(raw);
        Self::for_code(code)
    }

    /// Build a context directly from a known-supported code (e.g. one
    /// already resolved by the frontend's own cascade and passed through).
    pub fn for_code(code: &str) -> Self {
        let def = locale_definition(code)
            .or_else(|| locale_definition(DEFAULT_LOCALE))
            .expect("DEFAULT_LOCALE is always present in the registry");
        Self {
            locale: def.code.to_string(),
            language: def.english_name.to_string(),
            response_language: def.code.to_string(),
        }
    }

    /// The default locale context (en-GB), used when no locale is
    /// available at all (e.g. a background job with no request context).
    pub fn default_context() -> Self {
        Self::for_code(DEFAULT_LOCALE)
    }
}

impl Default for LocaleContext {
    fn default() -> Self {
        Self::default_context()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_maps_regional_variant_and_fills_in_language_name() {
        let ctx = LocaleContext::resolve(Some("de-AT"));
        assert_eq!(ctx.locale, "de-DE");
        assert_eq!(ctx.language, "German");
        assert_eq!(ctx.response_language, "de-DE");
    }

    #[test]
    fn resolve_falls_back_to_default_for_unsupported_input() {
        let ctx = LocaleContext::resolve(Some("ja-JP"));
        assert_eq!(ctx.locale, "en-GB");
        assert_eq!(ctx.language, "English");
    }

    #[test]
    fn resolve_falls_back_to_default_for_none() {
        assert_eq!(
            LocaleContext::resolve(None),
            LocaleContext::default_context()
        );
    }

    #[test]
    fn serializes_to_the_documented_shape() {
        let ctx = LocaleContext::for_code("it-IT");
        let value = serde_json::to_value(&ctx).unwrap();
        assert_eq!(value["locale"], "it-IT");
        assert_eq!(value["language"], "Italian");
        assert_eq!(value["response_language"], "it-IT");
    }
}
