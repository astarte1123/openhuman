import { describe, expect, it } from 'vitest';

import { callPlatformDisplayName, detectCallPlatform } from './detectCallPlatform';

describe('detectCallPlatform', () => {
  it('detects a Google Meet URL', () => {
    expect(detectCallPlatform('https://meet.google.com/abc-defg-hij')).toBe('google-meet');
  });

  it('detects Microsoft Teams URLs on both known hosts', () => {
    expect(
      detectCallPlatform('https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc%40thread.v2/0')
    ).toBe('microsoft-teams');
    expect(detectCallPlatform('https://teams.live.com/meet/123')).toBe('microsoft-teams');
  });

  it('detects Zoom URLs on all known hosts', () => {
    expect(detectCallPlatform('https://zoom.us/j/123456789')).toBe('zoom');
    expect(detectCallPlatform('https://app.zoom.us/j/123456789')).toBe('zoom');
    expect(detectCallPlatform('https://zoomgov.com/j/123456789')).toBe('zoom');
  });

  it('is case-insensitive on the hostname', () => {
    expect(detectCallPlatform('https://MEET.GOOGLE.COM/abc-defg-hij')).toBe('google-meet');
  });

  it('returns null for an unrelated host', () => {
    expect(detectCallPlatform('https://example.com/meeting')).toBeNull();
  });

  it('returns null for a lookalike host (no accidental substring match)', () => {
    expect(detectCallPlatform('https://zoom.us.evil.com/j/123456789')).toBeNull();
  });

  it('returns null for empty or whitespace-only input', () => {
    expect(detectCallPlatform('')).toBeNull();
    expect(detectCallPlatform('   ')).toBeNull();
  });

  it('returns null for unparseable input instead of throwing', () => {
    expect(detectCallPlatform('not a url')).toBeNull();
    expect(detectCallPlatform('meet.google.com/abc-defg-hij')).toBeNull();
  });
});

describe('callPlatformDisplayName', () => {
  it('returns the brand display name for each platform', () => {
    expect(callPlatformDisplayName('google-meet')).toBe('Google Meet');
    expect(callPlatformDisplayName('microsoft-teams')).toBe('Microsoft Teams');
    expect(callPlatformDisplayName('zoom')).toBe('Zoom');
  });
});
