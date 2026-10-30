import { useCallback } from 'react';

import {
  type ScreenIntelligenceState,
  useScreenIntelligenceState,
} from '../../features/screen-intelligence/useScreenIntelligenceState';
import { useT } from '../../lib/i18n/I18nContext';

const formatBytes = (bytes: number | null | undefined): string => {
  if (bytes == null) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
};

interface ScreenIntelligenceDebugPanelProps {
  state?: Pick<
    ScreenIntelligenceState,
    | 'status'
    | 'captureTestResult'
    | 'isCaptureTestRunning'
    | 'recentVisionSummaries'
    | 'lastError'
    | 'refreshStatus'
    | 'refreshVision'
    | 'runCaptureTest'
  >;
}

const ScreenIntelligenceDebugPanelContent = ({
  state: providedState,
}: Required<Pick<ScreenIntelligenceDebugPanelProps, 'state'>>) => {
  const { t } = useT();
  const {
    status,
    captureTestResult,
    isCaptureTestRunning,
    recentVisionSummaries,
    lastError,
    refreshStatus,
    refreshVision,
    runCaptureTest,
  } = providedState;

  const handleCaptureTest = useCallback(() => {
    void runCaptureTest();
  }, [runCaptureTest]);

  const handleRefreshStatus = useCallback(() => {
    void refreshStatus();
    void refreshVision(5);
  }, [refreshStatus, refreshVision]);

  const permissions = status?.permissions;
  const session = status?.session;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-stone-100">
          {t('intelligence.screen_debug_panel.debug_diagnostics_title')}
        </h3>
        <button
          onClick={handleRefreshStatus}
          className="rounded-lg border border-stone-700 bg-stone-800/60 px-3 py-1 text-xs text-stone-300 transition-colors hover:bg-stone-700/60">
          {t('intelligence.screen_debug_panel.refresh')}
        </button>
      </div>

      {/* Permissions */}
      <div className="rounded-xl border border-stone-700 bg-stone-900/50 p-3">
        <h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-stone-400">
          {t('intelligence.screen_debug_panel.permissions_heading')}
        </h4>
        {status?.platform_supported === false ? (
          <p className="text-xs text-stone-500">
            {t('intelligence.screen_debug_panel.platform_not_supported')}
          </p>
        ) : (
          <div className="grid grid-cols-3 gap-2 text-xs">
            <PermissionDot
              label={t('intelligence.screen_debug_panel.permission_screen')}
              value={permissions?.screen_recording}
            />
            <PermissionDot
              label={t('intelligence.screen_debug_panel.permission_accessibility')}
              value={permissions?.accessibility}
            />
            <PermissionDot
              label={t('intelligence.screen_debug_panel.permission_input')}
              value={permissions?.input_monitoring}
            />
          </div>
        )}
      </div>

      {/* Session Status */}
      <div className="rounded-xl border border-stone-700 bg-stone-900/50 p-3">
        <h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-stone-400">
          {t('intelligence.screen_debug_panel.session_heading')}
        </h4>
        <div className="space-y-1 text-xs text-stone-300">
          <div className="flex justify-between">
            <span>{t('intelligence.screen_debug_panel.active_label')}</span>
            <span className={session?.active ? 'text-green-400' : 'text-stone-500'}>
              {session?.active
                ? t('intelligence.screen_debug_panel.yes')
                : t('intelligence.screen_debug_panel.no')}
            </span>
          </div>
          <div className="flex justify-between">
            <span>{t('intelligence.screen_debug_panel.frames_label')}</span>
            <span>{session?.frames_in_memory ?? 0}</span>
          </div>
          <div className="flex justify-between">
            <span>{t('intelligence.screen_debug_panel.vision_state_label')}</span>
            <span>{session?.vision_state ?? 'idle'}</span>
          </div>
          <div className="flex justify-between">
            <span>{t('intelligence.screen_debug_panel.vision_queue_label')}</span>
            <span>{session?.vision_queue_depth ?? 0}</span>
          </div>
          {session?.last_context && (
            <div className="flex justify-between">
              <span>{t('intelligence.screen_debug_panel.last_app_label')}</span>
              <span className="max-w-[180px] truncate">{session.last_context}</span>
            </div>
          )}
        </div>
      </div>

      {/* Capture Test */}
      <div className="rounded-xl border border-stone-700 bg-stone-900/50 p-3">
        <h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-stone-400">
          {t('intelligence.screen_debug_panel.capture_test_heading')}
        </h4>
        <button
          onClick={handleCaptureTest}
          disabled={isCaptureTestRunning}
          className="mb-3 w-full rounded-lg border border-primary-600/40 bg-primary-600/20 px-3 py-2 text-sm font-medium text-primary-300 transition-colors hover:bg-primary-600/30 disabled:opacity-50">
          {isCaptureTestRunning
            ? t('intelligence.screen_debug_panel.capturing')
            : t('intelligence.screen_debug_panel.test_capture')}
        </button>

        {captureTestResult && (
          <div className="space-y-2">
            <div className="space-y-1 text-xs text-stone-300">
              <div className="flex justify-between">
                <span>{t('intelligence.screen_debug_panel.status_label')}</span>
                <span className={captureTestResult.ok ? 'text-green-400' : 'text-red-400'}>
                  {captureTestResult.ok
                    ? t('intelligence.screen_debug_panel.success')
                    : t('intelligence.screen_debug_panel.failed')}
                </span>
              </div>
              <div className="flex justify-between">
                <span>{t('intelligence.screen_debug_panel.mode_label')}</span>
                <span>{captureTestResult.capture_mode}</span>
              </div>
              <div className="flex justify-between">
                <span>{t('intelligence.screen_debug_panel.time_label')}</span>
                <span>{captureTestResult.timing_ms}ms</span>
              </div>
              {captureTestResult.bytes_estimate != null && (
                <div className="flex justify-between">
                  <span>{t('intelligence.screen_debug_panel.size_label')}</span>
                  <span>{formatBytes(captureTestResult.bytes_estimate)}</span>
                </div>
              )}
              {captureTestResult.context && (
                <div className="flex justify-between">
                  <span>{t('intelligence.screen_debug_panel.app_label')}</span>
                  <span className="max-w-[180px] truncate">
                    {captureTestResult.context.app_name ??
                      t('intelligence.screen_debug_panel.unknown')}
                  </span>
                </div>
              )}
              {captureTestResult.context?.bounds_width != null && (
                <div className="flex justify-between">
                  <span>{t('intelligence.screen_debug_panel.bounds_label')}</span>
                  <span>
                    {captureTestResult.context.bounds_width}x
                    {captureTestResult.context.bounds_height} at (
                    {captureTestResult.context.bounds_x},{captureTestResult.context.bounds_y})
                  </span>
                </div>
              )}
            </div>

            {captureTestResult.error && (
              <div className="rounded-lg border border-red-800/50 bg-red-900/20 p-2 text-xs text-red-300">
                {captureTestResult.error}
              </div>
            )}

            {captureTestResult.image_ref && (
              <div className="overflow-hidden rounded-lg border border-stone-700">
                <img
                  src={captureTestResult.image_ref}
                  alt="Capture test result"
                  className="w-full"
                />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Recent Vision Summaries */}
      {recentVisionSummaries.length > 0 && (
        <div className="rounded-xl border border-stone-700 bg-stone-900/50 p-3">
          <h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-stone-400">
            {t('intelligence.screen_debug_panel.recent_vision_summaries_heading')}
          </h4>
          <div className="space-y-2">
            {recentVisionSummaries.map(summary => (
              <div
                key={summary.id}
                className="rounded-lg border border-stone-700/50 bg-stone-800/30 p-2 text-xs">
                <div className="flex justify-between text-stone-400">
                  <span>{summary.app_name ?? t('intelligence.screen_debug_panel.unknown')}</span>
                  <span>
                    {new Date(summary.captured_at_ms).toLocaleTimeString()} &middot;{' '}
                    {(summary.confidence * 100).toFixed(0)}%
                  </span>
                </div>
                <div className="mt-1 text-stone-200">{summary.actionable_notes}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Error Display */}
      {lastError && (
        <div className="rounded-lg border border-red-800/50 bg-red-900/20 p-2 text-xs text-red-300">
          {lastError}
        </div>
      )}
    </div>
  );
};

const OwnedScreenIntelligenceDebugPanel = () => {
  const state = useScreenIntelligenceState({ loadVision: true, visionLimit: 5, pollMs: 2000 });
  return <ScreenIntelligenceDebugPanelContent state={state} />;
};

const ScreenIntelligenceDebugPanel = ({ state }: ScreenIntelligenceDebugPanelProps) => {
  if (state) {
    return <ScreenIntelligenceDebugPanelContent state={state} />;
  }
  return <OwnedScreenIntelligenceDebugPanel />;
};

const PermissionDot = ({ label, value }: { label: string; value?: string }) => {
  const color =
    value === 'granted' ? 'bg-green-500' : value === 'denied' ? 'bg-red-500' : 'bg-stone-600';
  return (
    <div className="flex items-center gap-1.5">
      <div className={`h-2 w-2 rounded-full ${color}`} />
      <span className="text-stone-300">{label}</span>
    </div>
  );
};

export default ScreenIntelligenceDebugPanel;
