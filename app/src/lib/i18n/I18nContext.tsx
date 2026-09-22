import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo } from 'react';

import { useAppSelector } from '../../store/hooks';
import bgBG from './bg-BG';
import csCZ from './cs-CZ';
import daDK from './da-DK';
import deDE from './de-DE';
import elGR from './el-GR';
import en from './en';
import esES from './es-ES';
import etEE from './et-EE';
import fiFI from './fi-FI';
import frFR from './fr-FR';
import gaIE from './ga-IE';
import hrHR from './hr-HR';
import huHU from './hu-HU';
import itIT from './it-IT';
import { DEFAULT_LOCALE, type SupportedLocale } from './locales';
import ltLT from './lt-LT';
import lvLV from './lv-LV';
import mtMT from './mt-MT';
import nbNO from './nb-NO';
import nlNL from './nl-NL';
import plPL from './pl-PL';
import ptPT from './pt-PT';
import roRO from './ro-RO';
import skSK from './sk-SK';
import slSI from './sl-SI';
import svSE from './sv-SE';
import { renderTranslation } from './translate';
import type { TranslationMap, TranslationParams } from './types';

interface I18nContextValue {
  t: (key: string, params?: TranslationParams) => string;
  locale: SupportedLocale;
}

// Resolve the effective English map at call time. `en` may be wrapped in
// `{ default: { ... } }` by CJS/ESM interop in test runners, or tree-shaken
// to an empty object. We check at each call to handle lazy module resolution.
function resolveEn(): TranslationMap {
  const raw: Record<string, unknown> = en as unknown as Record<string, unknown>;
  const unwrapped =
    raw != null && typeof raw === 'object' && 'default' in raw && typeof raw.default === 'object'
      ? (raw.default as TranslationMap)
      : (raw as unknown as TranslationMap);
  if (Object.keys(unwrapped).length > 0) return unwrapped;
  return {};
}

// Per-locale catalogs. `en-GB` is resolved lazily via `resolveEn()` so the
// CJS/ESM interop above still applies; every other supported locale maps to
// its generated catalog module. Keys missing from a locale fall back to
// en-GB in `resolveKey` below.
const CATALOGS: Partial<Record<SupportedLocale, TranslationMap>> = {
  'bg-BG': bgBG,
  'cs-CZ': csCZ,
  'da-DK': daDK,
  'de-DE': deDE,
  'el-GR': elGR,
  'es-ES': esES,
  'et-EE': etEE,
  'fi-FI': fiFI,
  'fr-FR': frFR,
  'ga-IE': gaIE,
  'hr-HR': hrHR,
  'hu-HU': huHU,
  'it-IT': itIT,
  'lt-LT': ltLT,
  'lv-LV': lvLV,
  'mt-MT': mtMT,
  'nb-NO': nbNO,
  'nl-NL': nlNL,
  'pl-PL': plPL,
  'pt-PT': ptPT,
  'ro-RO': roRO,
  'sk-SK': skSK,
  'sl-SI': slSI,
  'sv-SE': svSE,
};

/**
 * Non-hook translation lookup for call sites outside React (plain modules,
 * store logic) that can't use `useT()`. Same fallback chain as the hook:
 * locale catalog → en-GB → raw key. Callers read the current locale from
 * `store.getState().locale.current` and pass it in explicitly.
 */
export function translateForLocale(
  locale: SupportedLocale,
  key: string,
  params?: TranslationParams
): string {
  return renderTranslation(resolveKey(locale, key), locale, params);
}

function resolveKey(locale: SupportedLocale, key: string): string {
  const catalog = locale === DEFAULT_LOCALE ? resolveEn() : CATALOGS[locale];
  const value = catalog?.[key];
  if (value !== undefined) return value;
  // Fallback chain: locale catalog → en-GB → raw key.
  return resolveEn()[key] ?? key;
}

const I18nContext = createContext<I18nContextValue>({
  // Used by components rendered without an <I18nProvider> ancestor (most
  // commonly in tests) — must still support interpolation/plurals so `t()`
  // behaves identically to the provider-backed version, just pinned to
  // `DEFAULT_LOCALE`.
  t: (key: string, params?: TranslationParams) =>
    renderTranslation(resolveEn()[key] ?? key, DEFAULT_LOCALE, params),
  locale: DEFAULT_LOCALE,
});

export function I18nProvider({ children }: { children: ReactNode }) {
  const locale = useAppSelector(state => state.locale.current);

  useEffect(() => {
    try {
      document.documentElement.lang = locale;
    } catch {
      // non-DOM environment (SSR/tests) — nothing to sync.
    }
  }, [locale]);

  const t = useCallback(
    (key: string, params?: TranslationParams): string => {
      const raw = resolveKey(locale, key);
      return renderTranslation(raw, locale, params);
    },
    [locale]
  );

  const value = useMemo(() => ({ t, locale }), [t, locale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useT(): I18nContextValue {
  return useContext(I18nContext);
}

export type { SupportedLocale } from './locales';
export { type Locale } from './types';
