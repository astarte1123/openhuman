import { useState } from 'react';

import { useT } from '../../../../lib/i18n/I18nContext';

// ─────────────────────────────────────────────────────────────────────────────
// Minimal API-key dialog — shown when the user flips a provider toggle ON.
// No endpoint / model fields; default endpoint is derived from the slug and
// the model is left empty (the routing dialog picks the model per workload).
// ─────────────────────────────────────────────────────────────────────────────

const ProviderKeyDialog = ({
  slug,
  label,
  onCancel,
  onSubmit,
}: {
  slug: string;
  label: string;
  onCancel: () => void;
  onSubmit: (apiKey: string) => Promise<void> | void;
}) => {
  const { t } = useT();
  const [apiKey, setApiKey] = useState('');
  const [phase, setPhase] = useState<'idle' | 'saving'>('idle');
  const [error, setError] = useState<string | null>(null);
  const busy = phase !== 'idle';

  const placeholder =
    slug === 'openai'
      ? 'sk-...'
      : slug === 'anthropic'
        ? 'sk-ant-...'
        : slug === 'openrouter'
          ? 'sk-or-...'
          : 'your-api-key';

  const handleSave = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setError(t('settings.provider_key_dialog.api_key_required'));
      return;
    }
    setError(null);

    setPhase('saving');
    try {
      await onSubmit(trimmed);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setPhase('idle');
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('settings.provider_key_dialog.connect_title', { label })}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4">
      <div className="w-full max-w-md rounded-2xl border border-stone-200 bg-white p-6 shadow-soft">
        <div className="mb-4">
          <h3 className="text-base font-semibold text-stone-900">
            {t('settings.provider_key_dialog.connect_title', { label })}
          </h3>
          <p className="mt-0.5 text-xs text-stone-500">
            {t('settings.provider_key_dialog.subtitle')}
          </p>
        </div>

        <div className="flex flex-col gap-1.5">
          <label htmlFor="provider-key-input" className="text-xs font-medium text-stone-700">
            {t('settings.provider_key_dialog.api_key_label')}
          </label>
          <input
            id="provider-key-input"
            type="text"
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            data-form-type="other"
            data-lpignore="true"
            data-1p-ignore="true"
            value={apiKey}
            placeholder={placeholder}
            disabled={busy}
            onChange={e => {
              setApiKey(e.target.value);
              setError(null);
            }}
            className="rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 placeholder-stone-400 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 disabled:opacity-60"
          />
          {error ? <p className="text-xs font-medium text-red-600">{error}</p> : null}
        </div>

        <div className="mt-6 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="rounded-lg border border-stone-200 bg-white px-4 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
            {t('settings.provider_key_dialog.cancel_button')}
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={busy}
            className="rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:cursor-not-allowed disabled:opacity-50">
            {phase === 'saving'
              ? t('settings.provider_key_dialog.saving')
              : t('settings.provider_key_dialog.save_button')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ProviderKeyDialog;
