import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo } from 'react';

import { useAppSelector } from '../../store/hooks';
import { DEFAULT_LOCALE, type SupportedLocale } from './locales';
import { renderTranslation } from './translate';
import type { TranslationMap, TranslationParams } from './types';
import en from './en';

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

// Per-locale catalogs. Only `en-GB` has real content today (the other 24
// locales fall back to it via `resolveKey` below) — this is where a
// locale's catalog module gets registered once its translations land.
const CATALOGS: Partial<Record<SupportedLocale, TranslationMap>> = {
  'en-GB': undefined, // populated lazily via resolveEn() so CJS/ESM interop above still applies
};

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
