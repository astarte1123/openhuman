import { useT } from '../../../../lib/i18n/I18nContext';

import type { CloudProvider, OllamaModel, OllamaState, ProviderRef, Workload } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// Workload row (stacked, narrow-friendly)
// ─────────────────────────────────────────────────────────────────────────────

type WorkloadRowProps = {
  workload: Workload;
  ref_: ProviderRef;
  cloudProviders: CloudProvider[];
  localModels: OllamaModel[];
  ollamaState: OllamaState;
  onChange: (next: ProviderRef) => void;
};

const WorkloadRow = ({
  workload,
  ref_,
  cloudProviders,
  localModels,
  ollamaState,
  onChange,
  onCustomClick,
}: WorkloadRowProps & { onCustomClick: () => void }) => {
  const { t } = useT();
  const selectedCloud =
    ref_.kind === 'cloud' ? cloudProviders.find(c => c.slug === ref_.providerSlug) : undefined;

  const isDefault = ref_.kind === 'aura';

  let resolved: string;
  if (ref_.kind === 'aura') {
    resolved = t('settings.workload_row.aura_default');
  } else if (ref_.kind === 'cloud') {
    if (!selectedCloud) resolved = `${ref_.providerSlug} · ${ref_.model}`;
    else resolved = `${selectedCloud.label} · ${ref_.model}`;
  } else {
    resolved = `Ollama · ${ref_.model}`;
  }

  // Quiet `ollamaState` / `localModels` unused-prop warnings — they're still
  // consumed by the parent's onChange wiring through `onCustomClick`.
  void ollamaState;
  void localModels;

  const segmentBase =
    'flex-1 px-3 py-1.5 text-xs font-medium rounded-md transition-colors cursor-pointer';
  const activeSegment = 'bg-white text-stone-900 shadow-subtle ring-1 ring-stone-200';
  const inactiveSegment = 'text-stone-500 hover:text-stone-800';

  return (
    <div className="flex items-center justify-between gap-3 py-3">
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-stone-900">{workload.label}</div>
        <div className="truncate text-xs text-stone-500">{workload.description}</div>
        <div className="mt-0.5 font-mono text-[11px] text-stone-400 truncate">↳ {resolved}</div>
      </div>
      <div className="inline-flex shrink-0 items-center rounded-lg bg-stone-100 p-0.5">
        <button
          type="button"
          onClick={() => onChange({ kind: 'aura' })}
          className={`${segmentBase} ${isDefault ? activeSegment : inactiveSegment}`}>
          {t('settings.workload_row.default_button')}
        </button>
        <button
          type="button"
          onClick={onCustomClick}
          className={`${segmentBase} ${!isDefault ? activeSegment : inactiveSegment}`}>
          {t('settings.workload_row.custom_button')}
        </button>
      </div>
    </div>
  );
};

export default WorkloadRow;
