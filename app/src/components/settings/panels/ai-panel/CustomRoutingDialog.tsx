import { useState } from 'react';

import type { CloudProvider, OllamaModel, ProviderRef, Workload } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// Custom-routing dialog — opened when the user clicks "Custom" on a workload.
// Lets them pick a provider (cloud or local) and the specific model id.
// ─────────────────────────────────────────────────────────────────────────────

interface CustomRoutingDialogProps {
  workload: Workload;
  initial: ProviderRef;
  cloudProviders: CloudProvider[];
  localModels: OllamaModel[];
  ollamaRunning: boolean;
  onClose: () => void;
  onSubmit: (next: ProviderRef) => void;
}

type CustomDialogSource = { kind: 'cloud'; providerSlug: string } | { kind: 'local' };

const CustomRoutingDialog = ({
  workload,
  initial,
  cloudProviders,
  localModels,
  ollamaRunning,
  onClose,
  onSubmit,
}: CustomRoutingDialogProps) => {
  // Non-aura cloud providers + local-ollama (if available) are the
  // "Custom" options. Aura is excluded — it's the Default path.
  const customCloud = cloudProviders.filter(p => p.slug !== 'aura');
  const localAvailable = ollamaRunning && localModels.length > 0;

  const initialSource: CustomDialogSource | null =
    initial.kind === 'cloud'
      ? { kind: 'cloud', providerSlug: initial.providerSlug }
      : initial.kind === 'local'
        ? { kind: 'local' }
        : customCloud[0]
          ? { kind: 'cloud', providerSlug: customCloud[0].slug }
          : localAvailable
            ? { kind: 'local' }
            : null;

  const [source, setSource] = useState<CustomDialogSource | null>(initialSource);
  const [model, setModel] = useState<string>(() => {
    if (initial.kind === 'cloud' || initial.kind === 'local') return initial.model;
    if (initialSource?.kind === 'cloud') {
      const p = customCloud.find(c => c.slug === initialSource.providerSlug);
      return p ? '' : '';
    }
    return localModels[0]?.id ?? '';
  });

  const selectedCloud =
    source?.kind === 'cloud' ? customCloud.find(c => c.slug === source.providerSlug) : undefined;

  const canSave = source !== null && model.trim().length > 0;

  const handleSave = () => {
    if (!source || !canSave) return;
    if (source.kind === 'cloud') {
      onSubmit({ kind: 'cloud', providerSlug: source.providerSlug, model: model.trim() });
    } else {
      onSubmit({ kind: 'local', model: model.trim() });
    }
  };

  const noProviders = customCloud.length === 0 && !localAvailable;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Custom routing for ${workload.label}`}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4">
      <div className="w-full max-w-md rounded-2xl border border-stone-200 bg-white p-6 shadow-soft">
        <div className="flex items-start justify-between gap-3 mb-4">
          <div>
            <h3 className="text-base font-semibold text-stone-900">Custom routing</h3>
            <p className="mt-0.5 text-xs text-stone-500">{workload.label}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1 text-stone-400 hover:bg-stone-100 hover:text-stone-700">
            <span className="sr-only">Close</span>
            <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {noProviders ? (
          <div className="rounded-lg border border-amber-200 bg-amber-50 p-3 text-xs text-amber-800">
            No custom providers are set up yet. Add a cloud provider key above, or enable the local
            Ollama runtime, then come back to pick one.
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-medium text-stone-700">Provider</label>
              <select
                value={
                  source
                    ? `${source.kind}:${source.kind === 'cloud' ? source.providerSlug : ''}`
                    : ''
                }
                onChange={e => {
                  const colonIdx = e.target.value.indexOf(':');
                  const kind = e.target.value.slice(0, colonIdx);
                  const slug = e.target.value.slice(colonIdx + 1);
                  if (kind === 'local') {
                    setSource({ kind: 'local' });
                    setModel(localModels[0]?.id ?? '');
                  } else if (kind === 'cloud') {
                    setSource({ kind: 'cloud', providerSlug: slug });
                    setModel('');
                  }
                }}
                className="rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                {customCloud.map(p => (
                  <option key={p.slug} value={`cloud:${p.slug}`}>
                    {p.label}
                  </option>
                ))}
                {localAvailable && <option value="local:">Local (Ollama)</option>}
              </select>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-medium text-stone-700">Model</label>
              {source?.kind === 'local' ? (
                <select
                  value={model}
                  onChange={e => setModel(e.target.value)}
                  className="rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                  {localModels.map(m => (
                    <option key={m.id} value={m.id}>
                      {m.id}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={model}
                  onChange={e => setModel(e.target.value)}
                  placeholder={selectedCloud ? `${selectedCloud.slug} model id` : 'model-id'}
                  className="rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm font-mono text-stone-900 placeholder-stone-400 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500"
                />
              )}
            </div>
          </div>
        )}

        <div className="mt-6 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-stone-200 bg-white px-4 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50">
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={!canSave}
            className="rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white hover:bg-primary-600 disabled:cursor-not-allowed disabled:opacity-50">
            Save
          </button>
        </div>
      </div>
    </div>
  );
};

export default CustomRoutingDialog;
