import { useCallback, useEffect, useRef, useState } from 'react';

import { useT } from '../../../../lib/i18n/I18nContext';
import { listConnections as listComposioConnections } from '../../../../lib/composio/composioApi';
import type { ComposioConnection } from '../../../../lib/composio/types';
import {
  creditsApi,
  type CreditTransaction,
  type TeamUsage,
} from '../../../../services/api/creditsApi';
import {
  type HeartbeatPlannerSummary,
  type HeartbeatSettings,
  type HeartbeatSettingsPatch,
  auraHeartbeatSettingsGet,
  auraHeartbeatSettingsSet,
  auraHeartbeatTickNow,
} from '../../../../utils/tauriCommands/heartbeat';
import { FormulaRow, LoopToggle, MetricTile } from './PanelControls';
import {
  activeConnection,
  formatCount,
  formatDateTime,
  formatUsd,
  isCalendarConnection,
  spendAmount,
  summarizeSpendByAction,
  summarizeSpendByHour,
  summarizeSpendSample,
} from './spendFormatting';
import { describeProvider } from './transforms';
import type { CloudProvider, RoutingMap } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// Background loop controls + usage diagnostics
// ─────────────────────────────────────────────────────────────────────────────

const WEEK_MINUTES = 7 * 24 * 60;
const COMPOSIO_PERIODIC_TICK_MINUTES = 20;
const LEARNING_REBUILD_MINUTES = 30;
const MEMORY_WORKERS = 4;
const MEMORY_POLL_SECONDS = 5;

const BackgroundLoopControls = ({
  routing,
  cloudProviders,
}: {
  routing: RoutingMap;
  cloudProviders: CloudProvider[];
}) => {
  const { t } = useT();
  const [settings, setSettings] = useState<HeartbeatSettings | null>(null);
  const [usage, setUsage] = useState<TeamUsage | null>(null);
  const [transactions, setTransactions] = useState<CreditTransaction[]>([]);
  const [connections, setConnections] = useState<ComposioConnection[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [runningTick, setRunningTick] = useState(false);
  const [plannerSummary, setPlannerSummary] = useState<HeartbeatPlannerSummary | null>(null);
  const [error, setError] = useState<string>('');
  const settingsRef = useRef<HeartbeatSettings | null>(null);
  const patchRequestIdRef = useRef(0);

  const commitSettings = useCallback((nextSettings: HeartbeatSettings | null) => {
    settingsRef.current = nextSettings;
    setSettings(nextSettings);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError('');
    const [heartbeatResult, usageResult, transactionsResult, connectionsResult] =
      await Promise.allSettled([
        auraHeartbeatSettingsGet(),
        creditsApi.getTeamUsage(),
        creditsApi.getTransactions(200, 0),
        listComposioConnections(),
      ]);

    if (heartbeatResult.status === 'fulfilled') {
      commitSettings(heartbeatResult.value.result.settings);
    } else {
      setError(
        heartbeatResult.reason instanceof Error
          ? heartbeatResult.reason.message
          : t('settings.background_loop_controls.load_failed')
      );
    }

    if (usageResult.status === 'fulfilled') {
      setUsage(usageResult.value);
    }

    if (transactionsResult.status === 'fulfilled') {
      setTransactions(transactionsResult.value.transactions ?? []);
    }

    if (connectionsResult.status === 'fulfilled') {
      setConnections(connectionsResult.value.connections ?? []);
    }
    setLoading(false);
  }, [commitSettings, t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const applyHeartbeatPatch = useCallback(
    async (patch: HeartbeatSettingsPatch) => {
      const requestId = patchRequestIdRef.current + 1;
      patchRequestIdRef.current = requestId;
      const savingKey = Object.keys(patch).join(',');
      const previous = settingsRef.current;
      setError('');
      setSaving(savingKey);
      if (!previous) {
        // No baseline to patch against — abandon this request.
        if (patchRequestIdRef.current === requestId) {
          setSaving(null);
        }
        return;
      }
      commitSettings({ ...previous, ...patch });
      try {
        const response = await auraHeartbeatSettingsSet(patch);
        // Stale response — a newer patch superseded us; drop this result.
        if (patchRequestIdRef.current !== requestId) return;
        commitSettings(response.result.settings);
      } catch (err) {
        if (patchRequestIdRef.current !== requestId) return;
        commitSettings(previous);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (patchRequestIdRef.current === requestId) {
          setSaving(null);
        }
      }
    },
    [commitSettings]
  );

  const runPlannerNow = useCallback(async () => {
    setRunningTick(true);
    setError('');
    try {
      const response = await auraHeartbeatTickNow();
      setPlannerSummary(response.result.summary);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunningTick(false);
    }
  }, [refresh]);

  const spendSample = summarizeSpendSample(transactions);
  const spendRows = spendSample.rows;
  const actionSummary = summarizeSpendByAction(transactions);
  const hourSummary = summarizeSpendByHour(transactions);
  const latestSpend = spendRows[0] ?? null;
  const heartbeatIntervalMinutes = settings ? Math.max(settings.interval_minutes, 5) : 5;
  const heartbeatTicksPerWeek = settings?.enabled
    ? Math.ceil(WEEK_MINUTES / heartbeatIntervalMinutes)
    : 0;
  const activeConnections = connections.filter(activeConnection);
  const activeCalendarConnections = activeConnections.filter(isCalendarConnection);
  const maxCalendarConnectionsPerTick = settings
    ? Math.max(settings.max_calendar_connections_per_tick ?? 2, 1)
    : 2;
  const calendarConnectionsPolled = settings?.notify_meetings
    ? Math.min(activeCalendarConnections.length, maxCalendarConnectionsPerTick)
    : 0;
  const calendarConnectionsSkipped = settings?.notify_meetings
    ? Math.max(activeCalendarConnections.length - calendarConnectionsPolled, 0)
    : 0;
  const calendarPlannerCallsPerTick = settings?.notify_meetings ? 1 + calendarConnectionsPolled : 0;
  const calendarPlannerCallsPerWeek = heartbeatTicksPerWeek * calendarPlannerCallsPerTick;
  const subconsciousModelCallsPerWeek =
    settings?.enabled && settings.inference_enabled ? heartbeatTicksPerWeek : 0;
  const composioPeriodicTicksPerWeek = Math.ceil(WEEK_MINUTES / COMPOSIO_PERIODIC_TICK_MINUTES);
  const learningTicksPerWeek = Math.ceil(WEEK_MINUTES / LEARNING_REBUILD_MINUTES);
  const memoryPollsPerWeek = Math.ceil((WEEK_MINUTES * 60 * MEMORY_WORKERS) / MEMORY_POLL_SECONDS);
  const composioConnectionScansPerWeek = composioPeriodicTicksPerWeek * activeConnections.length;
  const backgroundApiReadsPerWeek = calendarPlannerCallsPerWeek + composioConnectionScansPerWeek;
  const backgroundWakeupsPerWeek =
    heartbeatTicksPerWeek +
    composioPeriodicTicksPerWeek +
    learningTicksPerWeek +
    memoryPollsPerWeek;
  const scheduledCallsPerRemainingDollar =
    usage && usage.remainingUsd > 0 ? backgroundApiReadsPerWeek / usage.remainingUsd : null;
  const estimatedRowsLeft =
    usage && spendSample.avgRowUsd > 0
      ? Math.floor(usage.remainingUsd / spendSample.avgRowUsd)
      : null;
  const estimatedRowsPerBudget =
    usage && spendSample.avgRowUsd > 0
      ? Math.floor(usage.cycleBudgetUsd / spendSample.avgRowUsd)
      : null;
  const projectedHoursLeft =
    usage && spendSample.spendPerHour > 0 ? usage.remainingUsd / spendSample.spendPerHour : null;
  const projectionAnchorMs = latestSpend ? new Date(latestSpend.createdAt).getTime() : Number.NaN;
  const projectedExhaustAt =
    projectedHoursLeft !== null && Number.isFinite(projectionAnchorMs)
      ? new Date(projectionAnchorMs + projectedHoursLeft * 3_600_000).toLocaleString([], {
          month: 'short',
          day: 'numeric',
          hour: 'numeric',
          minute: '2-digit',
        })
      : 'n/a';

  const minutesCadence = (minutes: number) =>
    t('settings.background_loop_controls.cadence_minutes', { minutes });

  const loops = [
    {
      name: t('settings.background_loop_controls.loop_heartbeat_planner_name'),
      enabled: Boolean(settings?.enabled),
      cadence: minutesCadence(settings?.interval_minutes ?? 5),
      route: describeProvider(routing.heartbeat, cloudProviders),
      work: t('settings.background_loop_controls.loop_heartbeat_planner_work'),
      risk: settings?.notify_meetings
        ? t('settings.background_loop_controls.loop_heartbeat_planner_risk_on', {
            calendarPlannerCallsPerTick,
            calendarConnectionsSkipped,
          })
        : t('settings.background_loop_controls.loop_heartbeat_planner_risk_off'),
    },
    {
      name: t('settings.background_loop_controls.loop_subconscious_tick_name'),
      enabled: Boolean(settings?.enabled && settings?.inference_enabled),
      cadence: minutesCadence(settings?.interval_minutes ?? 5),
      route: describeProvider(routing.subconscious, cloudProviders),
      work: t('settings.background_loop_controls.loop_subconscious_tick_work'),
      risk:
        subconsciousModelCallsPerWeek > 0
          ? t('settings.background_loop_controls.loop_subconscious_tick_risk_on', {
              calls: formatCount(subconsciousModelCallsPerWeek),
            })
          : t('settings.background_loop_controls.loop_subconscious_tick_risk_off'),
    },
    {
      name: t('settings.background_loop_controls.loop_memory_workers_name'),
      enabled: true,
      cadence: t('settings.background_loop_controls.cadence_queue'),
      route: describeProvider(routing.memory, cloudProviders),
      work: t('settings.background_loop_controls.loop_memory_workers_work'),
      risk: t('settings.background_loop_controls.loop_memory_workers_risk', {
        workers: MEMORY_WORKERS,
        pollSeconds: MEMORY_POLL_SECONDS,
      }),
    },
    {
      name: t('settings.background_loop_controls.loop_reflection_rebuild_name'),
      enabled: true,
      cadence: minutesCadence(30),
      route: describeProvider(routing.learning, cloudProviders),
      work: t('settings.background_loop_controls.loop_reflection_rebuild_work'),
      risk: t('settings.background_loop_controls.loop_reflection_rebuild_risk', {
        wakeups: formatCount(learningTicksPerWeek),
      }),
    },
    {
      name: t('settings.background_loop_controls.loop_composio_sync_name'),
      enabled: true,
      cadence: minutesCadence(20),
      route: t('settings.background_loop_controls.loop_composio_sync_route'),
      work: t('settings.background_loop_controls.loop_composio_sync_work'),
      risk: t('settings.background_loop_controls.loop_composio_sync_risk', {
        wakeups: formatCount(composioPeriodicTicksPerWeek),
        connections: activeConnections.length,
      }),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="border-b border-stone-200 pb-2">
        <h2 className="text-base font-semibold text-stone-900">
          {t('settings.background_loop_controls.background_loops_title')}
        </h2>
        <p className="mt-0.5 text-xs text-stone-500">
          {t('settings.background_loop_controls.background_loops_description')}
        </p>
      </div>

      {error && (
        <div className="rounded-md border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
          {error}
        </div>
      )}

      <section className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(300px,0.8fr)]">
        <div className="space-y-3">
          <div className="rounded-lg border border-stone-200 bg-stone-50 p-3">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-stone-900">
                  {t('settings.background_loop_controls.heartbeat_controls_title')}
                </div>
                <div className="text-xs text-stone-500">
                  {t('settings.background_loop_controls.heartbeat_controls_description')}
                </div>
              </div>
              <button
                type="button"
                onClick={() => void refresh()}
                disabled={loading}
                className="rounded-md border border-stone-200 bg-white px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
                {t('settings.background_loop_controls.refresh')}
              </button>
            </div>

            {settings ? (
              <div className="space-y-2">
                <LoopToggle
                  label={t('settings.background_loop_controls.heartbeat_loop_label')}
                  description={t('settings.background_loop_controls.heartbeat_loop_description')}
                  checked={settings.enabled}
                  busy={saving === 'enabled'}
                  onToggle={() => void applyHeartbeatPatch({ enabled: !settings.enabled })}
                />
                <LoopToggle
                  label={t('settings.background_loop_controls.subconscious_inference_label')}
                  description={t(
                    'settings.background_loop_controls.subconscious_inference_description'
                  )}
                  checked={settings.inference_enabled}
                  busy={saving === 'inference_enabled'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ inference_enabled: !settings.inference_enabled })
                  }
                />
                <LoopToggle
                  label={t('settings.background_loop_controls.calendar_meeting_checks_label')}
                  description={t(
                    'settings.background_loop_controls.calendar_meeting_checks_description'
                  )}
                  checked={settings.notify_meetings}
                  busy={saving === 'notify_meetings'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ notify_meetings: !settings.notify_meetings })
                  }
                />
                <div className="grid gap-2 rounded-lg border border-stone-200 bg-white px-3 py-2 md:grid-cols-3">
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>{t('settings.background_loop_controls.calendar_cap_label')}</span>
                    <select
                      value={maxCalendarConnectionsPerTick}
                      disabled={saving === 'max_calendar_connections_per_tick'}
                      onChange={e =>
                        void applyHeartbeatPatch({
                          max_calendar_connections_per_tick: Number(e.target.value),
                        })
                      }
                      className="w-full rounded-md border border-stone-200 bg-white px-2 py-1 text-xs text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                      {[1, 2, 3, 5, 10].map(count => (
                        <option key={count} value={count}>
                          {t('settings.background_loop_controls.conn_per_tick_option', { count })}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>{t('settings.background_loop_controls.meeting_lookahead_label')}</span>
                    <select
                      value={settings.meeting_lookahead_minutes}
                      disabled={saving === 'meeting_lookahead_minutes'}
                      onChange={e =>
                        void applyHeartbeatPatch({
                          meeting_lookahead_minutes: Number(e.target.value),
                        })
                      }
                      className="w-full rounded-md border border-stone-200 bg-white px-2 py-1 text-xs text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                      {[15, 30, 60, 120, 240].map(minutes => (
                        <option key={minutes} value={minutes}>
                          {minutesCadence(minutes)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>{t('settings.background_loop_controls.reminder_lookahead_label')}</span>
                    <select
                      value={settings.reminder_lookahead_minutes}
                      disabled={saving === 'reminder_lookahead_minutes'}
                      onChange={e =>
                        void applyHeartbeatPatch({
                          reminder_lookahead_minutes: Number(e.target.value),
                        })
                      }
                      className="w-full rounded-md border border-stone-200 bg-white px-2 py-1 text-xs text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                      {[5, 15, 30, 60, 120].map(minutes => (
                        <option key={minutes} value={minutes}>
                          {minutesCadence(minutes)}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <LoopToggle
                  label={t('settings.background_loop_controls.cron_reminder_checks_label')}
                  description={t(
                    'settings.background_loop_controls.cron_reminder_checks_description'
                  )}
                  checked={settings.notify_reminders}
                  busy={saving === 'notify_reminders'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ notify_reminders: !settings.notify_reminders })
                  }
                />
                <LoopToggle
                  label={t('settings.background_loop_controls.relevant_notification_checks_label')}
                  description={t(
                    'settings.background_loop_controls.relevant_notification_checks_description'
                  )}
                  checked={settings.notify_relevant_events}
                  busy={saving === 'notify_relevant_events'}
                  onToggle={() =>
                    void applyHeartbeatPatch({
                      notify_relevant_events: !settings.notify_relevant_events,
                    })
                  }
                />
                <LoopToggle
                  label={t('settings.background_loop_controls.external_delivery_label')}
                  description={t(
                    'settings.background_loop_controls.external_delivery_description'
                  )}
                  checked={settings.external_delivery_enabled}
                  busy={saving === 'external_delivery_enabled'}
                  onToggle={() =>
                    void applyHeartbeatPatch({
                      external_delivery_enabled: !settings.external_delivery_enabled,
                    })
                  }
                />

                <div className="flex flex-wrap items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 py-2">
                  <label
                    className="text-xs font-medium text-stone-700"
                    htmlFor="heartbeat-interval">
                    {t('settings.background_loop_controls.interval_label')}
                  </label>
                  <select
                    id="heartbeat-interval"
                    value={settings.interval_minutes}
                    disabled={saving === 'interval_minutes'}
                    onChange={e =>
                      void applyHeartbeatPatch({ interval_minutes: Number(e.target.value) })
                    }
                    className="rounded-md border border-stone-200 bg-white px-2 py-1 text-xs text-stone-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500">
                    {[5, 10, 15, 30, 60].map(minutes => (
                      <option key={minutes} value={minutes}>
                        {minutesCadence(minutes)}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={() => void runPlannerNow()}
                    disabled={runningTick}
                    className="ml-auto rounded-md border border-stone-200 bg-white px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
                    {runningTick
                      ? t('settings.background_loop_controls.running')
                      : t('settings.background_loop_controls.planner_tick_now')}
                  </button>
                </div>

                {plannerSummary && (
                  <div className="rounded-md border border-primary-100 bg-primary-50 px-3 py-2 text-xs text-primary-900">
                    {t('settings.background_loop_controls.planner_summary', {
                      sourceEvents: plannerSummary.source_events,
                      deliveriesSent: plannerSummary.deliveries_sent,
                      deliveriesSkippedDedup: plannerSummary.deliveries_skipped_dedup,
                    })}
                  </div>
                )}
              </div>
            ) : (
              <div className="text-xs text-stone-500">
                {loading
                  ? t('settings.background_loop_controls.loading_heartbeat_controls')
                  : t('settings.background_loop_controls.heartbeat_controls_unavailable')}
              </div>
            )}
          </div>

          <div className="overflow-hidden rounded-lg border border-stone-200 bg-stone-50">
            <div className="border-b border-stone-200 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-stone-400">
              {t('settings.background_loop_controls.loop_map_heading')}
            </div>
            <div className="divide-y divide-stone-200">
              {loops.map(loop => (
                <div key={loop.name} className="grid gap-2 px-3 py-3 md:grid-cols-[150px_1fr]">
                  <div>
                    <div className="text-sm font-medium text-stone-900">{loop.name}</div>
                    <div className="mt-0.5 flex flex-wrap gap-1 text-[11px] text-stone-500">
                      <span>
                        {loop.enabled
                          ? t('settings.background_loop_controls.loop_on')
                          : t('settings.background_loop_controls.loop_off')}
                      </span>
                      <span>{loop.cadence}</span>
                    </div>
                  </div>
                  <div className="text-xs text-stone-600">
                    <div>{loop.work}</div>
                    <div className="mt-1 font-mono text-[11px] text-stone-500">
                      {t('settings.background_loop_controls.route_prefix', { route: loop.route })}
                    </div>
                    <div className="mt-1 text-stone-500">{loop.risk}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="rounded-lg border border-stone-200 bg-white p-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-900">
                {t('settings.background_loop_controls.recent_usage_ledger_title')}
              </div>
              <div className="text-xs text-stone-500">
                {t('settings.background_loop_controls.recent_usage_ledger_description')}
              </div>
            </div>
            <button
              type="button"
              onClick={() => void refresh()}
              disabled={loading}
              className="rounded-md border border-stone-200 px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
              {t('settings.background_loop_controls.reload')}
            </button>
          </div>

          <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-3">
            <MetricTile
              label={t('settings.background_loop_controls.week_budget_label')}
              value={usage ? formatUsd(usage.cycleBudgetUsd) : 'n/a'}
              detail={t('settings.background_loop_controls.resets_detail', {
                dateTime: formatDateTime(usage?.cycleEndsAt),
              })}
            />
            <MetricTile
              label={t('settings.background_loop_controls.week_remaining_label')}
              value={usage ? formatUsd(usage.remainingUsd) : 'n/a'}
              detail={
                usage
                  ? t('settings.background_loop_controls.used_detail', {
                      amount: formatUsd(usage.cycleLimit7day),
                    })
                  : undefined
              }
            />
            <MetricTile
              label={t('settings.background_loop_controls.five_hour_window_label')}
              value={
                usage
                  ? `${formatUsd(usage.cycleLimit5hr)} / ${formatUsd(usage.fiveHourCapUsd)}`
                  : 'n/a'
              }
              detail={t('settings.background_loop_controls.resets_detail', {
                dateTime: formatDateTime(usage?.fiveHourResetsAt),
              })}
            />
            <MetricTile
              label={t('settings.background_loop_controls.avg_spend_row_label')}
              value={spendSample.avgRowUsd > 0 ? formatUsd(spendSample.avgRowUsd) : 'n/a'}
              detail={t('settings.background_loop_controls.recent_spend_rows_detail', {
                count: spendRows.length,
              })}
            />
            <MetricTile
              label={t('settings.background_loop_controls.bg_api_reads_label')}
              value={t('settings.background_loop_controls.per_week', {
                value: formatCount(backgroundApiReadsPerWeek),
              })}
              detail={t('settings.background_loop_controls.planner_plus_sync_detail', {
                planner: formatCount(calendarPlannerCallsPerWeek),
                sync: formatCount(composioConnectionScansPerWeek),
              })}
            />
            <MetricTile
              label={t('settings.background_loop_controls.bg_wakeups_label')}
              value={t('settings.background_loop_controls.per_week', {
                value: formatCount(backgroundWakeupsPerWeek),
              })}
              detail={t('settings.background_loop_controls.memory_polls_detail', {
                value: formatCount(memoryPollsPerWeek),
              })}
            />
          </div>

          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 p-3">
            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
              {t('settings.background_loop_controls.budget_math_heading')}
            </div>
            <div className="mt-2 grid gap-2">
              <FormulaRow
                label={t('settings.background_loop_controls.rows_left_label')}
                value={estimatedRowsLeft !== null ? formatCount(estimatedRowsLeft) : 'n/a'}
                detail={
                  estimatedRowsLeft !== null
                    ? t('settings.background_loop_controls.rows_left_formula', {
                        remaining: formatUsd(usage?.remainingUsd ?? 0),
                        avgRow: formatUsd(spendSample.avgRowUsd),
                      })
                    : t('settings.background_loop_controls.need_recent_spend_rows')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.rows_per_budget_label')}
                value={
                  estimatedRowsPerBudget !== null ? formatCount(estimatedRowsPerBudget) : 'n/a'
                }
                detail={
                  estimatedRowsPerBudget !== null
                    ? t('settings.background_loop_controls.rows_per_budget_formula', {
                        cycleBudget: formatUsd(usage?.cycleBudgetUsd ?? 0),
                        avgRow: formatUsd(spendSample.avgRowUsd),
                      })
                    : t('settings.background_loop_controls.need_recent_spend_rows')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.sample_burn_rate_label')}
                value={
                  spendSample.spendPerHour > 0
                    ? t('settings.background_loop_controls.per_hour', {
                        amount: formatUsd(spendSample.spendPerHour),
                      })
                    : 'n/a'
                }
                detail={
                  spendSample.sampleHours > 0
                    ? t('settings.background_loop_controls.sample_burn_rate_detail', {
                        rowsPerHour: formatCount(spendSample.rowsPerHour),
                        sampleHours: spendSample.sampleHours.toFixed(1),
                      })
                    : t('settings.background_loop_controls.need_two_spend_rows')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.projected_empty_label')}
                value={projectedExhaustAt}
                detail={
                  projectedHoursLeft !== null
                    ? t('settings.background_loop_controls.projected_empty_detail', {
                        hours: projectedHoursLeft.toFixed(1),
                      })
                    : t('settings.background_loop_controls.no_projection')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.api_reads_per_dollar_label')}
                value={
                  scheduledCallsPerRemainingDollar !== null
                    ? t('settings.background_loop_controls.reads_per_dollar', {
                        value: formatCount(scheduledCallsPerRemainingDollar),
                      })
                    : 'n/a'
                }
                detail={
                  usage
                    ? t('settings.background_loop_controls.api_reads_per_dollar_formula', {
                        reads: formatCount(backgroundApiReadsPerWeek),
                        remaining: formatUsd(usage.remainingUsd),
                      })
                    : t('settings.background_loop_controls.need_usage_response')
                }
              />
            </div>
          </div>

          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 p-3">
            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
              {t('settings.background_loop_controls.loop_call_budget_heading')}
            </div>
            <div className="mt-2 grid gap-2">
              <FormulaRow
                label={t('settings.background_loop_controls.heartbeat_ticks_label')}
                value={t('settings.background_loop_controls.per_week', {
                  value: formatCount(heartbeatTicksPerWeek),
                })}
                detail={t('settings.background_loop_controls.heartbeat_ticks_detail', {
                  intervalMinutes: heartbeatIntervalMinutes,
                })}
              />
              <FormulaRow
                label={t('settings.background_loop_controls.calendar_planner_calls_label')}
                value={t('settings.background_loop_controls.per_week', {
                  value: formatCount(calendarPlannerCallsPerWeek),
                })}
                detail={
                  settings?.notify_meetings
                    ? t('settings.background_loop_controls.calendar_planner_calls_detail', {
                        calendarConnectionsPolled,
                      })
                    : t('settings.background_loop_controls.meeting_collector_disabled')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.calendar_fanout_cap_label')}
                value={t('settings.background_loop_controls.calendar_fanout_cap_value', {
                  polled: formatCount(calendarConnectionsPolled),
                  active: formatCount(activeCalendarConnections.length),
                })}
                detail={t('settings.background_loop_controls.calendar_fanout_cap_detail', {
                  maxCalendarConnectionsPerTick,
                  calendarConnectionsSkipped,
                })}
              />
              <FormulaRow
                label={t('settings.background_loop_controls.subconscious_model_calls_label')}
                value={t('settings.background_loop_controls.per_week', {
                  value: formatCount(subconsciousModelCallsPerWeek),
                })}
                detail={
                  settings?.enabled && settings.inference_enabled
                    ? t('settings.background_loop_controls.subconscious_model_calls_detail_on')
                    : t('settings.background_loop_controls.heartbeat_inference_disabled')
                }
              />
              <FormulaRow
                label={t('settings.background_loop_controls.composio_sync_scans_label')}
                value={t('settings.background_loop_controls.per_week', {
                  value: formatCount(composioConnectionScansPerWeek),
                })}
                detail={t('settings.background_loop_controls.composio_sync_scans_detail', {
                  connections: activeConnections.length,
                  intervalMinutes: COMPOSIO_PERIODIC_TICK_MINUTES,
                })}
              />
              <FormulaRow
                label={t('settings.background_loop_controls.total_bg_api_read_budget_label')}
                value={t('settings.background_loop_controls.per_week', {
                  value: formatCount(backgroundApiReadsPerWeek),
                })}
                detail={t('settings.background_loop_controls.total_bg_api_read_budget_detail')}
              />
              <FormulaRow
                label={t('settings.background_loop_controls.memory_worker_polls_label')}
                value={t('settings.background_loop_controls.per_week_max', {
                  value: formatCount(memoryPollsPerWeek),
                })}
                detail={t('settings.background_loop_controls.memory_worker_polls_detail', {
                  workers: MEMORY_WORKERS,
                  pollSeconds: MEMORY_POLL_SECONDS,
                })}
              />
            </div>
          </div>

          {latestSpend && (
            <div className="mt-3 rounded-md border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
              {t('settings.background_loop_controls.latest_spend', {
                amount: formatUsd(spendAmount(latestSpend)),
                dateTime: new Date(latestSpend.createdAt).toLocaleString(),
                action: latestSpend.action,
              })}
            </div>
          )}

          <div className="mt-3 space-y-3">
            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                {t('settings.background_loop_controls.top_actions_heading')}
              </div>
              <div className="mt-1 space-y-1">
                {actionSummary.length > 0 ? (
                  actionSummary.map(([action, count, total]) => (
                    <div
                      key={action}
                      className="flex items-center justify-between gap-2 text-xs text-stone-600">
                      <span className="truncate font-mono">{action}</span>
                      <span className="shrink-0 text-stone-500">
                        {t('settings.background_loop_controls.count_over_amount', {
                          count,
                          amount: formatUsd(total),
                        })}
                      </span>
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-stone-500">
                    {t('settings.background_loop_controls.no_spend_rows_loaded')}
                  </div>
                )}
              </div>
            </div>

            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                {t('settings.background_loop_controls.top_hours_heading')}
              </div>
              <div className="mt-1 space-y-1">
                {hourSummary.length > 0 ? (
                  hourSummary.map(([hour, total]) => (
                    <div
                      key={hour}
                      className="flex items-center justify-between gap-2 text-xs text-stone-600">
                      <span>{hour}</span>
                      <span className="font-mono text-stone-500">{formatUsd(total)}</span>
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-stone-500">
                    {t('settings.background_loop_controls.no_hourly_spend_yet')}
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default BackgroundLoopControls;
