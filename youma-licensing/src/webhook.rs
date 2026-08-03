use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::{crypto, db, models::Webhook, AppState};

/// Deliver a webhook event to all registered hooks subscribing to `event`.
///
/// Delivery records are written to `webhook_deliveries` **before** the background
/// task is spawned, so every attempt is tracked even if the process dies mid-flight.
/// The background task then attempts immediate delivery and updates each record.
pub fn fire_event(
    state: Arc<AppState>,
    event: &'static str,
    license_key: Option<String>,
    org_id: Option<String>,
    details: Value,
) {
    let ts = chrono::Utc::now().to_rfc3339();

    let body = json!({
        "event": event,
        "ts": ts,
        "license_key": license_key,
        "org_id": org_id,
        "details": details,
    });
    let body_str = body.to_string();

    // ── Create delivery records + collect targets (lock held briefly) ──────────
    let deliveries: Vec<(String, Webhook)> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => {
                error!("[webhook] DB lock poisoned, cannot create delivery records for event={}", event);
                return;
            }
        };

        let webhooks = match db::get_webhooks_for_event(&db, event) {
            Ok(w) => w,
            Err(e) => {
                error!("[webhook] failed to load webhooks for event={}: {}", event, e);
                return;
            }
        };

        if webhooks.is_empty() {
            return;
        }

        webhooks
            .into_iter()
            .filter_map(|w| {
                let delivery_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = db::insert_webhook_delivery(&db, &delivery_id, &w.id, event, &body_str, &ts) {
                    error!(webhook_id = %w.id, error = %e, "[webhook] failed to create delivery record");
                    return None;
                }
                Some((delivery_id, w))
            })
            .collect()
    };

    if deliveries.is_empty() {
        return;
    }

    // ── Attempt delivery in background ────────────────────────────────────────
    tokio::spawn(async move {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        for (delivery_id, webhook) in deliveries {
            deliver(&client, &webhook, event, &delivery_id, &body_str, &state).await;
        }
    });
}

/// Attempt delivery of one record; update `webhook_deliveries` and `webhooks` tables.
async fn deliver(
    client: &Client,
    webhook: &Webhook,
    event: &str,
    delivery_id: &str,
    body_str: &str,
    state: &Arc<AppState>,
) {
    let signature = format!("sha256={}", crypto::hmac_sign(&webhook.secret, body_str));
    let now = chrono::Utc::now().to_rfc3339();

    info!(
        webhook_id = %webhook.id,
        url = %webhook.url,
        event = %event,
        delivery_id = %delivery_id,
        "[webhook] delivering"
    );

    let result = client
        .post(&webhook.url)
        .header("Content-Type", "application/json")
        .header("X-Youma-Event", event)
        .header("X-Youma-Signature", &signature)
        .header("X-Youma-Delivery", delivery_id)
        .body(body_str.to_string())
        .send()
        .await;

    match result {
        Ok(resp) => {
            let status = resp.status().as_u16() as i64;
            let success = resp.status().is_success();

            if success {
                info!(webhook_id = %webhook.id, status = status, delivery_id = %delivery_id, "[webhook] delivery succeeded");
            } else {
                warn!(webhook_id = %webhook.id, status = status, delivery_id = %delivery_id, "[webhook] delivery failed with non-2xx");
            }

            if let Ok(db) = state.db.lock() {
                if success {
                    let _ = db::mark_delivery_success(&db, delivery_id, status, &now);
                } else {
                    // Will be retried by scheduler
                    let _ = db::mark_delivery_failed(&db, delivery_id, &next_retry_at(1));
                }

                // Keep the webhooks table counters for admin UI compatibility
                let _ = db.execute(
                    "UPDATE webhooks SET last_delivery_at = ?1, last_status = ?2,
                     failure_count = CASE WHEN ?3 THEN 0 ELSE failure_count + 1 END
                     WHERE id = ?4",
                    rusqlite::params![now, status, success as i32, webhook.id],
                );

                let _ = db::insert_audit(
                    &db,
                    db::AuditParams {
                        event: "webhook.delivered",
                        license_key: None,
                        org_id: webhook.org_id.as_deref(),
                        pool_id: None,
                        machine_id_hash: None,
                        ip: None,
                        user_agent: None,
                        admin_id: None,
                        details: json!({
                            "webhook_id": webhook.id,
                            "url": webhook.url,
                            "event": event,
                            "delivery_id": delivery_id,
                            "status": status,
                            "success": success,
                        }),
                    },
                );
            }
        }
        Err(e) => {
            error!(
                webhook_id = %webhook.id,
                url = %webhook.url,
                delivery_id = %delivery_id,
                error = %e,
                "[webhook] delivery network error"
            );
            if let Ok(db) = state.db.lock() {
                let _ = db::mark_delivery_failed(&db, delivery_id, &next_retry_at(1));
                let _ = db.execute(
                    "UPDATE webhooks SET last_delivery_at = ?1, failure_count = failure_count + 1 WHERE id = ?2",
                    rusqlite::params![now, webhook.id],
                );
            }
        }
    }
}

/// Retry pending deliveries — replays the **original event payload** stored in the delivery record.
/// Called every minute by the scheduler. Uses exponential backoff: 2^attempts minutes.
pub async fn retry_failed_webhooks(state: Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();

    let pending = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return,
        };
        match db::list_pending_deliveries(&db, &now) {
            Ok(v) => v,
            Err(_) => return,
        }
    };

    if pending.is_empty() {
        return;
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    for delivery in pending {
        // Load the webhook — may have been deleted
        let webhook = {
            let db = match state.db.lock() {
                Ok(d) => d,
                Err(_) => continue,
            };
            match db::get_webhook(&db, &delivery.webhook_id) {
                Ok(Some(w)) => w,
                _ => continue,
            }
        };

        if !webhook.active {
            continue;
        }

        info!(
            delivery_id = %delivery.id,
            webhook_id = %webhook.id,
            attempts = delivery.attempts,
            event = %delivery.event,
            "[webhook] retrying failed delivery"
        );

        // Deliver using the stored original payload
        deliver(&client, &webhook, &delivery.event, &delivery.id, &delivery.payload, &state).await;
    }
}

/// Compute the timestamp of the next retry using exponential backoff: 2^attempts minutes.
fn next_retry_at(attempts: i64) -> String {
    let backoff_mins = 2_i64.saturating_pow(attempts as u32).min(60);
    (chrono::Utc::now() + chrono::Duration::minutes(backoff_mins)).to_rfc3339()
}
