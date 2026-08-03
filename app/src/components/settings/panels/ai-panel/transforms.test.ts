import { describe, expect, it } from 'vitest';

import { authStyleForSlug, toApiSettings, toPanelProvider } from './transforms';
import type { AISettings, CloudProvider } from './types';

describe('transforms — youworker field round-trip', () => {
  it('authStyleForSlug maps the youworker slug to the youworker auth style', () => {
    expect(authStyleForSlug('youworker')).toBe('youworker');
  });

  it('toPanelProvider lifts the youworker-only fields from the API view', () => {
    const panel = toPanelProvider({
      id: 'p_yw_1',
      slug: 'youworker',
      label: 'YouWorker',
      endpoint: 'https://dev.youworker.ai/api/v1',
      auth_style: 'youworker',
      has_api_key: true,
      autentika_url: 'https://example.autentika.test',
      persona: 'test-persona',
      client_id: '00000000-0000-0000-0000-000000000001',
      subject_uuid: '00000000-0000-0000-0000-000000000002',
    });

    expect(panel).toMatchObject({
      slug: 'youworker',
      authStyle: 'youworker',
      autentikaUrl: 'https://example.autentika.test',
      persona: 'test-persona',
      clientId: '00000000-0000-0000-0000-000000000001',
      subjectUuid: '00000000-0000-0000-0000-000000000002',
    });
  });

  it('toPanelProvider leaves the youworker fields undefined for non-youworker providers', () => {
    const panel = toPanelProvider({
      id: 'p_openai_1',
      slug: 'openai',
      label: 'OpenAI',
      endpoint: 'https://api.openai.com/v1',
      auth_style: 'bearer',
      has_api_key: true,
    });

    expect(panel.autentikaUrl).toBeUndefined();
    expect(panel.persona).toBeUndefined();
    expect(panel.clientId).toBeUndefined();
    expect(panel.subjectUuid).toBeUndefined();
  });

  it('toApiSettings serialises the youworker fields back onto the wire shape', () => {
    const youworkerProvider: CloudProvider = {
      id: 'p_yw_1',
      slug: 'youworker',
      label: 'YouWorker',
      endpoint: 'https://dev.youworker.ai/api/v1',
      authStyle: 'youworker',
      maskedKey: '•••• configured',
      autentikaUrl: 'https://example.autentika.test',
      persona: 'test-persona',
      clientId: '00000000-0000-0000-0000-000000000001',
      subjectUuid: '00000000-0000-0000-0000-000000000002',
    };
    const panel: AISettings = {
      cloudProviders: [youworkerProvider],
      routing: {
        reasoning: { kind: 'aura' },
        agentic: { kind: 'aura' },
        coding: { kind: 'aura' },
        memory: { kind: 'aura' },
        embeddings: { kind: 'aura' },
        heartbeat: { kind: 'aura' },
        learning: { kind: 'aura' },
        subconscious: { kind: 'aura' },
      },
    };

    const api = toApiSettings(panel);

    expect(api.cloudProviders[0]).toMatchObject({
      slug: 'youworker',
      auth_style: 'youworker',
      autentika_url: 'https://example.autentika.test',
      persona: 'test-persona',
      client_id: '00000000-0000-0000-0000-000000000001',
      subject_uuid: '00000000-0000-0000-0000-000000000002',
    });
  });

  it('toApiSettings sends null (not undefined) for unset youworker fields on other providers', () => {
    const panel: AISettings = {
      cloudProviders: [
        {
          id: 'p_openai_1',
          slug: 'openai',
          label: 'OpenAI',
          endpoint: 'https://api.openai.com/v1',
          authStyle: 'bearer',
          maskedKey: '•••• configured',
        },
      ],
      routing: {
        reasoning: { kind: 'aura' },
        agentic: { kind: 'aura' },
        coding: { kind: 'aura' },
        memory: { kind: 'aura' },
        embeddings: { kind: 'aura' },
        heartbeat: { kind: 'aura' },
        learning: { kind: 'aura' },
        subconscious: { kind: 'aura' },
      },
    };

    const api = toApiSettings(panel);
    expect(api.cloudProviders[0].autentika_url).toBeNull();
    expect(api.cloudProviders[0].persona).toBeNull();
    expect(api.cloudProviders[0].client_id).toBeNull();
    expect(api.cloudProviders[0].subject_uuid).toBeNull();
  });
});
