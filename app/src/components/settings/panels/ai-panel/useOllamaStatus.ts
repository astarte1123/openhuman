import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  loadLocalProviderSnapshot,
  type LocalProviderSnapshot,
} from '../../../../services/api/aiSettingsApi';
import type { OllamaState } from './types';

export function useOllamaStatus() {
  const [snapshot, setSnapshot] = useState<LocalProviderSnapshot | null>(null);
  const lastPollRef = useRef<number>(0);

  const refresh = useCallback(async (): Promise<LocalProviderSnapshot | null> => {
    try {
      const s = await loadLocalProviderSnapshot();
      setSnapshot(s);
      lastPollRef.current = Date.now();
      return s;
    } catch {
      // Swallow — keep last good snapshot, return null so callers can
      // detect failure without a try/catch.
      return null;
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(id);
  }, [refresh]);

  // Translate to the OllamaState the panel UI expects.
  //
  // `disabled` is the config-side master switch (user turned local AI off
  // via the toggle). `missing` is "user wants local AI but the daemon
  // isn't installed". Keep them distinct so the toggle's `checked` state
  // and the Install/Retry button can render the right thing.
  const state: OllamaState = useMemo(() => {
    if (!snapshot) return 'stopped';
    const stateStr = snapshot.status?.state ?? '';
    if (stateStr === 'disabled') return 'disabled';
    if (snapshot.diagnostics?.ollama_running) return 'running';
    if (stateStr === 'missing') return 'missing';
    if (stateStr === 'starting' || stateStr === 'downloading') return 'starting';
    if (stateStr === 'error') return 'error';
    return 'stopped';
  }, [snapshot]);

  const version = snapshot?.diagnostics?.ollama_binary_path
    ? // Diagnostics doesn't surface a version string today; show the binary path tail.
      (snapshot.diagnostics.ollama_binary_path.split(/[\\/]/).pop() ?? '')
    : '';

  return { state, version, snapshot, refresh };
}
