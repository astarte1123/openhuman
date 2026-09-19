import { useT } from '../../../../lib/i18n/I18nContext';
import type { AcceptedCompletion } from '../../../../utils/tauriCommands';

interface CompletionStyleSectionProps {
  enabled: boolean;
  debounceMs: string;
  maxChars: string;
  stylePreset: string;
  styleInstructions: string;
  styleExamplesText: string;
  disabledAppsText: string;
  acceptWithTab: boolean;
  overlayTtlMs: string;
  isSaving: boolean;
  historyEntries: AcceptedCompletion[];
  isHistoryLoading: boolean;
  isClearingHistory: boolean;
  onSetEnabled: (value: boolean) => void;
  onSetDebounceMs: (value: string) => void;
  onSetMaxChars: (value: string) => void;
  onSetStylePreset: (value: string) => void;
  onSetStyleInstructions: (value: string) => void;
  onSetStyleExamplesText: (value: string) => void;
  onSetDisabledAppsText: (value: string) => void;
  onSetAcceptWithTab: (value: boolean) => void;
  onSetOverlayTtlMs: (value: string) => void;
  onSaveConfig: () => void;
  onClearHistory: () => void;
}

const CompletionStyleSection = ({
  enabled,
  debounceMs,
  maxChars,
  stylePreset,
  styleInstructions,
  styleExamplesText,
  disabledAppsText,
  acceptWithTab,
  overlayTtlMs,
  isSaving,
  historyEntries,
  isHistoryLoading,
  isClearingHistory,
  onSetEnabled,
  onSetDebounceMs,
  onSetMaxChars,
  onSetStylePreset,
  onSetStyleInstructions,
  onSetStyleExamplesText,
  onSetDisabledAppsText,
  onSetAcceptWithTab,
  onSetOverlayTtlMs,
  onSaveConfig,
  onClearHistory,
}: CompletionStyleSectionProps) => {
  const { t } = useT();
  return (
    <>
      <section className="rounded-2xl border border-stone-200 bg-white p-4 space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.completion_style.settings_title')}
        </h3>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.enabled_label')}
          </span>
          <input
            type="checkbox"
            checked={enabled}
            onChange={event => onSetEnabled(event.target.checked)}
          />
        </label>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.accept_with_tab_label')}
          </span>
          <input
            type="checkbox"
            checked={acceptWithTab}
            onChange={event => onSetAcceptWithTab(event.target.checked)}
          />
        </label>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.debounce_label')}
          </span>
          <input
            type="number"
            min={50}
            max={2000}
            step={10}
            value={debounceMs}
            onChange={event => onSetDebounceMs(event.target.value)}
            className="w-28 rounded border border-stone-300 bg-white px-2 py-1 text-xs text-stone-700"
          />
        </label>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.max_chars_label')}
          </span>
          <input
            type="number"
            min={32}
            max={1200}
            step={8}
            value={maxChars}
            onChange={event => onSetMaxChars(event.target.value)}
            className="w-28 rounded border border-stone-300 bg-white px-2 py-1 text-xs text-stone-700"
          />
        </label>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.overlay_ttl_label')}
          </span>
          <input
            type="number"
            min={300}
            max={10000}
            step={100}
            value={overlayTtlMs}
            onChange={event => onSetOverlayTtlMs(event.target.value)}
            className="w-28 rounded border border-stone-300 bg-white px-2 py-1 text-xs text-stone-700"
          />
        </label>
        <label className="flex items-center justify-between rounded-xl border border-stone-200 bg-stone-50 px-3 py-2">
          <span className="text-sm text-stone-700">
            {t('settings.completion_style.style_preset_label')}
          </span>
          <select
            value={stylePreset}
            onChange={event => onSetStylePreset(event.target.value)}
            className="rounded border border-stone-300 bg-white px-2 py-1 text-xs text-stone-700">
            <option value="balanced">{t('settings.completion_style.preset_balanced')}</option>
            <option value="concise">{t('settings.completion_style.preset_concise')}</option>
            <option value="formal">{t('settings.completion_style.preset_formal')}</option>
            <option value="casual">{t('settings.completion_style.preset_casual')}</option>
            <option value="custom">{t('settings.completion_style.preset_custom')}</option>
          </select>
        </label>
        <div className="space-y-1">
          <div className="text-xs text-stone-600">
            {t('settings.completion_style.style_instructions_label')}
          </div>
          <textarea
            value={styleInstructions}
            onChange={event => onSetStyleInstructions(event.target.value)}
            rows={3}
            className="w-full rounded border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700"
          />
        </div>
        <div className="space-y-1">
          <div className="text-xs text-stone-600">
            {t('settings.completion_style.style_examples_label')}
          </div>
          <textarea
            value={styleExamplesText}
            onChange={event => onSetStyleExamplesText(event.target.value)}
            rows={3}
            className="w-full rounded border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700"
          />
        </div>
        <div className="space-y-1">
          <div className="text-xs text-stone-600">
            {t('settings.completion_style.disabled_apps_label')}
          </div>
          <textarea
            value={disabledAppsText}
            onChange={event => onSetDisabledAppsText(event.target.value)}
            rows={3}
            className="w-full rounded border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700"
          />
        </div>
        <button
          type="button"
          onClick={onSaveConfig}
          disabled={isSaving}
          className="rounded-lg border border-primary-500/60 bg-primary-50 px-3 py-2 text-sm text-primary-600 disabled:opacity-50">
          {isSaving
            ? t('settings.completion_style.saving')
            : t('settings.completion_style.save_autocomplete_settings')}
        </button>
      </section>

      <section className="rounded-2xl border border-stone-200 bg-white p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-stone-900">
            {t('settings.completion_style.personalization_history_title')}
          </h3>
          <button
            type="button"
            onClick={onClearHistory}
            disabled={isClearingHistory || historyEntries.length === 0}
            className="rounded-lg border border-red-500/60 bg-red-50 px-3 py-1.5 text-xs text-red-600 disabled:opacity-40">
            {isClearingHistory
              ? t('settings.completion_style.clearing')
              : t('settings.completion_style.clear_history')}
          </button>
        </div>
        <p className="text-xs text-stone-500">
          {isHistoryLoading
            ? t('settings.completion_style.loading')
            : historyEntries.length === 0
              ? t('settings.completion_style.no_history_yet')
              : t('settings.completion_style.history_count_stored', {
                  count: historyEntries.length,
                })}
        </p>
        {historyEntries.length > 0 && (
          <div className="max-h-48 overflow-y-auto rounded-xl border border-stone-200 bg-stone-50 p-2 space-y-1">
            {historyEntries.map((entry, idx) => (
              <div
                key={`${String(entry.timestamp_ms)}-${String(idx)}`}
                className="flex flex-col gap-0.5 rounded-lg bg-white px-2 py-1.5 text-xs border border-stone-100">
                <div className="flex items-center gap-2 text-stone-500">
                  <span className="shrink-0">{new Date(entry.timestamp_ms).toLocaleString()}</span>
                  {entry.app_name && (
                    <span className="rounded bg-stone-100 px-1 text-stone-600">
                      {entry.app_name}
                    </span>
                  )}
                </div>
                <div className="flex items-baseline gap-1 text-stone-700 truncate">
                  <span className="shrink-0 text-stone-400">…</span>
                  <span className="truncate text-stone-500">{entry.context.slice(-40)}</span>
                  <span className="shrink-0 text-stone-400">→</span>
                  <span className="font-medium text-primary-500 truncate">{entry.suggestion}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </>
  );
};

export default CompletionStyleSection;
