import type { AISettings, RoutingMap, Workload } from './types';

// ─────────────────────────────────────────────────────────────────────────────
// Static catalog
// ─────────────────────────────────────────────────────────────────────────────

// Slug-keyed display metadata for built-in provider slugs. Used only for
// chip rendering (label, tone). Custom providers use `provider.label` directly.
export const BUILTIN_PROVIDER_META: Record<string, { tone: string; label: string }> = {
  aura: { label: 'Aura', tone: 'bg-primary-50 ring-primary-200 text-primary-900' },
  openai: { label: 'OpenAI', tone: 'bg-emerald-50 ring-emerald-200 text-emerald-900' },
  anthropic: { label: 'Anthropic', tone: 'bg-orange-50 ring-orange-200 text-orange-900' },
  openrouter: { label: 'OpenRouter', tone: 'bg-slate-100 ring-slate-300 text-slate-900' },
  custom: { label: 'Custom', tone: 'bg-stone-100 ring-stone-300 text-stone-900' },
  youworker: { label: 'YouWorker', tone: 'bg-sky-50 ring-sky-200 text-sky-900' },
};

// Starting-point model ids per built-in cloud provider slug, offered as
// quick-pick suggestions in the Custom-routing dialog so a user doesn't
// need to already know a valid model id to fill the (required) field.
// First entry doubles as the auto-filled default when the provider is
// selected. Freely overridable — these are suggestions, not a whitelist.
export const KNOWN_MODEL_SUGGESTIONS: Record<string, string[]> = {
  openai: ['gpt-4o-mini', 'gpt-4o', 'o3-mini'],
  anthropic: ['claude-3-5-sonnet-20241022', 'claude-3-5-haiku-20241022'],
  openrouter: ['openai/gpt-4o-mini', 'anthropic/claude-3.5-sonnet'],
};

export const WORKLOADS: Workload[] = [
  {
    id: 'reasoning',
    group: 'chat',
    label: 'Reasoning',
    description: 'Main chat agent, meeting summarizer',
  },
  {
    id: 'agentic',
    group: 'chat',
    label: 'Agentic',
    description: 'Sub-agent runners, tool loops, GIF decisions',
  },
  {
    id: 'coding',
    group: 'chat',
    label: 'Coding',
    description: 'Code generation and refactor passes',
  },
  {
    id: 'memory',
    group: 'background',
    label: 'Memory summarization',
    description: 'Tree-extracts and consolidations',
  },
  {
    id: 'embeddings',
    group: 'background',
    label: 'Embeddings',
    description: 'Vector encoding for memory retrieval',
  },
  {
    id: 'heartbeat',
    group: 'background',
    label: 'Heartbeat',
    description: 'Background reasoning between user turns',
  },
  {
    id: 'learning',
    group: 'background',
    label: 'Learning · Reflections',
    description: 'Periodic reflection over recent history',
  },
  {
    id: 'subconscious',
    group: 'background',
    label: 'Subconscious',
    description: 'Eventfulness scoring + drift checks',
  },
];

// TIER_PRESETS removed alongside the Local provider section.

export const EMPTY_ROUTING: RoutingMap = {
  reasoning: { kind: 'aura' },
  agentic: { kind: 'aura' },
  coding: { kind: 'aura' },
  memory: { kind: 'aura' },
  embeddings: { kind: 'aura' },
  heartbeat: { kind: 'aura' },
  learning: { kind: 'aura' },
  subconscious: { kind: 'aura' },
};

export const EMPTY_SETTINGS: AISettings = { cloudProviders: [], routing: EMPTY_ROUTING };

// ─────────────────────────────────────────────────────────────────────────────
// Cloud provider card — local-runtime chip metadata
// ─────────────────────────────────────────────────────────────────────────────

// Local-runtime chip slugs (Ollama / LM Studio) that aren't actual slugs in
// the cloud_providers list but need the same chip affordance.
type LocalChipSlug = 'lmstudio' | 'ollama';

// Tints per local-runtime chip slug.
export const LOCAL_CHIP_TONE: Record<LocalChipSlug, string> = {
  lmstudio: 'bg-cyan-50 ring-cyan-200 text-cyan-900',
  ollama: 'bg-violet-50 ring-violet-200 text-violet-900',
};

export const LOCAL_CHIP_LABEL: Record<LocalChipSlug, string> = {
  lmstudio: 'LM Studio',
  ollama: 'Ollama',
};
