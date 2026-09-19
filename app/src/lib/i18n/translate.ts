import type { SupportedLocale } from './locales';
import type { TranslationParams } from './types';

/**
 * Matches a minimal plural block: `{count, plural, one{...} other{...}}`.
 * Only the `plural` selector is supported (no `select`/`selectordinal`) —
 * that's all the catalog needs today. Category bodies may themselves
 * contain `{param}` placeholders, which get interpolated afterward.
 */
// Category bodies may contain one level of nested `{placeholder}` braces
// (e.g. `other{{count} items}`), so the body pattern allows either a
// non-brace char or a fully-nested single-level brace group.
const CATEGORY_BODY = '(?:[^{}]|\\{[^{}]*\\})*';
const PLURAL_BLOCK = new RegExp(`\\{(\\w+),\\s*plural,\\s*((?:\\w+\\{${CATEGORY_BODY}\\}\\s*)+)\\}`);
const PLURAL_CATEGORY = new RegExp(`(\\w+)\\{(${CATEGORY_BODY})\\}`, 'g');

function interpolate(text: string, params: TranslationParams): string {
  return text.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
}

/**
 * Renders a raw catalog string against `params`, resolving one plural
 * block (if present) via `Intl.PluralRules` for `locale`, then
 * interpolating any remaining `{param}` placeholders.
 */
export function renderTranslation(
  raw: string,
  locale: SupportedLocale,
  params?: TranslationParams
): string {
  if (!params) return raw;

  let text = raw;
  const pluralMatch = PLURAL_BLOCK.exec(raw);
  if (pluralMatch) {
    const [fullMatch, countParamName, categoriesRaw] = pluralMatch;
    const count = params[countParamName];
    if (typeof count === 'number') {
      const categories = new Map<string, string>();
      let categoryMatch: RegExpExecArray | null;
      PLURAL_CATEGORY.lastIndex = 0;
      while ((categoryMatch = PLURAL_CATEGORY.exec(categoriesRaw)) !== null) {
        categories.set(categoryMatch[1], categoryMatch[2]);
      }
      let selected: string;
      try {
        selected = new Intl.PluralRules(locale).select(count);
      } catch {
        selected = 'other';
      }
      const branch = categories.get(selected) ?? categories.get('other') ?? '';
      text = raw.slice(0, pluralMatch.index) + branch + raw.slice(pluralMatch.index + fullMatch.length);
    }
  }

  return interpolate(text, params);
}
