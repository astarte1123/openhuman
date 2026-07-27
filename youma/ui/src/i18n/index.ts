import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import en from './locales/en.json';
import it from './locales/it.json';
import fr from './locales/fr.json';
import de from './locales/de.json';
import es from './locales/es.json';
import pt from './locales/pt.json';
import nl from './locales/nl.json';
import pl from './locales/pl.json';
import ro from './locales/ro.json';
import cs from './locales/cs.json';
import sk from './locales/sk.json';
import hu from './locales/hu.json';
import sv from './locales/sv.json';
import da from './locales/da.json';
import fi from './locales/fi.json';
import nb from './locales/nb.json';
import el from './locales/el.json';
import hr from './locales/hr.json';
import bg from './locales/bg.json';
import et from './locales/et.json';
import lv from './locales/lv.json';
import lt from './locales/lt.json';
import sl from './locales/sl.json';
import mt from './locales/mt.json';
import ga from './locales/ga.json';

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      it: { translation: it },
      fr: { translation: fr },
      de: { translation: de },
      es: { translation: es },
      pt: { translation: pt },
      nl: { translation: nl },
      pl: { translation: pl },
      ro: { translation: ro },
      cs: { translation: cs },
      sk: { translation: sk },
      hu: { translation: hu },
      sv: { translation: sv },
      da: { translation: da },
      fi: { translation: fi },
      nb: { translation: nb },
      el: { translation: el },
      hr: { translation: hr },
      bg: { translation: bg },
      et: { translation: et },
      lv: { translation: lv },
      lt: { translation: lt },
      sl: { translation: sl },
      mt: { translation: mt },
      ga: { translation: ga },
    },
    fallbackLng: 'en',
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'youma_language',
    },
    interpolation: { escapeValue: false },
  });

export default i18n;

export const SUPPORTED_LANGUAGES = [
  { code: 'en', label: 'English' },
  { code: 'it', label: 'Italiano' },
  { code: 'fr', label: 'Français' },
  { code: 'de', label: 'Deutsch' },
  { code: 'es', label: 'Español' },
  { code: 'pt', label: 'Português' },
  { code: 'nl', label: 'Nederlands' },
  { code: 'pl', label: 'Polski' },
  { code: 'ro', label: 'Română' },
  { code: 'cs', label: 'Čeština' },
  { code: 'sk', label: 'Slovenčina' },
  { code: 'hu', label: 'Magyar' },
  { code: 'sv', label: 'Svenska' },
  { code: 'da', label: 'Dansk' },
  { code: 'fi', label: 'Suomi' },
  { code: 'nb', label: 'Norsk bokmål' },
  { code: 'el', label: 'Ελληνικά' },
  { code: 'hr', label: 'Hrvatski' },
  { code: 'bg', label: 'Български' },
  { code: 'et', label: 'Eesti' },
  { code: 'lv', label: 'Latviešu' },
  { code: 'lt', label: 'Lietuvių' },
  { code: 'sl', label: 'Slovenščina' },
  { code: 'mt', label: 'Malti' },
  { code: 'ga', label: 'Gaeilge' },
] as const;
