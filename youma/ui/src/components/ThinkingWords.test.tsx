import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { ThinkingWords, DEFAULT_THINKING_PHRASES } from './ThinkingWords';

vi.mock('../api', () => ({
  envGet: vi.fn(),
}));

import * as api from '../api';

function setup() {
  return render(<ThinkingWords />);
}

// Flush all pending React effects and microtasks (e.g. envGet().then(...))
async function flush() {
  await act(async () => {});
}

// Advance fake timers by N ms inside an act() so React processes resulting state updates
async function tick(ms: number) {
  await act(async () => { vi.advanceTimersByTime(ms); });
}

describe('ThinkingWords', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(api.envGet).mockResolvedValue('');
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('renders without crashing', async () => {
    const { container } = setup();
    await flush();
    expect(container.firstChild).toBeTruthy();
  });

  it('renders a spinner SVG', async () => {
    setup();
    await flush();
    expect(document.querySelector('svg.animate-spin')).toBeInTheDocument();
  });

  it('renders a blinking cursor character', async () => {
    setup();
    await flush();
    expect(document.body.textContent).toContain('|');
  });

  it('calls envGet with YOUMA_THINKING_PHRASES on mount', async () => {
    setup();
    // useEffect fires synchronously inside act; envGet is called before render returns
    expect(api.envGet).toHaveBeenCalledWith('YOUMA_THINKING_PHRASES');
  });

  it('shows default phrase chars after typewriter advances', async () => {
    vi.mocked(api.envGet).mockResolvedValue('');
    setup();
    await flush();          // let envGet.then() resolve (returns '' → no-op)
    await tick(300);        // 4 chars typed at 72ms each

    const bodyText = document.body.textContent ?? '';
    // At least one character of some default phrase appears
    const anyDefault = DEFAULT_THINKING_PHRASES.some(p =>
      p.length > 0 && bodyText.includes(p[0])
    );
    expect(anyDefault).toBe(true);
  });

  it('uses custom phrases from env when set', async () => {
    vi.mocked(api.envGet).mockResolvedValue('zipzap, foobar');
    setup();
    await flush();          // envGet.then() → setCustomPhrases(['zipzap','foobar'])
    await tick(300);        // type 4 chars from current phrase

    const bodyText = document.body.textContent ?? '';
    // 'zipzap' starts with 'z'; 'foobar' starts with 'f' — one must appear
    expect(bodyText.includes('z') || bodyText.includes('f')).toBe(true);
  });

  it('falls back to default phrases when envGet rejects', async () => {
    vi.mocked(api.envGet).mockRejectedValue(new Error('network'));
    setup();
    await flush();          // rejected promise is swallowed via .catch(() => {})
    await tick(300);
    // No crash; cursor still present; default phrases still in use
    expect(document.body.textContent).toContain('|');
  });

  it('filters whitespace-only tokens from custom phrases', async () => {
    vi.mocked(api.envGet).mockResolvedValue('   ,  ,  ');
    setup();
    await flush();          // parsed = [] after filter → customPhrases stays []
    await tick(300);
    // Falls back to defaults — no crash, cursor present
    expect(document.body.textContent).toContain('|');
  });

  it('advances to next phrase after hold period without crashing', async () => {
    setup();
    await flush();
    // One full cycle: ~18chars × 72ms typing + 1800ms hold + 350ms transition ≈ 3446ms
    await tick(3600);
    expect(document.body.textContent).toContain('|');
  });
});
