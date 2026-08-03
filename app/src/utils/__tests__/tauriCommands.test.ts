import { invoke, isTauri } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, type Mock, test, vi } from 'vitest';

import { callCoreRpc } from '../../services/coreRpcClient';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

type TauriInternalsHolder = { __TAURI_INTERNALS__?: { invoke: unknown } };

describe('tauriCommands', () => {
  const mockIsTauri = isTauri as Mock;
  const mockInvoke = invoke as Mock;
  const mockCallCoreRpc = callCoreRpc as Mock;
  let getAuthState: typeof import('../tauriCommands').getAuthState;
  let resetAuraDataAndRestartCore: typeof import('../tauriCommands').resetAuraDataAndRestartCore;
  let storeSession: typeof import('../tauriCommands').storeSession;
  let auraLocalAiStatus: typeof import('../tauriCommands').auraLocalAiStatus;
  let auraServiceStatus: typeof import('../tauriCommands').auraServiceStatus;
  let prevInternals: TauriInternalsHolder['__TAURI_INTERNALS__'];

  beforeEach(async () => {
    vi.clearAllMocks();
    mockIsTauri.mockReturnValue(true);
    // The local `isTauri()` wrapper in `tauriCommands/common.ts` ALSO checks
    // `window.__TAURI_INTERNALS__.invoke` to detect the CEF bootstrap gap
    // (see AURA-REACT-S). Mocking only the upstream `coreIsTauri`
    // isn't enough — the wrapper would still return false in tests and
    // every helper would hit its `if (!isTauri()) return;` early-exit.
    // Stub a minimal internals shape so the wrapper resolves to true.
    const holder = window as unknown as TauriInternalsHolder;
    prevInternals = holder.__TAURI_INTERNALS__;
    holder.__TAURI_INTERNALS__ = { invoke: () => undefined };
    const actual = await vi.importActual<typeof import('../tauriCommands')>('../tauriCommands');
    getAuthState = actual.getAuthState;
    resetAuraDataAndRestartCore = actual.resetAuraDataAndRestartCore;
    storeSession = actual.storeSession;
    auraLocalAiStatus = actual.auraLocalAiStatus;
    auraServiceStatus = actual.auraServiceStatus;
  });

  afterEach(() => {
    const holder = window as unknown as TauriInternalsHolder;
    if (prevInternals === undefined) {
      delete holder.__TAURI_INTERNALS__;
    } else {
      holder.__TAURI_INTERNALS__ = prevInternals;
    }
  });

  test('getAuthState maps result shape from core response', async () => {
    mockCallCoreRpc.mockResolvedValueOnce({
      result: { isAuthenticated: true, user: { id: 'u1' } },
    });

    const response = await getAuthState();

    expect(mockCallCoreRpc).toHaveBeenCalledWith({ method: 'aura.auth_get_state' });
    expect(response).toEqual({ is_authenticated: true, user: { id: 'u1' } });
  });

  test('storeSession calls expected RPC method and params', async () => {
    await storeSession('jwt-token', { id: 'u1' });

    expect(mockCallCoreRpc).toHaveBeenCalledWith({
      method: 'aura.auth_store_session',
      params: { token: 'jwt-token', user: { id: 'u1' } },
    });
  });

  test('resetAuraDataAndRestartCore invokes the destructive Tauri command', async () => {
    await resetAuraDataAndRestartCore();

    // The helper used to call `aura.config_reset_local_data` over
    // JSON-RPC followed by `restart_core_process`, but the in-process
    // remove failed on Windows when the running core held open handles
    // inside the data directory (AURA-TAURI-AF). The Tauri shell
    // now owns the full sequence (stop core → remove paths → restart
    // core) behind a single `reset_local_data` command, so no core RPC
    // call should reach `callCoreRpc` from this helper.
    expect(mockCallCoreRpc).not.toHaveBeenCalled();
    expect(mockInvoke).toHaveBeenCalledWith('reset_local_data');
  });

  test('resetAuraDataAndRestartCore surfaces invoke failures to the caller', async () => {
    // Callers (e.g. `clearAllAppData`) treat a thrown error as unrecoverable
    // and abort the flow — so the helper must rethrow instead of swallowing
    // a `reset_local_data` failure (e.g. Windows `ERROR_SHARING_VIOLATION`
    // when a handle outside the embedded core still holds a path).
    const boom = new Error('reset_local_data failed');
    mockInvoke.mockRejectedValueOnce(boom);
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await expect(resetAuraDataAndRestartCore()).rejects.toBe(boom);
    expect(consoleErrorSpy).toHaveBeenCalled();

    consoleErrorSpy.mockRestore();
  });

  test('auraLocalAiStatus returns upgrade hint on unknown method', async () => {
    mockCallCoreRpc.mockRejectedValueOnce(new Error('unknown method: aura.inference_status'));

    await expect(auraLocalAiStatus()).rejects.toThrow(
      'Local model runtime is unavailable in this core build. Restart app after updating to the latest build.'
    );
  });

  test('auraServiceStatus throws when not running in Tauri', async () => {
    mockIsTauri.mockReturnValue(false);

    await expect(auraServiceStatus()).rejects.toThrow('Not running in Tauri');
    expect(mockCallCoreRpc).not.toHaveBeenCalled();
  });
});
