import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  clearRuntimeForThread,
  type InferenceStatus,
  type StreamingAssistantState,
} from '../../../../store/chatRuntimeSlice';
import { setActiveThread } from '../../../../store/threadSlice';
import { useSilenceTimer } from '../useSilenceTimer';

const t = (key: string) => key;

function renderSilenceTimer(props: {
  inferenceStatusByThread?: Record<string, InferenceStatus>;
  streamingAssistantByThread?: Record<string, StreamingAssistantState>;
  dispatch?: ReturnType<typeof vi.fn>;
  setSendError?: ReturnType<typeof vi.fn>;
}) {
  const dispatch = props.dispatch ?? vi.fn();
  const setSendError = props.setSendError ?? vi.fn();
  const { result, rerender } = renderHook(
    p =>
      useSilenceTimer({
        inferenceStatusByThread: p.inferenceStatusByThread,
        streamingAssistantByThread: p.streamingAssistantByThread,
        dispatch: dispatch as any,
        t,
        setSendError: setSendError as any,
      }),
    {
      initialProps: {
        inferenceStatusByThread: props.inferenceStatusByThread ?? {},
        streamingAssistantByThread: props.streamingAssistantByThread ?? {},
      },
    }
  );
  return { result, rerender, dispatch, setSendError };
}

describe('useSilenceTimer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('fires the safety timeout after 120s of silence for the armed thread', () => {
    const { result, dispatch, setSendError } = renderSilenceTimer({});

    act(() => {
      result.current.armSilenceTimer('thread-1');
    });

    expect(setSendError).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(119_999);
    });
    expect(setSendError).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(1);
    });

    expect(setSendError).toHaveBeenCalledTimes(1);
    expect(setSendError.mock.calls[0][0]).toMatchObject({ code: 'safety_timeout' });
    expect(dispatch).toHaveBeenCalledWith(clearRuntimeForThread({ threadId: 'thread-1' }));
    expect(dispatch).toHaveBeenCalledWith(setActiveThread(null));
  });

  it('cancelSilenceTimer clears a pending timer so it never fires', () => {
    const { result, setSendError } = renderSilenceTimer({});

    act(() => {
      result.current.armSilenceTimer('thread-1');
      result.current.cancelSilenceTimer();
    });

    act(() => {
      vi.advanceTimersByTime(200_000);
    });

    expect(setSendError).not.toHaveBeenCalled();
  });

  it('rearms on a fresh inference status for the armed thread instead of firing at the original 120s mark', () => {
    const { result, rerender, setSendError } = renderSilenceTimer({});

    act(() => {
      result.current.armSilenceTimer('thread-1');
    });

    act(() => {
      vi.advanceTimersByTime(100_000);
    });

    // A new inference signal arrives for the armed thread — the effect
    // should rearm, pushing the deadline out by another 120s.
    rerender({
      inferenceStatusByThread: {
        'thread-1': { phase: 'thinking', iteration: 1, maxIterations: 5 },
      },
      streamingAssistantByThread: {},
    });

    act(() => {
      vi.advanceTimersByTime(20_000);
    });
    // Would have fired at t=120s under the original arm; rearm pushed it out.
    expect(setSendError).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(100_000);
    });
    expect(setSendError).toHaveBeenCalledTimes(1);
  });

  it('drops the timer when the thread status is cleared (chat_done / chat_error)', () => {
    const { result, rerender, setSendError } = renderSilenceTimer({
      inferenceStatusByThread: {
        'thread-1': { phase: 'thinking', iteration: 1, maxIterations: 5 },
      },
    });

    act(() => {
      result.current.armSilenceTimer('thread-1');
    });

    // Status for the armed thread disappears (turn completed) — effect must
    // clear the timer rather than rearm it.
    rerender({ inferenceStatusByThread: {}, streamingAssistantByThread: {} });

    act(() => {
      vi.advanceTimersByTime(200_000);
    });

    expect(setSendError).not.toHaveBeenCalled();
  });

  it('re-arming for a new thread supersedes the timer for the previous one', () => {
    const { result, dispatch, setSendError } = renderSilenceTimer({});

    act(() => {
      result.current.armSilenceTimer('thread-1');
    });
    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    act(() => {
      result.current.armSilenceTimer('thread-2');
    });
    act(() => {
      vi.advanceTimersByTime(120_000);
    });

    expect(setSendError).toHaveBeenCalledTimes(1);
    expect(dispatch).toHaveBeenCalledWith(clearRuntimeForThread({ threadId: 'thread-2' }));
  });
});
