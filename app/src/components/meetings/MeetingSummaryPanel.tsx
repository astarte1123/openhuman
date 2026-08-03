import { useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { BubbleMarkdown } from '../../pages/conversations/components/AgentMessageBubble';
import { openhumanMeetingsSummarize } from '../../utils/tauriCommands/meetings';

interface Props {
  meetingId: string;
  summary: string | null;
  hasTranscript: boolean;
  onSummarized: () => void;
}

export default function MeetingSummaryPanel({
  meetingId,
  summary,
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
      await openhumanMeetingsSummarize(meetingId);
      onSummarized();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

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
            : summary
              ? t('meetings.detail.regenerateSummary')
              : t('meetings.detail.generateSummary')}
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
          {error}
        </div>
      )}

      {summary ? (
        <BubbleMarkdown content={summary} />
      ) : (
        !loading && <p className="text-sm text-stone-500">{t('meetings.detail.noSummary')}</p>
      )}
    </div>
  );
}
