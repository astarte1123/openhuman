import { useState } from 'react';

import type { CloudProvider } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// youworker.ai dialog — Autentika token-exchange params + the native WS base
// URL. More fields than the generic ProviderKeyDialog/CloudProviderEditor
// need, so it gets its own small form. Ships with every field empty: these
// are real credentials for a specific youworker.ai deployment (dev or prod)
// and must never be pre-filled with a shared/committed value.
// ─────────────────────────────────────────────────────────────────────────────

const YouWorkerProviderDialog = ({
  initial,
  hasExistingSecret,
  onClose,
  onSubmit,
  onClearSecret,
  onClearOutboundSecret,
}: {
  initial: CloudProvider | null;
  hasExistingSecret: boolean;
  onClose: () => void;
  onSubmit: (
    next: CloudProvider,
    clientSecret: string,
    outboundHmacSecret: string
  ) => Promise<void> | void;
  onClearSecret: () => Promise<void> | void;
  onClearOutboundSecret: () => Promise<void> | void;
}) => {
  const [label, setLabel] = useState(initial?.label ?? 'YouWorker');
  const [ywBase, setYwBase] = useState(initial?.endpoint ?? '');
  const [autentikaUrl, setAutentikaUrl] = useState(initial?.autentikaUrl ?? '');
  const [persona, setPersona] = useState(initial?.persona ?? '');
  const [clientId, setClientId] = useState(initial?.clientId ?? '');
  const [subjectUuid, setSubjectUuid] = useState(initial?.subjectUuid ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [outboundHmacSecret, setOutboundHmacSecret] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSave =
    ywBase.trim() &&
    autentikaUrl.trim() &&
    persona.trim() &&
    clientId.trim() &&
    subjectUuid.trim() &&
    (hasExistingSecret || clientSecret.trim());

  const field = (
    id: string,
    labelText: string,
    value: string,
    setValue: (v: string) => void,
    placeholder: string,
    type: 'text' | 'password' = 'text'
  ) => (
    <div>
      <label
        htmlFor={id}
        className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
        {labelText}
      </label>
      <input
        id={id}
        type={type}
        autoComplete="off"
        value={value}
        onChange={e => setValue(e.target.value)}
        className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
        placeholder={placeholder}
      />
    </div>
  );

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Connect YouWorker"
      className="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30 p-4">
      <div className="w-full max-w-md rounded-lg border border-stone-200 bg-white shadow-float">
        <div className="border-b border-stone-200 px-4 py-3">
          <div className="text-sm font-semibold text-stone-900">
            {initial ? 'Edit YouWorker' : 'Connect YouWorker'}
          </div>
          <div className="mt-0.5 text-xs text-stone-500">
            Native WebSocket transport. Params come from your youworker.ai deployment's integration
            handoff doc — the client secret is encrypted at rest in{' '}
            <span className="font-mono">auth-profiles.json</span>.
          </div>
        </div>
        <div className="max-h-[60vh] space-y-3 overflow-y-auto px-4 py-3">
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              Display label
            </label>
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder="YouWorker"
            />
          </div>
          {field(
            'yw-base',
            'YouWorker base URL (yw_base)',
            ywBase,
            setYwBase,
            'https://your-deployment.example/api/v1'
          )}
          {field(
            'autentika-url',
            'Autentika URL',
            autentikaUrl,
            setAutentikaUrl,
            'https://your-workspace.autentika.example'
          )}
          {field('persona', 'Persona', persona, setPersona, 'your-persona-slug')}
          {field('client-id', 'Client ID', clientId, setClientId, 'service-principal client_id')}
          {field(
            'subject-uuid',
            'Subject UUID',
            subjectUuid,
            setSubjectUuid,
            'youworker end-user UUID'
          )}
          <div>
            <label
              htmlFor="client-secret"
              className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              <span>Client secret</span>
              {hasExistingSecret && (
                <button
                  type="button"
                  onClick={() => void onClearSecret()}
                  className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                  Clear stored secret
                </button>
              )}
            </label>
            <input
              id="client-secret"
              type="password"
              autoComplete="off"
              value={clientSecret}
              onChange={e => setClientSecret(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder={
                hasExistingSecret ? 'Leave blank to keep existing secret' : 'client_secret'
              }
            />
          </div>
          <div>
            <label
              htmlFor="outbound-hmac-secret"
              className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              <span>Outbound webhook HMAC secret</span>
              <button
                type="button"
                onClick={() => void onClearOutboundSecret()}
                className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                Clear stored secret
              </button>
            </label>
            <input
              id="outbound-hmac-secret"
              type="password"
              autoComplete="off"
              value={outboundHmacSecret}
              onChange={e => setOutboundHmacSecret(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 font-mono text-xs text-stone-900 placeholder:text-stone-400 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder="whsec_… (leave blank to keep unchanged)"
            />
            <p className="mt-1 text-[10px] text-stone-500">
              Verifies youworker&apos;s async reply webhook. Optional to save now, but chat calls
              will time out without it once youworker.ai can reach this app&apos;s outbound
              endpoint.
            </p>
          </div>
          {error ? <p className="text-xs font-medium text-red-600">{error}</p> : null}
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
              setError(null);
              setSaving(true);
              try {
                await onSubmit(
                  {
                    id: initial?.id ?? '',
                    slug: 'youworker',
                    label: label.trim() || 'YouWorker',
                    endpoint: ywBase.trim(),
                    authStyle: 'youworker',
                    maskedKey:
                      hasExistingSecret || clientSecret.trim()
                        ? '•••• configured'
                        : 'Not configured',
                    autentikaUrl: autentikaUrl.trim(),
                    persona: persona.trim(),
                    clientId: clientId.trim(),
                    subjectUuid: subjectUuid.trim(),
                  },
                  clientSecret.trim(),
                  outboundHmacSecret.trim()
                );
              } catch (err) {
                setError(err instanceof Error ? err.message : String(err));
              } finally {
                setSaving(false);
              }
            }}
            disabled={saving || !canSave}
            className="rounded-lg bg-primary-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-600 disabled:opacity-50">
            {saving ? 'Saving…' : initial ? 'Save changes' : 'Connect'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default YouWorkerProviderDialog;
