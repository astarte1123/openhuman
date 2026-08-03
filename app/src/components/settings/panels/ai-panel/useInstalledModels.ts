import { useMemo } from 'react';

import type { LocalProviderSnapshot } from '../../../../services/api/aiSettingsApi';
import type { OllamaModel } from './types';

export function useInstalledModels(snapshot: LocalProviderSnapshot | null): OllamaModel[] {
  return useMemo(() => {
    const list = snapshot?.installedModels ?? [];
    return list.map(m => ({
      id: m.name,
      sizeBytes: m.size ?? 0,
      family: m.name.split(/[:/]/, 1)[0] ?? 'model',
    }));
  }, [snapshot]);
}
