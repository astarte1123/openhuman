import type { AuthStyle } from '../../../../utils/tauriCommands/config';

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export type CloudProvider = {
  id: string;
  slug: string;
  label: string;
  endpoint: string;
  authStyle: AuthStyle;
  maskedKey: string;
  // ── youworker.ai (authStyle: 'youworker' only) ───────────────────────────
  autentikaUrl?: string;
  persona?: string;
  clientId?: string;
  subjectUuid?: string;
};

export type OllamaState = 'disabled' | 'missing' | 'stopped' | 'starting' | 'running' | 'error';

export type OllamaModel = { id: string; sizeBytes: number; family: string };

export type WorkloadId =
  | 'reasoning'
  | 'agentic'
  | 'coding'
  | 'memory'
  | 'embeddings'
  | 'heartbeat'
  | 'learning'
  | 'subconscious';

export type WorkloadGroup = 'chat' | 'background';

export type ProviderRef =
  | { kind: 'openhuman' }
  | { kind: 'cloud'; providerSlug: string; model: string }
  | { kind: 'local'; model: string };

export type Workload = { id: WorkloadId; group: WorkloadGroup; label: string; description: string };

export type RoutingMap = Record<WorkloadId, ProviderRef>;

// The panel works in terms of `CloudProvider` (slug + maskedKey) and
// `ProviderRef` (slug-keyed). The wire format is identical — the API-adapter
// layer (see transforms.ts) just derives the `maskedKey` display string from
// `has_api_key`.
export type AISettings = { cloudProviders: CloudProvider[]; routing: RoutingMap };
