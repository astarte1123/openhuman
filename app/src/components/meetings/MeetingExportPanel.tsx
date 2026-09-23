import { useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import {
  type MeetingExportFormat,
  auraMeetingsExport,
} from '../../utils/tauriCommands/meetings';

interface Props {
  meetingId: string;
  hasTranscript: boolean;
}

export default function MeetingExportPanel({ meetingId, hasTranscript }: Props) {
  const { t } = useT();
  const [exportingFormat, setExportingFormat] = useState<MeetingExportFormat | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportedPath, setExportedPath] = useState<string | null>(null);

  const handleExport = async (format: MeetingExportFormat) => {
    setExportingFormat(format);
    setError(null);
    setExportedPath(null);
    try {
      const response = await auraMeetingsExport(meetingId, format);
      setExportedPath(response.result.path);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setExportingFormat(null);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-stone-900">{t('meetings.detail.export')}</h2>
        <div className="flex gap-2">
          {(['pdf', 'docx', 'pptx'] as const).map(format => (
            <button
              key={format}
              type="button"
              onClick={() => void handleExport(format)}
              disabled={exportingFormat !== null || !hasTranscript}
              title={!hasTranscript ? t('meetings.detail.recordFirst') : undefined}
              className="rounded-lg border border-stone-300 px-3 py-1.5 text-xs font-medium text-stone-700 transition-colors hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-50">
              {exportingFormat === format
                ? t('meetings.detail.exporting')
                : t('meetings.detail.exportAs').replace('{format}', format.toUpperCase())}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
          {error}
        </div>
      )}

      {exportedPath && (
        <div className="space-y-1 rounded-lg border border-sage-200 bg-sage-50 px-3 py-2">
          <p className="text-xs text-sage-700">{t('meetings.detail.exportSuccess')}</p>
          <p className="break-all font-mono text-xs text-sage-600">{exportedPath}</p>
        </div>
      )}
    </div>
  );
}
