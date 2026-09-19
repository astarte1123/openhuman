/**
 * Locale registry — single source of truth for every locale the app can run
 * in. Adding a language (e.g. `nn-NO`) means adding one entry here; nothing
 * else in the app should hardcode a locale list.
 *
 * Codes are BCP 47 language-region tags (e.g. `it-IT`), matching what the
 * rest of the stack (STT/TTS, backend `preferred_locale`) is expected to use.
 */
export interface LocaleDefinition {
  /** BCP 47 tag, e.g. "it-IT". */
  code: string;
  /** Name of the language written in that language itself, e.g. "Italiano". */
  nativeName: string;
  /** Name of the language in English, e.g. "Italian". */
  englishName: string;
}

export const SUPPORTED_LOCALES: readonly LocaleDefinition[] = [
  { code: 'en-GB', nativeName: 'English', englishName: 'English' },
  { code: 'bg-BG', nativeName: 'Български', englishName: 'Bulgarian' },
  { code: 'cs-CZ', nativeName: 'Čeština', englishName: 'Czech' },
  { code: 'hr-HR', nativeName: 'Hrvatski', englishName: 'Croatian' },
  { code: 'da-DK', nativeName: 'Dansk', englishName: 'Danish' },
  { code: 'et-EE', nativeName: 'Eesti', englishName: 'Estonian' },
  { code: 'fi-FI', nativeName: 'Suomi', englishName: 'Finnish' },
  { code: 'fr-FR', nativeName: 'Français', englishName: 'French' },
  { code: 'el-GR', nativeName: 'Ελληνικά', englishName: 'Greek' },
  { code: 'ga-IE', nativeName: 'Gaeilge', englishName: 'Irish' },
  { code: 'it-IT', nativeName: 'Italiano', englishName: 'Italian' },
  { code: 'lv-LV', nativeName: 'Latviešu', englishName: 'Latvian' },
  { code: 'lt-LT', nativeName: 'Lietuvių', englishName: 'Lithuanian' },
  { code: 'mt-MT', nativeName: 'Malti', englishName: 'Maltese' },
  { code: 'nl-NL', nativeName: 'Nederlands', englishName: 'Dutch' },
  { code: 'pl-PL', nativeName: 'Polski', englishName: 'Polish' },
  { code: 'pt-PT', nativeName: 'Português', englishName: 'Portuguese' },
  { code: 'ro-RO', nativeName: 'Română', englishName: 'Romanian' },
  { code: 'sk-SK', nativeName: 'Slovenčina', englishName: 'Slovak' },
  { code: 'sl-SI', nativeName: 'Slovenščina', englishName: 'Slovenian' },
  { code: 'es-ES', nativeName: 'Español', englishName: 'Spanish' },
  { code: 'sv-SE', nativeName: 'Svenska', englishName: 'Swedish' },
  { code: 'de-DE', nativeName: 'Deutsch', englishName: 'German' },
  { code: 'hu-HU', nativeName: 'Magyar', englishName: 'Hungarian' },
  { code: 'nb-NO', nativeName: 'Norsk bokmål', englishName: 'Norwegian Bokmål' },
] as const;

export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number]['code'];

export const DEFAULT_LOCALE: SupportedLocale = 'en-GB';

const SUPPORTED_CODES = new Set<string>(SUPPORTED_LOCALES.map(l => l.code));

export function isSupportedLocale(code: string): code is SupportedLocale {
  return SUPPORTED_CODES.has(code);
}

export function getLocaleDefinition(code: SupportedLocale): LocaleDefinition {
  return SUPPORTED_LOCALES.find(l => l.code === code) ?? SUPPORTED_LOCALES[0];
}

/**
 * Full-tag aliases (lowercased) that map a regional variant to one of our
 * supported locales — e.g. a Swiss-Italian browser (`it-CH`) should still
 * get Italian, not fall through to the base-language table below and pick
 * a different regional flavor than intended.
 */
const FULL_TAG_ALIASES: Record<string, SupportedLocale> = {
  'it-ch': 'it-IT',
  'de-at': 'de-DE',
  'de-ch': 'de-DE',
  'fr-be': 'fr-FR',
  'fr-ch': 'fr-FR',
  'en-us': 'en-GB',
  'en-ie': 'en-GB',
  'en-au': 'en-GB',
  'en-ca': 'en-GB',
  'en-nz': 'en-GB',
  'en-za': 'en-GB',
  'nl-be': 'nl-NL',
  // Brazilian Portuguese maps to pt-PT until a dedicated pt-BR catalog exists.
  'pt-br': 'pt-PT',
  nb: 'nb-NO',
  'nb-no': 'nb-NO',
  no: 'nb-NO',
};

/** Base (language-only) fallback used when no full-tag alias matches. */
const BASE_LANGUAGE_TO_LOCALE: Record<string, SupportedLocale> = {
  en: 'en-GB',
  bg: 'bg-BG',
  cs: 'cs-CZ',
  hr: 'hr-HR',
  da: 'da-DK',
  et: 'et-EE',
  fi: 'fi-FI',
  fr: 'fr-FR',
  el: 'el-GR',
  ga: 'ga-IE',
  it: 'it-IT',
  lv: 'lv-LV',
  lt: 'lt-LT',
  mt: 'mt-MT',
  nl: 'nl-NL',
  pl: 'pl-PL',
  pt: 'pt-PT',
  ro: 'ro-RO',
  sk: 'sk-SK',
  sl: 'sl-SI',
  es: 'es-ES',
  sv: 'sv-SE',
  de: 'de-DE',
  hu: 'hu-HU',
  nb: 'nb-NO',
};

/**
 * Resolve an arbitrary BCP-47-ish tag (from `navigator.language`, a stored
 * preference, a backend `preferred_locale`, anything) to one of our
 * supported locales. Never throws — always returns a supported locale,
 * falling back to `DEFAULT_LOCALE`.
 */
export function resolveSupportedLocale(input: string | null | undefined): SupportedLocale {
  if (!input) return DEFAULT_LOCALE;
  const trimmed = input.trim();
  if (!trimmed) return DEFAULT_LOCALE;

  // Exact match against a supported code (case-insensitive).
  const exact = SUPPORTED_LOCALES.find(l => l.code.toLowerCase() === trimmed.toLowerCase());
  if (exact) return exact.code;

  const lower = trimmed.toLowerCase();
  if (FULL_TAG_ALIASES[lower]) return FULL_TAG_ALIASES[lower];

  const base = lower.split(/[-_]/)[0];
  if (BASE_LANGUAGE_TO_LOCALE[base]) return BASE_LANGUAGE_TO_LOCALE[base];

  return DEFAULT_LOCALE;
}

/**
 * Walk a list of preferred tags (e.g. `navigator.languages`) in order and
 * return the first one that resolves to a supported locale other than the
 * default — falling back to `DEFAULT_LOCALE` only if nothing matches.
 */
export function resolveFirstSupportedLocale(tags: readonly string[]): SupportedLocale {
  for (const tag of tags) {
    const resolved = resolveSupportedLocale(tag);
    // Only accept the default via an exact/alias match on tags that are
    // actually English-ish; otherwise keep scanning the rest of the list
    // so a non-English primary tag isn't shadowed by an accidental
    // default fallback from an earlier unresolvable tag.
    if (resolved !== DEFAULT_LOCALE || /^en([-_]|$)/i.test(tag)) {
      return resolved;
    }
  }
  return DEFAULT_LOCALE;
}
