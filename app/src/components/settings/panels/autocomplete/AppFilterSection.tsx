import { useT } from '../../../../lib/i18n/I18nContext';
import type { AutocompleteStatus } from '../../../../utils/tauriCommands';

interface AppFilterSectionProps {
  status: AutocompleteStatus | null;
  isLoading: boolean;
  contextOverride: string;
  focusDebug: string;
  logs: string[];
  message: string | null;
  error: string | null;
  onSetContextOverride: (value: string) => void;
  onRefreshStatus: () => void;
  onStart: () => void;
  onStop: () => void;
  onTestCurrent: () => void;
  onAcceptSuggestion: () => void;
  onDebugFocus: () => void;
  onClearLogs: () => void;
}

const AppFilterSection = ({
  status,
  isLoading,
  contextOverride,
  focusDebug,
  logs,
  message,
  error,
  onSetContextOverride,
  onRefreshStatus,
  onStart,
  onStop,
  onTestCurrent,
  onAcceptSuggestion,
  onDebugFocus,
  onClearLogs,
}: AppFilterSectionProps) => {
  const { t } = useT();
  const yes = t('settings.app_filter.yes');
  const no = t('settings.app_filter.no');
  return (
    <>
      <section className="rounded-2xl border border-stone-200 bg-white p-4 space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.app_filter.runtime_heading')}
        </h3>
        <div className="text-sm text-stone-700 space-y-1">
          <div>
            {t('settings.app_filter.platform_supported', {
              value: status?.platform_supported ? yes : no,
            })}
          </div>
          <div>{t('settings.app_filter.enabled', { value: status?.enabled ? yes : no })}</div>
          <div>{t('settings.app_filter.running', { value: status?.running ? yes : no })}</div>
          <div>
            {t('settings.app_filter.phase', {
              value: status?.phase ?? t('settings.app_filter.unknown'),
            })}
          </div>
          <div>{t('settings.app_filter.debounce', { value: status?.debounce_ms ?? 0 })}</div>
          <div>
            {t('settings.app_filter.model', {
              value: status?.model_id ?? t('settings.app_filter.not_available'),
            })}
          </div>
          <div>
            {t('settings.app_filter.app', {
              value: status?.app_name ?? t('settings.app_filter.not_available'),
            })}
          </div>
          <div>
            {t('settings.app_filter.last_error', {
              value: status?.last_error ?? t('settings.app_filter.none'),
            })}
          </div>
          <div>
            {t('settings.app_filter.current_suggestion', {
              value: status?.suggestion?.value ?? t('settings.app_filter.none'),
            })}
          </div>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onRefreshStatus}
            disabled={isLoading}
            className="rounded-lg border border-stone-300 bg-stone-100 px-3 py-2 text-sm text-stone-700 disabled:opacity-50">
            {isLoading
              ? t('settings.app_filter.refreshing')
              : t('settings.app_filter.refresh_status')}
          </button>
          <button
            type="button"
            onClick={onStart}
            disabled={!status?.platform_supported || Boolean(status?.running)}
            className="rounded-lg border border-green-500/60 bg-green-50 px-3 py-2 text-sm text-green-700 disabled:opacity-50">
            {t('settings.app_filter.start')}
          </button>
          <button
            type="button"
            onClick={onStop}
            disabled={!status?.running}
            className="rounded-lg border border-red-500/60 bg-red-50 px-3 py-2 text-sm text-red-600 disabled:opacity-50">
            {t('settings.app_filter.stop')}
          </button>
        </div>
      </section>

      <section className="rounded-2xl border border-stone-200 bg-white p-4 space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.app_filter.test_heading')}
        </h3>
        <div className="space-y-1">
          <div className="text-xs text-stone-600">
            {t('settings.app_filter.context_override_label')}
          </div>
          <textarea
            value={contextOverride}
            onChange={event => onSetContextOverride(event.target.value)}
            rows={3}
            className="w-full rounded border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700"
          />
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onTestCurrent}
            className="rounded-lg border border-primary-500/60 bg-primary-50 px-3 py-2 text-sm text-primary-600">
            {t('settings.app_filter.get_suggestion')}
          </button>
          <button
            type="button"
            onClick={onAcceptSuggestion}
            className="rounded-lg border border-emerald-500/60 bg-emerald-50 px-3 py-2 text-sm text-emerald-700">
            {t('settings.app_filter.accept_suggestion')}
          </button>
          <button
            type="button"
            onClick={onDebugFocus}
            className="rounded-lg border border-amber-500/60 bg-amber-50 px-3 py-2 text-sm text-amber-700">
            {t('settings.app_filter.debug_focus')}
          </button>
        </div>
        {focusDebug && (
          <pre className="max-h-48 overflow-auto rounded-xl border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700">
            {focusDebug}
          </pre>
        )}
      </section>

      <section className="rounded-2xl border border-stone-200 bg-white p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-stone-900">
            {t('settings.app_filter.live_logs_heading')}
          </h3>
          <button
            type="button"
            onClick={onClearLogs}
            className="rounded-lg border border-stone-300 bg-stone-100 px-3 py-1.5 text-xs text-stone-700">
            {t('settings.app_filter.clear')}
          </button>
        </div>
        <pre className="max-h-56 overflow-auto rounded-xl border border-stone-200 bg-stone-50 p-2 text-xs text-stone-700">
          {logs.length > 0 ? logs.join('\n') : t('settings.app_filter.no_logs')}
        </pre>
      </section>

      {message && <div className="text-xs text-green-700">{message}</div>}
      {error && <div className="text-xs text-red-600">{error}</div>}
    </>
  );
};

export default AppFilterSection;
