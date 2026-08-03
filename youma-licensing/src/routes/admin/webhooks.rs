use axum::{
    extract::{Path, State},
    Json,
};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

use crate::{
    auth::AuthenticatedAdmin,
    crypto,
    db::{self, AuditParams},
    error::{AppError, AppResult},
    models::*,
    AppState,
};

pub async fn list_webhooks(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Webhook>>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let mut webhooks = db::list_webhooks(&db)?;
    // Mask secrets before returning
    for w in &mut webhooks {
        w.secret = "***".to_string();
    }
    Ok(Json(webhooks))
}

pub async fn create_webhook(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateWebhookRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.url.is_empty() {
        return Err(AppError::BadRequest("url is required".to_string()));
    }
    if !req.url.starts_with("https://") && !req.url.starts_with("http://") {
        return Err(AppError::BadRequest(
            "url must start with http:// or https://".to_string(),
        ));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let secret = req
        .secret
        .unwrap_or_else(|| crypto::generate_hex_secret(32));
    let events = req
        .events
        .unwrap_or(json!(["license.activated", "license.expired", "license.suspended"]))
        .to_string();

    db.execute(
        "INSERT INTO webhooks (id, org_id, url, events, secret, active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        rusqlite::params![id, req.org_id, req.url, events, secret, now],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "webhook.created",
            license_key: None,
            org_id: req.org_id.as_deref(),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "url": req.url }),
        },
    )?;

    tracing::info!(webhook_id = %id, url = %req.url, admin = %admin.0.username, "[admin/webhooks] created");

    Ok(Json(json!({
        "id": id,
        "url": req.url,
        "secret": secret,  // Only returned on creation
        "created_at": now,
    })))
}

pub async fn update_webhook(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateWebhookRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let existing = db::get_webhook(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Webhook {} not found", id)))?;

    let url = req.url.unwrap_or(existing.url);
    let events = req
        .events
        .map(|e| e.to_string())
        .unwrap_or(existing.events.to_string());
    let active = req.active.unwrap_or(existing.active) as i64;

    db.execute(
        "UPDATE webhooks SET url = ?1, events = ?2, active = ?3 WHERE id = ?4",
        rusqlite::params![url, events, active, id],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "webhook.updated",
            license_key: None,
            org_id: existing.org_id.as_deref(),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "url": url, "active": active }),
        },
    )?;

    Ok(Json(json!({ "ok": true })))
}

pub async fn test_webhook(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let webhook = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::Internal("DB lock".to_string()))?;
        db::get_webhook(&db, &id)?
            .ok_or_else(|| AppError::NotFound(format!("Webhook {} not found", id)))?
    };

    let delivery_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let body = json!({
        "event": "webhook.test",
        "ts": now,
        "license_key": null,
        "org_id": webhook.org_id,
        "details": { "test": true, "triggered_by": admin.0.username },
    });
    let body_str = body.to_string();
    let signature = format!("sha256={}", crypto::hmac_sign(&webhook.secret, &body_str));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client error: {}", e)))?;

    let result = client
        .post(&webhook.url)
        .header("Content-Type", "application/json")
        .header("X-Youma-Event", "webhook.test")
        .header("X-Youma-Signature", &signature)
        .header("X-Youma-Delivery", &delivery_id)
        .body(body_str)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Delivery failed: {}", e)))?;

    let status = result.status().as_u16();
    let success = result.status().is_success();

    tracing::info!(
        webhook_id = %id,
        status = status,
        success = success,
        admin = %admin.0.username,
        "[admin/webhooks] test delivery"
    );

    Ok(Json(json!({
        "ok": success,
        "status": status,
        "delivery_id": delivery_id,
    })))
}

pub async fn delete_webhook(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let webhook = db::get_webhook(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Webhook {} not found", id)))?;

    db.execute("DELETE FROM webhooks WHERE id = ?1", rusqlite::params![id])?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "webhook.deleted",
            license_key: None,
            org_id: webhook.org_id.as_deref(),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "url": webhook.url }),
        },
    )?;

    tracing::info!(webhook_id = %id, admin = %admin.0.username, "[admin/webhooks] deleted");
    Ok(Json(json!({ "ok": true })))
}
