import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import i18n, { SUPPORTED_LANGUAGES } from '../i18n/index';

import { setEnvVar, configGet, resetAllData, embeddingConfigGet, embeddingConfigSet, embeddingTest, settingsGet, settingsSet, envGet, envIsSet, licenseStatus, licenseDeactivate, licenseServerUrlSet, listSkills, toggleSkill, type LicenseStatus, type Skill } from '../api';
import { DEFAULT_THINKING_PHRASES, MILANESE_THINKING_PHRASES } from '../components/ThinkingWords';

// ---------------------------------------------------------------------------
// Settings page
// API keys, model selection, Ollama URL
// ---------------------------------------------------------------------------

interface KeyField {
  envKey: string;
  label: string;
  placeholder: string;
  helpText: string;
}

export function ApiKeyCard({ field, onSaved, isActive }: { field: KeyField; onSaved?: () => void; isActive?: boolean }) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [show, setShow] = useState(false);
  const [hasKey, setHasKey] = useState(false);

  useEffect(() => {
    // Read directly from process env (youma.env is loaded at startup — reliable source of truth).
    envIsSet(field.envKey).then(setHasKey).catch(() => {});
  }, [field.envKey]);

  async function handleSave() {
    if (!value.trim()) return;
    setSaving(true); setError(null); setSaved(false);
    try {
      await setEnvVar(field.envKey, value.trim());
      setHasKey(true);
      setValue('');
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
      onSaved?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleRemove() {
    setSaving(true); setError(null);
    try {
      await setEnvVar(field.envKey, '');
      setHasKey(false);
      setValue('');
      onSaved?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5">
      <div className="flex items-start justify-between mb-1">
        <h3 className="text-sm font-semibold text-stone-900">{field.label}</h3>
        {saved ? (
          <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">
            {t('settings.saved')}
          </span>
        ) : isActive ? (
          <span className="text-xs font-medium text-primary-600 bg-primary-50 border border-primary-200 rounded-full px-2.5 py-0.5">
            {t('settings.key_active', { defaultValue: 'Active' })}
          </span>
        ) : hasKey ? (
          <span className="text-xs font-medium text-stone-500 bg-stone-100 border border-stone-200 rounded-full px-2.5 py-0.5">
            {t('settings.key_set', { defaultValue: 'Key saved' })}
          </span>
        ) : null}
      </div>
      <p className="text-xs text-stone-400 mb-3">{field.helpText}</p>
      <div className="space-y-2">
        <div className="relative">
          <input
            type={show ? 'text' : 'password'}
            aria-label={field.label}
            value={value}
            onChange={e => setValue(e.target.value)}
            placeholder={hasKey ? t('settings.key_replace_placeholder', { defaultValue: '••••••••••••  (enter new key to replace)' }) : field.placeholder}
            className="w-full px-4 py-2.5 pr-10 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 placeholder-stone-400 text-sm transition-colors [&::-ms-reveal]:hidden [&::-ms-clear]:hidden"
            onKeyDown={e => { if (e.key === 'Enter') handleSave(); }}
          />
          {value && (
            <button
              type="button"
              aria-label={show ? t('settings.hide_key') : t('settings.show_key')}
              onClick={() => setShow(s => !s)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-stone-400 hover:text-stone-600"
              tabIndex={-1}
            >
              {show ? (
                // Visible → click to hide → show crossed eye
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
                </svg>
              ) : (
                // Hidden → click to show → show open eye
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                </svg>
              )}
            </button>
          )}
        </div>
        <div className="flex justify-end gap-2">
          {hasKey && (
            <button
              type="button"
              onClick={handleRemove}
              disabled={saving}
              className="px-4 py-2 text-coral-600 hover:text-coral-700 border border-coral-200 hover:border-coral-300 bg-white hover:bg-coral-50 disabled:opacity-50 text-sm font-medium rounded-lg transition-colors"
            >
              {t('settings.key_remove', { defaultValue: 'Remove' })}
            </button>
          )}
          <button
            type="button"
            onClick={handleSave}
            disabled={!value.trim() || saving}
            className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
          >
            {saving ? '…' : t('settings.save')}
          </button>
        </div>
      </div>
      {error && <p className="mt-2 text-xs text-coral-500">{error}</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Simple text env-var field
// ---------------------------------------------------------------------------

export function EnvField({
  envKey,
  label,
  placeholder,
  helpText,
  defaultValue = '',
  onSaved,
}: {
  envKey: string;
  label: string;
  placeholder: string;
  helpText: string;
  defaultValue?: string;
  onSaved?: () => void;
}) {
  const { t } = useTranslation();
  const settingsKey = `env.${envKey}`;
  const [value, setValue] = useState(defaultValue);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load from process env (youma.env is read at startup — reliable source of truth).
  useEffect(() => {
    envGet(envKey).then(v => { if (v) setValue(v); }).catch(() => {});
  }, [envKey]);

  async function handleSave() {
    setSaving(true); setError(null); setSaved(false);
    try {
      await Promise.all([
        setEnvVar(envKey, value.trim()),
        settingsSet(settingsKey, value.trim()),
      ]);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
      onSaved?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleClear() {
    setSaving(true); setError(null);
    try {
      await Promise.all([
        setEnvVar(envKey, ''),
        settingsSet(settingsKey, ''),
      ]);
      setValue('');
      onSaved?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5">
      <div className="flex items-start justify-between mb-1">
        <h3 className="text-sm font-semibold text-stone-900">{label}</h3>
        {saved && (
          <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">
            {t('settings.saved')}
          </span>
        )}
      </div>
      <p className="text-xs text-stone-400 mb-3">{helpText}</p>
      <div className="space-y-2">
        <input
          type="text"
          aria-label={label}
          value={value}
          onChange={e => setValue(e.target.value)}
          placeholder={placeholder}
          className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 placeholder-stone-400 text-sm transition-colors"
          onKeyDown={e => { if (e.key === 'Enter') handleSave(); }}
        />
        <div className="flex justify-end gap-2">
          {value && (
            <button
              type="button"
              onClick={handleClear}
              disabled={saving}
              className="px-4 py-2 text-coral-600 hover:text-coral-700 border border-coral-200 hover:border-coral-300 bg-white hover:bg-coral-50 disabled:opacity-50 text-sm font-medium rounded-lg transition-colors"
            >
              {t('settings.clear', { defaultValue: 'Clear' })}
            </button>
          )}
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
          >
            {saving ? '…' : t('settings.save')}
          </button>
        </div>
      </div>
      {error && <p className="mt-2 text-xs text-coral-500">{error}</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Voice section — TTS provider + STT language dropdowns (auto-save)
// ---------------------------------------------------------------------------

const TTS_PROVIDERS = [
  { value: 'native', label: 'Native (sistema operativo)' },
  { value: 'elevenlabs', label: 'ElevenLabs' },
  { value: 'openai', label: 'OpenAI TTS' },
];

const STT_LANGUAGES = [
  { code: 'it-IT', label: 'Italiano' },
  { code: 'en-US', label: 'English (US)' },
  { code: 'en-GB', label: 'English (UK)' },
  { code: 'fr-FR', label: 'Français' },
  { code: 'de-DE', label: 'Deutsch' },
  { code: 'es-ES', label: 'Español' },
  { code: 'pt-BR', label: 'Português (BR)' },
  { code: 'nl-NL', label: 'Nederlands' },
  { code: 'ja-JP', label: '日本語' },
  { code: 'zh-CN', label: '中文 (简体)' },
];

function VoiceSection() {
  const { t } = useTranslation();
  const [ttsProvider, setTtsProvider] = useState('native');
  const [sttLang, setSttLang] = useState('it-IT');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      settingsGet('env.TTS_PROVIDER'),
      settingsGet('env.STT_LANG'),
    ]).then(([tts, stt]) => {
      if (tts) setTtsProvider(tts);
      if (stt) setSttLang(stt);
    }).catch(() => {});
  }, []);

  async function save(tts: string, stt: string) {
    setSaving(true); setError(null); setSaved(false);
    try {
      await Promise.all([
        setEnvVar('TTS_PROVIDER', tts),
        settingsSet('env.TTS_PROVIDER', tts),
        setEnvVar('STT_LANG', stt),
        settingsSet('env.STT_LANG', stt),
      ]);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  function handleTtsChange(val: string) {
    setTtsProvider(val);
    void save(val, sttLang);
  }

  function handleSttChange(val: string) {
    setSttLang(val);
    void save(ttsProvider, val);
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5 space-y-4">
      <div className="flex items-start justify-between">
        <h3 className="text-sm font-semibold text-stone-900">{t('settings.section_voice')}</h3>
        <div className="flex items-center gap-2">
          {saving && <span className="text-xs text-stone-400">…</span>}
          {saved && (
            <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">
              {t('settings.saved')}
            </span>
          )}
        </div>
      </div>

      <div>
        <label htmlFor="tts-provider" className="block text-xs font-medium text-stone-700 mb-1">
          {t('settings.tts_provider_label')}
        </label>
        <p className="text-xs text-stone-400 mb-2">{t('settings.tts_provider_help')}</p>
        <select
          id="tts-provider"
          value={ttsProvider}
          onChange={e => handleTtsChange(e.target.value)}
          disabled={saving}
          className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors disabled:opacity-60"
        >
          {TTS_PROVIDERS.map(p => (
            <option key={p.value} value={p.value}>{p.label}</option>
          ))}
        </select>
      </div>

      <div>
        <label htmlFor="stt-lang" className="block text-xs font-medium text-stone-700 mb-1">
          {t('settings.stt_lang_label')}
        </label>
        <p className="text-xs text-stone-400 mb-2">{t('settings.stt_lang_help')}</p>
        <select
          id="stt-lang"
          value={sttLang}
          onChange={e => handleSttChange(e.target.value)}
          disabled={saving}
          className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors disabled:opacity-60"
        >
          {STT_LANGUAGES.map(l => (
            <option key={l.code} value={l.code}>{l.label}</option>
          ))}
        </select>
      </div>

      {error && <p className="text-xs text-coral-500">{error}</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Model selector
// ---------------------------------------------------------------------------

const ANTHROPIC_MODELS = [
  { value: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6 (default)' },
  { value: 'claude-opus-4-7', label: 'Claude Opus 4.7 (powerful)' },
  { value: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5 (fast)' },
];

const OPENAI_MODELS = [
  { value: 'gpt-4o', label: 'GPT-4o (default)' },
  { value: 'gpt-4o-mini', label: 'GPT-4o Mini (fast)' },
  { value: 'o1', label: 'o1 (reasoning)' },
];

const DEEPSEEK_MODELS = [
  { value: 'deepseek-chat', label: 'DeepSeek Chat (default)' },
  { value: 'deepseek-reasoner', label: 'DeepSeek Reasoner (R1)' },
];

function ModelSelector({ activeProvider }: { activeProvider?: string }) {
  const { t } = useTranslation();
  const [model, setModel] = useState('');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const allModels = [...ANTHROPIC_MODELS, ...OPENAI_MODELS, ...DEEPSEEK_MODELS];

  const hasKey = (provider: string) => activeProvider === provider;

  function providerLabel(name: string, provider: string) {
    return hasKey(provider) ? `${name} ✓` : `${name} — ${t('settings.key_missing', { defaultValue: 'chiave non configurata' })}`;
  }

  useEffect(() => {
    configGet().then(cfg => { if (cfg.model) setModel(cfg.model); }).catch(() => {});
  }, []);

  async function handleSave(val: string) {
    setModel(val);
    setSaving(true); setSaved(false); setError(null);
    try {
      await setEnvVar('YOUMA_MODEL', val);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5">
      <div className="flex items-start justify-between mb-1">
        <h3 className="text-sm font-semibold text-stone-900">{t('settings.model_override_title')}</h3>
        {saved && (
          <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">
            {t('settings.saved')}
          </span>
        )}
      </div>
      <p className="text-xs text-stone-400 mb-3">{t('settings.model_override_desc')}</p>
      <select
        aria-label={t('settings.model_override_title')}
        value={model}
        onChange={e => handleSave(e.target.value)}
        disabled={saving}
        className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
      >
        <option value="">{t('settings.model_default')}</option>
        <optgroup label={providerLabel('Anthropic', 'anthropic')}>
          {ANTHROPIC_MODELS.map(m => (
            <option key={m.value} value={m.value}>{m.label}</option>
          ))}
        </optgroup>
        <optgroup label={providerLabel('OpenAI', 'openai')}>
          {OPENAI_MODELS.map(m => (
            <option key={m.value} value={m.value}>{m.label}</option>
          ))}
        </optgroup>
        <optgroup label={providerLabel('DeepSeek', 'deepseek')}>
          {DEEPSEEK_MODELS.map(m => (
            <option key={m.value} value={m.value}>{m.label}</option>
          ))}
        </optgroup>
        {allModels.find(m => m.value === model) ? null : model ? (
          <option value={model}>{model} (custom)</option>
        ) : null}
      </select>
      {activeProvider && (
        <p className="mt-2 text-xs text-stone-400">
          {t('settings.active_provider', { defaultValue: 'Provider attivo' })}: <span className="font-medium text-sage-600 capitalize">{activeProvider}</span>
        </p>
      )}
      {error && <p className="mt-2 text-xs text-coral-500">{error}</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Language selector
// ---------------------------------------------------------------------------

function LanguageSelector() {
  const { t } = useTranslation();
  const [current, setCurrent] = useState(i18n.language?.slice(0, 2) ?? 'en');

  function handleChange(code: string) {
    setCurrent(code);
    void i18n.changeLanguage(code);
    // Keep meetings default lang in sync — user can still override it in Meetings settings.
    void settingsSet('meetings.default_lang', code).catch(() => {});
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5">
      <h3 className="text-sm font-semibold text-stone-900 mb-1">{t('settings.language')}</h3>
      <p className="text-xs text-stone-400 mb-3">{t('settings.language_desc')}</p>
      <select
        aria-label={t('settings.language')}
        value={current}
        onChange={e => handleChange(e.target.value)}
        className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
      >
        {SUPPORTED_LANGUAGES.map(lang => (
          <option key={lang.code} value={lang.code}>{lang.label}</option>
        ))}
      </select>
    </div>
  );
}


// ---------------------------------------------------------------------------
// Privacy section
// ---------------------------------------------------------------------------

const ALLOWED_LOGO_EXTENSIONS: Record<string, string> = {
  svg: 'image/svg+xml',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  png: 'image/png',
};
const ALLOWED_LOGO_MIME = new Set(['image/svg+xml', 'image/jpeg', 'image/png']);
const MAX_LOGO_BYTES = 2 * 1024 * 1024; // 2 MB

// Windows drag-drop often reports file.type as '' for SVG — fall back to extension.
function resolveLogoMime(file: File): string {
  if (file.type && ALLOWED_LOGO_MIME.has(file.type)) return file.type;
  const ext = file.name.split('.').pop()?.toLowerCase() ?? '';
  return ALLOWED_LOGO_EXTENSIONS[ext] ?? file.type;
}

function LogoUploadSection() {
  const { t } = useTranslation();
  const [logo, setLogo] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [logoError, setLogoError] = useState<string | null>(null);
  const [logoSaved, setLogoSaved] = useState(false);

  useEffect(() => {
    settingsGet('app.custom_logo').then(v => { if (v) setLogo(v); }).catch(() => {});
  }, []);

  function readFile(file: File) {
    setLogoError(null);
    const mime = resolveLogoMime(file);
    if (!ALLOWED_LOGO_MIME.has(mime)) {
      setLogoError(t('settings.logo_invalid_type'));
      return;
    }
    if (file.size > MAX_LOGO_BYTES) {
      setLogoError(t('settings.logo_too_large'));
      return;
    }
    const reader = new FileReader();
    reader.onload = async e => {
      // For SVG readAsDataURL produces data:image/svg+xml;base64,... which <img> handles fine.
      // Reconstruct a clean data URL using the resolved MIME in case browser left type blank.
      let dataUrl = e.target?.result as string;
      if (dataUrl && !dataUrl.startsWith(`data:${mime}`)) {
        // Strip whatever prefix the browser produced and replace with the correct MIME
        const base64 = dataUrl.split(',')[1] ?? '';
        dataUrl = `data:${mime};base64,${base64}`;
      }
      setLogo(dataUrl);
      try {
        await settingsSet('app.custom_logo', dataUrl);
        setLogoSaved(true);
        setTimeout(() => setLogoSaved(false), 3000);
      } catch (err) {
        setLogoError(String(err));
      }
    };
    reader.readAsDataURL(file);
  }

  function onDrop(e: React.DragEvent) {
    e.preventDefault();
    setDragging(false);
    const file = e.dataTransfer.files[0];
    if (file) readFile(file);
  }

  async function removeLogo() {
    setLogo(null);
    setLogoError(null);
    try {
      await settingsSet('app.custom_logo', '');
    } catch {}
  }

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-4">
      <div className="flex items-center justify-between mb-1">
        <p className="text-sm font-medium text-stone-800">{t('settings.logo_title')}</p>
        {logoSaved && (
          <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">{t('settings.saved')}</span>
        )}
      </div>
      <p className="text-xs text-stone-500 mb-3">{t('settings.logo_desc')}</p>

      {logo ? (
        <div className="flex items-center gap-4">
          {/* Fixed-size container so SVGs with no intrinsic size still render */}
          <div className="flex-shrink-0 w-32 h-16 rounded-lg border border-stone-200 bg-stone-50 flex items-center justify-center overflow-hidden p-1">
            <img
              src={logo}
              alt="Custom logo preview"
              className="w-full h-full object-contain"
            />
          </div>
          <button
            type="button"
            onClick={removeLogo}
            className="text-xs text-coral-500 hover:text-coral-700 font-medium transition-colors"
          >
            {t('settings.logo_remove')}
          </button>
        </div>
      ) : (
        <label
          className={`flex flex-col items-center justify-center w-full min-h-[80px] rounded-xl border-2 border-dashed cursor-pointer transition-colors ${
            dragging
              ? 'border-primary-400 bg-primary-50'
              : 'border-stone-300 bg-stone-50 hover:border-primary-300 hover:bg-primary-50/40'
          }`}
          onDragOver={e => { e.preventDefault(); setDragging(true); }}
          onDragLeave={() => setDragging(false)}
          onDrop={onDrop}
        >
          <input
            type="file"
            accept=".svg,.jpg,.jpeg,.png"
            className="sr-only"
            onChange={e => { const f = e.target.files?.[0]; if (f) readFile(f); e.target.value = ''; }}
          />
          <svg className="w-6 h-6 text-stone-400 mb-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 16M14 8h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
          <span className="text-xs text-stone-500">{t('settings.logo_drop')}</span>
          <span className="text-[10px] text-stone-400 mt-0.5">SVG · JPEG · PNG · max 2 MB</span>
        </label>
      )}
      {logoError && <p className="mt-2 text-xs text-coral-500">{logoError}</p>}
    </div>
  );
}

function PrivacySection() {
  const { t } = useTranslation();
  const [resetting, setResetting] = useState(false);
  const [resetDone, setResetDone] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleReset() {
    if (!confirm) { setConfirm(true); return; }
    setResetting(true); setError(null);
    try {
      await resetAllData();
      setResetDone(true);
      setConfirm(false);
    } catch (e) { setError(String(e)); }
    finally { setResetting(false); }
  }

  return (
    <section>
      <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_privacy')}</p>
      <div className="space-y-3">
        {/* Custom logo */}
        <LogoUploadSection />

        {/* Encryption badge */}
        <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-3 flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-sage-50 border border-sage-200 flex items-center justify-center flex-shrink-0">
            <svg className="w-4 h-4 text-sage-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
            </svg>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-stone-800">{t('settings.encrypted')}</p>
            <p className="text-xs text-stone-500">{t('settings.encrypted_desc')}</p>
          </div>
          <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5 flex-shrink-0">{t('settings.active_badge')}</span>
        </div>

        {/* Data storage info */}
        <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-3">
          <p className="text-sm font-medium text-stone-800 mb-1">{t('settings.local_storage_title')}</p>
          <p className="text-xs text-stone-500 leading-relaxed">
            {t('settings.local_storage_desc', { file: 'youma.env' })}
          </p>
        </div>

        {/* Reset all data */}
        <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-3">
          <div className="flex items-center justify-between">
            <div className="flex-1 min-w-0 mr-4">
              <p className="text-sm font-medium text-stone-800">{t('settings.reset_title')}</p>
              <p className="text-xs text-stone-500 mt-0.5">{t('settings.reset_desc')}</p>
            </div>
            {resetDone ? (
              <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-1 flex-shrink-0">{t('settings.reset_cleared')}</span>
            ) : (
              <button
                type="button"
                onClick={handleReset}
                disabled={resetting}
                className={`flex-shrink-0 px-3 py-1.5 text-xs font-medium rounded-lg transition-colors disabled:opacity-40 ${
                  confirm
                    ? 'bg-coral-500 hover:bg-coral-600 text-white'
                    : 'bg-stone-100 hover:bg-stone-200 text-stone-700'
                }`}
              >
                {resetting ? t('settings.reset_clearing') : confirm ? t('settings.reset_confirm') : t('settings.reset_btn')}
              </button>
            )}
          </div>
          {error && <p className="mt-2 text-xs text-coral-500">{error}</p>}
          {confirm && !resetting && (
            <p className="mt-2 text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded-lg px-3 py-1.5">
              {t('settings.reset_warning')}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Embedding section
// ---------------------------------------------------------------------------

const DEFAULT_MODELS: Record<string, string> = {
  ollama: 'nomic-embed-text',
  openai: 'text-embedding-3-small',
  deepseek: 'deepseek-embedding',
  custom: '',
  none: '',
};

const DEFAULT_URLS: Record<string, string> = {
  ollama: 'http://localhost:11434',
  custom: 'http://localhost:11434',
};

function EmbeddingSection() {
  const { t } = useTranslation();
  const EMBEDDING_PROVIDERS = [
    { value: 'none', label: t('settings.emb_none') },
    { value: 'ollama', label: t('settings.emb_ollama') },
    { value: 'openai', label: t('settings.emb_openai') },
    { value: 'deepseek', label: t('settings.emb_deepseek') },
    { value: 'custom', label: t('settings.emb_custom') },
  ];
  const [provider, setProvider] = useState('none');
  const [model, setModel] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    embeddingConfigGet().then(cfg => {
      setProvider(cfg.provider || 'none');
      setModel(cfg.model || '');
      setBaseUrl(cfg.base_url || '');
      setLoaded(true);
    }).catch(() => setLoaded(true));
  }, []);

  function handleProviderChange(val: string) {
    setProvider(val);
    setModel(DEFAULT_MODELS[val] ?? '');
    setBaseUrl(DEFAULT_URLS[val] ?? '');
    setTestResult(null);
  }

  async function handleSave() {
    setSaving(true); setError(null); setSaved(false);
    try {
      await embeddingConfigSet({
        provider,
        model,
        ...(baseUrl ? { base_url: baseUrl } : {}),
        ...(apiKey ? { api_key: apiKey } : {}),
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleTest() {
    setTesting(true); setTestResult(null);
    try {
      const res = await embeddingTest();
      setTestResult(
        res.ok
          ? { ok: true, msg: `OK — ${res.dims} dims · ${res.provider}/${res.model}` }
          : { ok: false, msg: res.error ?? 'Test failed' },
      );
    } catch (e) {
      setTestResult({ ok: false, msg: String(e) });
    } finally {
      setTesting(false);
    }
  }

  const needsUrl = provider === 'ollama' || provider === 'custom';
  const needsKey = provider === 'openai' || provider === 'deepseek' || provider === 'custom';

  if (!loaded) return null;

  return (
    <section>
      <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">
        {t('settings.section_embeddings')}
      </p>
      <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5 space-y-4">
        <p className="text-xs text-stone-400 leading-relaxed">{t('settings.emb_desc')}</p>

        {/* Provider */}
        <div>
          <label htmlFor="emb-provider" className="block text-xs font-medium text-stone-600 mb-1">{t('settings.provider')}</label>
          <select
            id="emb-provider"
            value={provider}
            onChange={e => handleProviderChange(e.target.value)}
            className="w-full px-3 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
          >
            {EMBEDDING_PROVIDERS.map(p => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
        </div>

        {provider !== 'none' && (
          <>
            {/* Model */}
            <div>
              <label className="block text-xs font-medium text-stone-600 mb-1">{t('settings.model')}</label>
              <input
                type="text"
                value={model}
                onChange={e => setModel(e.target.value)}
                placeholder={DEFAULT_MODELS[provider] || 'model-name'}
                className="w-full px-3 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
              />
            </div>

            {/* Base URL */}
            {needsUrl && (
              <div>
                <label className="block text-xs font-medium text-stone-600 mb-1">{t('settings.base_url')}</label>
                <input
                  type="text"
                  value={baseUrl}
                  onChange={e => setBaseUrl(e.target.value)}
                  placeholder={DEFAULT_URLS[provider] || 'http://…'}
                  className="w-full px-3 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
                />
              </div>
            )}

            {/* API Key */}
            {needsKey && (
              <div>
                <label className="block text-xs font-medium text-stone-600 mb-1">
                  {t('settings.api_key')} <span className="text-stone-300">{t('settings.api_key_optional')}</span>
                </label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={e => setApiKey(e.target.value)}
                  placeholder="sk-…"
                  className="w-full px-3 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors"
                />
              </div>
            )}
          </>
        )}

        {/* Actions */}
        <div className="flex items-center gap-2 pt-1">
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white text-sm font-medium rounded-lg transition-colors"
          >
            {saving ? t('settings.saving') : t('settings.save')}
          </button>
          {provider !== 'none' && (
            <button
              type="button"
              onClick={handleTest}
              disabled={testing}
              className="px-4 py-2 bg-stone-100 hover:bg-stone-200 disabled:opacity-50 text-stone-700 text-sm font-medium rounded-lg transition-colors"
            >
              {testing ? t('settings.testing') : t('settings.test')}
            </button>
          )}
          {saved && (
            <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">
              {t('settings.saved')}
            </span>
          )}
        </div>

        {error && <p className="text-xs text-coral-500">{error}</p>}
        {testResult && (
          <p className={`text-xs font-medium ${testResult.ok ? 'text-sage-600' : 'text-coral-500'}`}>
            {testResult.ok ? '✓ ' : '✗ '}{testResult.msg}
          </p>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Skills settings section
// ---------------------------------------------------------------------------

const SKILL_CATEGORY_ICONS: Record<string, string> = {
  tools: '🔧', voice: '🎙️', intelligence: '🧠', interface: '✨', ai: '🤖',
};

function SkillsSection() {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [toggling, setToggling] = useState<string | null>(null);

  useEffect(() => {
    listSkills()
      .then(data => setSkills(data))
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function handleToggle(skill: Skill) {
    setToggling(skill.id);
    try {
      const updated = await toggleSkill(skill.id, !skill.enabled);
      setSkills(prev => prev.map(s => s.id === updated.id ? updated : s));
    } catch (e) { setError(String(e)); }
    finally { setToggling(null); }
  }

  if (loading) {
    return <div className="space-y-3">{[1, 2, 3, 4].map(i => <div key={i} className="h-20 bg-stone-200 rounded-xl animate-pulse" />)}</div>;
  }

  return (
    <div className="space-y-2">
      {error && (
        <div className="px-4 py-2 bg-red-50 border border-red-200 rounded-xl text-xs text-red-700 flex justify-between items-center mb-2">
          <span>{error}</span>
          <button type="button" onClick={() => setError(null)}>✕</button>
        </div>
      )}

      {skills.length === 0 ? (
        <div className="text-center py-12 text-stone-400">
          <p className="text-sm">{t('skills.no_skills_found', { defaultValue: 'No skills available.' })}</p>
        </div>
      ) : (
        skills.map(skill => (
          <div
            key={skill.id}
            className={`bg-white rounded-xl border px-4 py-3 flex items-center gap-3 shadow-sm transition-opacity ${
              skill.enabled ? 'border-stone-200' : 'border-stone-100 opacity-60'
            }`}
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5">
                <p className="text-sm font-medium text-stone-900">
                  {t(`skills.name_${skill.id.replace(/-/g, '_')}`, { defaultValue: skill.name })}
                </p>
                <span className="text-[11px]" aria-hidden="true">{SKILL_CATEGORY_ICONS[skill.category] ?? '⚙️'}</span>
              </div>
              <p className="text-xs text-stone-500 mt-0.5">
                {t(`skills.desc_${skill.id.replace(/-/g, '_')}`, { defaultValue: skill.description })}
              </p>
            </div>
            <button
              type="button"
              onClick={() => handleToggle(skill)}
              disabled={toggling === skill.id}
              aria-label={skill.enabled ? t('skills.disable', { name: skill.name }) : t('skills.enable', { name: skill.name })}
              className={`relative flex-shrink-0 w-10 h-6 rounded-full transition-colors duration-200 disabled:opacity-50 ${
                skill.enabled ? 'bg-primary-500' : 'bg-stone-300'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform duration-200 ${
                skill.enabled ? 'translate-x-4' : 'translate-x-0'
              }`} />
            </button>
          </div>
        ))
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Meetings settings section
// ---------------------------------------------------------------------------
const MEETING_TEMPLATES = [
  { id: 'general',       label: 'General' },
  { id: 'standup',       label: 'Standup' },
  { id: 'one_on_one',    label: '1:1' },
  { id: 'retrospective', label: 'Retrospective' },
  { id: 'planning',      label: 'Planning' },
];

function MeetingsSettingsSection() {
  const { t } = useTranslation();
  const [defaultTemplate, setDefaultTemplate] = useState('general');
  const [defaultLang, setDefaultLang] = useState(() => i18n.language?.slice(0, 2) ?? 'it');
  const [watchFolder, setWatchFolder] = useState('');
  const [autoImport, setAutoImport] = useState(false);
  const [autoSummarize, setAutoSummarize] = useState(true);
  const [diarize, setDiarize] = useState(false);
  const [shareSlack, setShareSlack] = useState('');
  const [shareGithub, setShareGithub] = useState('');
  const [shareJira, setShareJira] = useState('');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    async function load() {
      const [tpl, lang, folder, imp, sum, diar, slack, github, jira] = await Promise.all([
        settingsGet('meetings.default_template'),
        settingsGet('meetings.default_lang'),
        settingsGet('meetings.watch_folder'),
        settingsGet('meetings.auto_import'),
        settingsGet('meetings.auto_summarize'),
        settingsGet('meetings.diarize'),
        settingsGet('meetings.share_slack_channel'),
        settingsGet('meetings.share_github_repo'),
        settingsGet('meetings.share_jira_project'),
      ]);
      if (tpl) setDefaultTemplate(tpl);
      if (lang) setDefaultLang(lang);
      if (folder) setWatchFolder(folder);
      if (imp !== null) setAutoImport(imp === '1');
      if (sum !== null) setAutoSummarize(sum !== '0');
      if (diar !== null) setDiarize(diar === '1');
      if (slack) setShareSlack(slack);
      if (github) setShareGithub(github);
      if (jira) setShareJira(jira);
    }
    load().catch(() => {});
  }, []);

  async function handleDiarizeToggle() {
    const next = !diarize;
    setDiarize(next);
    try {
      await settingsSet('meetings.diarize', next ? '1' : '0');
    } catch {
      // revert on failure
      setDiarize(!next);
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaveError(null);
    try {
      await Promise.all([
        settingsSet('meetings.default_template', defaultTemplate),
        settingsSet('meetings.default_lang', defaultLang),
        settingsSet('meetings.watch_folder', watchFolder),
        settingsSet('meetings.auto_import', autoImport ? '1' : '0'),
        settingsSet('meetings.auto_summarize', autoSummarize ? '1' : '0'),
        settingsSet('meetings.diarize', diarize ? '1' : '0'),
        settingsSet('meetings.share_slack_channel', shareSlack),
        settingsSet('meetings.share_github_repo', shareGithub),
        settingsSet('meetings.share_jira_project', shareJira),
      ]);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-4">
      <section className="bg-white rounded-xl border border-stone-200 shadow-soft p-4 space-y-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-1">
            {t('meetings_settings.defaults_title')}
          </p>
          <p className="text-xs text-stone-400 leading-relaxed">{t('meetings_settings.defaults_desc')}</p>
        </div>
        <div>
          <label htmlFor="mtg-tpl" className="block text-xs font-medium text-stone-700 mb-1">{t('meetings_settings.default_template')}</label>
          <select
            id="mtg-tpl"
            value={defaultTemplate}
            onChange={e => setDefaultTemplate(e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none"
          >
            {MEETING_TEMPLATES.map(tpl => (
              <option key={tpl.id} value={tpl.id}>{tpl.label}</option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="mtg-lang" className="block text-xs font-medium text-stone-700 mb-1">{t('meetings_settings.default_lang')}</label>
          <select
            id="mtg-lang"
            value={defaultLang}
            onChange={e => setDefaultLang(e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none"
          >
            {SUPPORTED_LANGUAGES.map(l => (
              <option key={l.code} value={l.code}>{l.label}</option>
            ))}
          </select>
        </div>
      </section>

      <section className="bg-white rounded-xl border border-stone-200 shadow-soft p-4 space-y-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-1">
            {t('meetings_settings.auto_import_title')}
          </p>
          <p className="text-xs text-stone-400 leading-relaxed">{t('meetings_settings.auto_import_desc')}</p>
        </div>
        <div>
          <label className="block text-xs font-medium text-stone-700 mb-1">{t('meetings_settings.watch_folder_label')}</label>
          <input
            type="text"
            value={watchFolder}
            onChange={e => setWatchFolder(e.target.value)}
            placeholder={t('meetings_settings.watch_folder_placeholder')}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none placeholder-stone-400"
          />
        </div>
        <div className="flex items-center justify-between">
          <span className="text-xs text-stone-700">{t('meetings_settings.auto_import_toggle')}</span>
          <button
            type="button"
            aria-label={t('meetings_settings.auto_import_toggle')}
            onClick={() => setAutoImport(v => !v)}
            className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${autoImport ? 'bg-primary-500' : 'bg-stone-300'}`}
          >
            <span className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform ${autoImport ? 'translate-x-4' : 'translate-x-1'}`} />
          </button>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-xs text-stone-700">{t('meetings_settings.auto_summarize_toggle')}</span>
          <button
            type="button"
            aria-label={t('meetings_settings.auto_summarize_toggle')}
            onClick={() => setAutoSummarize(v => !v)}
            className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${autoSummarize ? 'bg-primary-500' : 'bg-stone-300'}`}
          >
            <span className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform ${autoSummarize ? 'translate-x-4' : 'translate-x-1'}`} />
          </button>
        </div>
      </section>

      <section className="bg-white rounded-xl border border-stone-200 shadow-soft p-4 space-y-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-1">
            {t('meetings_settings.diarize_title')}
          </p>
          <p className="text-xs text-stone-400 leading-relaxed">{t('meetings_settings.diarize_desc')}</p>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-xs text-stone-700">{t('meetings_settings.diarize_toggle')}</span>
          <button
            type="button"
            aria-label={t('meetings_settings.diarize_toggle')}
            onClick={() => { void handleDiarizeToggle(); }}
            className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${diarize ? 'bg-primary-500' : 'bg-stone-300'}`}
          >
            <span className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform ${diarize ? 'translate-x-4' : 'translate-x-1'}`} />
          </button>
        </div>
        <p className="text-xs text-amber-600">{t('meetings_settings.diarize_feature_note')}</p>
      </section>

      {/* Share defaults */}
      <section className="bg-white rounded-xl border border-stone-200 shadow-soft p-4 space-y-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-1">
            {t('meetings_settings.share_title')}
          </p>
          <p className="text-xs text-stone-400 leading-relaxed">{t('meetings_settings.share_desc')}</p>
        </div>

        {/* Slack */}
        <div>
          <label className="block text-xs font-medium text-stone-700 mb-1">
            {t('meetings_settings.share_slack_label')}
          </label>
          <input
            type="text"
            value={shareSlack}
            onChange={e => setShareSlack(e.target.value)}
            placeholder={t('meetings_settings.share_slack_ph')}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none placeholder-stone-400"
          />
          <p className="text-xs text-stone-400 mt-1">{t('meetings_settings.share_slack_help')}</p>
        </div>

        {/* GitHub */}
        <div>
          <label className="block text-xs font-medium text-stone-700 mb-1">
            {t('meetings_settings.share_github_label')}
          </label>
          <input
            type="text"
            value={shareGithub}
            onChange={e => setShareGithub(e.target.value)}
            placeholder={t('meetings_settings.share_github_ph')}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none placeholder-stone-400"
          />
          <p className="text-xs text-stone-400 mt-1">{t('meetings_settings.share_github_help')}</p>
        </div>

        {/* Jira */}
        <div>
          <label className="block text-xs font-medium text-stone-700 mb-1">
            {t('meetings_settings.share_jira_label')}
          </label>
          <input
            type="text"
            value={shareJira}
            onChange={e => setShareJira(e.target.value.toUpperCase())}
            placeholder={t('meetings_settings.share_jira_ph')}
            className="w-full px-3 py-2 rounded-lg border border-stone-200 bg-stone-50 text-sm text-stone-800 focus:border-primary-400 focus:outline-none placeholder-stone-400"
          />
          <p className="text-xs text-stone-400 mt-1">{t('meetings_settings.share_jira_help')}</p>
        </div>

        {/* Notion — no extra config */}
        <div className="rounded-lg bg-stone-50 border border-stone-100 px-3 py-2">
          <p className="text-xs font-medium text-stone-700 mb-0.5">{t('meetings_settings.share_notion_label')}</p>
          <p className="text-xs text-stone-400">{t('meetings_settings.share_notion_help')}</p>
        </div>
      </section>

      <div className="flex items-center justify-end gap-3">
        {saveError && <p className="text-xs text-coral-500 flex-1">{saveError}</p>}
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="px-4 py-2 rounded-lg bg-primary-500 hover:bg-primary-600 text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {saved ? t('meetings_settings.saved') : saving ? t('meetings_settings.saving') : t('meetings_settings.save')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// License section
// ---------------------------------------------------------------------------

function LicenseSection() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<LicenseStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [serverUrl, setServerUrl] = useState('');
  const [savingUrl, setSavingUrl] = useState(false);
  const [savedUrl, setSavedUrl] = useState(false);
  const [deactivating, setDeactivating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    licenseStatus()
      .then(s => { setInfo(s); })
      .catch(() => {})
      .finally(() => setLoading(false));
    settingsGet('license.server_url')
      .then(v => { if (v) setServerUrl(v); })
      .catch(() => {});
  }, []);

  async function handleDeactivate() {
    setDeactivating(true);
    setError(null);
    try {
      await licenseDeactivate();
      const updated = await licenseStatus();
      setInfo(updated);
    } catch (e) {
      setError(String(e));
    } finally {
      setDeactivating(false);
    }
  }

  async function handleSaveUrl() {
    setSavingUrl(true);
    try {
      await licenseServerUrlSet(serverUrl.trim());
      setSavedUrl(true);
      setTimeout(() => setSavedUrl(false), 2500);
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingUrl(false);
    }
  }

  const statusColor: Record<string, string> = {
    active:   'text-sage-600 bg-sage-50 border-sage-200',
    trial:    'text-primary-600 bg-primary-50 border-primary-200',
    grace:    'text-amber-600 bg-amber-50 border-amber-200',
    expired:  'text-coral-600 bg-coral-50 border-coral-200',
    inactive: 'text-stone-500 bg-stone-100 border-stone-200',
  };

  const statusLabel: Record<string, string> = {
    active:   t('license.status_active'),
    trial:    t('license.status_trial'),
    grace:    t('license.status_grace'),
    expired:  t('license.status_expired'),
    inactive: t('license.status_inactive'),
  };

  if (loading) {
    return <div className="h-24 bg-stone-100 rounded-xl animate-pulse" />;
  }

  return (
    <section>
      <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">
        {t('license.section')}
      </p>
      <div className="space-y-3">

        {/* Status card */}
        <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-4 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-stone-800">{t('license.title')}</span>
            {info && (
              <span className={`text-xs font-medium border rounded-full px-2.5 py-0.5 ${statusColor[info.status] ?? statusColor.inactive}`}>
                {statusLabel[info.status] ?? info.status}
              </span>
            )}
          </div>

          {info && (
            <dl className="space-y-1.5 text-xs">
              {info.key && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.key_label')}</dt>
                  <dd className="text-stone-700 font-mono">{info.key.slice(0, 9)}…</dd>
                </div>
              )}
              {info.plan && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.plan')}</dt>
                  <dd className="text-stone-700 capitalize">{info.plan}</dd>
                </div>
              )}
              {info.org && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.org')}</dt>
                  <dd className="text-stone-700">{info.org}</dd>
                </div>
              )}
              {info.expires_at && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.expires')}</dt>
                  <dd className="text-stone-700">{new Date(info.expires_at).toLocaleDateString()}</dd>
                </div>
              )}
              {info.trial_days_left !== null && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.status_trial')}</dt>
                  <dd className="text-primary-600 font-medium">{t('license.trial_days_left', { count: info.trial_days_left })}</dd>
                </div>
              )}
              {info.grace_hours_left !== null && info.status === 'grace' && (
                <div className="flex justify-between">
                  <dt className="text-stone-500">{t('license.status_grace')}</dt>
                  <dd className="text-amber-600 font-medium">{t('license.grace_hours_left', { count: info.grace_hours_left })}</dd>
                </div>
              )}
              <div className="flex justify-between">
                <dt className="text-stone-500">{t('license.machine_id')}</dt>
                <dd className="text-stone-400 font-mono text-[10px]">{info.machine_id.slice(0, 16)}…</dd>
              </div>
            </dl>
          )}

          {info?.status !== 'inactive' && info?.key && (
            <div className="pt-1">
              <button
                type="button"
                onClick={handleDeactivate}
                disabled={deactivating}
                className="px-3 py-1.5 text-xs font-medium rounded-lg bg-stone-100 hover:bg-coral-50 hover:text-coral-600 text-stone-600 transition-colors disabled:opacity-40"
              >
                {deactivating ? '…' : t('license.deactivate_confirm')}
              </button>
            </div>
          )}

          {error && <p className="text-xs text-coral-500">{error}</p>}
        </div>

        {/* Server URL */}
        <div className="bg-white rounded-xl border border-stone-200 shadow-soft px-4 py-4">
          <div className="flex items-center justify-between mb-1">
            <p className="text-sm font-medium text-stone-800">{t('license.server_url_label')}</p>
            {savedUrl && (
              <span className="text-xs font-medium text-sage-600 bg-sage-50 border border-sage-200 rounded-full px-2.5 py-0.5">{t('settings.saved')}</span>
            )}
          </div>
          <p className="text-xs text-stone-400 mb-3">{t('license.server_url_help')}</p>
          <div className="space-y-2">
            <input
              type="text"
              value={serverUrl}
              onChange={e => setServerUrl(e.target.value)}
              placeholder="https://license.youma.ai"
              className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 placeholder-stone-400 text-sm transition-colors"
              onKeyDown={e => { if (e.key === 'Enter') handleSaveUrl(); }}
            />
            <div className="flex justify-end">
              <button
                type="button"
                onClick={handleSaveUrl}
                disabled={savingUrl}
                className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
              >
                {savingUrl ? '…' : t('settings.save')}
              </button>
            </div>
          </div>
        </div>

      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Thinking phrases — 2-tab UI
// Tab 1 "Default": read-only, shows Claude-style phrases (used when no custom set)
// Tab 2 "Personalizzate": editable, saves to YOUMA_THINKING_PHRASES (overrides default)
// ---------------------------------------------------------------------------

const DEFAULT_PHRASES_STRING = DEFAULT_THINKING_PHRASES.join(', ');
const MILANESE_PHRASES_STRING = MILANESE_THINKING_PHRASES.join(', ');

export function ThinkingPhrasesField() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<'default' | 'custom'>('default');
  // Pre-populate tab 2 with Milanese phrases so user can edit/copy/save them immediately.
  const [customValue, setCustomValue] = useState(MILANESE_PHRASES_STRING);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    // If user already saved custom phrases, load those; otherwise keep Milanese as starting point.
    envGet('YOUMA_THINKING_PHRASES').then(v => { if (v) setCustomValue(v); }).catch(() => {});
  }, []);

  async function handleSave() {
    setSaving(true); setError(null); setSaved(false);
    try {
      await setEnvVar('YOUMA_THINKING_PHRASES', customValue);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleClear() {
    setSaving(true); setError(null);
    try {
      await setEnvVar('YOUMA_THINKING_PHRASES', '');
      setCustomValue('');
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  const tabs = [
    { id: 'default' as const, label: t('settings.thinking_tab_default') },
    { id: 'custom'  as const, label: t('settings.thinking_tab_custom') },
  ];

  return (
    <div className="space-y-3">
      {/* Tab bar */}
      <div className="flex border-b border-stone-200">
        {tabs.map(tab => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.id
                ? 'border-primary-500 text-primary-600'
                : 'border-transparent text-stone-500 hover:text-stone-700'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab 1: Default (read-only + copyable) */}
      {activeTab === 'default' && (
        <div className="space-y-2">
          <p className="text-xs text-stone-400">{t('settings.thinking_default_desc')}</p>
          <textarea
            rows={5}
            readOnly
            value={DEFAULT_PHRASES_STRING}
            className="w-full px-4 py-2.5 rounded-lg border border-stone-200 bg-stone-100 text-stone-500 text-sm resize-none select-all"
          />
          <div className="flex items-center justify-between">
            {customValue !== MILANESE_PHRASES_STRING && customValue ? (
              <p className="text-xs text-amber-600">{t('settings.thinking_default_overridden')}</p>
            ) : <span />}
            <button
              type="button"
              onClick={() => { void navigator.clipboard.writeText(DEFAULT_PHRASES_STRING).then(() => { setCopied(true); setTimeout(() => setCopied(false), 2000); }); }}
              className="px-3 py-1.5 text-xs text-stone-500 hover:text-stone-700 border border-stone-200 hover:border-stone-300 bg-white hover:bg-stone-50 rounded-lg transition-colors"
            >
              {copied ? '✓ Copiato' : t('settings.copy', { defaultValue: 'Copia' })}
            </button>
          </div>
        </div>
      )}

      {/* Tab 2: Custom (editable + copyable + saveable) */}
      {activeTab === 'custom' && (
        <div className="space-y-2">
          <p className="text-xs text-stone-400">{t('settings.thinking_custom_desc')}</p>
          <textarea
            rows={5}
            value={customValue}
            onChange={e => setCustomValue(e.target.value)}
            className="w-full px-4 py-2.5 rounded-lg border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 text-sm transition-colors resize-none"
          />
          <p className="text-xs text-stone-400">{t('settings.thinking_phrases_hint')}</p>
          <div className="flex justify-end gap-2">
            {customValue && (
              <button
                type="button"
                onClick={handleClear}
                disabled={saving}
                className="px-4 py-2 text-coral-600 hover:text-coral-700 border border-coral-200 hover:border-coral-300 bg-white hover:bg-coral-50 disabled:opacity-50 text-sm font-medium rounded-lg transition-colors"
              >
                {t('settings.clear', { defaultValue: 'Clear' })}
              </button>
            )}
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white text-sm font-medium rounded-lg transition-colors"
            >
              {saving ? '…' : t('settings.save')}
            </button>
          </div>
          {saved && <p className="text-xs text-sage-600">{t('settings.saved')}</p>}
          {error && <p className="text-xs text-coral-500">{error}</p>}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------

type SettingsSection = 'keys' | 'voice' | 'kb' | 'ai' | 'thinking' | 'meetings' | 'skills' | 'privacy' | 'license';

export default function SettingsPage() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<{ provider: string; model: string } | null>(null);
  const [activeSection, setActiveSection] = useState<SettingsSection>('keys');
  const [sidebarOpen, setSidebarOpen] = useState(true);

  function refreshConfig() {
    configGet().then(cfg => setConfig(cfg)).catch(() => {});
  }

  const KEY_FIELDS: KeyField[] = [
    { envKey: 'ANTHROPIC_API_KEY', label: 'Anthropic API Key', placeholder: 'sk-ant-…', helpText: t('settings.anthropic_key_help') },
    { envKey: 'OPENAI_API_KEY', label: 'OpenAI API Key', placeholder: 'sk-…', helpText: t('settings.openai_key_help') },
    { envKey: 'ELEVENLABS_API_KEY', label: 'ElevenLabs API Key', placeholder: 'el-…', helpText: t('settings.elevenlabs_key_help') },
    { envKey: 'COMPOSIO_API_KEY', label: 'Composio API Key', placeholder: 'composio-…', helpText: t('settings.composio_key_help') },
    { envKey: 'DEEPSEEK_API_KEY', label: 'DeepSeek API Key', placeholder: 'sk-…', helpText: t('settings.deepseek_key_help') },
  ];

  useEffect(() => {
    refreshConfig();
  }, []);

  const settingsSections: { id: SettingsSection; label: string }[] = [
    { id: 'keys',     label: t('settings.section_keys') },
    { id: 'voice',    label: t('settings.section_voice') },
    { id: 'kb',       label: t('settings.section_embeddings') },
    { id: 'ai',       label: t('settings.section_custom_ai') },
    { id: 'thinking', label: t('settings.section_thinking') },
    { id: 'meetings', label: t('meetings_settings.section') },
    { id: 'skills',   label: t('settings.section_skills') },
    { id: 'privacy',  label: t('settings.section_privacy') },
    { id: 'license',  label: t('license.section') },
  ];

  return (
    <div className="flex h-full min-h-0">

      {/* ---- Left nav sidebar ---- */}
      <aside className={`flex-shrink-0 bg-white border-r border-stone-200 flex flex-col min-h-0 transition-all duration-200 ${sidebarOpen ? 'w-40' : 'w-0 overflow-hidden border-r-0'}`}>
        <div className="px-3 py-2 border-b border-stone-200">
          <span className="text-sm font-semibold text-stone-900">{t('settings.title')}</span>
          {config && (
            <span className="text-[10px] text-stone-400 mt-0.5 block truncate">
              {config.provider} · {config.model || 'default'}
            </span>
          )}
        </div>
        <nav className="p-2 space-y-0.5 flex-1 overflow-y-auto">
          {settingsSections.map(sec => (
            <button
              key={sec.id}
              type="button"
              onClick={() => setActiveSection(sec.id)}
              className={`w-full text-left px-2 py-1.5 rounded-lg text-xs transition-colors ${
                activeSection === sec.id
                  ? 'bg-primary-50 text-primary-700 font-medium'
                  : 'text-stone-600 hover:bg-stone-100'
              }`}
            >
              <span className="truncate block">{sec.label}</span>
            </button>
          ))}
        </nav>
      </aside>

      {/* ---- Right content ---- */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0">

        {/* Header with sidebar toggle */}
        <div className="flex-shrink-0 px-3 py-2 border-b border-stone-200 bg-white flex items-center gap-2">
          <button
            type="button"
            onClick={() => setSidebarOpen(v => !v)}
            title={sidebarOpen ? t('chat.hide_threads') : t('chat.show_threads')}
            className="flex-shrink-0 p-1.5 rounded-md text-stone-400 hover:text-stone-700 hover:bg-stone-100 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d={sidebarOpen ? 'M4 6h16M4 12h16M4 18h16' : 'M4 6h16M4 12h10M4 18h16'} />
            </svg>
          </button>
          <span className="text-sm font-semibold text-stone-900">{t('settings.title')}</span>
        </div>

        <div className="flex-1 overflow-y-auto bg-stone-50">
        <div className="px-4 py-4 space-y-4">

          {/* Keys + Model */}
          {activeSection === 'keys' && (() => {
            const PROVIDER_ENV_MAP: Record<string, string> = {
              anthropic: 'ANTHROPIC_API_KEY',
              openai: 'OPENAI_API_KEY',
              deepseek: 'DEEPSEEK_API_KEY',
            };
            const activeEnvKey = PROVIDER_ENV_MAP[config?.provider ?? ''];
            return (
              <div className="space-y-4">
                <section>
                  <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_keys')}</p>
                  <div className="space-y-3">
                    {KEY_FIELDS.map(field => (
                      <ApiKeyCard
                        key={field.envKey}
                        field={field}
                        onSaved={refreshConfig}
                        isActive={activeEnvKey === field.envKey}
                      />
                    ))}
                  </div>
                </section>
                <section>
                  <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_model')}</p>
                  <ModelSelector activeProvider={config?.provider} />
                </section>
              </div>
            );
          })()}

          {/* Voice */}
          {activeSection === 'voice' && (
            <section>
              <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_voice')}</p>
              <VoiceSection />
            </section>
          )}

          {/* Embeddings */}
          {activeSection === 'kb' && <EmbeddingSection />}

          {/* Custom AI + Ollama + Provider priority */}
          {activeSection === 'ai' && (
            <div className="space-y-4">
              <section>
                <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_custom_ai')}</p>
                <div className="space-y-3">
                  <p className="text-xs text-stone-400 leading-relaxed -mt-1">{t('settings.custom_ai_desc')}</p>
                  <EnvField
                    envKey="CUSTOM_AI_URL"
                    label={t('settings.custom_ai_url_label')}
                    placeholder="http://localhost:1234"
                    helpText={t('settings.custom_ai_url_help')}
                    onSaved={refreshConfig}
                  />
                  <ApiKeyCard
                    field={{
                      envKey: 'CUSTOM_AI_KEY',
                      label: t('settings.api_key'),
                      placeholder: 'sk-… (optional)',
                      helpText: t('settings.custom_ai_key_help'),
                    }}
                    onSaved={refreshConfig}
                  />
                  <EnvField
                    envKey="CUSTOM_AI_MODEL"
                    label={t('settings.custom_ai_model_label')}
                    placeholder="mistral-7b-instruct or ~/llm/models/model.gguf"
                    helpText={t('settings.custom_ai_model_help')}
                  />
                </div>
              </section>
              <section>
                <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.section_ollama')}</p>
                <div className="space-y-3">
                  <EnvField
                    envKey="OLLAMA_URL"
                    label={t('settings.ollama_url_label')}
                    placeholder="http://localhost:11434"
                    helpText={t('settings.ollama_url_help')}
                    defaultValue="http://localhost:11434"
                  />
                  <EnvField
                    envKey="OLLAMA_MODEL"
                    label={t('settings.ollama_model_label')}
                    placeholder="llama3.2"
                    helpText={t('settings.ollama_model_help')}
                    defaultValue="llama3.2"
                  />
                </div>
              </section>
              <div className="bg-white rounded-xl border border-stone-200 p-4">
                <h3 className="text-sm font-semibold text-stone-700 mb-2">{t('settings.section_priority')}</h3>
                <ol className="text-xs text-stone-500 space-y-1 list-decimal list-inside">
                  <li className="text-primary-600 font-medium">{t('settings.priority_custom_priority')}</li>
                  <li>{t('settings.priority_anthropic')}</li>
                  <li>{t('settings.priority_openai')}</li>
                  <li>{t('settings.priority_deepseek')}</li>
                  <li>{t('settings.priority_custom')}</li>
                  <li>{t('settings.priority_ollama')}</li>
                </ol>
                <p className="text-xs text-stone-400 mt-2">
                  {t('settings.priority_footer', { file: 'youma.env' })}
                </p>
              </div>
            </div>
          )}

          {/* Thinking */}
          {activeSection === 'thinking' && (
            <div className="space-y-4">
              <section>
                <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">
                  {t('settings.section_thinking')}
                </p>
                <div className="space-y-3">
                  <p className="text-xs text-stone-400 leading-relaxed -mt-1">
                    {t('settings.thinking_desc')}
                  </p>
                  <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-5">
                    <h3 className="text-sm font-semibold text-stone-900 mb-1">
                      {t('settings.thinking_phrases_label')}
                    </h3>
                    <p className="text-xs text-stone-400 mb-3">{t('settings.thinking_phrases_help')}</p>
                    <ThinkingPhrasesField />
                  </div>
                </div>
              </section>
            </div>
          )}

          {/* Meetings */}
          {activeSection === 'meetings' && <MeetingsSettingsSection />}

          {/* Skills */}
          {activeSection === 'skills' && (
            <div className="space-y-6">
              <SkillsSection />
              <section>
                <p className="text-xs font-semibold uppercase tracking-wide text-stone-400 mb-3">{t('settings.language')}</p>
                <LanguageSelector />
              </section>
            </div>
          )}

          {/* License */}
          {activeSection === 'license' && <LicenseSection />}

          {/* Privacy + About */}
          {activeSection === 'privacy' && (
            <div className="space-y-4">
              <PrivacySection />
              <div className="bg-white rounded-xl border border-stone-200 p-4 text-center">
                <p className="text-sm font-semibold text-stone-700">{t('settings.about')}</p>
                <p className="text-xs text-stone-400 mt-0.5">{t('settings.about_version')}</p>
                <p className="text-xs text-stone-300 mt-1">{t('settings.about_desc')}</p>
              </div>
            </div>
          )}

        </div>
        </div>
      </div>
    </div>
  );
}
