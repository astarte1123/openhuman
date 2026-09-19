import { describe, expect, it } from 'vitest';

import { renderTranslation } from '../translate';

describe('renderTranslation', () => {
  it('returns the raw string unchanged when no params are given', () => {
    expect(renderTranslation('hello {name}', 'en-GB')).toBe('hello {name}');
  });

  it('interpolates a simple placeholder', () => {
    expect(renderTranslation('hello {name}', 'en-GB', { name: 'Aura' })).toBe('hello Aura');
  });

  it('leaves an unmatched placeholder untouched', () => {
    expect(renderTranslation('hello {name}', 'en-GB', { other: 'x' })).toBe('hello {name}');
  });

  it('selects the plural "one" branch for count = 1 in English', () => {
    const raw = '{count, plural, one{1 item} other{{count} items}}';
    expect(renderTranslation(raw, 'en-GB', { count: 1 })).toBe('1 item');
  });

  it('selects the plural "other" branch for count = 5 in English', () => {
    const raw = '{count, plural, one{1 item} other{{count} items}}';
    expect(renderTranslation(raw, 'en-GB', { count: 5 })).toBe('5 items');
  });

  it('falls back to the "other" branch when the locale selects an unlisted category', () => {
    // Polish has "few"/"many" categories with no matching branch here.
    const raw = '{count, plural, one{1 item} other{{count} items}}';
    expect(renderTranslation(raw, 'pl-PL', { count: 3 })).toBe('3 items');
  });
});
