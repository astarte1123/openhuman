//! Pure provider policy helpers: URLs, allowed hosts, deep-link
//! rewriting, popup/navigation classification, and display names.
//! Split out of `mod.rs` — no behavior changes.

use tauri::Url;

const LINKEDIN_RECIPE_JS: &str = include_str!("../../recipes/linkedin/recipe.js");
const GOOGLE_MEET_RECIPE_JS: &str = include_str!("../../recipes/google-meet/recipe.js");

/// Registered providers and their service URLs. Add a new arm here plus a
/// recipe.js file under `recipes/<id>/` to support another provider.
pub(crate) fn provider_url(provider: &str) -> Option<&'static str> {
    match provider {
        "whatsapp" => Some("https://web.whatsapp.com/"),
        "wechat" => Some("https://web.wechat.com/"),
        "telegram" => Some("https://web.telegram.org/k/"),
        "linkedin" => Some("https://www.linkedin.com/messaging/"),
        "slack" => Some("https://app.slack.com/client/"),
        "discord" => Some("https://discord.com/channels/@me"),
        "google-meet" => Some("https://meet.google.com/"),
        "zoom" => Some("https://zoom.us/"),
        "browserscan" => Some("https://www.browserscan.net/bot-detection"),
        _ => None,
    }
}

/// Returns the injected recipe.js for providers that still rely on the
/// JS-bridge ingest path. Migrated providers (whatsapp, telegram, slack,
/// discord, browserscan) return `None` — their scraping runs natively via
/// CDP in the per-provider scanner modules.
pub(crate) fn provider_recipe_js(provider: &str) -> Option<&'static str> {
    match provider {
        "linkedin" => Some(LINKEDIN_RECIPE_JS),
        "google-meet" => Some(GOOGLE_MEET_RECIPE_JS),
        _ => None,
    }
}

/// Whether this provider is supported at all. Derived from
/// `provider_url` so there's one canonical list — new providers added
/// to the `provider_url` match automatically become "supported" here.
pub(crate) fn provider_is_supported(provider: &str) -> bool {
    provider_url(provider).is_some()
}

/// Host suffixes the embedded webview is allowed to navigate within. Any
/// navigation to a host outside this set is cancelled and opened in the
/// user's default browser instead. Meet includes Google's auth and
/// static asset hosts so the OAuth redirect loop works; Discord includes
/// its CDN subdomains for the same reason.
pub(crate) fn provider_allowed_hosts(provider: &str) -> &'static [&'static str] {
    match provider {
        "whatsapp" => &["whatsapp.com", "whatsapp.net", "wa.me"],
        "wechat" => &[
            "wechat.com",
            "wx.qq.com",
            "weixin.qq.com",
            "login.weixin.qq.com",
        ],
        "telegram" => &["telegram.org", "t.me"],
        "linkedin" => &[
            "linkedin.com",
            "licdn.com",
            "accounts.google.com",
            "accounts.googleusercontent.com",
            "ssl.gstatic.com",
            "fonts.gstatic.com",
            "lh3.googleusercontent.com",
            "oauth2.googleapis.com",
            "www.googleapis.com",
        ],
        "slack" => &[
            "slack.com",
            "slack-edge.com",
            "slackb.com",
            "accounts.google.com",
            "accounts.googleusercontent.com",
            "ssl.gstatic.com",
            "fonts.gstatic.com",
            "lh3.googleusercontent.com",
            "oauth2.googleapis.com",
            "www.googleapis.com",
        ],
        "discord" => &[
            "discord.com",
            "discord.gg",
            "discordapp.com",
            "discordapp.net",
        ],
        "google-meet" => &[
            "google.com",
            "googleusercontent.com",
            "gstatic.com",
            "googleapis.com",
        ],
        "zoom" => &[
            "zoom.us",
            "zoom.com",
            "zoomgov.com",
            "zdassets.com",
            "accounts.google.com",
            "accounts.googleusercontent.com",
            "ssl.gstatic.com",
            "fonts.gstatic.com",
            "lh3.googleusercontent.com",
            "oauth2.googleapis.com",
            "www.googleapis.com",
        ],
        "browserscan" => &["browserscan.net"],
        _ => &[],
    }
}

/// Rewrite a provider-specific native-app deep link (e.g. Zoom's
/// `zoomus://zoom.us/join?...`) into a web-client URL so the meeting stays
/// inside the embedded webview instead of failing with
/// ERR_UNKNOWN_URL_SCHEME (CEF has no handler for these schemes).
///
/// Returns `Some(rewritten)` when the provider claims the scheme and a
/// valid web-client URL can be built; `None` otherwise (caller should
/// leave the navigation alone).
pub(crate) fn rewrite_provider_deep_link(provider: &str, url: &Url) -> Option<Url> {
    if provider != "zoom" {
        return None;
    }
    if !matches!(url.scheme(), "zoomus" | "zoommtg") {
        return None;
    }
    // Pull the meeting id out of the query string. Zoom uses `confno` on
    // both `action=join` (joining) and `action=start` (hosting) flows.
    let confno = url
        .query_pairs()
        .find(|(k, _)| k == "confno")
        .map(|(_, v)| v.into_owned());
    let pwd = url
        .query_pairs()
        .find(|(k, _)| k == "pwd" || k == "tk")
        .map(|(_, v)| v.into_owned());
    // Build the rewritten URL via `Url` so `confno` and `pwd` are
    // percent-encoded — inbound Zoom tokens can contain reserved chars
    // (`&`, `#`, `%`, `+`, …) that would corrupt a hand-rolled
    // `format!(…)` string and silently break the join/host flow.
    match confno {
        Some(id) if !id.is_empty() => {
            // Base without trailing slash; `path_segments_mut().push(id)`
            // appends `/id` cleanly. A trailing `/` on the base would yield
            // `/wc/join//id` (empty segment preserved by the Url spec).
            let mut rewritten = Url::parse("https://app.zoom.us/wc/join").ok()?;
            rewritten.path_segments_mut().ok()?.push(&id);
            if let Some(p) = pwd.filter(|p| !p.is_empty()) {
                rewritten.query_pairs_mut().append_pair("pwd", &p);
            }
            Some(rewritten)
        }
        _ => Url::parse("https://app.zoom.us/wc/home").ok(),
    }
}

/// `true` if `url` is considered in-app for `provider`. Non-HTTP(S)
/// schemes (`about:blank`, `data:`, `blob:`) have no host and are always
/// allowed so the webview's own internal navigations keep working.
/// Unknown providers are also permissive — better to accidentally keep a
/// link in-app than to leak it to the system browser.
pub(crate) fn url_is_internal(provider: &str, url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return true;
    };
    // Google services route the post-2FA `SetSID` cookie-setting hop
    // through `accounts.youtube.com` and ccTLD `accounts.google.<rest>`
    // hosts that aren't covered by the suffix-based allowlist. Without
    // this, the auth chain breaks mid-flight and leaks to the system
    // browser (#1053 sign-in leak surfaced in dev:app log line:
    // "external navigation https://accounts.youtube.com/accounts/SetSID?...
    // → system browser"). Whitelist the full Google SSO host family for
    // any provider that uses Google identity.
    if (provider == "gmail" || provider_supports_google_sso(provider)) && is_google_sso_host(host) {
        return true;
    }
    let allowed = provider_allowed_hosts(provider);
    if allowed.is_empty() {
        return true;
    }
    allowed
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{}", suffix)))
}

/// `true` if the provider needs `window.open(url)` to return a live
/// window-handle (i.e. the calling site reads the return value and aborts
/// on falsey). Slack Huddles go through `openManagedChildWindow` which
/// calls `window.open("about:blank", …)` and then programmatically
/// navigates the returned popup to the huddle UI. Denying the popup
/// makes the huddle call fail silently with a `beacon/error`. For these
/// cases we allow the default popup so CEF spawns an in-app child window
/// and returns a real handle to the caller.
///
/// Match is intentionally narrow — only the popup URLs the provider
/// actually needs in-app pass. Cmd/Ctrl-click and `target="_blank"`
/// on ordinary links (which carry a concrete URL) still route out to
/// the user's default browser.
pub(crate) fn popup_should_stay_in_app(provider: &str, url: &Url) -> bool {
    match provider {
        "slack" => {
            // Slack's huddle flow opens `about:blank` first, then navigates
            // the popup to the huddle URL — at popup-creation time there is
            // no host yet. Also accept same-origin slack.com hosts so direct
            // `window.open("https://app.slack.com/...")` calls stay in-app.
            if url.scheme() == "about" {
                return true;
            }
            match url.host_str() {
                Some(host) => host == "app.slack.com" || host.ends_with(".slack.com"),
                None => false,
            }
        }
        "zoom" => {
            // Zoom's "Join from browser" / WebClient launch can go through a
            // `window.open("https://app.zoom.us/wc/...")` popup instead of an
            // in-page navigation. Keep those (and any deep-link-rewritten
            // popup targeting the same path) inside the embedded webview so
            // the meeting doesn't pop out to the system browser.
            match url.host_str() {
                Some(host) => {
                    (host == "app.zoom.us" || host == "zoom.us") && url.path().starts_with("/wc/")
                }
                None => false,
            }
        }
        "linkedin" => {
            // LinkedIn's "Sign in with Google" button is rendered as a Google
            // Identity Services (GSI) iframe loaded from
            // `accounts.google.com/gsi/button`. When the user clicks it, GSI
            // calls `window.open("https://accounts.google.com/gsi/select?...",
            // "gsig", "width=500,height=600,...")` to show the account chooser.
            //
            // This popup MUST stay as a real in-app child window — NOT routed
            // to the system browser (blank screen) and NOT a parent navigation
            // (the parent page would be replaced, so the postMessage credential
            // callback from the popup can never reach LinkedIn's JS handler).
            //
            // After the user selects an account the popup postMessages the
            // signed credential back to the opener; LinkedIn's GSI callback
            // receives it and completes sign-in (#1021).
            match url.host_str() {
                Some(host) => {
                    is_google_sso_host(host) && url.path().to_ascii_lowercase().contains("gsi")
                }
                None => false,
            }
        }
        _ => false,
    }
}

/// `true` if `scheme` is a known provider native-desktop-app deep-link
/// scheme. We suppress these instead of routing them to the system
/// browser because macOS hands them to the native provider app
/// (e.g. `slack://magic-login/<token>` signs the native Slack app into
/// the workspace, breaking embedded-webview isolation: the workspace's
/// session ends up inside the native client even though the user only
/// signed in via OpenHuman's embedded webview).
///
/// The HTTPS fallback in each provider's web flow handles sign-in
/// without the deep link, so suppression is safe — the page just
/// continues on the next link in the sequence.
///
/// Caller contract: only suppress when [`rewrite_provider_deep_link`]
/// has already returned `None` for the URL. Schemes we DO know how to
/// rewrite into a web-client URL (e.g. `zoomus://`) must take the
/// rewrite path first; those flows expect to stay in-app, not be
/// silently dropped.
pub(crate) fn is_provider_native_deep_link_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "slack" | "discord" | "tg" | "msteams" | "zoomus" | "zoommtg"
    )
}

/// `true` if this provider lets users sign in with their Google
/// account from inside the embedded webview.
///
/// Slack workspaces commonly enable "Sign in with Google" SSO, so the
/// Google OAuth popup flow (`window.open("https://accounts.google.com/...")`)
/// must stay in the per-account CEF session — exactly the same way it
/// has to for Google Meet. Routing it to the system browser leaks the
/// auth cookie into the wrong jar and breaks sign-in (#1036).
///
/// Keep this list narrow: only providers that actually need to issue
/// `accounts.google.com` popups should be listed. Other providers
/// continue to fall through to the default popup-handling path.
pub(crate) fn provider_supports_google_sso(provider: &str) -> bool {
    matches!(provider, "google-meet" | "slack" | "zoom" | "linkedin")
}

/// `true` if a popup request should be denied AND the parent webview
/// should be navigated to the popup URL instead.
///
/// Used for Google's "Sign in" / "Use another account" flow on embedded
/// providers that support Google SSO: clicking the link issues
/// `window.open("https://accounts.google.com/...")`. We can't route
/// that to the system browser (the auth cookie would land in the
/// wrong jar) and we don't want to let CEF spawn an unmanaged child
/// window (it has no host rect, so it renders blank/black). The safe
/// option is to deny the popup and replace the parent's URL so the
/// in-app webview finishes the auth flow inside the embedded session.
pub(crate) fn popup_should_navigate_parent(provider: &str, url: &Url) -> Option<Url> {
    if !provider_supports_google_sso(provider) {
        return None;
    }
    if url.scheme() == "about" {
        return None;
    }
    if is_google_auth_popup(url) {
        return Some(url.clone());
    }
    // Gmeet: "Start an instant meeting" / "New meeting" / clicking
    // a meeting code link calls `window.open(meet.google.com/<roomid>)`
    // to launch the room. Default popup handling would route the
    // URL to the user's system browser, leaking the Meet session
    // out of OpenHuman entirely. Deny the popup and navigate the
    // embedded parent into the room URL instead — matches the
    // user's expectation that the meeting stays in-app.
    if provider == "google-meet" {
        if let Some(host) = url.host_str() {
            if host == "meet.google.com" {
                return Some(url.clone());
            }
        }
    }
    None
}

/// `true` if `host` is a Google SSO / account-handoff host that may
/// participate in the OAuth flow for any Google service (Meet, Gmail,
/// Drive, etc.). Google rotates the post-2FA `SetSID` cookie-setting hop
/// across `accounts.google.<cctld>` and `accounts.youtube.com` (sic — the
/// YouTube subdomain is part of the Google identity infra), so a literal
/// `accounts.google.com` match misses real auth popups and leaks them to
/// the system browser.
///
/// Match family:
/// - `accounts.google.com`
/// - `accounts.google.<cctld>` — e.g. `accounts.google.co.in`, `accounts.google.co.uk`,
///   `accounts.google.de`
/// - `accounts.googleusercontent.com`
/// - `accounts.youtube.com` (post-2FA `SetSID` hop)
/// - `myaccount.google.com`
pub(crate) fn is_google_sso_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "accounts.google.com"
        || host == "accounts.googleusercontent.com"
        || host == "accounts.youtube.com"
        || host == "myaccount.google.com"
    {
        return true;
    }
    // ccTLD variants: `accounts.google.<cctld>`. We must reject phishing
    // shapes like `accounts.google.com.evil` and `accounts.google.co.attacker`
    // — the dots-only check we used previously accepted both because
    // `com.evil` and `co.attacker` each have one dot. Anchor the suffix
    // against a real ccTLD shape: either a single 2-letter cc tld
    // (`accounts.google.de`, `accounts.google.fr`) OR a 2-label form
    // `<sld>.<cc>` where sld ∈ {co, com, net, org} (`accounts.google.co.in`,
    // `accounts.google.com.au`).
    if let Some(rest) = host.strip_prefix("accounts.google.") {
        let labels: Vec<&str> = rest.split('.').collect();
        let is_cc = |s: &str| s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic());
        return match labels.as_slice() {
            [tld] => is_cc(tld),
            [sld, tld] => matches!(*sld, "co" | "com" | "net" | "org") && is_cc(tld),
            _ => false,
        };
    }
    false
}

pub(crate) fn is_google_auth_popup(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if !is_google_sso_host(host) {
        return false;
    }

    let path = url.path().to_ascii_lowercase();
    if path.contains("signin")
        || path.contains("servicelogin")
        || path.contains("accountchooser")
        || path.contains("chooseaccount")
        || path.contains("setsid")
        || path.contains("oauth2")
    {
        return true;
    }

    url.query_pairs().any(|(key, value)| {
        let k = key.to_ascii_lowercase();
        let v = value.to_ascii_lowercase();
        matches!(k.as_str(), "flowname" | "service" | "continue")
            && (v.contains("signin")
                || v.contains("servicelogin")
                || v.contains("accountchooser")
                || v.contains("chooseaccount")
                || v.contains("meet.google.com")
                || v.contains("mail.google.com")
                || v.contains("linkedin.com"))
    })
}

/// `true` if a gmeet navigation lands on Google's Workspace marketing page
/// for Meet — the host bounce that fires when an unauthenticated webview hits
/// `meet.google.com`.
///
/// The `on_navigation` rewrite is scoped to this exact path family so we
/// don't hijack legitimate `workspace.google.com` pages a user might reach
/// from inside Meet (admin console links, Workspace Status, support pages,
/// etc.). Matches `workspace.google.com` (and any subdomain) AND a path
/// starting with `/products/meet` — empirically the only path Google's
/// edge bounces unauthenticated Meet GETs to (`/products/meet/` or
/// `/products/meet/<sub>`).
pub(crate) fn is_gmeet_marketing_redirect(host: &str, path: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let host_matches = host == "workspace.google.com" || host.ends_with(".workspace.google.com");
    if !host_matches {
        return false;
    }
    let p = path.to_ascii_lowercase();
    p == "/products/meet" || p == "/products/meet/" || p.starts_with("/products/meet/")
}

pub(crate) fn redact_navigation_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

pub(crate) fn redact_native_deep_link_url(url: &Url) -> String {
    format!("{}://<redacted>", url.scheme())
}
/// Unwrap provider-side "link safety" redirects so the system browser
/// lands on the real destination.
///
/// These wrappers (LinkedIn's `/safety/go/?url=…`, etc.) require the
/// user to be logged into the provider in the destination browser. In
/// our setup the session lives inside the embedded CEF webview's cookie
/// jar, not the user's default browser — opening the wrapper URL there
/// shows a broken safety page instead of completing the redirect.
/// Extract the `url` query param and return the resolved destination.
pub(crate) fn unwrap_provider_redirect(url: &Url) -> Option<Url> {
    let host = url.host_str()?;
    let path = url.path();
    let matches_linkedin = (host == "www.linkedin.com" || host == "linkedin.com")
        && (path == "/safety/go/" || path == "/safety/go" || path == "/redir/redirect");
    if !matches_linkedin {
        return None;
    }
    let (_, raw) = url.query_pairs().find(|(k, _)| k == "url")?;
    Url::parse(&raw).ok()
}

/// Fire-and-forget handoff to the OS default URL handler. Any error is
/// logged but not propagated — we've already cancelled the in-app
/// navigation so there's nowhere to surface a failure to.
///
/// On macOS we shell out to `/usr/bin/open` directly rather than via
/// `tauri_plugin_opener::open_url`: the plugin returned Ok but no browser
/// actually launched in the CEF runtime (suspected sandbox/launch-service
/// interaction with the `open` crate's detached spawn). The direct
/// Command call is equivalent to what a user would type in Terminal and
/// works reliably.
pub(crate) fn open_in_system_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("/usr/bin/open").arg(url).spawn() {
            Ok(_) => log::info!("[webview-accounts] opened externally (macos open): {}", url),
            Err(e) => log::warn!(
                "[webview-accounts] /usr/bin/open {} failed: {} — falling back to opener plugin",
                url,
                e
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        match tauri_plugin_opener::open_url(url, None::<&str>) {
            Ok(()) => log::info!("[webview-accounts] opened externally: {}", url),
            Err(e) => log::warn!("[webview-accounts] open_url({}) failed: {}", url, e),
        }
    }
}

/// Human-readable label used as the title prefix on native notifications
/// so users can tell which provider fired the ping. Matches the labels
/// in the frontend `PROVIDERS` registry.
pub fn provider_display_name(provider: &str) -> &'static str {
    match provider {
        "whatsapp" => "WhatsApp",
        "wechat" => "WeChat",
        "telegram" => "Telegram",
        "linkedin" => "LinkedIn",
        "slack" => "Slack",
        "discord" => "Discord",
        "google-meet" => "Google Meet",
        "zoom" => "Zoom",
        "browserscan" => "BrowserScan",
        _ => "OpenHuman",
    }
}
