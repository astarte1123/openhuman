/*
 * AI settings — three orthogonal sections:
 *   1. Cloud providers (credentials + primary selection)
 *   2. Local provider (Ollama runtime + installed models)
 *   3. Workload routing (8-row matrix; per-workload provider + model)
 *
 * "Primary cloud" is an abstraction: any workload set to "Primary" inherits
 * whichever cloud provider is currently marked primary. Overrides are explicit
 * per row, so the resolved provider+model is always rendered inline.
 *
 * Implementation is split across ./ai-panel/* — this file wires the pieces
 * together (state, dialogs, layout) but owns no rendering primitives itself.
 */
import { useMemo, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import {
  clearCloudProviderKey,
  clearYouWorkerOutboundSecret,
  setCloudProviderKey,
  setYouWorkerOutboundSecret,
} from '../../../services/api/aiSettingsApi';
import SettingsHeader from '../components/SettingsHeader';
import { useSettingsNavigation } from '../hooks/useSettingsNavigation';
import BackgroundLoopControls from './ai-panel/BackgroundLoopControls';
import {
  BUILTIN_PROVIDER_META,
  LOCAL_CHIP_LABEL,
  LOCAL_CHIP_TONE,
  WORKLOADS,
} from './ai-panel/catalog';
import CloudProviderEditor from './ai-panel/CloudProviderEditor';
import CustomRoutingDialog from './ai-panel/CustomRoutingDialog';
import ProviderKeyDialog from './ai-panel/ProviderKeyDialog';
import ProviderToggleChip from './ai-panel/ProviderToggleChip';
import SaveBar from './ai-panel/SaveBar';
import { authStyleForSlug, defaultEndpointFor, maskKeyLabel } from './ai-panel/transforms';
import type { CloudProvider, ProviderRef, WorkloadId } from './ai-panel/types';
import { useAISettings } from './ai-panel/useAISettings';
import { useInstalledModels } from './ai-panel/useInstalledModels';
import { useOllamaStatus } from './ai-panel/useOllamaStatus';
import WorkloadRow from './ai-panel/WorkloadRow';
import YouWorkerProviderDialog from './ai-panel/YouWorkerProviderDialog';

interface AIPanelProps {
  /** When true, the panel is rendered embedded inside another flow (e.g. the
   *  onboarding custom wizard) and skips its own SettingsHeader chrome so the
   *  host frame's title/back controls aren't duplicated. */
  embedded?: boolean;
}

const AIPanel = ({ embedded = false }: AIPanelProps = {}) => {
  const { t } = useT();
  const { navigateBack, breadcrumbs } = useSettingsNavigation();
  const { saved, draft, setDraft, isDirty, save, discard, loading, error, reload } =
    useAISettings();
  const ollama = useOllamaStatus();
  const installed = useInstalledModels(ollama.snapshot);
  const [editing, setEditing] = useState<CloudProvider | 'new' | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  // Which workload's "Custom" dialog is currently open (null = closed).
  const [customDialogFor, setCustomDialogFor] = useState<WorkloadId | null>(null);
  // Which provider slug's API-key dialog is currently open (null = closed).
  const [keyDialogFor, setKeyDialogFor] = useState<string | null>(null);
  // Whether the YouWorker connect dialog is open (its own multi-field form).
  const [youworkerDialogOpen, setYouworkerDialogOpen] = useState(false);
  // When the user toggles LM Studio / Ollama (local runtimes), we
  // need to remember which label to attach to the upserted provider so the
  // chip can find it again. Cleared when the dialog closes.
  const [pendingLocalLabel, setPendingLocalLabel] = useState<string | null>(null);

  const updateRouting = (id: WorkloadId, next: ProviderRef) =>
    setDraft({ ...draft, routing: { ...draft.routing, [id]: next } });

  // applyPreset removed alongside the Cloud / Local / Mixed preset pills —
  // the new Default/Custom binary toggle handles routing per workload.

  const diffSummary = useMemo(() => {
    const out: string[] = [];

    // Provider connect/disconnect/edit — added or removed providers, or
    // an existing one whose fields changed (new key, new endpoint, …).
    const savedById = new Map(saved.cloudProviders.map(cp => [cp.id, cp]));
    const draftById = new Map(draft.cloudProviders.map(cp => [cp.id, cp]));
    for (const [id, cp] of draftById) {
      if (!savedById.has(id)) {
        out.push(`+ ${cp.label}`);
      } else if (JSON.stringify(savedById.get(id)) !== JSON.stringify(cp)) {
        out.push(`~ ${cp.label}`);
      }
    }
    for (const [id, cp] of savedById) {
      if (!draftById.has(id)) {
        out.push(`- ${cp.label}`);
      }
    }

    for (const w of WORKLOADS) {
      const a = saved.routing[w.id];
      const b = draft.routing[w.id];
      if (JSON.stringify(a) !== JSON.stringify(b)) {
        const describe = (r: ProviderRef) =>
          r.kind === 'aura'
            ? 'aura'
            : r.kind === 'cloud'
              ? `${r.providerSlug}:${r.model}`
              : `local:${r.model}`;
        out.push(`${w.label} → ${describe(b)}`);
      }
    }
    return out;
  }, [saved, draft]);

  const chatRows = WORKLOADS.filter(w => w.group === 'chat');
  const bgRows = WORKLOADS.filter(w => w.group === 'background');

  return (
    <div className="relative">
      {!embedded && (
        <SettingsHeader
          title={t('settings.ai_panel.header_title')}
          showBackButton
          onBack={navigateBack}
          breadcrumbs={breadcrumbs}
        />
      )}

      <div className={embedded ? 'space-y-6' : 'space-y-6 p-4'}>
        {/* ═══════════════════════════════════════════════════════════════
            AUTH — provider authentication (cloud providers + local Ollama
            setup). Everything the user needs to wire a model up.
            ═══════════════════════════════════════════════════════════════ */}
        <div className="space-y-4">
          <div className="border-b border-stone-200 pb-2">
            <h2 className="text-base font-semibold text-stone-900">
              {t('settings.ai_panel.providers_heading')}
            </h2>
            <p className="text-xs text-stone-500 mt-0.5">
              {t('settings.ai_panel.providers_subtitle')}
            </p>
          </div>

          {/* ─── Provider chip-toggle list ────────────────────────────────── */}
          <section className="space-y-3">
            {loading && (
              <div className="text-xs text-stone-500">{t('settings.ai_panel.loading')}</div>
            )}
            {error && (
              <div className="rounded-md border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
                {error}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              {/* Built-in cloud providers — openai/anthropic/openrouter/youworker/custom */}
              {(['openai', 'anthropic', 'openrouter', 'youworker', 'custom'] as const).map(slug => {
                const meta = BUILTIN_PROVIDER_META[slug];
                const label = meta?.label ?? slug;
                const existing = draft.cloudProviders.find(cp => cp.slug === slug);
                const enabled = !!existing;
                return (
                  <ProviderToggleChip
                    key={slug}
                    slug={slug}
                    label={label}
                    enabled={enabled}
                    busy={busyAction === `toggle-${slug}`}
                    onToggle={() => {
                      if (enabled && existing) {
                        // Toggle OFF: remove the provider + scrub any
                        // routing entries that pin to it.
                        const remaining = draft.cloudProviders.filter(cp => cp.id !== existing.id);
                        const nextRouting = Object.fromEntries(
                          Object.entries(draft.routing).map(([wid, ref]) => [
                            wid,
                            ref.kind === 'cloud' && ref.providerSlug === existing.slug
                              ? ({ kind: 'aura' } as const)
                              : ref,
                          ])
                        ) as typeof draft.routing;
                        setDraft({ ...draft, cloudProviders: remaining, routing: nextRouting });
                      } else if (slug === 'custom') {
                        // Custom providers need slug + endpoint + label, not
                        // just an API key — defer to the full editor modal.
                        setEditing('new');
                      } else if (slug === 'youworker') {
                        // Needs several params (yw_base, Autentika URL,
                        // persona, client id/secret, subject uuid) — its own
                        // dialog, not the single-field API-key popup.
                        setYouworkerDialogOpen(true);
                      } else {
                        // Toggle ON: open the API-key popup. The chip
                        // only flips after the dialog saves.
                        setKeyDialogFor(slug);
                      }
                    }}
                  />
                );
              })}

              {/* LM Studio + Ollama — local runtimes stored with a slug of
                  "lmstudio" / "ollama" so they're distinct from generic custom. */}
              {(['lmstudio', 'ollama'] as const).map(localKind => {
                const label = LOCAL_CHIP_LABEL[localKind];
                const tone = LOCAL_CHIP_TONE[localKind];
                const existing = draft.cloudProviders.find(cp => cp.slug === localKind);
                const enabled = !!existing;
                // Use a styled chip directly for local runtimes — they have
                // non-standard tones not in BUILTIN_PROVIDER_META.
                return (
                  <div
                    key={localKind}
                    className={`inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-medium ring-1 transition-colors ${tone}`}>
                    <span>{label}</span>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={enabled}
                      aria-label={t(
                        enabled
                          ? 'settings.ai_panel.local_runtime_disconnect_aria'
                          : 'settings.ai_panel.local_runtime_connect_aria',
                        { label }
                      )}
                      disabled={busyAction === `toggle-${localKind}`}
                      onClick={() => {
                        if (enabled && existing) {
                          const remaining = draft.cloudProviders.filter(
                            cp => cp.id !== existing.id
                          );
                          const nextRouting = Object.fromEntries(
                            Object.entries(draft.routing).map(([wid, ref]) => [
                              wid,
                              ref.kind === 'cloud' && ref.providerSlug === localKind
                                ? ({ kind: 'aura' } as const)
                                : ref,
                            ])
                          ) as typeof draft.routing;
                          setDraft({ ...draft, cloudProviders: remaining, routing: nextRouting });
                        } else {
                          setKeyDialogFor(localKind);
                          setPendingLocalLabel(label);
                        }
                      }}
                      className={`relative inline-flex h-4 w-7 shrink-0 items-center rounded-full transition-colors disabled:cursor-wait disabled:opacity-60 ${enabled ? 'bg-primary-500' : 'bg-stone-300'}`}>
                      <span
                        aria-hidden
                        className={`inline-block h-3 w-3 transform rounded-full bg-white shadow transition-transform ${enabled ? 'translate-x-3.5' : 'translate-x-0.5'}`}
                      />
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        </div>
        {/* end of Auth section */}

        {/* ═══════════════════════════════════════════════════════════════
            ROUTING — which workload uses which model. Each row is a
            binary toggle: Default (let Aura pick) or Custom (opens
            a popup to choose provider + model).
            ═══════════════════════════════════════════════════════════════ */}
        <div className="space-y-4">
          <div className="border-b border-stone-200 pb-2">
            <h2 className="text-base font-semibold text-stone-900">
              {t('settings.ai_panel.routing_heading')}
            </h2>
            <p className="text-xs text-stone-500 mt-0.5">
              {t('settings.ai_panel.routing_subtitle')}
            </p>
          </div>

          <section className="space-y-3">
            <div className="overflow-hidden rounded-lg border border-stone-200 bg-stone-50 px-3">
              <div className="pt-3">
                <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                  {t('settings.ai_panel.chat_group_label')}
                </div>
                <div className="divide-y divide-stone-200">
                  {chatRows.map(w => (
                    <WorkloadRow
                      key={w.id}
                      workload={w}
                      ref_={draft.routing[w.id]}
                      cloudProviders={draft.cloudProviders}
                      localModels={installed}
                      ollamaState={ollama.state}
                      onChange={next => updateRouting(w.id, next)}
                      onCustomClick={() => setCustomDialogFor(w.id)}
                    />
                  ))}
                </div>
              </div>
              <div className="pb-3 pt-3">
                <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                  {t('settings.ai_panel.background_group_label')}
                </div>
                <div className="divide-y divide-stone-200">
                  {bgRows.map(w => (
                    <WorkloadRow
                      key={w.id}
                      workload={w}
                      ref_={draft.routing[w.id]}
                      cloudProviders={draft.cloudProviders}
                      localModels={installed}
                      ollamaState={ollama.state}
                      onChange={next => updateRouting(w.id, next)}
                      onCustomClick={() => setCustomDialogFor(w.id)}
                    />
                  ))}
                </div>
              </div>
            </div>

            <div className="text-[11px] text-stone-500">
              {t('settings.ai_panel.default_resolves_prefix')}{' '}
              <span className="font-mono text-stone-700">Youma</span>
              {t('settings.ai_panel.default_resolves_suffix')}
            </div>
          </section>
        </div>
        {/* end of Routing section */}

        <BackgroundLoopControls routing={draft.routing} cloudProviders={draft.cloudProviders} />
      </div>

      {isDirty && (
        <SaveBar
          diffSummary={diffSummary}
          changeCount={diffSummary.length}
          onSave={() => void save()}
          onDiscard={discard}
        />
      )}

      {editing && (
        <CloudProviderEditor
          initial={editing === 'new' ? null : editing}
          existingSlugs={draft.cloudProviders
            .filter(p => p.id !== (editing === 'new' ? '' : editing.id))
            .map(p => p.slug)}
          onClose={() => setEditing(null)}
          onSubmit={async (next, apiKey) => {
            setBusyAction('save-provider');
            try {
              const id =
                editing === 'new' || !editing.id
                  ? `p_${next.slug}_${Math.random().toString(36).slice(2, 7)}`
                  : editing.id;
              const upserted: CloudProvider = {
                ...next,
                id,
                maskedKey: maskKeyLabel(apiKey ? true : next.maskedKey.startsWith('••••')),
              };
              // Persist the credential BEFORE mutating draft, so a key-write
              // failure doesn't leave the config referencing a provider with
              // no stored key.
              if (apiKey && upserted.slug !== 'aura') {
                try {
                  await setCloudProviderKey(upserted.slug, apiKey);
                } catch (err) {
                  const msg = err instanceof Error ? err.message : String(err);
                  console.warn('[ai-settings] setCloudProviderKey failed', msg);
                  return;
                }
              }
              const list =
                editing === 'new'
                  ? [...draft.cloudProviders, upserted]
                  : draft.cloudProviders.map(p => (p.id === editing.id ? upserted : p));
              setDraft({ ...draft, cloudProviders: list });
              setEditing(null);
            } finally {
              setBusyAction(null);
            }
          }}
          onClearKey={async slug => {
            try {
              await clearCloudProviderKey(slug);
              await reload();
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              console.warn('[ai-settings] clearCloudProviderKey failed', msg);
            }
          }}
        />
      )}

      {customDialogFor &&
        (() => {
          const w = WORKLOADS.find(x => x.id === customDialogFor);
          if (!w) return null;
          return (
            <CustomRoutingDialog
              workload={w}
              initial={draft.routing[customDialogFor]}
              cloudProviders={draft.cloudProviders}
              localModels={installed}
              ollamaRunning={ollama.state === 'running'}
              onClose={() => setCustomDialogFor(null)}
              onSubmit={next => {
                updateRouting(customDialogFor, next);
                setCustomDialogFor(null);
              }}
            />
          );
        })()}

      {keyDialogFor && (
        <ProviderKeyDialog
          slug={keyDialogFor}
          label={pendingLocalLabel ?? BUILTIN_PROVIDER_META[keyDialogFor]?.label ?? keyDialogFor}
          onCancel={() => {
            setKeyDialogFor(null);
            setPendingLocalLabel(null);
          }}
          onSubmit={async apiKey => {
            const slug = keyDialogFor;
            const localLabel = pendingLocalLabel;
            setBusyAction(
              `toggle-${localLabel ? localLabel.toLowerCase().replace(/\s/g, '') : slug}`
            );
            try {
              // For LM Studio / Ollama the dialog's "API key" field is
              // actually the local endpoint URL, so persist it as endpoint
              // and skip the credential save (no remote key to store).
              const isLocalRuntime = Boolean(localLabel);
              const upserted: CloudProvider = {
                id: `p_${slug}_${Math.random().toString(36).slice(2, 7)}`,
                slug,
                label: localLabel ?? BUILTIN_PROVIDER_META[slug]?.label ?? slug,
                endpoint: isLocalRuntime ? apiKey.trim() : defaultEndpointFor(slug),
                authStyle: authStyleForSlug(slug),
                maskedKey: maskKeyLabel(true),
              };
              // Persist the credential BEFORE mutating draft, so a key-write
              // failure can't leave config + secrets out of sync.
              if (!isLocalRuntime && slug !== 'aura') {
                try {
                  await setCloudProviderKey(slug, apiKey);
                } catch (err) {
                  const msg = err instanceof Error ? err.message : String(err);
                  console.warn('[ai-settings] setCloudProviderKey failed', msg);
                  return;
                }
              }
              setDraft({ ...draft, cloudProviders: [...draft.cloudProviders, upserted] });
              setKeyDialogFor(null);
              setPendingLocalLabel(null);
            } finally {
              setBusyAction(null);
            }
          }}
        />
      )}

      {youworkerDialogOpen && (
        <YouWorkerProviderDialog
          initial={draft.cloudProviders.find(cp => cp.slug === 'youworker') ?? null}
          hasExistingSecret={(
            draft.cloudProviders.find(cp => cp.slug === 'youworker')?.maskedKey ?? ''
          ).startsWith('••••')}
          onClose={() => setYouworkerDialogOpen(false)}
          onSubmit={async (next, clientSecret, outboundHmacSecret) => {
            setBusyAction('toggle-youworker');
            try {
              const existing = draft.cloudProviders.find(cp => cp.slug === 'youworker');
              const id = existing?.id ?? `p_youworker_${Math.random().toString(36).slice(2, 7)}`;
              const upserted: CloudProvider = { ...next, id };
              if (clientSecret) {
                await setCloudProviderKey('youworker', clientSecret);
              }
              if (outboundHmacSecret) {
                await setYouWorkerOutboundSecret('youworker', outboundHmacSecret);
              }
              const list = existing
                ? draft.cloudProviders.map(p => (p.id === existing.id ? upserted : p))
                : [...draft.cloudProviders, upserted];
              setDraft({ ...draft, cloudProviders: list });
              setYouworkerDialogOpen(false);
            } finally {
              setBusyAction(null);
            }
          }}
          onClearSecret={async () => {
            try {
              await clearCloudProviderKey('youworker');
              await reload();
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              console.warn('[ai-settings] clearCloudProviderKey(youworker) failed', msg);
            }
          }}
          onClearOutboundSecret={async () => {
            try {
              await clearYouWorkerOutboundSecret('youworker');
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              console.warn('[ai-settings] clearYouWorkerOutboundSecret failed', msg);
            }
          }}
        />
      )}
    </div>
  );
};

export default AIPanel;
