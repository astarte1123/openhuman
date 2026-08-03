import { useEffect, useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import {
  type AudioDeviceInfo,
  meetingRecorderListDevices,
  meetingRecorderStart,
} from '../../utils/tauriCommands/meetingRecorder';

interface Props {
  onClose: () => void;
  onStarted: (meetingId: string) => void;
}

/**
 * Inline "New recording" panel — structurally modeled on
 * `components/intelligence/ConfirmationModal.tsx` (backdrop + centered white
 * card), but owns its own local state rather than the global
 * `ConfirmationModal` data shape, since it needs a multi-field form
 * (device selects + title) rather than a yes/no prompt.
 */
export default function MeetingRecordModal({ onClose, onStarted }: Props) {
  const { t } = useT();

  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [loadingDevices, setLoadingDevices] = useState(true);
  const [devicesError, setDevicesError] = useState<string | null>(null);

  const [micDeviceName, setMicDeviceName] = useState('');
  const [systemDeviceName, setSystemDeviceName] = useState('');
  const [title, setTitle] = useState('');

  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setLoadingDevices(true);
      setDevicesError(null);
      try {
        const list = await meetingRecorderListDevices();
        if (cancelled) return;
        setDevices(list);
        const firstMic = list.find(d => d.deviceType === 'input');
        const firstSystem = list.find(d => d.deviceType === 'output');
        if (firstMic) setMicDeviceName(firstMic.name);
        if (firstSystem) setSystemDeviceName(firstSystem.name);
      } catch (err) {
        if (cancelled) return;
        setDevicesError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoadingDevices(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const micOptions = devices.filter(d => d.deviceType === 'input');
  const systemOptions = devices.filter(d => d.deviceType === 'output');

  const handleStart = async () => {
    setStarting(true);
    setStartError(null);
    try {
      const meetingId = await meetingRecorderStart(
        micDeviceName || undefined,
        systemDeviceName || undefined,
        title.trim() || undefined
      );
      onStarted(meetingId);
    } catch (err) {
      setStartError(err instanceof Error ? err.message : String(err));
      setStarting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4 animate-fade-in"
      onClick={onClose}>
      <div
        className="w-full max-w-md rounded-2xl border border-stone-200 bg-white p-6 shadow-large animate-slide-up"
        onClick={e => e.stopPropagation()}>
        <h2 className="text-lg font-semibold text-stone-900">{t('meetings.recordModal.title')}</h2>

        {devicesError && (
          <div className="mt-4 rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
            {devicesError}
          </div>
        )}

        {loadingDevices ? (
          <p className="mt-4 text-sm text-stone-500">{t('common.loading')}</p>
        ) : (
          !devicesError && (
            <div className="mt-4 space-y-4">
              <div>
                <label className="mb-1 block text-xs font-medium text-stone-600">
                  {t('meetings.recordModal.microphone')}
                </label>
                <select
                  value={micDeviceName}
                  onChange={e => setMicDeviceName(e.target.value)}
                  className="w-full rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900">
                  {micOptions.length === 0 && (
                    <option value="">{t('meetings.recordModal.noDevices')}</option>
                  )}
                  {micOptions.map(device => (
                    <option key={device.name} value={device.name}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="mb-1 block text-xs font-medium text-stone-600">
                  {t('meetings.recordModal.systemAudio')}
                </label>
                <select
                  value={systemDeviceName}
                  onChange={e => setSystemDeviceName(e.target.value)}
                  className="w-full rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900">
                  <option value="">{t('meetings.recordModal.none')}</option>
                  {systemOptions.map(device => (
                    <option key={device.name} value={device.name}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="mb-1 block text-xs font-medium text-stone-600">
                  {t('meetings.recordModal.titleLabel')}
                </label>
                <input
                  type="text"
                  value={title}
                  onChange={e => setTitle(e.target.value)}
                  placeholder={t('meetings.recordModal.titlePlaceholder')}
                  className="w-full rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900"
                />
              </div>
            </div>
          )
        )}

        {startError && (
          <div className="mt-4 rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-xs text-coral-700">
            {startError}
          </div>
        )}

        <div className="mt-6 flex items-center justify-end gap-3 border-t border-stone-200 pt-4">
          <button
            type="button"
            onClick={onClose}
            disabled={starting}
            className="rounded-lg px-4 py-2 text-sm font-medium text-stone-600 transition-colors hover:bg-stone-100 hover:text-stone-900 disabled:opacity-50">
            {t('common.cancel')}
          </button>
          <button
            type="button"
            onClick={() => void handleStart()}
            disabled={starting || loadingDevices || !!devicesError || micOptions.length === 0}
            className="rounded-lg bg-primary-500 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-primary-600 disabled:cursor-not-allowed disabled:opacity-50">
            {starting ? t('meetings.recordModal.starting') : t('meetings.recordModal.start')}
          </button>
        </div>
      </div>
    </div>
  );
}
