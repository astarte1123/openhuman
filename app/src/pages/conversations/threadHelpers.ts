export function isComposerInteractionBlocked(args: {
  activeThreadId: string | null;
  welcomePending: boolean;
  rustChat: boolean;
}): boolean {
  return !args.rustChat || Boolean(args.activeThreadId) || args.welcomePending;
}

interface ImeKeyboardEventLike {
  isComposing?: boolean;
  keyCode?: number;
  which?: number;
  nativeEvent?: { isComposing?: boolean; keyCode?: number; which?: number };
}

export function isImeCompositionKeyEvent(event: ImeKeyboardEventLike): boolean {
  return (
    event.isComposing === true ||
    event.nativeEvent?.isComposing === true ||
    event.nativeEvent?.keyCode === 229 ||
    event.nativeEvent?.which === 229 ||
    event.keyCode === 229 ||
    event.which === 229
  );
}

/**
 * Normalise the value thrown out of `dispatch(loadThreads()).unwrap()` into a
 * displayable string. `createAsyncThunk` re-throws Redux's `SerializedError`
 * (a plain object, not an `Error` instance) when the thunk rejects — which is
 * why the original Sentry report (OPENHUMAN-REACT-X) showed up as
 * "Non-Error promise rejection captured with value: …" rather than a stack.
 * Exported so the mount-effect's `.catch` stays a one-liner and the message
 * shape can be unit-tested without mounting the full page.
 */
export function formatThreadLoadError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (err && typeof err === 'object' && 'message' in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return String(err);
}

export function formatAgentProfileAgentLabel(agentId: string): string {
  return agentId
    .split(/[_-]+/)
    .filter(Boolean)
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}
