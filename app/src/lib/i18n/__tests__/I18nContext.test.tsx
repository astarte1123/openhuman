import { configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it } from 'vitest';

import localeReducer, { setLocale } from '../../../store/localeSlice';
import type { SupportedLocale } from '../locales';
import { I18nProvider, useT } from '../I18nContext';

function Probe() {
  const { locale, t } = useT();

  return (
    <>
      <span data-testid="locale">{locale}</span>
      <span data-testid="known-key">{t('settings.language')}</span>
      <span data-testid="missing-key">{t('this.key.does.not.exist')}</span>
    </>
  );
}

function renderWithLocale(locale: SupportedLocale) {
  const store = configureStore({ reducer: { locale: localeReducer } });
  store.dispatch(setLocale(locale));

  return render(
    <Provider store={store}>
      <I18nProvider>
        <Probe />
      </I18nProvider>
    </Provider>
  );
}

describe('I18nProvider', () => {
  it('resolves a known en-GB key', () => {
    renderWithLocale('en-GB');

    expect(screen.getByTestId('locale')).toHaveTextContent('en-GB');
    expect(screen.getByTestId('known-key').textContent).not.toBe('settings.language');
  });

  it('falls back to the raw key when nothing in the catalog or en-GB matches', () => {
    renderWithLocale('en-GB');

    expect(screen.getByTestId('missing-key')).toHaveTextContent('this.key.does.not.exist');
  });

  it('falls back to en-GB content for a locale with no catalog registered yet', () => {
    renderWithLocale('it-IT');

    expect(screen.getByTestId('locale')).toHaveTextContent('it-IT');
    // it-IT has no catalog yet — must render the en-GB string, not the raw key.
    expect(screen.getByTestId('known-key').textContent).not.toBe('settings.language');
  });

  it('syncs document.documentElement.lang to the active locale', () => {
    renderWithLocale('de-DE');

    expect(document.documentElement.lang).toBe('de-DE');
  });
});
