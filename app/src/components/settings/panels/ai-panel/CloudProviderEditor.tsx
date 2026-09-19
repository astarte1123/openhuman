import { useState } from 'react';

import { useT } from '../../../../lib/i18n/I18nContext';
import { BUILTIN_PROVIDER_META } from './catalog';
import { authStyleForSlug, defaultEndpointFor, maskKeyLabel } from './transforms';
import type { CloudProvider } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// Cloud provider editor modal
// ─────────────────────────────────────────────────────────────────────────────

const CloudProviderEditor = ({
  initial,
  existingSlugs,
  onClose,
  onSubmit,
  onClearKey,
}: {
  initial: CloudProvider | null;
  existingSlugs: string[];
  onClose: () => void;
  onSubmit: (next: CloudProvider, apiKey: string) => Promise<void> | void;
  onClearKey: (slug: string) => Promise<void> | void;
}) => {
  const { t } = useT();
  const defaultSlug: string =
    initial?.slug ??
    (['openai', 'anthropic', 'openrouter', 'custom'] as const).find(
      s => !existingSlugs.includes(s)
    ) ??
    'custom';
  const [slug, setSlug] = useState<string>(defaultSlug);
  const [label, setLabel] = useState<string>(
    initial?.label ?? BUILTIN_PROVIDER_META[defaultSlug]?.label ?? defaultSlug
  );
  const [endpoint, setEndpoint] = useState(initial?.endpoint ?? defaultEndpointFor(defaultSlug));
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const isAura = slug === 'aura';
  const hasExistingKey = (initial?.maskedKey ?? '').startsWith('••••');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30 p-4">
      <div className="w-full max-w-md rounded-lg border border-stone-200 bg-white shadow-float">
        <div className="border-b border-stone-200 px-4 py-3">
          <div className="text-sm font-semibold text-stone-900">
            {initial
              ? t('settings.cloud_provider_editor.edit_title', { label: initial.label })
              : t('settings.cloud_provider_editor.add_title')}
          </div>
          <div className="mt-0.5 text-xs text-stone-500">
            {t('settings.cloud_provider_editor.encrypted_hint_prefix')}{' '}
            <span className="font-mono">auth-profiles.json</span>
            {t('settings.cloud_provider_editor.encrypted_hint_suffix')}
          </div>
        </div>
        <div className="space-y-3 px-4 py-3">
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              {t('settings.cloud_provider_editor.provider_slug_label')}
            </label>
            <select
              value={slug}
              onChange={e => {
                const next = e.target.value;
                setSlug(next);
                setLabel(BUILTIN_PROVIDER_META[next]?.label ?? next);
                if (!initial) {
                  setEndpoint(defaultEndpointFor(next));
                }
              }}
              disabled={!!initial}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 disabled:opacity-60 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200">
              {(['openai', 'anthropic', 'openrouter', 'custom'] as const)
                .filter(s => s === slug || !existingSlugs.includes(s))
                .map(s => (
                  <option key={s} value={s}>
                    {BUILTIN_PROVIDER_META[s]?.label ?? s}
                  </option>
                ))}
            </select>
          </div>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              {t('settings.cloud_provider_editor.display_label_label')}
            </label>
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder={t('settings.cloud_provider_editor.display_label_placeholder')}
            />
          </div>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              {t('settings.cloud_provider_editor.endpoint_label')}
            </label>
            <input
              value={endpoint}
              onChange={e => setEndpoint(e.target.value)}
              disabled={isAura}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 disabled:opacity-60 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder={t('settings.cloud_provider_editor.endpoint_placeholder')}
            />
          </div>
          {!isAura && (
            <div>
              <label className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                <span>{t('settings.cloud_provider_editor.api_key_label')}</span>
                {hasExistingKey && (
                  <button
                    onClick={() => void onClearKey(slug)}
                    className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                    {t('settings.cloud_provider_editor.clear_stored_key')}
                  </button>
                )}
              </label>
              <input
                type="password"
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
                placeholder={
                  hasExistingKey
                    ? t('settings.cloud_provider_editor.api_key_placeholder_existing')
                    : t('settings.cloud_provider_editor.api_key_placeholder_new')
                }
              />
            </div>
          )}
        </div>
        <div className="flex items-center justify-end gap-2 border-t border-stone-200 px-4 py-3">
          <button
            onClick={onClose}
            disabled={saving}
            className="rounded-lg border border-stone-200 px-3 py-1.5 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
            {t('settings.cloud_provider_editor.cancel')}
          </button>
          <button
            onClick={async () => {
              setSaving(true);
              try {
                await onSubmit(
                  {
                    id: initial?.id ?? '',
                    slug,
                    label: label.trim() || slug,
                    endpoint: endpoint.trim(),
                    authStyle: initial?.authStyle ?? authStyleForSlug(slug),
                    maskedKey: maskKeyLabel(hasExistingKey || apiKey.length > 0),
                  },
                  apiKey.trim()
                );
              } finally {
                setSaving(false);
              }
            }}
            disabled={saving || !endpoint.trim()}
            className="rounded-lg bg-primary-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-600 disabled:opacity-50">
            {saving
              ? t('settings.cloud_provider_editor.saving')
              : initial
                ? t('settings.cloud_provider_editor.save_changes')
                : t('settings.cloud_provider_editor.add_provider')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default CloudProviderEditor;
