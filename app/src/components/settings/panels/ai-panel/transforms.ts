import {
  type AISettings as ApiAISettings,
  type ProviderRef as ApiProviderRef,
  type CloudProviderView,
} from '../../../../services/api/aiSettingsApi';
import type { AuthStyle } from '../../../../utils/tauriCommands/config';
import { BUILTIN_PROVIDER_META } from './catalog';
import type { AISettings, CloudProvider, ProviderRef, RoutingMap } from './types';

export function maskKeyLabel(hasKey: boolean): string {
  return hasKey ? '•••• configured' : 'Not configured';
}

/**
 * Default auth style for a slug. Built-in slugs map to their known styles;
 * everything else (custom + third-party slugs the user types in) defaults
 * to bearer, matching the OpenAI-compatible majority.
 */
export function authStyleForSlug(slug: string): AuthStyle {
  if (slug === 'aura') return 'aura_jwt';
  if (slug === 'anthropic') return 'anthropic';
  if (slug === 'lmstudio' || slug === 'ollama') return 'none';
  if (slug === 'youworker') return 'youworker';
  return 'bearer';
}

export function toPanelProvider(p: CloudProviderView): CloudProvider {
  return {
    id: p.id,
    slug: p.slug,
    label: p.label,
    endpoint: p.endpoint,
    authStyle: p.auth_style,
    maskedKey: maskKeyLabel(p.has_api_key),
    autentikaUrl: p.autentika_url ?? undefined,
    persona: p.persona ?? undefined,
    clientId: p.client_id ?? undefined,
    subjectUuid: p.subject_uuid ?? undefined,
    youworkerTransport: (p.youworker_transport as 'webhook' | 'websocket' | null | undefined) ?? undefined,
  };
}

export function toPanelRoutingFromApi(api: ApiAISettings): { panel: AISettings } {
  const cloudProviders = api.cloudProviders.map(toPanelProvider);
  // ApiProviderRef and ProviderRef share the same shape — pass through directly.
  const liftRef = (r: ApiProviderRef): ProviderRef => r;
  const routing: RoutingMap = {
    reasoning: liftRef(api.routing.reasoning),
    agentic: liftRef(api.routing.agentic),
    coding: liftRef(api.routing.coding),
    memory: liftRef(api.routing.memory),
    embeddings: liftRef(api.routing.embeddings),
    heartbeat: liftRef(api.routing.heartbeat),
    learning: liftRef(api.routing.learning),
    subconscious: liftRef(api.routing.subconscious),
  };
  return { panel: { cloudProviders, routing } };
}

export function toApiSettings(panel: AISettings): ApiAISettings {
  return {
    cloudProviders: panel.cloudProviders.map(p => ({
      id: p.id,
      slug: p.slug,
      label: p.label,
      endpoint: p.endpoint,
      auth_style: p.authStyle,
      has_api_key: p.maskedKey.startsWith('••••'),
      autentika_url: p.autentikaUrl ?? null,
      persona: p.persona ?? null,
      client_id: p.clientId ?? null,
      subject_uuid: p.subjectUuid ?? null,
      youworker_transport: p.youworkerTransport ?? null,
    })),
    routing: {
      reasoning: panel.routing.reasoning,
      agentic: panel.routing.agentic,
      coding: panel.routing.coding,
      memory: panel.routing.memory,
      embeddings: panel.routing.embeddings,
      heartbeat: panel.routing.heartbeat,
      learning: panel.routing.learning,
      subconscious: panel.routing.subconscious,
    },
  };
}

export function slugTone(slug: string): string {
  return BUILTIN_PROVIDER_META[slug]?.tone ?? 'bg-stone-100 ring-stone-300 text-stone-900';
}

export function describeProvider(ref: ProviderRef, providers: CloudProvider[]): string {
  if (ref.kind === 'aura') return 'Youma';
  if (ref.kind === 'local') return `Local ${ref.model}`;
  const provider = providers.find(p => p.slug === ref.providerSlug);
  return `${provider?.label ?? ref.providerSlug} ${ref.model || 'custom model'}`;
}

export function defaultEndpointFor(slug: string): string {
  switch (slug) {
    case 'aura':
      return 'https://api.aura.ai/v1';
    case 'openai':
      return 'https://api.openai.com/v1';
    case 'anthropic':
      return 'https://api.anthropic.com/v1';
    case 'openrouter':
      return 'https://openrouter.ai/api/v1';
    default:
      return '';
  }
}
