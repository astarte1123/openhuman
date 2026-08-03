use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::{
    auth::AuthenticatedAdmin,
    db::{self, AuditParams},
    error::{AppError, AppResult},
    models::*,
    webhook,
    AppState,
};

pub async fn list_licenses(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<LicenseFilterParams>,
) -> AppResult<Json<PagedResponse<serde_json::Value>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 200);

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let (licenses, total) = db::list_licenses(
        &db,
        page,
        limit,
        params.key.as_deref(),
        params.org.as_deref(),
        params.status.as_deref(),
        params.model.as_deref(),
    )?;

    // Build org_id → org_name lookup for unique orgs in the result set
    let mut org_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let unique_orgs: std::collections::HashSet<&str> =
        licenses.iter().map(|l| l.org_id.as_str()).collect();
    for org_id in unique_orgs {
        if let Ok(Some(org)) = db::get_org(&db, org_id) {
            org_names.insert(org_id.to_string(), org.name);
        }
    }

    let enriched: Vec<serde_json::Value> = licenses
        .into_iter()
        .map(|l| {
            let org_name = org_names.get(&l.org_id).cloned().unwrap_or_default();
            let mut v = serde_json::to_value(&l).unwrap_or(json!({}));
            if let Some(obj) = v.as_object_mut() {
                obj.insert("org_name".to_string(), json!(org_name));
            }
            v
        })
        .collect();

    Ok(Json(PagedResponse::new(enriched, total, page, limit)))
}

pub async fn get_license(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    // Audit trail for this license (last 50 entries)
    let (audit_entries, _) = db::list_audit(&db, 1, 50, Some(&key), None, None, None, None)?;

    Ok(Json(json!({
        "license": license,
        "audit": audit_entries,
    })))
}

pub async fn suspend_license(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<SuspendRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    if license.status == "revoked" {
        return Err(AppError::Conflict(
            "Cannot suspend a revoked license".to_string(),
        ));
    }
    if license.status == "suspended" {
        return Err(AppError::Conflict("License is already suspended".to_string()));
    }

    db.execute(
        "UPDATE licenses SET status = 'suspended' WHERE key = ?1",
        rusqlite::params![key],
    )?;

    // Decrement pool seat if was active
    if license.status == "active" {
        db.execute(
            "UPDATE pools SET used_seats = MAX(0, used_seats - 1) WHERE id = ?1",
            rusqlite::params![license.pool_id],
        )?;
    }

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.suspended",
            license_key: Some(&key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: license.machine_id_hash.as_deref(),
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "reason": req.reason }),
        },
    )?;

    drop(db);

    webhook::fire_event(
        Arc::clone(&state),
        "license.suspended",
        Some(key.clone()),
        Some(license.org_id.clone()),
        json!({ "reason": req.reason }),
    );

    tracing::info!(key = %key, admin = %admin.0.username, "[admin/licenses] suspended");
    Ok(Json(json!({ "ok": true })))
}

pub async fn unsuspend_license(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    if license.status != "suspended" {
        return Err(AppError::Conflict("License is not suspended".to_string()));
    }

    db.execute(
        "UPDATE licenses SET status = 'active' WHERE key = ?1",
        rusqlite::params![key],
    )?;

    // Re-increment pool seat
    db.execute(
        "UPDATE pools SET used_seats = used_seats + 1 WHERE id = ?1",
        rusqlite::params![license.pool_id],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.unsuspended",
            license_key: Some(&key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: license.machine_id_hash.as_deref(),
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({}),
        },
    )?;

    tracing::info!(key = %key, admin = %admin.0.username, "[admin/licenses] unsuspended");
    Ok(Json(json!({ "ok": true })))
}

pub async fn revoke_license(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<RevokeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    if license.status == "revoked" {
        return Err(AppError::Conflict("License is already revoked".to_string()));
    }

    let now = chrono::Utc::now().to_rfc3339();

    db.execute(
        "UPDATE licenses SET status = 'revoked', revoked_at = ?1, revoke_reason = ?2 WHERE key = ?3",
        rusqlite::params![now, req.reason, key],
    )?;

    // Decrement pool used_seats if was active
    if license.status == "active" {
        db.execute(
            "UPDATE pools SET used_seats = MAX(0, used_seats - 1) WHERE id = ?1",
            rusqlite::params![license.pool_id],
        )?;
    }

    // Remove any floating checkouts
    db.execute(
        "DELETE FROM floating_checkouts WHERE license_key = ?1",
        rusqlite::params![key],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.revoked",
            license_key: Some(&key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: license.machine_id_hash.as_deref(),
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "reason": req.reason }),
        },
    )?;

    drop(db);

    webhook::fire_event(
        Arc::clone(&state),
        "license.revoked",
        Some(key.clone()),
        Some(license.org_id.clone()),
        json!({ "reason": req.reason }),
    );

    tracing::info!(key = %key, admin = %admin.0.username, "[admin/licenses] revoked");
    Ok(Json(json!({ "ok": true })))
}

pub async fn extend_license(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<ExtendRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.days < 1 || req.days > 36500 {
        return Err(AppError::BadRequest(
            "days must be between 1 and 36500".to_string(),
        ));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    if license.status == "revoked" {
        return Err(AppError::Conflict("Cannot extend a revoked license".to_string()));
    }

    // Extend from current expiry, or from now if expired/no expiry
    let base = license
        .expires_at
        .as_deref()
        .and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .filter(|&dt| dt > chrono::Utc::now())
        .unwrap_or_else(chrono::Utc::now);

    let new_expires = (base + chrono::Duration::days(req.days)).to_rfc3339();

    db.execute(
        "UPDATE licenses SET expires_at = ?1, status = CASE WHEN status = 'expired' THEN 'active' ELSE status END WHERE key = ?2",
        rusqlite::params![new_expires, key],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.extended",
            license_key: Some(&key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: license.machine_id_hash.as_deref(),
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "days": req.days, "new_expires_at": new_expires }),
        },
    )?;

    drop(db);

    webhook::fire_event(
        Arc::clone(&state),
        "license.extended",
        Some(key.clone()),
        Some(license.org_id.clone()),
        json!({ "days": req.days, "new_expires_at": new_expires }),
    );

    tracing::info!(key = %key, days = req.days, admin = %admin.0.username, "[admin/licenses] extended");
    Ok(Json(json!({ "ok": true, "new_expires_at": new_expires })))
}

pub async fn transfer_license(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &key)?
        .ok_or_else(|| AppError::NotFound(format!("License {} not found", key)))?;

    if license.status == "revoked" {
        return Err(AppError::Conflict(
            "Cannot transfer a revoked license".to_string(),
        ));
    }

    let old_hash = license.machine_id_hash.clone();

    db.execute(
        "UPDATE licenses SET machine_id_hash = NULL, status = 'inactive', transfer_count = transfer_count + 1 WHERE key = ?1",
        rusqlite::params![key],
    )?;

    // Remove any floating checkouts
    db.execute(
        "DELETE FROM floating_checkouts WHERE license_key = ?1",
        rusqlite::params![key],
    )?;

    db.execute(
        "UPDATE pools SET used_seats = MAX(0, used_seats - 1) WHERE id = ?1",
        rusqlite::params![license.pool_id],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.transferred",
            license_key: Some(&key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: old_hash.as_deref(),
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "admin_transfer": true }),
        },
    )?;

    tracing::info!(key = %key, admin = %admin.0.username, "[admin/licenses] transferred");
    Ok(Json(json!({ "ok": true })))
}

pub async fn set_machine_label(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<SetMachineLabelRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let rows = db.execute(
        "UPDATE licenses SET machine_label = ?1 WHERE key = ?2",
        rusqlite::params![req.label, key],
    )?;

    if rows == 0 {
        return Err(AppError::NotFound(format!("License {} not found", key)));
    }

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.label_set",
            license_key: Some(&key),
            org_id: None,
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "label": req.label }),
        },
    )?;

    Ok(Json(json!({ "ok": true })))
}

pub async fn set_max_transfers(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<SetMaxTransfersRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.max < 0 || req.max > 100 {
        return Err(AppError::BadRequest(
            "max must be between 0 and 100".to_string(),
        ));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let rows = db.execute(
        "UPDATE licenses SET max_transfers = ?1 WHERE key = ?2",
        rusqlite::params![req.max, key],
    )?;

    if rows == 0 {
        return Err(AppError::NotFound(format!("License {} not found", key)));
    }

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.max_transfers_set",
            license_key: Some(&key),
            org_id: None,
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "max": req.max }),
        },
    )?;

    Ok(Json(json!({ "ok": true })))
}
