import { isTauri } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, type Mock, test, vi } from 'vitest';

import { callCoreRpc } from '../../services/coreRpcClient';
import type { ServiceState } from '../tauriCommands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

describe('Web → frontend JSON-RPC → Core bridge', () => {
  const mockIsTauri = isTauri as Mock;
  const mockCallCoreRpc = callCoreRpc as Mock;
  let auraServiceStatus: typeof import('../tauriCommands').auraServiceStatus;
  let auraAgentServerStatus: typeof import('../tauriCommands').auraAgentServerStatus;

  beforeEach(async () => {
    vi.clearAllMocks();
    mockIsTauri.mockReturnValue(true);
    const actual = await vi.importActual<typeof import('../tauriCommands')>('../tauriCommands');
    auraServiceStatus = actual.auraServiceStatus;
    auraAgentServerStatus = actual.auraAgentServerStatus;
  });

  test('routes service status via JSON-RPC client and returns core payload', async () => {
    const expectedState: ServiceState = 'Running';
    const rpcResponse = { result: { state: expectedState }, logs: ['service status fetched'] };

    mockCallCoreRpc.mockResolvedValueOnce(rpcResponse);

    const response = await auraServiceStatus();

    expect(mockCallCoreRpc).toHaveBeenCalledWith({ method: 'aura.service_status' });
    expect(response).toEqual(rpcResponse);
    expect(response.result.state).toBe(expectedState);
  });

  test('routes agent server status via JSON-RPC client', async () => {
    const rpcResponse = {
      result: { running: true, url: 'http://127.0.0.1:8421/rpc' },
      logs: ['agent server status checked'],
    };

    mockCallCoreRpc.mockResolvedValueOnce(rpcResponse);

    const response = await auraAgentServerStatus();

    expect(mockCallCoreRpc).toHaveBeenCalledWith({ method: 'aura.agent_server_status' });
    expect(response.result.running).toBe(true);
    expect(response.result.url).toContain('127.0.0.1');
  });

  test('fails fast in web-only mode without Tauri runtime', async () => {
    mockIsTauri.mockReturnValue(false);

    await expect(auraServiceStatus()).rejects.toThrow('Not running in Tauri');
    expect(mockCallCoreRpc).not.toHaveBeenCalled();
  });
});
