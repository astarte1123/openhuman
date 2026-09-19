import { afterEach, describe, expect, it, vi } from 'vitest';

async function loadReducer() {
  vi.resetModules();
  const mod = await import('./localeSlice');
  return mod;
}

describe('localeSlice', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('detects a supported locale from navigator.languages, in order', async () => {
    vi.stubGlobal('navigator', { languages: ['fr-FR', 'en-US'], language: 'fr-FR' });
    const { default: reducer } = await loadReducer();

    expect(reducer(undefined, { type: '@@INIT' }).current).toBe('fr-FR');
  });

  it('maps a regional variant to its canonical supported locale', async () => {
    vi.stubGlobal('navigator', { languages: ['de-AT'], language: 'de-AT' });
    const { default: reducer } = await loadReducer();

    expect(reducer(undefined, { type: '@@INIT' }).current).toBe('de-DE');
  });

  it('falls back to en-GB when nothing matches', async () => {
    vi.stubGlobal('navigator', { languages: ['ja-JP'], language: 'ja-JP' });
    const { default: reducer } = await loadReducer();

    expect(reducer(undefined, { type: '@@INIT' }).current).toBe('en-GB');
  });

  it('falls back to en-GB when navigator is unavailable', async () => {
    vi.stubGlobal('navigator', undefined);
    const { default: reducer } = await loadReducer();

    expect(reducer(undefined, { type: '@@INIT' }).current).toBe('en-GB');
  });

  it('marks a locale set via setLocale as explicit', async () => {
    const { default: reducer, setLocale } = await loadReducer();

    const next = reducer(undefined, setLocale('it-IT'));
    expect(next.current).toBe('it-IT');
    expect(next.isExplicit).toBe(true);
  });

  it('never lets applyDetectedLocale override an explicit choice', async () => {
    const { default: reducer, setLocale, applyDetectedLocale } = await loadReducer();

    const explicit = reducer(undefined, setLocale('it-IT'));
    const afterDetect = reducer(explicit, applyDetectedLocale('de-DE'));

    expect(afterDetect.current).toBe('it-IT');
  });

  it('lets applyDetectedLocale update state when no explicit choice was made', async () => {
    const { default: reducer, applyDetectedLocale } = await loadReducer();

    const initial = reducer(undefined, { type: '@@INIT' });
    const next = reducer(initial, applyDetectedLocale('de-DE'));

    expect(next.current).toBe('de-DE');
    expect(next.isExplicit).toBe(false);
  });
});
