import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock Tauri IPC — not available in jsdom
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

// Mock i18next — t(key) returns the key so tests are locale-agnostic
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      (opts?.defaultValue as string | undefined) ?? key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  Trans: ({ children }: { children: React.ReactNode }) => children,
}));

// Mock i18n index so LanguageSelector doesn't blow up
vi.mock('../i18n/index', () => ({
  default: { language: 'en', changeLanguage: vi.fn() },
  SUPPORTED_LANGUAGES: [{ code: 'en', label: 'English' }, { code: 'it', label: 'Italiano' }],
}));
