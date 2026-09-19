import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import { DEFAULT_LOCALE, resolveFirstSupportedLocale, type SupportedLocale } from '../lib/i18n/locales';

/**
 * First-run detection cascade (only runs when there's no persisted state
 * yet — redux-persist rehydration overrides `initialState` on every
 * subsequent load, so an explicit or previously-detected choice is never
 * clobbered by this function again):
 *   1. `navigator.languages` in order (OS/browser preference list)
 *   2. `navigator.language` (single primary tag)
 *   3. `DEFAULT_LOCALE` ("en-GB")
 */
function detectLocale(): SupportedLocale {
  try {
    const nav = navigator as { languages?: readonly string[]; language?: string };
    if (nav.languages && nav.languages.length > 0) {
      return resolveFirstSupportedLocale(nav.languages);
    }
    if (nav.language) {
      return resolveFirstSupportedLocale([nav.language]);
    }
  } catch {
    // browser API unavailable (non-DOM environment).
  }
  return DEFAULT_LOCALE;
}

interface LocaleState {
  current: SupportedLocale;
  /**
   * True once the user has explicitly picked a language (vs. it having
   * been auto-detected). Consumers that might otherwise re-run detection
   * (e.g. after a fresh sign-in) must check this before overwriting
   * `current`.
   */
  isExplicit: boolean;
}

const initialState: LocaleState = { current: detectLocale(), isExplicit: false };

const localeSlice = createSlice({
  name: 'locale',
  initialState,
  reducers: {
    /** Explicit user pick — from the language selector, or a synced backend preference. */
    setLocale(state, action: PayloadAction<SupportedLocale>) {
      state.current = action.payload;
      state.isExplicit = true;
    },
    /**
     * Applies an auto-detected/OS-derived locale without marking it
     * explicit — used when re-detecting is appropriate (e.g. no signed-in
     * preference exists yet) and must never override an explicit choice.
     */
    applyDetectedLocale(state, action: PayloadAction<SupportedLocale>) {
      if (state.isExplicit) return;
      state.current = action.payload;
    },
  },
});

export const { setLocale, applyDetectedLocale } = localeSlice.actions;
export { detectLocale };
export default localeSlice.reducer;
