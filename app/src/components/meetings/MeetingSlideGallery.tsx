import { convertFileSrc } from '@tauri-apps/api/core';
import { useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import type { MeetingScreenshot } from '../../utils/tauriCommands/meetings';
import { formatClockMs } from './meetingTime';

interface Props {
  screenshots: MeetingScreenshot[];
}

/** OCR text longer than this is truncated behind a "show more" toggle. */
const OCR_TEXT_TRUNCATE_LENGTH = 160;

function SlideCard({ screenshot }: { screenshot: MeetingScreenshot }) {
  const { t } = useT();
  const [expanded, setExpanded] = useState(false);

  const ocrText = screenshot.ocr_text.trim();
  const isLong = ocrText.length > OCR_TEXT_TRUNCATE_LENGTH;
  const displayedText =
    expanded || !isLong ? ocrText : `${ocrText.slice(0, OCR_TEXT_TRUNCATE_LENGTH)}…`;

  return (
    <li className="space-y-2 rounded-xl border border-stone-200 p-2">
      <img
        src={convertFileSrc(screenshot.image_path)}
        alt={t('meetings.detail.slideAt').replace(
          '{time}',
          formatClockMs(screenshot.captured_at_ms)
        )}
        className="aspect-video w-full rounded-lg border border-stone-100 object-contain"
      />
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-xs text-stone-400">
          {formatClockMs(screenshot.captured_at_ms)}
        </span>
      </div>
      {ocrText && (
        <div className="space-y-1">
          <p className="whitespace-pre-wrap text-xs text-stone-700">{displayedText}</p>
          {isLong && (
            <button
              type="button"
              onClick={() => setExpanded(prev => !prev)}
              className="text-xs font-medium text-primary-600 hover:text-primary-700">
              {expanded
                ? t('meetings.detail.slideTextShowLess')
                : t('meetings.detail.slideTextShowMore')}
            </button>
          )}
        </div>
      )}
    </li>
  );
}

export default function MeetingSlideGallery({ screenshots }: Props) {
  const { t } = useT();

  if (screenshots.length === 0) {
    return <p className="text-sm text-stone-500">{t('meetings.detail.noSlides')}</p>;
  }

  return (
    <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2">
      {screenshots.map(screenshot => (
        <SlideCard key={screenshot.id} screenshot={screenshot} />
      ))}
    </ul>
  );
}
