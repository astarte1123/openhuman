import { useT } from '../../lib/i18n/I18nContext';
import type { TranscriptChunk } from '../../utils/tauriCommands/meetings';
import { formatClockMs } from './meetingTime';

interface Props {
  chunks: TranscriptChunk[];
}

export default function TranscriptChunkList({ chunks }: Props) {
  const { t } = useT();

  if (chunks.length === 0) {
    return <p className="text-sm text-stone-500">{t('meetings.detail.noTranscript')}</p>;
  }

  return (
    <ul className="space-y-3">
      {chunks.map(chunk => (
        <li key={chunk.id} className="flex gap-3 text-sm">
          <span className="flex-shrink-0 pt-0.5 font-mono text-xs text-stone-400">
            {formatClockMs(chunk.start_ms)}
          </span>
          <div className="min-w-0">
            {chunk.speaker && (
              <span className="mr-2 text-xs font-semibold text-stone-600">{chunk.speaker}</span>
            )}
            <span className="text-stone-800">{chunk.text}</span>
          </div>
        </li>
      ))}
    </ul>
  );
}
