import { useT } from '../../lib/i18n/I18nContext';
import type { MeetingStatus } from '../../utils/tauriCommands/meetings';

const STATUS_CLASSES: Record<MeetingStatus, string> = {
  recording: 'bg-amber-50 text-amber-700 ring-amber-200',
  completed: 'bg-sage-50 text-sage-700 ring-sage-200',
  error: 'bg-coral-50 text-coral-700 ring-coral-200',
};

interface Props {
  status: MeetingStatus;
}

export default function MeetingStatusBadge({ status }: Props) {
  const { t } = useT();
  const label =
    status === 'recording'
      ? t('meetings.status.recording')
      : status === 'completed'
        ? t('meetings.status.completed')
        : t('meetings.status.error');

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-[11px] font-medium ring-1 ${STATUS_CLASSES[status]}`}>
      {status === 'recording' && (
        <span className="h-1.5 w-1.5 rounded-full bg-amber-500 animate-pulse" aria-hidden="true" />
      )}
      {label}
    </span>
  );
}
