import type { SupportedLocale } from './locales';

/**
 * Locale-aware number formatting for UI counts.
 *
 * Uses the active app locale rather than the host runtime default so grouped
 * digits (e.g. "1,234" vs "1.234") match the language the UI renders in.
 * Never throws — falls back to the plain string form.
 */
export function formatNumber(value: number, locale: SupportedLocale): string {
  try {
    return new Intl.NumberFormat(locale).format(value);
  } catch {
    return String(value);
  }
}
