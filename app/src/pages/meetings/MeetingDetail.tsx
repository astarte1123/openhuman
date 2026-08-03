import createDebug from 'debug';
import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';

import { ConfirmationModal } from '../../components/intelligence/ConfirmationModal';
import MeetingExportPanel from '../../components/meetings/MeetingExportPanel';
import MeetingSlideGallery from '../../components/meetings/MeetingSlideGallery';
import MeetingStatusBadge from '../../components/meetings/MeetingStatusBadge';
import MeetingSummaryPanel from '../../components/meetings/MeetingSummaryPanel';
import { formatClockMs } from '../../components/meetings/meetingTime';
import TranscriptChunkList from '../../components/meetings/TranscriptChunkList';
import { useT } from '../../lib/i18n/I18nContext';
import type { ConfirmationModal as ConfirmationModalType } from '../../types/intelligence';
import { meetingRecorderStop } from '../../utils/tauriCommands/meetingRecorder';
import {
  type Meeting,
  type MeetingScreenshot,
  auraMeetingsDelete,
  auraMeetingsGet,
  auraMeetingsUpdateTitle,
  type TranscriptChunk,
} from '../../utils/tauriCommands/meetings';

const log = createDebug('app:pages:MeetingDetail');

const EMPTY_DELETE_MODAL: ConfirmationModalType = {
  isOpen: false,
  title: '',
  message: '',
  onConfirm: () => {},
  onCancel: () => {},
};

const MeetingDetail = () => {
  const { t } = useT();
  const navigate = useNavigate();
  const { meetingId } = useParams<{ meetingId: string }>();

  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [chunks, setChunks] = useState<TranscriptChunk[]>([]);
  const [screenshots, setScreenshots] = useState<MeetingScreenshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [coreError, setCoreError] = useState<string | null>(null);

  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');
  const [savingTitle, setSavingTitle] = useState(false);

  const [stopping, setStopping] = useState(false);
  const [stopError, setStopError] = useState<string | null>(null);

  const [deleteModal, setDeleteModal] = useState<ConfirmationModalType>(EMPTY_DELETE_MODAL);

  const loadMeeting = useCallback(async () => {
    if (!meetingId) return;
    log('loadMeeting: start meeting_id=%s', meetingId);
    setLoading(true);
    setCoreError(null);
    try {
      const response = await auraMeetingsGet(meetingId);
      setMeeting(response.result.meeting);
      setChunks(response.result.chunks);
      setScreenshots(response.result.screenshots);
      log(
        'loadMeeting: success chunk_count=%d screenshot_count=%d',
        response.result.chunks.length,
        response.result.screenshots.length
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log('loadMeeting: failure %s', message);
      setCoreError(message);
    } finally {
      setLoading(false);
    }
  }, [meetingId]);

  useEffect(() => {
    void loadMeeting();
  }, [loadMeeting]);

  const handleStartEditTitle = () => {
    if (!meeting) return;
    setTitleDraft(meeting.title);
    setEditingTitle(true);
  };

  const commitTitle = async () => {
    setEditingTitle(false);
    if (!meeting || !meetingId) return;
    const nextTitle = titleDraft.trim();
    if (!nextTitle || nextTitle === meeting.title) return;
    setSavingTitle(true);
    try {
      const response = await auraMeetingsUpdateTitle(meetingId, nextTitle);
      setMeeting(response.result);
      log('commitTitle: success meeting_id=%s', meetingId);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log('commitTitle: failure %s', message);
      setCoreError(message);
    } finally {
      setSavingTitle(false);
    }
  };

  const handleStopRecording = async () => {
    setStopping(true);
    setStopError(null);
    try {
      await meetingRecorderStop();
      log('handleStopRecording: success');
      await loadMeeting();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log('handleStopRecording: failure %s', message);
      setStopError(message);
    } finally {
      setStopping(false);
    }
  };

  const handleDelete = () => {
    if (!meeting || !meetingId) return;
    setDeleteModal({
      isOpen: true,
      title: t('meetings.detail.deleteTitle'),
      message: t('meetings.detail.deleteConfirm').replace('{title}', meeting.title),
      confirmText: t('common.delete'),
      cancelText: t('common.cancel'),
      destructive: true,
      onConfirm: () => {
        void (async () => {
          try {
            await auraMeetingsDelete(meetingId);
            log('handleDelete: success meeting_id=%s', meetingId);
            navigate('/meetings');
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            log('handleDelete: failure %s', message);
            setCoreError(message);
          }
        })();
      },
      onCancel: () => {},
    });
  };

  if (loading) {
    return <div className="p-6 text-center text-sm text-stone-500">{t('common.loading')}</div>;
  }

  if (!meeting) {
    return (
      <div className="p-4">
        <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-sm text-coral-700">
          {coreError ?? t('meetings.detail.notFound')}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4 p-4 pt-6">
      <div className="mx-auto max-w-2xl space-y-4">
        <button
          type="button"
          onClick={() => navigate('/meetings')}
          className="text-xs font-medium text-stone-500 hover:text-stone-700">
          {`← ${t('meetings.title')}`}
        </button>

        <div className="space-y-4 rounded-2xl border border-stone-200 bg-white p-4 shadow-soft">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              {editingTitle ? (
                <input
                  autoFocus
                  type="text"
                  value={titleDraft}
                  onChange={e => setTitleDraft(e.target.value)}
                  onBlur={() => void commitTitle()}
                  onKeyDown={e => {
                    if (e.key === 'Enter') e.currentTarget.blur();
                    if (e.key === 'Escape') setEditingTitle(false);
                  }}
                  className="w-full rounded-lg border border-primary-300 px-2 py-1 text-lg font-semibold text-stone-900"
                />
              ) : (
                <h1
                  onClick={handleStartEditTitle}
                  className="cursor-text truncate text-lg font-semibold text-stone-900"
                  title={t('common.edit')}>
                  {meeting.title}
                  {savingTitle && (
                    <span className="ml-2 text-xs font-normal text-stone-400">
                      {t('common.loading')}
                    </span>
                  )}
                </h1>
              )}
              <div className="mt-1 flex items-center gap-2">
                <MeetingStatusBadge status={meeting.status} />
                {meeting.duration_ms != null && (
                  <span className="font-mono text-xs text-stone-500">
                    {formatClockMs(meeting.duration_ms)}
                  </span>
                )}
              </div>
            </div>
            <button
              type="button"
              onClick={handleDelete}
              className="flex-shrink-0 rounded-lg border border-coral-200 px-3 py-1.5 text-xs font-medium text-coral-700 transition-colors hover:bg-coral-50">
              {t('meetings.detail.deleteMeeting')}
            </button>
          </div>

          {coreError && (
            <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
              {coreError}
            </div>
          )}

          {meeting.status === 'recording' && (
            <div className="flex items-center justify-between gap-3 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2">
              <span className="text-xs text-amber-700">
                {t('meetings.detail.recordingInProgress')}
              </span>
              <button
                type="button"
                onClick={() => void handleStopRecording()}
                disabled={stopping}
                className="rounded-lg bg-coral-500 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-coral-600 disabled:opacity-50">
                {stopping ? t('meetings.detail.stopping') : t('meetings.detail.stopRecording')}
              </button>
            </div>
          )}
          {stopError && (
            <div className="rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
              {stopError}
            </div>
          )}
        </div>

        <div className="space-y-3 rounded-2xl border border-stone-200 bg-white p-4 shadow-soft">
          <h2 className="text-sm font-semibold text-stone-900">
            {t('meetings.detail.transcript')}
          </h2>
          <TranscriptChunkList chunks={chunks} />
        </div>

        <div className="space-y-3 rounded-2xl border border-stone-200 bg-white p-4 shadow-soft">
          <h2 className="text-sm font-semibold text-stone-900">{t('meetings.detail.slides')}</h2>
          <MeetingSlideGallery screenshots={screenshots} />
        </div>

        <div className="rounded-2xl border border-stone-200 bg-white p-4 shadow-soft">
          <MeetingExportPanel meetingId={meeting.id} hasTranscript={chunks.length > 0} />
        </div>

        <div className="rounded-2xl border border-stone-200 bg-white p-4 shadow-soft">
          <MeetingSummaryPanel
            meetingId={meeting.id}
            summary={meeting.summary}
            hasTranscript={chunks.length > 0}
            onSummarized={() => void loadMeeting()}
          />
        </div>
      </div>

      <ConfirmationModal
        modal={deleteModal}
        onClose={() => setDeleteModal(prev => ({ ...prev, isOpen: false }))}
      />
    </div>
  );
};

export default MeetingDetail;
