import { useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { BubbleMarkdown } from '../../pages/conversations/components/AgentMessageBubble';
import {
  auraMeetingsSummarizeStructured,
  type MeetingSummary,
} from '../../utils/tauriCommands/meetings';

interface Props {
  meetingId: string;
  summary: string | null;
  structuredSummary: MeetingSummary | null;
  hasTranscript: boolean;
  onSummarized: () => void;
}

/**
 * Renders the meeting minutes: prose overview, decisions, and action items
 * when a structured summary is available (generated via
 * `auraMeetingsSummarizeStructured`, which also persists the flat-prose
 * `summary` used by `MeetingExportPanel`'s PDF/DOCX/PPTX export). Falls
 * back to the plain prose `summary` for meetings summarized before the
 * structured RPC existed, or if structuring failed server-side.
 */
export default function MeetingSummaryPanel({
  meetingId,
  summary,
  structuredSummary,
  hasTranscript,
  onSummarized,
}: Props) {
  const { t } = useT();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSummarize = async () => {
    setLoading(true);
    setError(null);
    try {
      await auraMeetingsSummarizeStructured(meetingId);
      onSummarized();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const hasAnySummary = !!structuredSummary || !!summary;

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-stone-900">{t('meetings.detail.summary')}</h2>
        <button
          type="button"
          onClick={() => void handleSummarize()}
          disabled={loading || !hasTranscript}
          title={!hasTranscript ? t('meetings.detail.recordFirst') : undefined}
          className="rounded-lg bg-primary-500 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-primary-600 disabled:cursor-not-allowed disabled:opacity-50">
          {loading
            ? t('meetings.detail.summarizing')
            : hasAnySummary
              ? t('meetings.detail.regenerateSummary')
              : t('meetings.detail.generateSummary')}
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
          {error}
        </div>
      )}

      {structuredSummary ? (
        <div className="space-y-4">
          {structuredSummary.overview && <BubbleMarkdown content={structuredSummary.overview} />}

          {structuredSummary.decisions.length > 0 && (
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-stone-500">
                {t('meetings.detail.decisions')}
              </h3>
              <ul className="list-disc space-y-1 pl-5 text-sm text-stone-800">
                {structuredSummary.decisions.map((decision, i) => (
                  <li key={i}>{decision}</li>
                ))}
              </ul>
            </div>
          )}

          {structuredSummary.action_items.length > 0 && (
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-stone-500">
                {t('meetings.detail.actionItems')}
              </h3>
              <ul className="list-disc space-y-1 pl-5 text-sm text-stone-800">
                {structuredSummary.action_items.map((item, i) => (
                  <li key={i}>
                    {item.text}
                    {item.owner && <span className="text-stone-500"> — {item.owner}</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      ) : summary ? (
        <BubbleMarkdown content={summary} />
      ) : (
        !loading && <p className="text-sm text-stone-500">{t('meetings.detail.noSummary')}</p>
      )}
    </div>
  );
}
