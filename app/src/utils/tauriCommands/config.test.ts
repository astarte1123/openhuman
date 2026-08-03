import { isTauri } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, type Mock, test, vi } from 'vitest';

import { callCoreRpc } from '../../services/coreRpcClient';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

describe('tauriCommands/config', () => {
  const mockIsTauri = isTauri as Mock;
  const mockCallCoreRpc = callCoreRpc as Mock;
  let auraUpdateLocalAiSettings: typeof import('./config').auraUpdateLocalAiSettings;
  let auraUpdateMeetSettings: typeof import('./config').auraUpdateMeetSettings;
  let auraGetMeetSettings: typeof import('./config').auraGetMeetSettings;

  beforeEach(async () => {
    vi.clearAllMocks();
    mockIsTauri.mockReturnValue(true);
    const actual = await vi.importActual<typeof import('./config')>('./config');
    auraUpdateLocalAiSettings = actual.auraUpdateLocalAiSettings;
    auraUpdateMeetSettings = actual.auraUpdateMeetSettings;
    auraGetMeetSettings = actual.auraGetMeetSettings;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('auraUpdateLocalAiSettings', () => {
    test('throws when not running in Tauri', async () => {
      mockIsTauri.mockReturnValue(false);
      await expect(auraUpdateLocalAiSettings({ runtime_enabled: true })).rejects.toThrow(
        'Not running in Tauri'
      );
      expect(mockCallCoreRpc).not.toHaveBeenCalled();
    });

    test('forwards the patch to aura.inference_update_local_settings', async () => {
      mockCallCoreRpc.mockResolvedValue({
        result: { config: {}, workspace_dir: '/tmp', config_path: '/tmp/cfg.toml' },
        logs: [],
      });
      const patch = {
        runtime_enabled: true,
        opt_in_confirmed: true,
        provider: 'lm_studio',
        base_url: 'http://localhost:1234/v1',
        model_id: 'local-model',
        chat_model_id: 'local-model',
        usage_embeddings: true,
        usage_subconscious: false,
      };
      await auraUpdateLocalAiSettings(patch);
      expect(mockCallCoreRpc).toHaveBeenCalledWith({
        method: 'aura.inference_update_local_settings',
        params: patch,
      });
    });
  });

  describe('auraUpdateMeetSettings (#1299)', () => {
    test('throws when not running in Tauri', async () => {
      mockIsTauri.mockReturnValue(false);
      await expect(
        auraUpdateMeetSettings({ auto_orchestrator_handoff: true })
      ).rejects.toThrow('Not running in Tauri');
      expect(mockCallCoreRpc).not.toHaveBeenCalled();
    });

    test('forwards the patch to aura.config_update_meet_settings', async () => {
      mockCallCoreRpc.mockResolvedValue({
        result: { config: {}, workspace_dir: '/tmp', config_path: '/tmp/cfg.toml' },
        logs: [],
      });
      await auraUpdateMeetSettings({ auto_orchestrator_handoff: true });
      expect(mockCallCoreRpc).toHaveBeenCalledWith({
        method: 'aura.config_update_meet_settings',
        params: { auto_orchestrator_handoff: true },
      });
    });
  });

  describe('auraGetMeetSettings (#1299)', () => {
    test('throws when not running in Tauri', async () => {
      mockIsTauri.mockReturnValue(false);
      await expect(auraGetMeetSettings()).rejects.toThrow('Not running in Tauri');
      expect(mockCallCoreRpc).not.toHaveBeenCalled();
    });

    test('reads via aura.config_get_meet_settings', async () => {
      mockCallCoreRpc.mockResolvedValue({ result: { auto_orchestrator_handoff: true }, logs: [] });
      const out = await auraGetMeetSettings();
      expect(mockCallCoreRpc).toHaveBeenCalledWith({
        method: 'aura.config_get_meet_settings',
      });
      expect(out.result.auto_orchestrator_handoff).toBe(true);
    });
  });

  describe('auraUpdateComposioTriggerSettings', () => {
    let auraUpdateComposioTriggerSettings: typeof import('./config').auraUpdateComposioTriggerSettings;

    beforeEach(async () => {
      const actual = await vi.importActual<typeof import('./config')>('./config');
      auraUpdateComposioTriggerSettings = actual.auraUpdateComposioTriggerSettings;
    });

    test('throws when not running in Tauri', async () => {
      mockIsTauri.mockReturnValue(false);
      await expect(
        auraUpdateComposioTriggerSettings({ triage_disabled: true })
      ).rejects.toThrow('Not running in Tauri');
      expect(mockCallCoreRpc).not.toHaveBeenCalled();
    });

    test('forwards the patch to aura.config_update_composio_trigger_settings', async () => {
      mockCallCoreRpc.mockResolvedValue({
        result: { config: {}, workspace_dir: '/tmp', config_path: '/tmp/cfg.toml' },
        logs: [],
      });
      const patch = { triage_disabled: true, triage_disabled_toolkits: ['gmail', 'slack'] };
      await auraUpdateComposioTriggerSettings(patch);
      expect(mockCallCoreRpc).toHaveBeenCalledWith({
        method: 'aura.config_update_composio_trigger_settings',
        params: patch,
      });
    });

    test('returns no-op on unknown method from stale core (#1597)', async () => {
      mockCallCoreRpc.mockRejectedValue(
        new Error('unknown method: aura.config_update_composio_trigger_settings')
      );
      const out = await auraUpdateComposioTriggerSettings({ triage_disabled: true });
      expect(out).toEqual({ result: { config: {}, workspace_dir: '', config_path: '' }, logs: [] });
    });

    test('rethrows non-unknown-method errors', async () => {
      mockCallCoreRpc.mockRejectedValue(new Error('network timeout'));
      await expect(
        auraUpdateComposioTriggerSettings({ triage_disabled: true })
      ).rejects.toThrow('network timeout');
    });
  });

  describe('auraGetComposioTriggerSettings', () => {
    let auraGetComposioTriggerSettings: typeof import('./config').auraGetComposioTriggerSettings;

    beforeEach(async () => {
      const actual = await vi.importActual<typeof import('./config')>('./config');
      auraGetComposioTriggerSettings = actual.auraGetComposioTriggerSettings;
    });

    test('throws when not running in Tauri', async () => {
      mockIsTauri.mockReturnValue(false);
      await expect(auraGetComposioTriggerSettings()).rejects.toThrow('Not running in Tauri');
      expect(mockCallCoreRpc).not.toHaveBeenCalled();
    });

    test('reads via aura.config_get_composio_trigger_settings', async () => {
      mockCallCoreRpc.mockResolvedValue({
        result: { triage_disabled: false, triage_disabled_toolkits: ['slack'] },
        logs: [],
      });
      const out = await auraGetComposioTriggerSettings();
      expect(mockCallCoreRpc).toHaveBeenCalledWith({
        method: 'aura.config_get_composio_trigger_settings',
      });
      expect(out.result.triage_disabled).toBe(false);
      expect(out.result.triage_disabled_toolkits).toEqual(['slack']);
    });

    test('returns defaults on unknown method from stale core (#1597)', async () => {
      mockCallCoreRpc.mockRejectedValue(
        new Error('unknown method: aura.config_get_composio_trigger_settings')
      );
      const out = await auraGetComposioTriggerSettings();
      expect(out.result.triage_disabled).toBe(false);
      expect(out.result.triage_disabled_toolkits).toEqual([]);
    });

    test('rethrows non-unknown-method errors', async () => {
      mockCallCoreRpc.mockRejectedValue(new Error('network timeout'));
      await expect(auraGetComposioTriggerSettings()).rejects.toThrow('network timeout');
    });
  });
});
