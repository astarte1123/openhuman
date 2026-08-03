import { useNavigate } from 'react-router-dom';

import { formatRelativeTime } from '../../pages/conversations/utils/format';
import type { Meeting } from '../../utils/tauriCommands/meetings';
import MeetingStatusBadge from './MeetingStatusBadge';
import { formatClockMs } from './meetingTime';

interface Props {
  meeting: Meeting;
}

export default function MeetingListRow({ meeting }: Props) {
  const navigate = useNavigate();

  return (
    <li>
      <button
        type="button"
        onClick={() => navigate(`/meetings/${meeting.id}`)}
        className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-stone-50">
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-semibold text-stone-900">{meeting.title}</p>
          <p className="mt-0.5 text-xs text-stone-500">{formatRelativeTime(meeting.created_at)}</p>
        </div>
        <div className="flex flex-shrink-0 items-center gap-3">
          {meeting.duration_ms != null && (
            <span className="font-mono text-xs text-stone-500">
              {formatClockMs(meeting.duration_ms)}
            </span>
          )}
          <MeetingStatusBadge status={meeting.status} />
        </div>
      </button>
    </li>
  );
}
