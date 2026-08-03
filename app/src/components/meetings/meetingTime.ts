/**
 * Small time-formatting helpers shared by the meetings list, detail, and
 * transcript views. `duration_ms` / `start_ms` / `end_ms` are always
 * non-negative integer milliseconds coming from the core (see
 * `src/aura/meetings/types.rs`).
 */
export function formatClockMs(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}
