use std::sync::Arc;
use tracing::{error, info, warn};

use crate::{db, webhook, AppState};

/// Run all scheduled background tasks. Called every minute by the cron scheduler in main.rs.
pub async fn run_scheduled_tasks(state: Arc<AppState>) {
    let state_clone = Arc::clone(&state);
    tokio::join!(
        expire_licenses(Arc::clone(&state_clone)),
        expire_floating_checkouts(Arc::clone(&state_clone)),
        send_expiry_warnings(Arc::clone(&state_clone)),
        webhook::retry_failed_webhooks(Arc::clone(&state_clone)),
    );
}

/// Expire licenses whose expires_at has passed and status is still 'active'.
async fn expire_licenses(state: Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();

    // Collect keys to expire
    let keys_to_expire: Vec<(String, Option<String>)> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => {
                error!("[scheduler] DB lock poisoned in expire_licenses");
                return;
            }
        };
        let mut stmt = match db.prepare(
            "SELECT key, org_id FROM licenses WHERE status = 'active' AND expires_at IS NOT NULL AND expires_at < ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                error!("[scheduler] prepare expire query failed: {}", e);
                return;
            }
        };
        stmt.query_map(rusqlite::params![now], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    if keys_to_expire.is_empty() {
        return;
    }

    info!(
        count = keys_to_expire.len(),
        "[scheduler] expiring {} licenses",
        keys_to_expire.len()
    );

    for (key, org_id) in &keys_to_expire {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Expire and decrement pool used_seats atomically
        let result = db.execute(
            "UPDATE licenses SET status = 'expired' WHERE key = ?1 AND status = 'active'",
            rusqlite::params![key],
        );

        match result {
            Ok(changed) if changed > 0 => {
                // Decrement pool seat count
                let _ = db.execute(
                    "UPDATE pools SET used_seats = MAX(0, used_seats - 1)
                     WHERE id = (SELECT pool_id FROM licenses WHERE key = ?1)",
                    rusqlite::params![key],
                );

                let _ = db::insert_audit(
                    &db,
                    db::AuditParams {
                        event: "license.expired",
                        license_key: Some(key),
                        org_id: org_id.as_deref(),
                        pool_id: None,
                        machine_id_hash: None,
                        ip: None,
                        user_agent: None,
                        admin_id: None,
                        details: serde_json::json!({ "reason": "scheduled_expiry" }),
                    },
                );

                info!(key = %key, "[scheduler] license expired");
                drop(db);

                // Fire webhook (after releasing lock)
                webhook::fire_event(
                    Arc::clone(&state),
                    "license.expired",
                    Some(key.clone()),
                    org_id.clone(),
                    serde_json::json!({ "reason": "scheduled_expiry" }),
                );
            }
            Ok(_) => {} // Already expired
            Err(e) => error!(key = %key, error = %e, "[scheduler] failed to expire license"),
        }
    }
}

/// Remove floating checkouts whose heartbeat has expired; recalculate used_seats.
async fn expire_floating_checkouts(state: Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();

    let db = match state.db.lock() {
        Ok(d) => d,
        Err(_) => {
            error!("[scheduler] DB lock poisoned in expire_floating_checkouts");
            return;
        }
    };

    // Find expired checkouts and their license keys
    let expired: Vec<(String, String)> = {
        let mut stmt = match db.prepare(
            "SELECT id, license_key FROM floating_checkouts WHERE expires_at < ?1",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        stmt.query_map(rusqlite::params![now], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    if expired.is_empty() {
        return;
    }

    info!(
        count = expired.len(),
        "[scheduler] expiring {} floating checkouts",
        expired.len()
    );

    for (checkout_id, license_key) in &expired {
        let _ = db.execute(
            "DELETE FROM floating_checkouts WHERE id = ?1",
            rusqlite::params![checkout_id],
        );

        // Recalculate used_seats from actual checkout count
        let active_checkouts: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM floating_checkouts WHERE license_key = ?1",
                rusqlite::params![license_key],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let _ = db.execute(
            "UPDATE pools SET used_seats = ?1
             WHERE id = (SELECT pool_id FROM licenses WHERE key = ?2)",
            rusqlite::params![active_checkouts, license_key],
        );

        tracing::debug!(
            checkout_id = %checkout_id,
            license_key = %license_key,
            "[scheduler] floating checkout expired"
        );
    }
}

/// Fire expiry-warning webhooks for subscriptions expiring in 3 days (once per license).
async fn send_expiry_warnings(state: Arc<AppState>) {
    let soon = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(3))
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339();
    let now = chrono::Utc::now().to_rfc3339();

    let expiring: Vec<(String, Option<String>)> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return,
        };

        let mut stmt = match db.prepare(
            // Only fire for subscription model; check last audit to avoid duplicate warnings
            "SELECT l.key, l.org_id FROM licenses l
             WHERE l.license_model = 'subscription'
               AND l.status = 'active'
               AND l.expires_at IS NOT NULL
               AND l.expires_at <= ?1
               AND l.expires_at > ?2
               AND NOT EXISTS (
                 SELECT 1 FROM audit_log a
                 WHERE a.license_key = l.key
                   AND a.event = 'license.expiring_soon'
                   AND a.ts > date('now', '-1 day')
               )",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("[scheduler] expiry warning query failed: {}", e);
                return;
            }
        };

        stmt.query_map(rusqlite::params![soon, now], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    for (key, org_id) in &expiring {
        {
            let db = match state.db.lock() {
                Ok(d) => d,
                Err(_) => continue,
            };
            let _ = db::insert_audit(
                &db,
                db::AuditParams {
                    event: "license.expiring_soon",
                    license_key: Some(key),
                    org_id: org_id.as_deref(),
                    pool_id: None,
                    machine_id_hash: None,
                    ip: None,
                    user_agent: None,
                    admin_id: None,
                    details: serde_json::json!({ "warning": "expires_in_3_days" }),
                },
            );
        }

        webhook::fire_event(
            Arc::clone(&state),
            "license.expiring_soon",
            Some(key.clone()),
            org_id.clone(),
            serde_json::json!({ "warning": "expires_in_3_days" }),
        );

        info!(key = %key, "[scheduler] sent expiry warning webhook");
    }
}
