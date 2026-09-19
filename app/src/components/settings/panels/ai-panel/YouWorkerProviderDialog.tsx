import { useState } from 'react';

import { useT } from '../../../../lib/i18n/I18nContext';
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
  const { t } = useT();
  const [label, setLabel] = useState(initial?.label ?? 'YouWorker');
  const [ywBase, setYwBase] = useState(initial?.endpoint ?? '');
  const [autentikaUrl, setAutentikaUrl] = useState(initial?.autentikaUrl ?? '');
  const [persona, setPersona] = useState(initial?.persona ?? '');
  const [clientId, setClientId] = useState(initial?.clientId ?? '');
  const [subjectUuid, setSubjectUuid] = useState(initial?.subjectUuid ?? '');
  const [transport, setTransport] = useState<'webhook' | 'websocket'>(
    initial?.youworkerTransport ?? 'webhook'
  );
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
      aria-label={t('settings.you_worker_provider_dialog.connect_youworker_aria_label')}
      className="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30 p-4">
      <div className="w-full max-w-md rounded-lg border border-stone-200 bg-white shadow-float">
        <div className="border-b border-stone-200 px-4 py-3">
          <div className="text-sm font-semibold text-stone-900">
            {initial
              ? t('settings.you_worker_provider_dialog.edit_youworker_title')
              : t('settings.you_worker_provider_dialog.connect_youworker_title')}
          </div>
          <div className="mt-0.5 text-xs text-stone-500">
            {t('settings.you_worker_provider_dialog.intro_prefix')}{' '}
            <span className="font-mono">auth-profiles.json</span>.
          </div>
        </div>
        <div className="max-h-[60vh] space-y-3 overflow-y-auto px-4 py-3">
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              {t('settings.you_worker_provider_dialog.display_label_label')}
            </label>
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200"
              placeholder="YouWorker"
            />
          </div>
          <div>
            <label
              htmlFor="transport"
              className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              {t('settings.you_worker_provider_dialog.transport_label')}
            </label>
            <select
              id="transport"
              value={transport}
              onChange={e => setTransport(e.target.value as 'webhook' | 'websocket')}
              className="mt-1 w-full rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-900 focus:border-primary-400 focus:outline-none focus:ring-1 focus:ring-primary-200">
              <option value="webhook">
                {t('settings.you_worker_provider_dialog.transport_webhook_option')}
              </option>
              <option value="websocket">
                {t('settings.you_worker_provider_dialog.transport_websocket_option')}
              </option>
            </select>
            <p className="mt-1 text-[10px] text-stone-500">
              {t('settings.you_worker_provider_dialog.transport_hint')}
            </p>
          </div>
          {field(
            'yw-base',
            t('settings.you_worker_provider_dialog.yw_base_label'),
            ywBase,
            setYwBase,
            'https://your-deployment.example/api/v1'
          )}
          {field(
            'autentika-url',
            t('settings.you_worker_provider_dialog.autentika_url_label'),
            autentikaUrl,
            setAutentikaUrl,
            'https://your-workspace.autentika.example'
          )}
          {field(
            'persona',
            t('settings.you_worker_provider_dialog.persona_label'),
            persona,
            setPersona,
            'your-persona-slug'
          )}
          {field(
            'client-id',
            t('settings.you_worker_provider_dialog.client_id_label'),
            clientId,
            setClientId,
            'service-principal client_id'
          )}
          {field(
            'subject-uuid',
            t('settings.you_worker_provider_dialog.subject_uuid_label'),
            subjectUuid,
            setSubjectUuid,
            'youworker end-user UUID'
          )}
          <div>
            <label
              htmlFor="client-secret"
              className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
              <span>{t('settings.you_worker_provider_dialog.client_secret_label')}</span>
              {hasExistingSecret && (
                <button
                  type="button"
                  onClick={() => void onClearSecret()}
                  className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                  {t('settings.you_worker_provider_dialog.clear_stored_secret')}
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
                hasExistingSecret
                  ? t('settings.you_worker_provider_dialog.client_secret_placeholder_existing')
                  : 'client_secret'
              }
            />
          </div>
          {transport === 'webhook' && (
            <div>
              <label
                htmlFor="outbound-hmac-secret"
                className="flex items-center justify-between text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                <span>{t('settings.you_worker_provider_dialog.outbound_hmac_secret_label')}</span>
                <button
                  type="button"
                  onClick={() => void onClearOutboundSecret()}
                  className="text-[10px] font-medium normal-case text-coral-600 hover:text-coral-700">
                  {t('settings.you_worker_provider_dialog.clear_stored_secret')}
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
                {t('settings.you_worker_provider_dialog.outbound_hmac_hint')}
              </p>
            </div>
          )}
          {error ? <p className="text-xs font-medium text-red-600">{error}</p> : null}
        </div>
        <div className="flex items-center justify-end gap-2 border-t border-stone-200 px-4 py-3">
          <button
            onClick={onClose}
            disabled={saving}
            className="rounded-lg border border-stone-200 px-3 py-1.5 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
            {t('settings.you_worker_provider_dialog.cancel')}
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
                        ? t('settings.you_worker_provider_dialog.masked_key_configured')
                        : t('settings.you_worker_provider_dialog.masked_key_not_configured'),
                    autentikaUrl: autentikaUrl.trim(),
                    persona: persona.trim(),
                    clientId: clientId.trim(),
                    subjectUuid: subjectUuid.trim(),
                    youworkerTransport: transport,
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
            {saving
              ? t('settings.you_worker_provider_dialog.saving')
              : initial
                ? t('settings.you_worker_provider_dialog.save_changes')
                : t('settings.you_worker_provider_dialog.connect')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default YouWorkerProviderDialog;
