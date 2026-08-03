import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { isTauri, auraAutocompleteCurrent } from '../../../../utils/tauriCommands';
import { useInlineAutocomplete } from '../useInlineAutocomplete';

vi.mock('../../../../utils/tauriCommands', () => ({
  isTauri: vi.fn(() => true),
  auraAutocompleteCurrent: vi.fn(),
}));

const DEBOUNCE_MS = 320;

function renderAutocomplete(initial: {
  activeThreadId?: string | null;
  inputValue?: string;
  inputMode?: 'text' | 'voice';
  rustChat?: boolean;
}) {
  return renderHook(
    p =>
      useInlineAutocomplete({
        activeThreadId: p.activeThreadId ?? null,
        inputValue: p.inputValue ?? '',
        inputMode: p.inputMode ?? 'text',
        rustChat: p.rustChat ?? true,
      }),
    { initialProps: initial }
  );
}

describe('useInlineAutocomplete', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(auraAutocompleteCurrent).mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not fetch when not running under Tauri', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    renderAutocomplete({ inputValue: 'hello there' });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });

    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();
  });

  it('does not fetch when rustChat is false', async () => {
    renderAutocomplete({ inputValue: 'hello there', rustChat: false });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });

    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();
  });

  it('does not fetch outside text input mode', async () => {
    renderAutocomplete({ inputValue: 'hello there', inputMode: 'voice' });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });

    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();
  });

  it('does not fetch while a thread turn is active', async () => {
    renderAutocomplete({ inputValue: 'hello there', activeThreadId: 'thread-1' });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });

    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();
  });

  it('does not fetch below the minimum context length and clears any suggestion', async () => {
    vi.mocked(auraAutocompleteCurrent).mockResolvedValue({
      result: { suggestion: { value: 'stale' } },
    } as any);
    const { result, rerender } = renderAutocomplete({ inputValue: 'he' });
    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });
    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();

    rerender({ inputValue: 'h' });
    expect(result.current.inlineSuggestionValue).toBe('');
  });

  it('debounces rapid keystrokes into a single fetch after settling', async () => {
    vi.mocked(auraAutocompleteCurrent).mockResolvedValue({
      result: { suggestion: { value: 'suggestion' } },
    } as any);
    const { rerender } = renderAutocomplete({ inputValue: 'hel' });

    rerender({ inputValue: 'hell' });
    act(() => {
      vi.advanceTimersByTime(100);
    });
    rerender({ inputValue: 'hello' });
    act(() => {
      vi.advanceTimersByTime(100);
    });
    rerender({ inputValue: 'hello there' });

    expect(auraAutocompleteCurrent).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });

    expect(auraAutocompleteCurrent).toHaveBeenCalledTimes(1);
    expect(auraAutocompleteCurrent).toHaveBeenCalledWith({ context: 'hello there' });
  });

  it('sets the suggestion from a successful response', async () => {
    vi.mocked(auraAutocompleteCurrent).mockResolvedValue({
      result: { suggestion: { value: ' world' } },
    } as any);
    const { result } = renderAutocomplete({ inputValue: 'hello' });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
      await Promise.resolve();
    });

    expect(result.current.inlineSuggestionValue).toBe(' world');
  });

  it('clears the suggestion when the fetch rejects', async () => {
    vi.mocked(auraAutocompleteCurrent).mockRejectedValue(new Error('boom'));
    const { result } = renderAutocomplete({ inputValue: 'hello' });

    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
      await Promise.resolve();
    });

    expect(result.current.inlineSuggestionValue).toBe('');
  });

  it('ignores a stale response that resolves after a newer request has superseded it', async () => {
    let resolveFirst!: (value: unknown) => void;
    vi.mocked(auraAutocompleteCurrent)
      .mockImplementationOnce(
        () =>
          new Promise(resolve => {
            resolveFirst = resolve as (value: unknown) => void;
          })
      )
      .mockResolvedValueOnce({ result: { suggestion: { value: 'second' } } } as any);

    const { result, rerender } = renderAutocomplete({ inputValue: 'hello' });
    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
    });
    expect(auraAutocompleteCurrent).toHaveBeenCalledTimes(1);

    // Second keystroke fires a second debounced request before the first resolves.
    rerender({ inputValue: 'hello!' });
    await act(async () => {
      vi.advanceTimersByTime(DEBOUNCE_MS);
      await Promise.resolve();
    });
    expect(auraAutocompleteCurrent).toHaveBeenCalledTimes(2);
    expect(result.current.inlineSuggestionValue).toBe('second');

    // The stale first request now resolves — must not clobber the second's result.
    await act(async () => {
      resolveFirst({ result: { suggestion: { value: 'first-stale' } } });
      await Promise.resolve();
    });
    expect(result.current.inlineSuggestionValue).toBe('second');
  });

  it('setInlineSuggestionValue lets the caller clear the suggestion synchronously (e.g. on accept)', () => {
    const { result } = renderAutocomplete({ inputValue: 'hello' });

    act(() => {
      result.current.setInlineSuggestionValue('');
    });

    expect(result.current.inlineSuggestionValue).toBe('');
  });
});
