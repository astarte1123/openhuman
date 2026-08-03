import { useCallback, useEffect, useState } from 'react';

import { loadAISettings, saveAISettings } from '../../../../services/api/aiSettingsApi';
import { EMPTY_SETTINGS } from './catalog';
import { toApiSettings, toPanelRoutingFromApi } from './transforms';
import type { AISettings } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// API-adapter hooks
//
// The panel works in terms of `CloudProvider` (slug + maskedKey) and
// `ProviderRef` (slug-keyed). The wire format is identical — this layer
// just derives the `maskedKey` display string from `has_api_key`.
// ─────────────────────────────────────────────────────────────────────────────

export function useAISettings() {
  const [saved, setSaved] = useState<AISettings>(EMPTY_SETTINGS);
  const [draft, setDraft] = useState<AISettings>(EMPTY_SETTINGS);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string>('');

  const reload = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const api = await loadAISettings();
      const { panel } = toPanelRoutingFromApi(api);
      setSaved(panel);
      setDraft(panel);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load AI settings';
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const isDirty = JSON.stringify(saved) !== JSON.stringify(draft);

  const save = useCallback(async () => {
    try {
      const prevApi = toApiSettings(saved);
      const nextApi = toApiSettings(draft);
      await saveAISettings(prevApi, nextApi);
      setSaved(draft);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save AI settings';
      setError(message);
    }
  }, [saved, draft]);

  const discard = useCallback(() => setDraft(saved), [saved]);

  return { saved, draft, setDraft, isDirty, save, discard, loading, error, reload };
}
