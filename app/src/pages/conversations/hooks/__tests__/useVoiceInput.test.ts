import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  auraVoiceStatus,
  auraVoiceTranscribeBytes,
  auraVoiceTts,
  notifyOverlaySttState,
} from '../../../../utils/tauriCommands';
import { useVoiceInput } from '../useVoiceInput';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));

vi.mock('../../../../utils/tauriCommands', () => ({
  notifyOverlaySttState: vi.fn(),
  auraVoiceStatus: vi.fn(),
  auraVoiceTranscribeBytes: vi.fn(),
  auraVoiceTts: vi.fn(),
}));

class FakeMediaStreamTrack {
  stop = vi.fn();
}

class FakeMediaStream {
  private tracks = [new FakeMediaStreamTrack()];
  getTracks() {
    return this.tracks;
  }
}

/** Minimal controllable stand-in for the browser `MediaRecorder`. */
class FakeMediaRecorder {
  static isTypeSupported = vi.fn(() => true);
  static instances: FakeMediaRecorder[] = [];

  mimeType: string;
  ondataavailable: ((event: { data: { size: number } }) => void) | null = null;
  onerror: (() => void) | null = null;
  onstop: (() => void) | null = null;
  start = vi.fn();
  stop = vi.fn(() => {
    this.onstop?.();
  });

  constructor(_stream: unknown, options?: { mimeType?: string }) {
    this.mimeType = options?.mimeType ?? 'audio/webm';
    FakeMediaRecorder.instances.push(this);
  }
}

function emitDataAvailable(recorder: FakeMediaRecorder, size = 10) {
  recorder.ondataavailable?.({ data: { size } as Blob });
}

const baseMessages = [{ id: 'm1', sender: 'user', content: 'hi' } as any];

function renderVoiceInput(overrides: Partial<Parameters<typeof useVoiceInput>[0]> = {}) {
  const setReplyMode = vi.fn();
  const setSendError = vi.fn();
  const onTranscript = vi.fn().mockResolvedValue(undefined);
  const { result, rerender } = renderHook(
    (props: Partial<Parameters<typeof useVoiceInput>[0]>) =>
      useVoiceInput({
        rustChat: props.rustChat ?? true,
        activeThreadId: props.activeThreadId ?? null,
        messages: props.messages ?? baseMessages,
        inputMode: props.inputMode ?? 'voice',
        replyMode: props.replyMode ?? 'voice',
        setReplyMode,
        setSendError,
        onTranscript,
      }),
    { initialProps: overrides }
  );
  return { result, rerender, setReplyMode, setSendError, onTranscript };
}

describe('useVoiceInput', () => {
  const getUserMedia = vi.fn();

  beforeEach(() => {
    FakeMediaRecorder.instances = [];
    vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
    vi.stubGlobal(
      'Audio',
      class {
        play = vi.fn().mockResolvedValue(undefined);
        pause = vi.fn();
      }
    );
    getUserMedia.mockReset().mockResolvedValue(new FakeMediaStream());
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: { getUserMedia },
      configurable: true,
    });
    vi.mocked(notifyOverlaySttState).mockReset();
    vi.mocked(auraVoiceStatus)
      .mockReset()
      .mockResolvedValue({ stt_available: true } as any);
    vi.mocked(auraVoiceTranscribeBytes).mockReset();
    vi.mocked(auraVoiceTts).mockReset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('reports the microphone as unavailable and refuses to record when getUserMedia is missing', async () => {
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: undefined,
      configurable: true,
    });
    const { result, setSendError } = renderVoiceInput();

    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });

    expect(result.current.canUseMicrophoneApi).toBe(false);
    expect(setSendError).toHaveBeenCalledWith(
      expect.objectContaining({ code: 'microphone_unavailable' })
    );
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it('does nothing when rustChat is false, a thread is active, or already transcribing', async () => {
    const { result: blockedByRustChat } = renderVoiceInput({ rustChat: false });
    await act(async () => {
      await blockedByRustChat.current.handleVoiceRecordToggle();
    });
    expect(getUserMedia).not.toHaveBeenCalled();

    const { result: blockedByActiveThread } = renderVoiceInput({ activeThreadId: 'thread-1' });
    await act(async () => {
      await blockedByActiveThread.current.handleVoiceRecordToggle();
    });
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it('starts recording on toggle: acquires the mic, creates a recorder, and flips isRecording', async () => {
    const { result } = renderVoiceInput();

    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });

    expect(getUserMedia).toHaveBeenCalledWith({ audio: true });
    expect(FakeMediaRecorder.instances).toHaveLength(1);
    expect(FakeMediaRecorder.instances[0].start).toHaveBeenCalledTimes(1);
    expect(result.current.isRecording).toBe(true);
  });

  it('stops the active recorder on a second toggle instead of requesting the mic again', async () => {
    const { result } = renderVoiceInput();
    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });
    getUserMedia.mockClear();

    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });

    expect(getUserMedia).not.toHaveBeenCalled();
    expect(FakeMediaRecorder.instances[0].stop).toHaveBeenCalledTimes(1);
  });

  it('transcribeAndSendAudio: no captured audio → status message, no transcript sent', async () => {
    const { result, onTranscript } = renderVoiceInput();
    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });
    const recorder = FakeMediaRecorder.instances[0];

    await act(async () => {
      recorder.stop();
    });

    expect(result.current.voiceStatus).toBe('No audio captured. Try again.');
    expect(onTranscript).not.toHaveBeenCalled();
    expect(vi.mocked(notifyOverlaySttState)).toHaveBeenCalledWith('cancelled');
  });

  it('transcribeAndSendAudio: happy path transcribes captured audio and forwards it to onTranscript', async () => {
    vi.mocked(auraVoiceTranscribeBytes).mockResolvedValue({ text: '  hello world  ' } as any);
    const { result, onTranscript } = renderVoiceInput();
    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });
    const recorder = FakeMediaRecorder.instances[0];
    emitDataAvailable(recorder);

    await act(async () => {
      recorder.stop();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(auraVoiceTranscribeBytes).toHaveBeenCalled();
    expect(onTranscript).toHaveBeenCalledWith('hello world');
    expect(result.current.isTranscribing).toBe(false);
  });

  it('transcribeAndSendAudio: empty transcript → status message, no transcript sent', async () => {
    vi.mocked(auraVoiceTranscribeBytes).mockResolvedValue({ text: '   ' } as any);
    const { result, onTranscript } = renderVoiceInput();
    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });
    const recorder = FakeMediaRecorder.instances[0];
    emitDataAvailable(recorder);

    await act(async () => {
      recorder.stop();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(onTranscript).not.toHaveBeenCalled();
    expect(result.current.voiceStatus).toBe('No speech detected. Try again.');
  });

  it('transcribeAndSendAudio: surfaces a setup-specific error when transcription fails for a missing STT model', async () => {
    vi.mocked(auraVoiceTranscribeBytes).mockRejectedValue(new Error('STT model not found'));
    const { result, setSendError } = renderVoiceInput();
    await act(async () => {
      await result.current.handleVoiceRecordToggle();
    });
    const recorder = FakeMediaRecorder.instances[0];
    emitDataAvailable(recorder);

    await act(async () => {
      recorder.stop();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(setSendError).toHaveBeenCalledWith(expect.objectContaining({ code: 'stt_not_ready' }));
  });

  it('syncs replyMode with inputMode: entering voice mode forces voice replies, leaving it restores text', () => {
    const { rerender, setReplyMode } = renderVoiceInput({ inputMode: 'text', replyMode: 'text' });
    rerender({ inputMode: 'voice', replyMode: 'text' });
    expect(setReplyMode).toHaveBeenCalledWith('voice');

    setReplyMode.mockClear();
    rerender({ inputMode: 'text', replyMode: 'voice' });
    expect(setReplyMode).toHaveBeenCalledWith('text');
  });

  it('TTS playback effect: speaks a new agent message once when replyMode is voice', async () => {
    vi.mocked(auraVoiceTts).mockResolvedValue({ output_path: '/tmp/reply.wav' } as any);
    const messages = [{ id: 'a1', sender: 'agent', content: 'hello human' } as any];
    const { result, rerender } = renderVoiceInput({ messages: [] });

    await act(async () => {
      rerender({ messages, replyMode: 'voice', rustChat: true });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(auraVoiceTts).toHaveBeenCalledWith('hello human');
    expect(result.current.isPlayingReply).toBe(false); // resolved by the time we assert

    // Re-rendering with the same latest agent message must not re-speak it.
    vi.mocked(auraVoiceTts).mockClear();
    await act(async () => {
      rerender({ messages, replyMode: 'voice', rustChat: true });
    });
    expect(auraVoiceTts).not.toHaveBeenCalled();
  });

  it('TTS playback effect: does not speak when replyMode is text', async () => {
    const messages = [{ id: 'a1', sender: 'agent', content: 'hello human' } as any];
    renderVoiceInput({ messages, replyMode: 'text' });

    await act(async () => {
      await Promise.resolve();
    });

    expect(auraVoiceTts).not.toHaveBeenCalled();
  });
});
