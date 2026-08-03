import debugFactory from 'debug';
import { useEffect, useRef } from 'react';

import { type ChatSendError, chatSendError } from '../../../chat/chatSendError';
import type { AppDispatch } from '../../../store';
import {
  clearRuntimeForThread,
  type InferenceStatus,
  type StreamingAssistantState,
} from '../../../store/chatRuntimeSlice';
import { setActiveThread } from '../../../store/threadSlice';

const debug = debugFactory('conversations');

export const useSilenceTimer = ({
  inferenceStatusByThread,
  streamingAssistantByThread,
  dispatch,
  t,
  setSendError,
}: {
  inferenceStatusByThread: Record<string, InferenceStatus>;
  streamingAssistantByThread: Record<string, StreamingAssistantState>;
  dispatch: AppDispatch;
  t: (key: string) => string;
  setSendError: (error: ChatSendError | null) => void;
}) => {
  const sendingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Thread id whose send started the current silence timer. Tracked separately
  // from `selectedThreadId` so switching threads mid-turn doesn't move the
  // timer's reference point.
  const sendingThreadIdRef = useRef<string | null>(null);

  const armSilenceTimer = (threadId: string) => {
    if (sendingTimeoutRef.current) clearTimeout(sendingTimeoutRef.current);
    sendingThreadIdRef.current = threadId;
    sendingTimeoutRef.current = setTimeout(() => {
      debug('armSilenceTimer: no inference signal for 120s — clearing runtime');
      setSendError(chatSendError('safety_timeout', t('chat.safetyTimeout')));
      dispatch(clearRuntimeForThread({ threadId }));
      dispatch(setActiveThread(null));
      sendingTimeoutRef.current = null;
      sendingThreadIdRef.current = null;
    }, 120_000);
  };

  // Consolidates the silence-timer cleanup previously duplicated inline in
  // `handleSendMessage`'s catch block: clear the pending timeout (if any)
  // and drop the thread-id reference point.
  const cancelSilenceTimer = () => {
    if (sendingTimeoutRef.current) {
      clearTimeout(sendingTimeoutRef.current);
      sendingTimeoutRef.current = null;
    }
    sendingThreadIdRef.current = null;
  };

  // Rearm the silence timer on every inference signal for the sending
  // thread. Tool / iteration / subagent events bump `inferenceStatusByThread`;
  // pure-text streams (no tools) only bump `streamingAssistantByThread`, so
  // both must be watched — otherwise a long text stream would trip the
  // safety timer mid-reply. When the status is cleared (chat_done /
  // chat_error), drop the timer — the completion handlers own UI cleanup.
  useEffect(() => {
    const threadId = sendingThreadIdRef.current;
    if (!threadId || !sendingTimeoutRef.current) return;
    const status = inferenceStatusByThread[threadId];
    if (status === undefined) {
      clearTimeout(sendingTimeoutRef.current);
      sendingTimeoutRef.current = null;
      sendingThreadIdRef.current = null;
      return;
    }
    armSilenceTimer(threadId);
    // armSilenceTimer is stable (refs + dispatch); depending on the
    // selector references is enough to rearm on every progress event.
    // (No eslint-disable needed here: this file is plain `.ts`, and the
    // `react-hooks/exhaustive-deps` rule is only wired up for `.tsx` files
    // in this repo's eslint config — see `app/eslint.config.js`.)
  }, [inferenceStatusByThread, streamingAssistantByThread]);

  return { armSilenceTimer, cancelSilenceTimer };
};
