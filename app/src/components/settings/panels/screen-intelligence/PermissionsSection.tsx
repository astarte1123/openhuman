import { useT } from '../../../../lib/i18n/I18nContext';
import type { AccessibilityPermissionKind } from '../../../../utils/tauriCommands';

interface PermissionsBadgeProps {
  label: string;
  value: string;
}

const PermissionBadge = ({ label, value }: PermissionsBadgeProps) => {
  const colorClass =
    value === 'granted'
      ? 'bg-green-50 text-green-700 border-green-200'
      : value === 'denied'
        ? 'bg-red-50 text-red-700 border-red-200'
        : 'bg-stone-100 text-stone-600 border-stone-200';

  return (
    <div className="flex items-center justify-between rounded-xl border border-stone-200 bg-white p-3">
      <span className="text-sm text-stone-700">{label}</span>
      <span className={`rounded-md border px-2 py-1 text-xs uppercase tracking-wide ${colorClass}`}>
        {value}
      </span>
    </div>
  );
};

interface PermissionsSectionProps {
  screenRecording: string;
  accessibility: string;
  inputMonitoring: string;
  anyPermissionDenied: boolean;
  lastRestartSummary: string | null;
  permissionCheckProcessPath: string | null | undefined;
  isRequestingPermissions: boolean;
  isRestartingCore: boolean;
  isLoading: boolean;
  requestPermission: (permission: AccessibilityPermissionKind) => Promise<unknown>;
  refreshPermissionsWithRestart: () => Promise<unknown>;
  refreshStatus: () => Promise<unknown>;
}

const PermissionsSection = ({
  screenRecording,
  accessibility,
  inputMonitoring,
  anyPermissionDenied,
  lastRestartSummary,
  permissionCheckProcessPath,
  isRequestingPermissions,
  isRestartingCore,
  isLoading,
  requestPermission,
  refreshPermissionsWithRestart,
  refreshStatus,
}: PermissionsSectionProps) => {
  const { t } = useT();
  return (
    <section className="space-y-3">
      <h3 className="text-sm font-semibold text-stone-900">
        {t('settings.screen_intelligence_permissions.permissions_heading')}
      </h3>
      <PermissionBadge
        label={t('settings.screen_intelligence_permissions.screen_recording_label')}
        value={screenRecording}
      />
      <PermissionBadge
        label={t('settings.screen_intelligence_permissions.accessibility_label')}
        value={accessibility}
      />
      <PermissionBadge
        label={t('settings.screen_intelligence_permissions.input_monitoring_label')}
        value={inputMonitoring}
      />

      {anyPermissionDenied && (
        <div className="rounded-xl border border-amber-300 bg-amber-50 p-3 text-sm text-amber-700 space-y-1">
          <p>{t('settings.screen_intelligence_permissions.grant_restart_hint')}</p>
          {permissionCheckProcessPath ? (
            <p className="opacity-75 text-xs">
              {t('settings.screen_intelligence_permissions.macos_privacy_prefix')}{' '}
              <span className="font-mono break-all text-stone-600">
                {permissionCheckProcessPath}
              </span>
            </p>
          ) : null}
        </div>
      )}

      {lastRestartSummary ? (
        <div className="rounded-xl border border-green-300 bg-green-50 p-3 text-sm text-green-700">
          {lastRestartSummary}
        </div>
      ) : null}

      <button
        type="button"
        onClick={() => void requestPermission('screen_recording')}
        disabled={isRequestingPermissions || isRestartingCore}
        className="mt-1 rounded-lg border border-primary-400 bg-primary-50 px-3 py-2 text-sm text-primary-700 disabled:opacity-50">
        {isRequestingPermissions
          ? t('settings.screen_intelligence_permissions.requesting')
          : t('settings.screen_intelligence_permissions.request_screen_recording')}
      </button>
      <button
        type="button"
        onClick={() => void requestPermission('accessibility')}
        disabled={isRequestingPermissions || isRestartingCore}
        className="rounded-lg border border-primary-400 bg-primary-50 px-3 py-2 text-sm text-primary-700 disabled:opacity-50">
        {isRequestingPermissions
          ? t('settings.screen_intelligence_permissions.requesting')
          : t('settings.screen_intelligence_permissions.request_accessibility')}
      </button>
      <button
        type="button"
        onClick={() => void requestPermission('input_monitoring')}
        disabled={isRequestingPermissions || isRestartingCore}
        className="rounded-lg border border-primary-400 bg-primary-50 px-3 py-2 text-sm text-primary-700 disabled:opacity-50">
        {isRequestingPermissions
          ? t('settings.screen_intelligence_permissions.requesting')
          : t('settings.screen_intelligence_permissions.open_input_monitoring')}
      </button>
      {anyPermissionDenied ? (
        <button
          type="button"
          onClick={() => void refreshPermissionsWithRestart()}
          disabled={isRestartingCore || isLoading}
          className="rounded-lg border border-amber-400 bg-amber-50 px-3 py-2 text-sm text-amber-700 disabled:opacity-50">
          {isRestartingCore
            ? t('settings.screen_intelligence_permissions.restarting_core')
            : t('settings.screen_intelligence_permissions.restart_and_refresh_permissions')}
        </button>
      ) : (
        <button
          type="button"
          onClick={() => void refreshStatus()}
          disabled={isLoading || isRestartingCore}
          className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-sm text-stone-700 disabled:opacity-50">
          {isLoading
            ? t('settings.screen_intelligence_permissions.refreshing')
            : t('settings.screen_intelligence_permissions.refresh_status')}
        </button>
      )}
    </section>
  );
};

export default PermissionsSection;
