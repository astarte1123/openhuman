import { SUPPORTED_LOCALES } from '../lib/i18n/locales';
import { useAppDispatch, useAppSelector } from '../store/hooks';
import { setLocale } from '../store/localeSlice';
import { auraUpdateLocale } from '../utils/tauriCommands/config';
import { isTauri } from '../services/webviewAccountService';

interface LanguageSelectProps {
  /** Accessible label for the underlying <select>. */
  ariaLabel?: string;
  /** Optional id for label association. */
  id?: string;
  /** Override the default classNames if a host needs different padding/size. */
  className?: string;
}

const DEFAULT_CLASS =
  "appearance-none rounded-lg border border-stone-200 bg-white bg-[url('data:image/svg+xml;utf8,<svg%20xmlns=%22http://www.w3.org/2000/svg%22%20viewBox=%220%200%2020%2020%22%20fill=%22%2378716c%22><path%20d=%22M5.293%207.293a1%201%200%20011.414%200L10%2010.586l3.293-3.293a1%201%200%20111.414%201.414l-4%204a1%201%200%2001-1.414%200l-4-4a1%201%200%20010-1.414z%22/></svg>')] bg-no-repeat bg-[right_0.5rem_center] bg-[length:1rem_1rem] py-2 pl-3 pr-8 text-xs font-medium text-stone-700 hover:border-stone-300 hover:bg-stone-50 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 cursor-pointer";

/**
 * Shared language picker used by the boot-check gate and the Settings home
 * screen. Each language is shown by its own native name (e.g. "Italiano",
 * "Deutsch") — never translated — and dispatches `setLocale` (an explicit
 * pick, persisted and never auto-overridden) to the redux store on change.
 *
 * Also persists the choice to the core via `aura.config_update_locale` —
 * the redux store only drives UI strings; the core's `config.locale` is
 * what AI response language, STT/TTS language hints, and localized
 * notifications actually read. Without this call the two drift and the
 * app silently keeps responding in English regardless of the UI language.
 */
const LanguageSelect = ({ ariaLabel = 'Language', id, className }: LanguageSelectProps) => {
  const dispatch = useAppDispatch();
  const current = useAppSelector(state => state.locale.current);

  const handleChange = (code: (typeof SUPPORTED_LOCALES)[number]['code']) => {
    dispatch(setLocale(code));
    if (isTauri()) {
      auraUpdateLocale({ locale: code }).catch(err => {
        console.error('[LanguageSelect] failed to persist locale to core', err);
      });
    }
  };

  return (
    <select
      id={id}
      value={current}
      onChange={e => handleChange(e.target.value as (typeof SUPPORTED_LOCALES)[number]['code'])}
      aria-label={ariaLabel}
      className={className ?? DEFAULT_CLASS}>
      {SUPPORTED_LOCALES.map(locale => (
        <option key={locale.code} value={locale.code}>
          {locale.nativeName}
        </option>
      ))}
    </select>
  );
};

export default LanguageSelect;
