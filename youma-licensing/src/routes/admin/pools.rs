use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
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

pub async fn list_pools(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PoolFilterParams>,
) -> AppResult<Json<PagedResponse<Pool>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let (pools, total) = db::list_pools(
        &db,
        page,
        limit,
        params.org_id.as_deref(),
        params.model.as_deref(),
        params.plan.as_deref(),
    )?;

    Ok(Json(PagedResponse::new(pools, total, page, limit)))
}

pub async fn create_pool(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePoolRequest>,
) -> AppResult<Json<Pool>> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Pool name is required".to_string()));
    }
    if req.total_seats < 1 {
        return Err(AppError::BadRequest(
            "total_seats must be at least 1".to_string(),
        ));
    }

    let valid_models = ["seat", "floating", "trial", "subscription", "perpetual", "oem"];
    if !valid_models.contains(&req.license_model.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid license_model. Must be one of: {}",
            valid_models.join(", ")
        )));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    // Verify org exists
    db::get_org(&db, &req.org_id)?
        .ok_or_else(|| AppError::NotFound(format!("Organization {} not found", req.org_id)))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let features = req
        .features
        .unwrap_or(json!(["meetings", "ai", "inbox"]))
        .to_string();
    let metadata = req.metadata.unwrap_or(json!({})).to_string();
    let plan = req.plan.unwrap_or_else(|| "pro".to_string());
    let grace_period_hours = req.grace_period_hours.unwrap_or(72);

    db.execute(
        "INSERT INTO pools (id, org_id, name, license_model, total_seats, plan, duration_days,
                            maintenance_days, grace_period_hours, concurrent_limit, features,
                            created_at, created_by, notes, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            id, req.org_id, req.name, req.license_model, req.total_seats, plan,
            req.duration_days, req.maintenance_days, grace_period_hours,
            req.concurrent_limit, features, now, admin.0.id, req.notes, metadata
        ],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "pool.created",
            license_key: None,
            org_id: Some(&req.org_id),
            pool_id: Some(&id),
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "name": req.name, "model": req.license_model, "seats": req.total_seats }),
        },
    )?;

    tracing::info!(pool_id = %id, admin = %admin.0.username, "[admin/pools] created");

    let pool = db::get_pool(&db, &id)?
        .ok_or_else(|| AppError::Internal("Failed to load created pool".to_string()))?;
    Ok(Json(pool))
}

pub async fn get_pool(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let pool = db::get_pool(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Pool {} not found", id)))?;

    // First 10 keys preview
    let mut stmt = db.prepare(
        "SELECT key, status, machine_id_hash, activated_at, expires_at FROM licenses WHERE pool_id = ?1 ORDER BY created_at DESC LIMIT 10",
    )?;
    let preview: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok(json!({
                "key": row.get::<_,String>(0)?,
                "status": row.get::<_,String>(1)?,
                "machine_id_hash": row.get::<_,Option<String>>(2)?,
                "activated_at": row.get::<_,Option<String>>(3)?,
                "expires_at": row.get::<_,Option<String>>(4)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(json!({
        "pool": pool,
        "key_preview": preview,
    })))
}

pub async fn generate_keys(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<GenerateKeysRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.count == 0 || req.count > 10_000 {
        return Err(AppError::BadRequest(
            "count must be between 1 and 10000".to_string(),
        ));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let pool = db::get_pool(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Pool {} not found", id)))?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut keys = Vec::with_capacity(req.count as usize);

    // Use a transaction for bulk insert
    let tx = db.unchecked_transaction()?;
    for _ in 0..req.count {
        let key = generate_unique_key(&tx)?;
        tx.execute(
            "INSERT INTO licenses (key, pool_id, org_id, license_model, status, created_at, metadata)
             VALUES (?1, ?2, ?3, ?4, 'inactive', ?5, '{}')",
            rusqlite::params![key, id, pool.org_id, pool.license_model, now],
        )?;
        keys.push(key);
    }
    tx.commit()?;

    // Update total_seats if needed
    db.execute(
        "UPDATE pools SET total_seats = total_seats + ?1 WHERE id = ?2",
        rusqlite::params![req.count as i64, id],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "pool.keys_generated",
            license_key: None,
            org_id: Some(&pool.org_id),
            pool_id: Some(&id),
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "count": req.count }),
        },
    )?;

    tracing::info!(
        pool_id = %id,
        count = req.count,
        admin = %admin.0.username,
        "[admin/pools] generated keys"
    );

    Ok(Json(json!({
        "generated": keys.len(),
        "keys": keys,
    })))
}

pub async fn list_pool_keys(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<PagedResponse<License>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = (page.saturating_sub(1)) * limit;

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    db::get_pool(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Pool {} not found", id)))?;

    let total: i64 = db.query_row(
        "SELECT COUNT(*) FROM licenses WHERE pool_id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;

    let mut stmt = db.prepare(
        "SELECT key, pool_id, org_id, license_model, status, machine_id_hash, machine_label,
                email, activated_at, expires_at, maintenance_expires_at, last_validated_at,
                revoked_at, revoke_reason, transfer_count, max_transfers, created_at, metadata
         FROM licenses WHERE pool_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
    )?;

    let rows: Vec<License> = stmt
        .query_map(rusqlite::params![id, limit, offset], |row| {
            let metadata_str: String = row.get(17)?;
            let metadata =
                serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
            Ok(License {
                key: row.get(0)?,
                pool_id: row.get(1)?,
                org_id: row.get(2)?,
                license_model: row.get(3)?,
                status: row.get(4)?,
                machine_id_hash: row.get(5)?,
                machine_label: row.get(6)?,
                email: row.get(7)?,
                activated_at: row.get(8)?,
                expires_at: row.get(9)?,
                maintenance_expires_at: row.get(10)?,
                last_validated_at: row.get(11)?,
                revoked_at: row.get(12)?,
                revoke_reason: row.get(13)?,
                transfer_count: row.get(14)?,
                max_transfers: row.get(15)?,
                created_at: row.get(16)?,
                metadata,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(PagedResponse::new(rows, total, page, limit)))
}

pub async fn export_pool_keys(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    db::get_pool(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Pool {} not found", id)))?;

    let mut stmt = db.prepare(
        "SELECT key, status, email, activated_at, expires_at, machine_id_hash, transfer_count
         FROM licenses WHERE pool_id = ?1 ORDER BY created_at",
    )?;

    let mut csv = String::from("key,status,email,activated_at,expires_at,machine_id_hash,transfer_count\n");
    let rows = stmt.query_map(rusqlite::params![id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;

    for row in rows.flatten() {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            row.0,
            row.1,
            row.2.unwrap_or_default(),
            row.3.unwrap_or_default(),
            row.4.unwrap_or_default(),
            row.5.unwrap_or_default(),
            row.6
        ));
    }

    let content_disposition = format!("attachment; filename=\"pool-{}-keys.csv\"", id);
    let mut response_headers = HeaderMap::new();
    response_headers.insert("Content-Type", "text/csv".parse().unwrap());
    response_headers.insert(
        "Content-Disposition",
        content_disposition.parse().unwrap(),
    );

    Ok((response_headers, csv))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Generate a license key guaranteed to be unique in the DB. Retries up to 10 times
/// on the (astronomically unlikely) event of a collision.
fn generate_unique_key(conn: &rusqlite::Connection) -> crate::error::AppResult<String> {
    for _ in 0..10 {
        let key = crypto::generate_license_key();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM licenses WHERE key = ?1",
                rusqlite::params![key],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);
        if !exists {
            return Ok(key);
        }
        tracing::warn!(key = %key, "[pools] key collision, regenerating");
    }
    Err(crate::error::AppError::Internal(
        "Failed to generate a unique license key after 10 attempts".to_string(),
    ))
}
