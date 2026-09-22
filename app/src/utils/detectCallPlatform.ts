// Purely cosmetic, client-side call-platform detection.
//
// Mirrors the *host* half of `detect_platform` in `src/aura/meet/ops.rs` —
// deliberately not the path-shape validation half (`is_meet_code`,
// `is_teams_meetup_join_path`, `is_zoom_join_path`). That keeps this
// utility a single, easy-to-audit host lookup instead of a second copy of
// backend validation logic that could silently drift out of sync.
//
// This is non-authoritative: it only drives a best-effort "Detected: X"
// badge before submit. The core RPC (`aura.meet_join_call` ->
// `validate_meet_url`) remains the sole source of truth for accept/reject,
// and an unmatched or malformed URL simply shows no badge rather than an
// error.
export type CallPlatform = 'google-meet' | 'microsoft-teams' | 'zoom';

const PLATFORM_BY_HOST: Record<string, CallPlatform> = {
  'meet.google.com': 'google-meet',
  'teams.microsoft.com': 'microsoft-teams',
  'teams.live.com': 'microsoft-teams',
  'zoom.us': 'zoom',
  'app.zoom.us': 'zoom',
  'zoomgov.com': 'zoom',
};

/**
 * Detects the call platform from a URL's hostname only. Returns `null` for
 * an empty/unparseable input or a host outside the known set — never
 * throws, so it's safe to call on every keystroke of a partially-typed URL.
 */
export function detectCallPlatform(rawUrl: string): CallPlatform | null {
  const trimmed = rawUrl.trim();
  if (!trimmed) return null;

  let host: string;
  try {
    host = new URL(trimmed).hostname.toLowerCase();
  } catch {
    return null;
  }

  return PLATFORM_BY_HOST[host] ?? null;
}

/** Display name for a detected platform. Brand names — not translated. */
export function callPlatformDisplayName(platform: CallPlatform): string {
  switch (platform) {
    case 'google-meet':
      return 'Google Meet';
    case 'microsoft-teams':
      return 'Microsoft Teams';
    case 'zoom':
      return 'Zoom';
  }
}
