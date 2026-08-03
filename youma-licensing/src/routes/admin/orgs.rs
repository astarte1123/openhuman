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
    AppState,
};

pub async fn list_orgs(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<PagedResponse<Organization>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let (orgs, total) = db::list_orgs(&db, page, limit, params.q.as_deref())?;
    Ok(Json(PagedResponse::new(orgs, total, page, limit)))
}

pub async fn create_org(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateOrgRequest>,
) -> AppResult<Json<Organization>> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name is required".to_string()));
    }

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let metadata = req.metadata.unwrap_or(json!({})).to_string();
    let reseller = req.reseller.unwrap_or(false) as i64;

    db.execute(
        "INSERT INTO organizations (id, name, email, country, reseller, parent_org_id, metadata, created_at, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id, req.name, req.email, req.country, reseller, req.parent_org_id, metadata, now, req.notes
        ],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "org.created",
            license_key: None,
            org_id: Some(&id),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "name": req.name }),
        },
    )?;

    tracing::info!(org_id = %id, admin = %admin.0.username, "[admin/orgs] created");

    let org = db::get_org(&db, &id)?
        .ok_or_else(|| AppError::Internal("Failed to load created org".to_string()))?;

    Ok(Json(org))
}

pub async fn get_org(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let org = db::get_org(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Organization {} not found", id)))?;

    // Load associated pools
    let (pools, _) = db::list_pools(&db, 1, 100, Some(&id), None, None)?;

    Ok(Json(json!({
        "org": org,
        "pools": pools,
    })))
}

pub async fn update_org(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> AppResult<Json<Organization>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let existing = db::get_org(&db, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Organization {} not found", id)))?;

    let name = req.name.unwrap_or(existing.name);
    let email = req.email.or(existing.email);
    let country = req.country.or(existing.country);
    let reseller = req.reseller.unwrap_or(existing.reseller) as i64;
    let notes = req.notes.or(existing.notes);
    let metadata = req
        .metadata
        .map(|m| m.to_string())
        .unwrap_or(existing.metadata.to_string());

    db.execute(
        "UPDATE organizations SET name = ?1, email = ?2, country = ?3, reseller = ?4, notes = ?5, metadata = ?6 WHERE id = ?7",
        rusqlite::params![name, email, country, reseller, notes, metadata, id],
    )?;

    db::insert_audit(
        &db,
        AuditParams {
            event: "org.updated",
            license_key: None,
            org_id: Some(&id),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({ "name": name }),
        },
    )?;

    let org = db::get_org(&db, &id)?
        .ok_or_else(|| AppError::Internal("Failed to reload org".to_string()))?;
    Ok(Json(org))
}

pub async fn delete_org(
    admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    // Check for active licenses
    let active: i64 = db.query_row(
        "SELECT COUNT(*) FROM licenses WHERE org_id = ?1 AND status = 'active'",
        rusqlite::params![id],
        |r| r.get(0),
    )?;

    if active > 0 {
        return Err(AppError::Conflict(format!(
            "Cannot delete organization with {} active license(s)",
            active
        )));
    }

    let rows = db.execute(
        "DELETE FROM organizations WHERE id = ?1",
        rusqlite::params![id],
    )?;

    if rows == 0 {
        return Err(AppError::NotFound(format!(
            "Organization {} not found",
            id
        )));
    }

    db::insert_audit(
        &db,
        AuditParams {
            event: "org.deleted",
            license_key: None,
            org_id: Some(&id),
            pool_id: None,
            machine_id_hash: None,
            ip: None,
            user_agent: None,
            admin_id: Some(&admin.0.id),
            details: json!({}),
        },
    )?;

    tracing::info!(org_id = %id, admin = %admin.0.username, "[admin/orgs] deleted");
    Ok(Json(json!({ "ok": true })))
}
