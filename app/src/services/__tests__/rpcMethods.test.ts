import * as fs from 'node:fs';
import * as path from 'node:path';
import { describe, expect, test } from 'vitest';

import { CORE_RPC_METHODS, LEGACY_METHOD_ALIASES, normalizeRpcMethod } from '../rpcMethods';

describe('rpcMethods catalog', () => {
  describe('normalizeRpcMethod', () => {
    test('resolves all legacy aliases to their canonical core method', () => {
      for (const [legacyMethod, coreMethod] of Object.entries(LEGACY_METHOD_ALIASES)) {
        expect(normalizeRpcMethod(legacyMethod)).toBe(coreMethod);
      }
    });

    test('transforms auth methods by replacing dots with underscores', () => {
      expect(normalizeRpcMethod('aura.auth.login')).toBe('aura.auth_login');
      expect(normalizeRpcMethod('aura.auth.get.state')).toBe('aura.auth_get_state');
      expect(normalizeRpcMethod('aura.auth.a.b.c')).toBe('aura.auth_a_b_c');
    });

    test('transforms accessibility prefix to screen_intelligence prefix', () => {
      expect(normalizeRpcMethod('aura.accessibility_status')).toBe(
        'aura.screen_intelligence_status'
      );
      expect(normalizeRpcMethod('aura.accessibility_enable')).toBe(
        'aura.screen_intelligence_enable'
      );
    });

    test('returns unmapped or unrecognized methods unchanged', () => {
      expect(normalizeRpcMethod('aura.threads_list')).toBe('aura.threads_list');
      expect(normalizeRpcMethod('aura.unknown_method')).toBe('aura.unknown_method');
      expect(normalizeRpcMethod('')).toBe('');
      expect(normalizeRpcMethod('random_string')).toBe('random_string');
    });

    test('trims whitespace and converts to lower case', () => {
      expect(normalizeRpcMethod('  Aura.Auth.Login  ')).toBe('aura.auth_login');
      expect(normalizeRpcMethod('  AURA.GET_CONFIG ')).toBe(CORE_RPC_METHODS.configGet);
      expect(normalizeRpcMethod('Aura.Accessibility_Status  ')).toBe(
        'aura.screen_intelligence_status'
      );
      expect(normalizeRpcMethod('   some_RANDOM_method  ')).toBe('some_random_method');
    });
  });

  test('legacy aliases point at canonical method values', () => {
    expect(LEGACY_METHOD_ALIASES['aura.update_model_settings']).toBe(
      CORE_RPC_METHODS.inferenceUpdateModelSettings
    );
    expect(LEGACY_METHOD_ALIASES['aura.workspace_onboarding_flag_set']).toBe(
      CORE_RPC_METHODS.configWorkspaceOnboardingFlagSet
    );
  });

  test('catalog canonical methods exist in core schema registry (drift guard)', () => {
    const schemaSources = [
      fs.readFileSync(path.resolve(__dirname, '../../../../src/aura/config/schemas.rs'), 'utf8'),
      fs.readFileSync(
        path.resolve(__dirname, '../../../../src/aura/screen_intelligence/schemas.rs'),
        'utf8'
      ),
      fs.readFileSync(
        path.resolve(__dirname, '../../../../src/aura/inference/provider/schemas.rs'),
        'utf8'
      ),
      fs.readFileSync(path.resolve(__dirname, '../../../../src/aura/inference/schemas.rs'), 'utf8'),
    ].join('\n');

    for (const method of Object.values(CORE_RPC_METHODS)) {
      // core.* methods (e.g. core.ping) are special dispatch methods, not in the schema catalog.
      if (!method.startsWith('aura.')) continue;
      const methodRoot = method.slice('aura.'.length);
      const namespace = methodRoot.startsWith('screen_intelligence_')
        ? 'screen_intelligence'
        : methodRoot.startsWith('inference_')
          ? 'inference'
          : methodRoot.startsWith('providers_')
            ? 'providers'
            : 'config';
      const fnName = methodRoot.slice(`${namespace}_`.length);
      expect(schemaSources).toContain(`namespace: "${namespace}"`);
      expect(schemaSources).toContain(`function: "${fnName}"`);
    }
  });
});
