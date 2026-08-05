import { getCoreStateSnapshot } from '../lib/coreState/store';
import type { RootState } from './index';

const PENDING_USER = '__pending__';

/**
 * Derive the socket user ID — must match the key used by socketService.ts
 * when writing to byUser[] (`snapshot.auth.userId`, same fallback sentinel).
 *
 * Previously decoded the session token as a JWT (splitting on `.` and
 * base64-decoding the payload) to pull out a user id claim. That only works
 * for real backend-issued JWTs — the `local` runtime mode's device-local
 * session token (`local-<uuid>`, no dots) always failed to parse, silently
 * falling back to `PENDING_USER` forever. Since `state.socket.byUser` is
 * never populated under that key (socketService correctly uses
 * `auth.userId`), every read through this selector reported `'disconnected'`
 * regardless of actual connection state — blocking chat sends with no
 * visible error (`composer_blocked`, which shows nothing) or the unhelpful
 * "socket not connected" message, depending on which guard tripped first.
 */
function selectSocketUserId(_state: RootState): string {
  return getCoreStateSnapshot().snapshot?.auth?.userId ?? PENDING_USER;
}

export const selectSocketStatus = (state: RootState) => {
  const userId = selectSocketUserId(state);
  const userState = state.socket.byUser[userId];
  return userState?.status ?? 'disconnected';
};

export const selectSocketId = (state: RootState): string | null => {
  const userId = selectSocketUserId(state);
  const userState = state.socket.byUser[userId];
  return userState?.socketId ?? null;
};
