//! Locale registry and resolution — the Rust-side mirror of
//! `app/src/lib/i18n/locales.ts`. Keep the two in sync: this is the single
//! source of truth for which locales the *core* considers supported, and
//! for mapping an arbitrary BCP-47-ish tag onto one of them.

/// One entry in the supported-locale registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocaleDefinition {
    /// BCP 47 tag, e.g. "it-IT".
    pub code: &'static str,
    /// Name of the language written in that language itself.
    pub native_name: &'static str,
    /// Name of the language in English.
    pub english_name: &'static str,
}

pub const SUPPORTED_LOCALES: &[LocaleDefinition] = &[
    LocaleDefinition {
        code: "en-GB",
        native_name: "English",
        english_name: "English",
    },
    LocaleDefinition {
        code: "bg-BG",
        native_name: "Български",
        english_name: "Bulgarian",
    },
    LocaleDefinition {
        code: "cs-CZ",
        native_name: "Čeština",
        english_name: "Czech",
    },
    LocaleDefinition {
        code: "hr-HR",
        native_name: "Hrvatski",
        english_name: "Croatian",
    },
    LocaleDefinition {
        code: "da-DK",
        native_name: "Dansk",
        english_name: "Danish",
    },
    LocaleDefinition {
        code: "et-EE",
        native_name: "Eesti",
        english_name: "Estonian",
    },
    LocaleDefinition {
        code: "fi-FI",
        native_name: "Suomi",
        english_name: "Finnish",
    },
    LocaleDefinition {
        code: "fr-FR",
        native_name: "Français",
        english_name: "French",
    },
    LocaleDefinition {
        code: "el-GR",
        native_name: "Ελληνικά",
        english_name: "Greek",
    },
    LocaleDefinition {
        code: "ga-IE",
        native_name: "Gaeilge",
        english_name: "Irish",
    },
    LocaleDefinition {
        code: "it-IT",
        native_name: "Italiano",
        english_name: "Italian",
    },
    LocaleDefinition {
        code: "lv-LV",
        native_name: "Latviešu",
        english_name: "Latvian",
    },
    LocaleDefinition {
        code: "lt-LT",
        native_name: "Lietuvių",
        english_name: "Lithuanian",
    },
    LocaleDefinition {
        code: "mt-MT",
        native_name: "Malti",
        english_name: "Maltese",
    },
    LocaleDefinition {
        code: "nl-NL",
        native_name: "Nederlands",
        english_name: "Dutch",
    },
    LocaleDefinition {
        code: "pl-PL",
        native_name: "Polski",
        english_name: "Polish",
    },
    LocaleDefinition {
        code: "pt-PT",
        native_name: "Português",
        english_name: "Portuguese",
    },
    LocaleDefinition {
        code: "ro-RO",
        native_name: "Română",
        english_name: "Romanian",
    },
    LocaleDefinition {
        code: "sk-SK",
        native_name: "Slovenčina",
        english_name: "Slovak",
    },
    LocaleDefinition {
        code: "sl-SI",
        native_name: "Slovenščina",
        english_name: "Slovenian",
    },
    LocaleDefinition {
        code: "es-ES",
        native_name: "Español",
        english_name: "Spanish",
    },
    LocaleDefinition {
        code: "sv-SE",
        native_name: "Svenska",
        english_name: "Swedish",
    },
    LocaleDefinition {
        code: "de-DE",
        native_name: "Deutsch",
        english_name: "German",
    },
    LocaleDefinition {
        code: "hu-HU",
        native_name: "Magyar",
        english_name: "Hungarian",
    },
    LocaleDefinition {
        code: "nb-NO",
        native_name: "Norsk bokmål",
        english_name: "Norwegian Bokmål",
    },
];

pub const DEFAULT_LOCALE: &str = "en-GB";

pub fn is_supported_locale(code: &str) -> bool {
    SUPPORTED_LOCALES
        .iter()
        .any(|l| l.code.eq_ignore_ascii_case(code))
}

pub fn locale_definition(code: &str) -> Option<&'static LocaleDefinition> {
    SUPPORTED_LOCALES
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(code))
}

/// Full-tag aliases (lowercased) mapped to a supported locale — mirrors
/// `FULL_TAG_ALIASES` in `locales.ts`.
fn full_tag_alias(lower: &str) -> Option<&'static str> {
    Some(match lower {
        "it-ch" => "it-IT",
        "de-at" | "de-ch" => "de-DE",
        "fr-be" | "fr-ch" => "fr-FR",
        "en-us" | "en-ie" | "en-au" | "en-ca" | "en-nz" | "en-za" => "en-GB",
        "nl-be" => "nl-NL",
        "pt-br" => "pt-PT",
        "nb" | "nb-no" | "no" => "nb-NO",
        _ => return None,
    })
}

/// Base-language fallback (mirrors `BASE_LANGUAGE_TO_LOCALE` in locales.ts).
fn base_language_locale(base: &str) -> Option<&'static str> {
    Some(match base {
        "en" => "en-GB",
        "bg" => "bg-BG",
        "cs" => "cs-CZ",
        "hr" => "hr-HR",
        "da" => "da-DK",
        "et" => "et-EE",
        "fi" => "fi-FI",
        "fr" => "fr-FR",
        "el" => "el-GR",
        "ga" => "ga-IE",
        "it" => "it-IT",
        "lv" => "lv-LV",
        "lt" => "lt-LT",
        "mt" => "mt-MT",
        "nl" => "nl-NL",
        "pl" => "pl-PL",
        "pt" => "pt-PT",
        "ro" => "ro-RO",
        "sk" => "sk-SK",
        "sl" => "sl-SI",
        "es" => "es-ES",
        "sv" => "sv-SE",
        "de" => "de-DE",
        "hu" => "hu-HU",
        "nb" => "nb-NO",
        _ => return None,
    })
}

/// Resolve an arbitrary BCP-47-ish tag to one of [`SUPPORTED_LOCALES`].
/// Never fails — always returns a supported locale, falling back to
/// [`DEFAULT_LOCALE`]. Mirrors `resolveSupportedLocale` in `locales.ts`.
pub fn resolve_supported_locale(input: Option<&str>) -> &'static str {
    let Some(trimmed) = input.map(str::trim).filter(|s| !s.is_empty()) else {
        return DEFAULT_LOCALE;
    };

    if let Some(exact) = locale_definition(trimmed) {
        return exact.code;
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(alias) = full_tag_alias(&lower) {
        return alias;
    }

    let base = lower.split(['-', '_']).next().unwrap_or("");
    if let Some(mapped) = base_language_locale(base) {
        return mapped;
    }

    DEFAULT_LOCALE
}

/// Walk a list of preferred tags in priority order (e.g. an `Accept-Language`
/// header already split into tags) and return the first one that resolves to
/// a non-default locale, or the first English-ish tag that resolves to the
/// default — mirrors `resolveFirstSupportedLocale` in locales.ts. Falls back
/// to [`DEFAULT_LOCALE`] if nothing matches.
pub fn resolve_first_supported_locale(tags: &[&str]) -> &'static str {
    for tag in tags {
        let resolved = resolve_supported_locale(Some(tag));
        let is_english_tag = tag.to_ascii_lowercase().starts_with("en");
        if resolved != DEFAULT_LOCALE || is_english_tag {
            return resolved;
        }
    }
    DEFAULT_LOCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_supported_code_matches_case_insensitively() {
        assert_eq!(resolve_supported_locale(Some("it-IT")), "it-IT");
        assert_eq!(resolve_supported_locale(Some("IT-it")), "it-IT");
    }

    #[test]
    fn regional_variants_map_to_canonical_locale() {
        assert_eq!(resolve_supported_locale(Some("it-CH")), "it-IT");
        assert_eq!(resolve_supported_locale(Some("de-AT")), "de-DE");
        assert_eq!(resolve_supported_locale(Some("de-CH")), "de-DE");
        assert_eq!(resolve_supported_locale(Some("fr-BE")), "fr-FR");
        assert_eq!(resolve_supported_locale(Some("en-US")), "en-GB");
        assert_eq!(resolve_supported_locale(Some("en-IE")), "en-GB");
        assert_eq!(resolve_supported_locale(Some("pt-BR")), "pt-PT");
        assert_eq!(resolve_supported_locale(Some("nb-NO")), "nb-NO");
        assert_eq!(resolve_supported_locale(Some("no")), "nb-NO");
    }

    #[test]
    fn base_language_fallback_when_no_exact_or_alias_match() {
        assert_eq!(resolve_supported_locale(Some("de")), "de-DE");
        assert_eq!(resolve_supported_locale(Some("pl-XX")), "pl-PL");
    }

    #[test]
    fn unsupported_or_empty_falls_back_to_default() {
        assert_eq!(resolve_supported_locale(Some("ja-JP")), DEFAULT_LOCALE);
        assert_eq!(resolve_supported_locale(Some("")), DEFAULT_LOCALE);
        assert_eq!(resolve_supported_locale(None), DEFAULT_LOCALE);
    }

    #[test]
    fn is_supported_locale_checks_the_registry() {
        assert!(is_supported_locale("it-IT"));
        assert!(!is_supported_locale("ja-JP"));
    }

    #[test]
    fn first_supported_locale_scans_in_priority_order() {
        assert_eq!(
            resolve_first_supported_locale(&["ja-JP", "fr-FR", "en-US"]),
            "fr-FR"
        );
        assert_eq!(resolve_first_supported_locale(&["en-US", "fr-FR"]), "en-GB");
        assert_eq!(
            resolve_first_supported_locale(&["ja-JP", "ko-KR"]),
            DEFAULT_LOCALE
        );
    }

    #[test]
    fn registry_has_25_locales_with_unique_codes() {
        assert_eq!(SUPPORTED_LOCALES.len(), 25);
        let mut codes: Vec<&str> = SUPPORTED_LOCALES.iter().map(|l| l.code).collect();
        let before = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), before, "duplicate locale code in registry");
    }
}
