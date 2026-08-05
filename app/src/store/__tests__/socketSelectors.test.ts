import { beforeEach, describe, expect, it } from 'vitest';

import { type CoreState, setCoreStateSnapshot } from '../../lib/coreState/store';
import type { RootState } from '../index';
import { selectSocketId, selectSocketStatus } from '../socketSelectors';

function makeCoreState(userId: string | null): CoreState {
  return {
    isBootstrapping: false,
    isReady: true,
    snapshot: {
      auth: { isAuthenticated: !!userId, userId, user: null, profileId: null },
      sessionToken: userId ? `token-for-${userId}` : null,
      currentUser: null,
      onboardingCompleted: false,
      chatOnboardingCompleted: false,
      analyticsEnabled: false,
      meetAutoOrchestratorHandoff: false,
      localState: { encryptionKey: null, onboardingTasks: null },
      runtime: { screenIntelligence: null, localAi: null, autocomplete: null, service: null },
    },
    teams: [],
    teamMembersById: {},
    teamInvitesById: {},
  };
}

function makeState(
  byUser: Record<string, { status: string; socketId: string | null }> = {}
): RootState {
  return { socket: { byUser } } as RootState;
}

describe('selectSocketStatus', () => {
  beforeEach(() => {
    setCoreStateSnapshot(makeCoreState(null));
  });

  it('returns disconnected when no userId', () => {
    const state = makeState();
    expect(selectSocketStatus(state)).toBe('disconnected');
  });

  it('returns status from user state keyed by auth.userId', () => {
    setCoreStateSnapshot(makeCoreState('tg123'));
    const state = makeState({ tg123: { status: 'connected', socketId: 'sock-1' } });

    expect(selectSocketStatus(state)).toBe('connected');
  });

  it('returns disconnected when the user has no socket state yet', () => {
    setCoreStateSnapshot(makeCoreState('tg123'));
    const state = makeState();

    expect(selectSocketStatus(state)).toBe('disconnected');
  });

  it('uses __pending__ when there is no userId at all', () => {
    setCoreStateSnapshot(makeCoreState(null));
    const state = makeState({ __pending__: { status: 'connecting', socketId: null } });

    expect(selectSocketStatus(state)).toBe('connecting');
  });

  // Regression test: the `local` runtime mode's device-local session
  // (`create_local_session`) sets `auth.userId` to a plain string like
  // `local-device` — not a JWT. The selector previously tried to decode
  // `sessionToken` as a JWT to recover a user id, which always failed for
  // this non-JWT token and silently fell back to `__pending__`, so
  // `state.socket.byUser['local-device']` (where the socket connection is
  // actually keyed, via the same `auth.userId` field) was never found —
  // `selectSocketStatus` reported 'disconnected' forever regardless of the
  // real connection state, permanently blocking chat sends with no visible
  // error. `auth.userId` is used directly now, matching socketService.ts.
  it('resolves status correctly for a device-local (non-JWT) session', () => {
    setCoreStateSnapshot(makeCoreState('local-device'));
    const state = makeState({ 'local-device': { status: 'connected', socketId: 'sock-local' } });

    expect(selectSocketStatus(state)).toBe('connected');
  });
});

describe('selectSocketId', () => {
  beforeEach(() => {
    setCoreStateSnapshot(makeCoreState(null));
  });

  it('returns null when no userId', () => {
    const state = makeState();
    expect(selectSocketId(state)).toBeNull();
  });

  it('returns socketId from user state', () => {
    setCoreStateSnapshot(makeCoreState('tg123'));
    const state = makeState({ tg123: { status: 'connected', socketId: 'sock-abc' } });

    expect(selectSocketId(state)).toBe('sock-abc');
  });
});
