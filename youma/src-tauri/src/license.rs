// license.rs — License validation, activation and grace-period logic for Youma.
//
// Storage: app_settings keys under the "license." prefix.
// Grace period: 72 hours offline tolerance after last successful server check.
// Trial: 30-day free trial, starts on first request.

use crate::db;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const GRACE_HOURS: i64 = 72;
const TRIAL_DAYS: i64 = 30;

// ---------------------------------------------------------------------------
// Machine fingerprint
// ---------------------------------------------------------------------------

/// Return a stable, hashed machine identifier.
/// The raw hardware ID is hashed with SHA-256 before use or storage.
pub fn get_machine_id() -> String {
    let raw = raw_machine_id();
    let mut hasher = Sha256::new();
    hasher.update(raw.trim().as_bytes());
    hasher.update(b"youma-machine-v1");
    hex::encode(hasher.finalize())
}

#[cfg(target_os = "windows")]
fn raw_machine_id() -> String {
    let out = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                if line.contains("MachineGuid") {
                    if let Some(guid) = line.split_whitespace().last() {
                        return guid.to_string();
                    }
                }
            }
            eprintln!("[license] MachineGuid not found in reg output");
            "windows-guid-unavailable".to_string()
        }
        Err(e) => {
            eprintln!("[license] reg query error: {e}");
            "windows-fallback".to_string()
        }
    }
}

#[cfg(target_os = "macos")]
fn raw_machine_id() -> String {
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return uuid.to_string();
                    }
                }
            }
            "macos-uuid-unavailable".to_string()
        }
        Err(_) => "macos-fallback".to_string(),
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn raw_machine_id() -> String {
    std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_else(|_| "linux-fallback".to_string())
}

// ---------------------------------------------------------------------------
// Settings helpers
// ---------------------------------------------------------------------------

fn get_setting(key: &str) -> Option<String> {
    db::settings_get(key)
        .ok()
        .and_then(|v| v.get("value").and_then(|v| v.as_str()).map(String::from))
        .filter(|s| !s.is_empty() && s != "null")
}

fn set_setting(key: &str, value: &str) {
    if let Err(e) = db::settings_set(key, value) {
        eprintln!("[license] set_setting {key}: {e}");
    }
}

fn server_url() -> String {
    get_setting("license.server_url")
        .unwrap_or_else(|| "https://license.youma.ai".to_string())
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

// ---------------------------------------------------------------------------
// Local status (fast, no network)
// ---------------------------------------------------------------------------

pub fn local_status() -> Value {
    let status = get_setting("license.status").unwrap_or_else(|| "inactive".to_string());
    let key = get_setting("license.key").unwrap_or_default();
    let plan = get_setting("license.plan").unwrap_or_default();
    let expires_at_str = get_setting("license.expires_at");
    let org = get_setting("license.org").unwrap_or_default();
    let last_valid_str = get_setting("license.last_valid_at");
    let trial_started_str = get_setting("license.trial_started_at");
    let machine_id = get_machine_id();

    let now = Utc::now();

    let is_expired = expires_at_str
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .map(|exp| now > exp)
        .unwrap_or(false);

    let grace_hours_left: Option<i64> = last_valid_str
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .map(|lv| (GRACE_HOURS - now.signed_duration_since(lv).num_hours()).max(0));

    let trial_start = trial_started_str
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let valid: bool;
    let effective_status: &str;

    match status.as_str() {
        "active" if !is_expired => {
            valid = true;
            effective_status = "active";
        }
        "active" => {
            // Expired — fall into grace if we have a recent check
            let in_grace = grace_hours_left.map(|h| h > 0).unwrap_or(false);
            if in_grace {
                valid = true;
                effective_status = "grace";
            } else {
                valid = false;
                effective_status = "expired";
            }
        }
        "trial" => {
            let trial_expired = trial_start
                .map(|start| now.signed_duration_since(start).num_days() >= TRIAL_DAYS)
                .unwrap_or(true);
            if trial_expired {
                valid = false;
                effective_status = "expired";
            } else {
                valid = true;
                effective_status = "trial";
            }
        }
        "grace" => {
            let in_grace = grace_hours_left.map(|h| h > 0).unwrap_or(false);
            if in_grace {
                valid = true;
                effective_status = "grace";
            } else {
                valid = false;
                effective_status = "expired";
            }
        }
        _ => {
            valid = false;
            effective_status = "inactive";
        }
    }

    let trial_days_left: Option<i64> = if effective_status == "trial" {
        trial_start.map(|start| {
            (TRIAL_DAYS - now.signed_duration_since(start).num_days()).max(0)
        })
    } else {
        None
    };

    eprintln!("[license] local_status status={effective_status} valid={valid}");

    json!({
        "valid": valid,
        "status": effective_status,
        "key": if key.is_empty() { Value::Null } else { Value::String(key) },
        "plan": if plan.is_empty() { Value::Null } else { Value::String(plan) },
        "expires_at": expires_at_str,
        "org": if org.is_empty() { Value::Null } else { Value::String(org) },
        "grace_hours_left": grace_hours_left,
        "trial_days_left": trial_days_left,
        "machine_id": machine_id,
    })
}

// ---------------------------------------------------------------------------
// Start trial
// ---------------------------------------------------------------------------

pub fn start_trial() -> Value {
    // Don't allow restarting an already-used trial.
    if get_setting("license.trial_started_at").is_some() {
        return json!({ "ok": false, "error": "Trial already used on this machine" });
    }

    let now = Utc::now();
    let expires_at = now + Duration::days(TRIAL_DAYS);
    set_setting("license.status", "trial");
    set_setting("license.plan", "trial");
    set_setting("license.trial_started_at", &now.to_rfc3339());
    set_setting("license.expires_at", &expires_at.to_rfc3339());
    set_setting("license.last_valid_at", &now.to_rfc3339());
    eprintln!("[license] trial started expires={}", expires_at.to_rfc3339());

    json!({
        "ok": true,
        "status": "trial",
        "expires_at": expires_at.to_rfc3339(),
        "trial_days": TRIAL_DAYS,
    })
}

// ---------------------------------------------------------------------------
// Activate
// ---------------------------------------------------------------------------

pub async fn activate(key: &str, server_url_override: Option<&str>) -> Value {
    let machine_id = get_machine_id();
    let url = server_url_override
        .map(String::from)
        .unwrap_or_else(server_url);

    eprintln!("[license] activate key={} server={url}", &key[..key.len().min(12)]);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": format!("HTTP client error: {e}") }),
    };

    let body = json!({
        "key": key,
        "machine_id": machine_id,
        "hostname": hostname(),
    });

    match client
        .post(format!("{url}/api/v1/activate"))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let http_status = resp.status();
            match resp.json::<Value>().await {
                Ok(data) => {
                    if http_status.is_success() {
                        set_setting("license.key", key);
                        set_setting("license.status", "active");
                        set_setting("license.last_valid_at", &Utc::now().to_rfc3339());
                        if let Some(p) = data.get("plan").and_then(|v| v.as_str()) {
                            set_setting("license.plan", p);
                        }
                        if let Some(e) = data.get("expires_at").and_then(|v| v.as_str()) {
                            set_setting("license.expires_at", e);
                        }
                        if let Some(o) = data
                            .get("org")
                            .or_else(|| data.get("organization"))
                            .and_then(|v| v.as_str())
                        {
                            set_setting("license.org", o);
                        }
                        if let Some(override_url) = server_url_override {
                            set_setting("license.server_url", override_url);
                        }
                        eprintln!("[license] activated ok");
                        json!({ "ok": true, "status": "active", "data": data })
                    } else {
                        let msg = data
                            .get("error")
                            .or_else(|| data.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Activation failed");
                        eprintln!("[license] activate rejected: {msg}");
                        json!({ "ok": false, "error": msg })
                    }
                }
                Err(e) => json!({ "ok": false, "error": format!("Response parse error: {e}") }),
            }
        }
        Err(e) => {
            eprintln!("[license] activate network error: {e}");
            json!({ "ok": false, "error": format!("Network error: {e}") })
        }
    }
}

// ---------------------------------------------------------------------------
// Validate (background — refresh last_valid_at)
// ---------------------------------------------------------------------------

pub async fn validate_remote() -> bool {
    let key = match get_setting("license.key") {
        Some(k) if !k.is_empty() => k,
        _ => return false,
    };
    let status = get_setting("license.status").unwrap_or_default();
    if matches!(status.as_str(), "trial" | "inactive" | "") {
        // Trial and inactive don't validate against server
        return status == "trial";
    }

    let machine_id = get_machine_id();
    let url = server_url();
    eprintln!("[license] validate_remote server={url}");

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return true, // client build failure → treat as offline grace
    };

    let body = json!({ "key": key, "machine_id": machine_id });

    match client
        .post(format!("{url}/api/v1/validate"))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<Value>().await {
                let valid = data.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
                if valid {
                    set_setting("license.last_valid_at", &Utc::now().to_rfc3339());
                    set_setting("license.status", "active");
                    if let Some(e) = data.get("expires_at").and_then(|v| v.as_str()) {
                        set_setting("license.expires_at", e);
                    }
                    eprintln!("[license] validate_remote ok");
                    return true;
                }
                // Server rejected — enter grace mode
                set_setting("license.status", "grace");
                eprintln!("[license] validate_remote: server rejected → grace");
            }
            false
        }
        Ok(resp) => {
            eprintln!("[license] validate_remote non-2xx: {}", resp.status());
            // Network reachable but error → grace period
            false
        }
        Err(e) => {
            // Network unreachable → grace period (don't change status)
            eprintln!("[license] validate_remote offline: {e} — grace period applies");
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Deactivate
// ---------------------------------------------------------------------------

pub async fn deactivate() -> Value {
    let key = get_setting("license.key").unwrap_or_default();
    let machine_id = get_machine_id();
    let url = server_url();

    if !key.is_empty() {
        if let Ok(client) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
        {
            let body = json!({ "key": key, "machine_id": machine_id });
            let _ = client
                .post(format!("{url}/api/v1/deactivate"))
                .json(&body)
                .send()
                .await;
        }
    }

    // Clear local state regardless of remote outcome
    for k in &[
        "license.status",
        "license.key",
        "license.plan",
        "license.expires_at",
        "license.org",
        "license.last_valid_at",
    ] {
        set_setting(k, "");
    }
    eprintln!("[license] deactivated");
    json!({ "ok": true })
}
