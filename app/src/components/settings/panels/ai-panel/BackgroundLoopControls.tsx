import { useCallback, useEffect, useRef, useState } from 'react';

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
  openhumanHeartbeatSettingsGet,
  openhumanHeartbeatSettingsSet,
  openhumanHeartbeatTickNow,
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
        openhumanHeartbeatSettingsGet(),
        creditsApi.getTeamUsage(),
        creditsApi.getTransactions(200, 0),
        listComposioConnections(),
      ]);

    if (heartbeatResult.status === 'fulfilled') {
      commitSettings(heartbeatResult.value.result.settings);
    } else {
      setError(
        heartbeatResult.reason instanceof Error ? heartbeatResult.reason.message : 'Load failed'
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
  }, [commitSettings]);

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
        const response = await openhumanHeartbeatSettingsSet(patch);
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
      const response = await openhumanHeartbeatTickNow();
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

  const loops = [
    {
      name: 'Heartbeat planner',
      enabled: Boolean(settings?.enabled),
      cadence: `${settings?.interval_minutes ?? 5} min`,
      route: describeProvider(routing.heartbeat, cloudProviders),
      work: 'Runs proactive collectors: cron reminders, calendar meetings, relevant notifications.',
      risk: settings?.notify_meetings
        ? `${calendarPlannerCallsPerTick} Composio read call(s)/tick; ${calendarConnectionsSkipped} calendar link(s) over cap skipped.`
        : 'Calendar collector off; planner reads only local enabled categories.',
    },
    {
      name: 'Subconscious tick',
      enabled: Boolean(settings?.enabled && settings?.inference_enabled),
      cadence: `${settings?.interval_minutes ?? 5} min`,
      route: describeProvider(routing.subconscious, cloudProviders),
      work: 'Evaluates subconscious tasks/reflections through kind=subconscious_tick.',
      risk:
        subconsciousModelCallsPerWeek > 0
          ? `${formatCount(subconsciousModelCallsPerWeek)} model call(s)/week at current interval.`
          : 'Inference off; no scheduled subconscious model calls.',
    },
    {
      name: 'Memory tree workers',
      enabled: true,
      cadence: 'queue',
      route: describeProvider(routing.memory, cloudProviders),
      work: 'Extracts chunks, seals branches, runs daily digests, routes topics.',
      risk: `${MEMORY_WORKERS} workers poll every ${MEMORY_POLL_SECONDS}s; LLM calls only when queue has extract/seal/digest/topic jobs.`,
    },
    {
      name: 'Reflection rebuild',
      enabled: true,
      cadence: '30 min',
      route: describeProvider(routing.learning, cloudProviders),
      work: 'Refreshes reflection state after memory activity.',
      risk: `${formatCount(learningTicksPerWeek)} wakeups/week; LLM work only when rebuild needs reflection.`,
    },
    {
      name: 'Composio sync',
      enabled: true,
      cadence: '20 min',
      route: 'Integration APIs',
      work: 'Polls connected tools when provider sync is due.',
      risk: `${formatCount(composioPeriodicTicksPerWeek)} wakeups/week; scans ${activeConnections.length} active connection(s).`,
    },
  ];

  return (
    <div className="space-y-4">
      <div className="border-b border-stone-200 pb-2">
        <h2 className="text-base font-semibold text-stone-900">Background loops</h2>
        <p className="mt-0.5 text-xs text-stone-500">
          See what runs without a chat message, pause heartbeat work, and inspect recent credit
          ledger rows.
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
                <div className="text-sm font-semibold text-stone-900">Heartbeat controls</div>
                <div className="text-xs text-stone-500">
                  Defaults off. Enabling starts the loop; disabling aborts the running task.
                </div>
              </div>
              <button
                type="button"
                onClick={() => void refresh()}
                disabled={loading}
                className="rounded-md border border-stone-200 bg-white px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
                Refresh
              </button>
            </div>

            {settings ? (
              <div className="space-y-2">
                <LoopToggle
                  label="Heartbeat loop"
                  description="Master scheduler for planner + optional subconscious inference."
                  checked={settings.enabled}
                  busy={saving === 'enabled'}
                  onToggle={() => void applyHeartbeatPatch({ enabled: !settings.enabled })}
                />
                <LoopToggle
                  label="Subconscious inference"
                  description="Runs model-backed task/reflection evaluation on heartbeat ticks."
                  checked={settings.inference_enabled}
                  busy={saving === 'inference_enabled'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ inference_enabled: !settings.inference_enabled })
                  }
                />
                <LoopToggle
                  label="Calendar meeting checks"
                  description="Calls calendar event list for active Google Calendar connections."
                  checked={settings.notify_meetings}
                  busy={saving === 'notify_meetings'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ notify_meetings: !settings.notify_meetings })
                  }
                />
                <div className="grid gap-2 rounded-lg border border-stone-200 bg-white px-3 py-2 md:grid-cols-3">
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>Calendar cap</span>
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
                          {count} conn/tick
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>Meeting lookahead</span>
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
                          {minutes} min
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="space-y-1 text-xs font-medium text-stone-700">
                    <span>Reminder lookahead</span>
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
                          {minutes} min
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <LoopToggle
                  label="Cron reminder checks"
                  description="Scans enabled cron jobs for reminder-like upcoming items."
                  checked={settings.notify_reminders}
                  busy={saving === 'notify_reminders'}
                  onToggle={() =>
                    void applyHeartbeatPatch({ notify_reminders: !settings.notify_reminders })
                  }
                />
                <LoopToggle
                  label="Relevant notification checks"
                  description="Promotes urgent local notifications into proactive alerts."
                  checked={settings.notify_relevant_events}
                  busy={saving === 'notify_relevant_events'}
                  onToggle={() =>
                    void applyHeartbeatPatch({
                      notify_relevant_events: !settings.notify_relevant_events,
                    })
                  }
                />
                <LoopToggle
                  label="External delivery"
                  description="Lets heartbeat alerts send proactive messages to external channels."
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
                    Interval
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
                        {minutes} min
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={() => void runPlannerNow()}
                    disabled={runningTick}
                    className="ml-auto rounded-md border border-stone-200 bg-white px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
                    {runningTick ? 'Running...' : 'Planner tick now'}
                  </button>
                </div>

                {plannerSummary && (
                  <div className="rounded-md border border-primary-100 bg-primary-50 px-3 py-2 text-xs text-primary-900">
                    Planner: {plannerSummary.source_events} source events,{' '}
                    {plannerSummary.deliveries_sent} sent, {plannerSummary.deliveries_skipped_dedup}{' '}
                    deduped.
                  </div>
                )}
              </div>
            ) : (
              <div className="text-xs text-stone-500">
                {loading ? 'Loading heartbeat controls...' : 'Heartbeat controls unavailable.'}
              </div>
            )}
          </div>

          <div className="overflow-hidden rounded-lg border border-stone-200 bg-stone-50">
            <div className="border-b border-stone-200 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-stone-400">
              Loop map
            </div>
            <div className="divide-y divide-stone-200">
              {loops.map(loop => (
                <div key={loop.name} className="grid gap-2 px-3 py-3 md:grid-cols-[150px_1fr]">
                  <div>
                    <div className="text-sm font-medium text-stone-900">{loop.name}</div>
                    <div className="mt-0.5 flex flex-wrap gap-1 text-[11px] text-stone-500">
                      <span>{loop.enabled ? 'on' : 'off'}</span>
                      <span>{loop.cadence}</span>
                    </div>
                  </div>
                  <div className="text-xs text-stone-600">
                    <div>{loop.work}</div>
                    <div className="mt-1 font-mono text-[11px] text-stone-500">
                      route: {loop.route}
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
              <div className="text-sm font-semibold text-stone-900">Recent usage ledger</div>
              <div className="text-xs text-stone-500">
                Backend rows expose action/time today; source tags need backend support.
              </div>
            </div>
            <button
              type="button"
              onClick={() => void refresh()}
              disabled={loading}
              className="rounded-md border border-stone-200 px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50">
              Reload
            </button>
          </div>

          <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-3">
            <MetricTile
              label="Week budget"
              value={usage ? formatUsd(usage.cycleBudgetUsd) : 'n/a'}
              detail={`resets ${formatDateTime(usage?.cycleEndsAt)}`}
            />
            <MetricTile
              label="Week remaining"
              value={usage ? formatUsd(usage.remainingUsd) : 'n/a'}
              detail={usage ? `${formatUsd(usage.cycleLimit7day)} used` : undefined}
            />
            <MetricTile
              label="5h window"
              value={
                usage
                  ? `${formatUsd(usage.cycleLimit5hr)} / ${formatUsd(usage.fiveHourCapUsd)}`
                  : 'n/a'
              }
              detail={`resets ${formatDateTime(usage?.fiveHourResetsAt)}`}
            />
            <MetricTile
              label="Avg spend row"
              value={spendSample.avgRowUsd > 0 ? formatUsd(spendSample.avgRowUsd) : 'n/a'}
              detail={`${spendRows.length} recent spend rows`}
            />
            <MetricTile
              label="Bg API reads"
              value={`${formatCount(backgroundApiReadsPerWeek)}/week`}
              detail={`${formatCount(calendarPlannerCallsPerWeek)} planner + ${formatCount(composioConnectionScansPerWeek)} sync`}
            />
            <MetricTile
              label="Bg wakeups"
              value={`${formatCount(backgroundWakeupsPerWeek)}/week`}
              detail={`${formatCount(memoryPollsPerWeek)} memory polls`}
            />
          </div>

          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 p-3">
            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
              Budget math
            </div>
            <div className="mt-2 grid gap-2">
              <FormulaRow
                label="Rows left"
                value={estimatedRowsLeft !== null ? formatCount(estimatedRowsLeft) : 'n/a'}
                detail={
                  estimatedRowsLeft !== null
                    ? `remaining / avg row = ${formatUsd(usage?.remainingUsd ?? 0)} / ${formatUsd(spendSample.avgRowUsd)}`
                    : 'Need recent spend rows to estimate.'
                }
              />
              <FormulaRow
                label="Rows per full week budget"
                value={
                  estimatedRowsPerBudget !== null ? formatCount(estimatedRowsPerBudget) : 'n/a'
                }
                detail={
                  estimatedRowsPerBudget !== null
                    ? `cycle budget / avg row = ${formatUsd(usage?.cycleBudgetUsd ?? 0)} / ${formatUsd(spendSample.avgRowUsd)}`
                    : 'Need recent spend rows to estimate.'
                }
              />
              <FormulaRow
                label="Sample burn rate"
                value={
                  spendSample.spendPerHour > 0 ? `${formatUsd(spendSample.spendPerHour)}/hr` : 'n/a'
                }
                detail={
                  spendSample.sampleHours > 0
                    ? `${formatCount(spendSample.rowsPerHour)} rows/hr across ${spendSample.sampleHours.toFixed(1)}h sample`
                    : 'Need timestamps from at least two spend rows.'
                }
              />
              <FormulaRow
                label="Projected empty"
                value={projectedExhaustAt}
                detail={
                  projectedHoursLeft !== null
                    ? `${projectedHoursLeft.toFixed(1)}h after latest spend at recent burn rate`
                    : 'No projection without recent hourly spend.'
                }
              />
              <FormulaRow
                label="API reads per $ remaining"
                value={
                  scheduledCallsPerRemainingDollar !== null
                    ? `${formatCount(scheduledCallsPerRemainingDollar)} reads/$`
                    : 'n/a'
                }
                detail={
                  usage
                    ? `background API reads/week / remaining = ${formatCount(backgroundApiReadsPerWeek)} / ${formatUsd(usage.remainingUsd)}`
                    : 'Need usage response to estimate.'
                }
              />
            </div>
          </div>

          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 p-3">
            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
              Loop call budget
            </div>
            <div className="mt-2 grid gap-2">
              <FormulaRow
                label="Heartbeat ticks"
                value={`${formatCount(heartbeatTicksPerWeek)}/week`}
                detail={`10080 min/week / ${heartbeatIntervalMinutes} min interval`}
              />
              <FormulaRow
                label="Calendar planner calls"
                value={`${formatCount(calendarPlannerCallsPerWeek)}/week`}
                detail={
                  settings?.notify_meetings
                    ? `ticks * (1 list_connections + ${calendarConnectionsPolled} GOOGLECALENDAR_EVENTS_LIST)`
                    : 'Meeting collector disabled.'
                }
              />
              <FormulaRow
                label="Calendar fanout cap"
                value={`${formatCount(calendarConnectionsPolled)}/${formatCount(activeCalendarConnections.length)} conn/tick`}
                detail={`max_calendar_connections_per_tick = ${maxCalendarConnectionsPerTick}; skipped now = ${calendarConnectionsSkipped}`}
              />
              <FormulaRow
                label="Subconscious model calls"
                value={`${formatCount(subconsciousModelCallsPerWeek)}/week`}
                detail={
                  settings?.enabled && settings.inference_enabled
                    ? 'one kind=subconscious_tick model call per heartbeat tick'
                    : 'Heartbeat inference disabled.'
                }
              />
              <FormulaRow
                label="Composio sync scans"
                value={`${formatCount(composioConnectionScansPerWeek)}/week`}
                detail={`${activeConnections.length} active integration connection(s) scanned every ${COMPOSIO_PERIODIC_TICK_MINUTES} min`}
              />
              <FormulaRow
                label="Total bg API read budget"
                value={`${formatCount(backgroundApiReadsPerWeek)}/week`}
                detail={`calendar planner reads + periodic integration scans; excludes user-initiated chat tools`}
              />
              <FormulaRow
                label="Memory worker polls"
                value={`${formatCount(memoryPollsPerWeek)}/week max`}
                detail={`${MEMORY_WORKERS} workers * ${MEMORY_POLL_SECONDS}s poll; LLM calls only for queued jobs`}
              />
            </div>
          </div>

          {latestSpend && (
            <div className="mt-3 rounded-md border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
              Latest spend: {formatUsd(spendAmount(latestSpend))} at{' '}
              {new Date(latestSpend.createdAt).toLocaleString()} ({latestSpend.action})
            </div>
          )}

          <div className="mt-3 space-y-3">
            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                Top actions
              </div>
              <div className="mt-1 space-y-1">
                {actionSummary.length > 0 ? (
                  actionSummary.map(([action, count, total]) => (
                    <div
                      key={action}
                      className="flex items-center justify-between gap-2 text-xs text-stone-600">
                      <span className="truncate font-mono">{action}</span>
                      <span className="shrink-0 text-stone-500">
                        {count} / {formatUsd(total)}
                      </span>
                    </div>
                  ))
                ) : (
                  <div className="text-xs text-stone-500">No spend rows loaded.</div>
                )}
              </div>
            </div>

            <div>
              <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">
                Top hours
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
                  <div className="text-xs text-stone-500">No hourly spend yet.</div>
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
