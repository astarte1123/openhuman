//! Pure helpers for the `meet` domain.
//!
//! Validation lives here so it can be unit-tested without standing up the
//! full RPC machinery.

/// A call platform the shell knows how to open a CEF window for and (for
/// some platforms) drive a CDP-based join automation against.
///
/// `GoogleMeet`, `MicrosoftTeams`, and `Zoom` are all recognised by
/// [`detect_platform`] and accepted by [`validate_meet_url`]. Other
/// providers remain unrecognised (`None`) until a later block adds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallPlatform {
    GoogleMeet,
    MicrosoftTeams,
    Zoom,
}

/// Detect which [`CallPlatform`] a call URL belongs to, purely from its
/// host. Returns `None` for any host we don't yet recognise.
pub fn detect_platform(url: &url::Url) -> Option<CallPlatform> {
    match url.host_str() {
        Some("meet.google.com") => Some(CallPlatform::GoogleMeet),
        Some("teams.microsoft.com") | Some("teams.live.com") => Some(CallPlatform::MicrosoftTeams),
        Some("zoom.us") | Some("app.zoom.us") | Some("zoomgov.com") => Some(CallPlatform::Zoom),
        _ => None,
    }
}

/// Validate that a string is a Google Meet, Microsoft Teams, or Zoom call
/// URL we're willing to hand to the embedded webview.
///
/// Google Meet — we accept:
///  - `https://meet.google.com/<code>` where `<code>` looks like a Meet
///    meeting code (three lowercase-letter groups separated by `-`).
///  - `https://meet.google.com/lookup/<id>` (Calendar deep links).
///
/// Microsoft Teams — we accept:
///  - `https://teams.microsoft.com/l/meetup-join/<...>` (work/school
///    tenant meeting links).
///  - `https://teams.live.com/meet/<id>` (personal Teams meeting links).
///
/// Zoom — we accept:
///  - `https://zoom.us/j/<id>`, `https://app.zoom.us/j/<id>`, or
///    `https://zoomgov.com/j/<id>` (classic join links).
///  - `https://zoom.us/wc/join/<id>`, `https://app.zoom.us/wc/join/<id>`,
///    or `https://zoomgov.com/wc/join/<id>` (explicit web-client join
///    links — `wc/join` is the "join from your browser" path Zoom's web
///    client itself deep-links to, as opposed to the native-app-launch
///    interstitial most `zoom.us/j/<id>` links land on first).
///
/// We reject any other host, scheme, or path shape to keep the surface
/// small — this RPC is *not* a generic "open any URL in CEF" entrypoint.
pub fn validate_meet_url(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw.trim()).map_err(|e| format!("invalid meet_url: {e}"))?;

    if url.scheme() != "https" {
        return Err(format!(
            "invalid meet_url: scheme `{}` not allowed (only https)",
            url.scheme()
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| "invalid meet_url: missing host".to_string())?;

    let path = url.path().trim_matches('/');
    let allowed_path = match host {
        "meet.google.com" => is_meet_code(path) || is_lookup_path(path),
        "teams.microsoft.com" => is_teams_meetup_join_path(path),
        "teams.live.com" => is_teams_live_meet_path(path),
        "zoom.us" | "app.zoom.us" | "zoomgov.com" => is_zoom_join_path(path),
        _ => {
            return Err(format!(
                "invalid meet_url: host `{host}` not allowed (only meet.google.com, \
                 teams.microsoft.com, teams.live.com, zoom.us, app.zoom.us, zoomgov.com)"
            ));
        }
    };
    if !allowed_path {
        return Err(format!(
            "invalid meet_url: path `/{path}` is not a recognised {host} meeting link"
        ));
    }

    Ok(url)
}

/// Accept `l/meetup-join/<...>` — Teams work/school tenant meeting links.
/// The segment after `meetup-join` is an opaque, tenant-specific thread
/// id/context blob we don't attempt to shape-validate beyond "non-empty",
/// mirroring how `is_lookup_path` treats Meet's `lookup/<id>` deep links.
fn is_teams_meetup_join_path(path: &str) -> bool {
    let mut parts = path.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("l"), Some("meetup-join"), Some(rest)) if !rest.is_empty()
    )
}

/// Accept `meet/<id>` — Teams personal (teams.live.com) meeting links.
fn is_teams_live_meet_path(path: &str) -> bool {
    let mut parts = path.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("meet"), Some(id), None) if !id.is_empty()
    )
}

/// Accept `j/<id>` (Zoom's classic join link) or `wc/join/<id>` (Zoom's
/// explicit web-client join link). We don't shape-validate `<id>` beyond
/// "non-empty" — Zoom meeting IDs are numeric today but that's not a
/// contract we want to hard-code here, mirroring how the Teams/Meet
/// helpers treat their opaque id segments.
fn is_zoom_join_path(path: &str) -> bool {
    let mut parts = path.split('/');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("j"), Some(id), None, None) => !id.is_empty(),
        (Some("wc"), Some("join"), Some(id), None) => !id.is_empty(),
        _ => false,
    }
}

/// Accept exactly `lookup/<id>` with a single non-empty segment. Permitting
/// nested paths under `lookup/` would broaden the attack surface beyond a
/// call deep-link.
fn is_lookup_path(path: &str) -> bool {
    let mut parts = path.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("lookup"), Some(id), None) if !id.is_empty()
    )
}

/// Trim and validate the display name. Meet's "Your name" field accepts a
/// wide range, but we cap length to a sane value so a malformed payload
/// can't push a 10MB string into the webview.
pub fn validate_display_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("display_name must not be empty".into());
    }
    if trimmed.chars().count() > 64 {
        return Err("display_name exceeds 64 characters".into());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("display_name contains control characters".into());
    }
    Ok(trimmed.to_string())
}

fn is_meet_code(path: &str) -> bool {
    let parts: Vec<&str> = path.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let lengths_ok = parts[0].len() >= 3 && parts[1].len() >= 3 && parts[2].len() >= 3;
    let alpha_only = parts
        .iter()
        .all(|p| p.chars().all(|c| c.is_ascii_lowercase()));
    lengths_ok && alpha_only
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod ops_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_platform_recognizes_google_meet() {
        let url = url::Url::parse("https://meet.google.com/abc-defg-hij").unwrap();
        assert_eq!(detect_platform(&url), Some(CallPlatform::GoogleMeet));
    }

    #[test]
    fn detect_platform_returns_none_for_other_hosts() {
        for raw in [
            "https://webex.com/j/123456789",
            "https://example.com/abc-defg-hij",
            // A lookalike suffix must not match via substring/suffix
            // confusion with the real `zoom.us` host.
            "https://zoom.us.evil.com/j/123456789",
        ] {
            let url = url::Url::parse(raw).unwrap();
            assert_eq!(
                detect_platform(&url),
                None,
                "expected no platform match for {raw}"
            );
        }
    }

    #[test]
    fn detect_platform_recognizes_teams() {
        for raw in [
            "https://teams.microsoft.com/l/meetup-join/abc",
            "https://teams.live.com/meet/abc",
        ] {
            let url = url::Url::parse(raw).unwrap();
            assert_eq!(
                detect_platform(&url),
                Some(CallPlatform::MicrosoftTeams),
                "expected MicrosoftTeams match for {raw}"
            );
        }
    }

    #[test]
    fn detect_platform_recognizes_zoom() {
        for raw in [
            "https://zoom.us/j/123456789",
            "https://app.zoom.us/j/123456789",
            "https://zoomgov.com/j/123456789",
            "https://zoom.us/wc/join/123456789",
        ] {
            let url = url::Url::parse(raw).unwrap();
            assert_eq!(
                detect_platform(&url),
                Some(CallPlatform::Zoom),
                "expected Zoom match for {raw}"
            );
        }
    }

    #[test]
    fn accepts_canonical_meet_code_url() {
        let u = validate_meet_url("https://meet.google.com/abc-defg-hij").unwrap();
        assert_eq!(u.host_str(), Some("meet.google.com"));
    }

    #[test]
    fn accepts_lookup_url() {
        validate_meet_url("https://meet.google.com/lookup/abcdef1234").unwrap();
    }

    #[test]
    fn accepts_teams_meetup_join_url() {
        let u = validate_meet_url(
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc%40thread.v2/0",
        )
        .unwrap();
        assert_eq!(u.host_str(), Some("teams.microsoft.com"));
    }

    #[test]
    fn accepts_teams_live_meet_url() {
        let u = validate_meet_url("https://teams.live.com/meet/1234567890").unwrap();
        assert_eq!(u.host_str(), Some("teams.live.com"));
    }

    #[test]
    fn accepts_zoom_classic_join_url() {
        for host in ["zoom.us", "app.zoom.us", "zoomgov.com"] {
            let raw = format!("https://{host}/j/123456789");
            let u = validate_meet_url(&raw).unwrap();
            assert_eq!(u.host_str(), Some(host));
        }
    }

    #[test]
    fn accepts_zoom_web_client_join_url() {
        let u = validate_meet_url("https://app.zoom.us/wc/join/123456789").unwrap();
        assert_eq!(u.host_str(), Some("app.zoom.us"));
    }

    #[test]
    fn rejects_malformed_zoom_urls() {
        // Missing the id segment.
        assert!(validate_meet_url("https://zoom.us/j/").is_err());
        assert!(validate_meet_url("https://zoom.us/j").is_err());
        // wc/join without an id.
        assert!(validate_meet_url("https://zoom.us/wc/join/").is_err());
        // Unrecognised path shape entirely.
        assert!(validate_meet_url("https://zoom.us/").is_err());
        assert!(validate_meet_url("https://zoom.us/my/room").is_err());
    }

    #[test]
    fn rejects_malformed_teams_urls() {
        // Missing the `meetup-join` segment.
        assert!(validate_meet_url("https://teams.microsoft.com/l/abc").is_err());
        // `l/meetup-join/` with nothing after it.
        assert!(validate_meet_url("https://teams.microsoft.com/l/meetup-join/").is_err());
        // teams.live.com without a meeting id.
        assert!(validate_meet_url("https://teams.live.com/meet/").is_err());
        assert!(validate_meet_url("https://teams.live.com/").is_err());
    }

    #[test]
    fn rejects_http_scheme() {
        assert!(validate_meet_url("http://meet.google.com/abc-defg-hij").is_err());
    }

    #[test]
    fn rejects_other_hosts() {
        assert!(validate_meet_url("https://example.com/abc-defg-hij").is_err());
        assert!(validate_meet_url("https://meet.google.evil.com/abc-defg-hij").is_err());
    }

    #[test]
    fn rejects_nonsense_paths() {
        assert!(validate_meet_url("https://meet.google.com/").is_err());
        assert!(validate_meet_url("https://meet.google.com/foo").is_err());
        assert!(validate_meet_url("https://meet.google.com/AB-CD-EF").is_err());
        // Nested paths under `lookup/` must stay rejected — only a single
        // non-empty id segment is allowed.
        assert!(validate_meet_url("https://meet.google.com/lookup/").is_err());
        assert!(validate_meet_url("https://meet.google.com/lookup/abc/extra").is_err());
    }

    #[test]
    fn trims_and_validates_display_name() {
        assert_eq!(validate_display_name("  Alice  ").unwrap(), "Alice");
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name("   ").is_err());
        assert!(validate_display_name(&"x".repeat(65)).is_err());
        assert!(validate_display_name("hi\nthere").is_err());
    }
}
