import { useState } from 'react';

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
            {initial ? `Edit ${initial.label}` : 'Add cloud provider'}
          </div>
          <div className="mt-0.5 text-xs text-stone-500">
            API keys are encrypted at rest in <span className="font-mono">auth-profiles.json</span>.
          </div>
        </div>
        <div className="space-y-3 px-4 py-3">
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              Provider slug
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
              Display label
            </label>
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder="My Provider"
            />
          </div>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              Endpoint
            </label>
            <input
              value={endpoint}
              onChange={e => setEndpoint(e.target.value)}
              disabled={isAura}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 disabled:opacity-60 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder="https://api.example.com/v1"
            />
          </div>
          {!isAura && (
            <div>
              <label className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                <span>API key</span>
                {hasExistingKey && (
                  <button
                    onClick={() => void onClearKey(slug)}
                    className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                    Clear stored key
                  </button>
                )}
              </label>
              <input
                type="password"
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
                placeholder={hasExistingKey ? 'Leave blank to keep existing key' : 'sk-...'}
              />
            </div>
          )}
        </div>
        <div className="flex items-center justify-end gap-2 border-t border-stone-200 px-4 py-3">
          <button
            onClick={onClose}
            disabled={saving}
            className="rounded-lg border border-stone-200 px-3 py-1.5 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
            Cancel
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
            {saving ? 'Saving…' : initial ? 'Save changes' : 'Add provider'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default CloudProviderEditor;
