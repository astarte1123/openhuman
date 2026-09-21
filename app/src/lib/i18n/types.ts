import type { SupportedLocale } from './locales';

export type { SupportedLocale } from './locales';

/** @deprecated use `SupportedLocale` — kept as an alias so existing imports keep compiling. */
export type Locale = SupportedLocale;

export interface TranslationMap {
  [key: string]: string;
}

/** Params passed to `t(key, params)` for interpolation and pluralization. */
export type TranslationParams = Record<string, string | number> & {
  /** When present, selects a plural branch via `Intl.PluralRules`. */
  count?: number;
};
