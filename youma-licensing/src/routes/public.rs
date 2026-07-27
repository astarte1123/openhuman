use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};

use crate::{
    crypto,
    db::{self, AuditParams},
    error::{AppError, AppResult},
    models::*,
    rate_limit::RateLimited,
    webhook,
    AppState,
};

// ─── Input validation ─────────────────────────────────────────────────────────

fn validate_license_key(key: &str) -> AppResult<()> {
    if key.is_empty() || key.len() > 64 {
        return Err(AppError::BadRequest("key must be 1–64 characters".to_string()));
    }
    Ok(())
}

fn validate_machine_id(machine_id: &str) -> AppResult<()> {
    if machine_id.is_empty() || machine_id.len() > 512 {
        return Err(AppError::BadRequest("machine_id must be 1–512 characters".to_string()));
    }
    Ok(())
}

fn extract_ip_from_headers(headers: &HeaderMap, addr: Option<SocketAddr>) -> Option<String> {
    if let Some(xff) = headers.get("X-Forwarded-For") {
        if let Ok(val) = xff.to_str() {
            if let Some(first) = val.split(',').next() {
                return Some(first.trim().to_string());
            }
        }
    }
    if let Some(xri) = headers.get("X-Real-IP") {
        if let Ok(val) = xri.to_str() {
            return Some(val.trim().to_string());
        }
    }
    addr.map(|a| a.ip().to_string())
}

fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ─── POST /api/v1/license/activate ───────────────────────────────────────────

pub async fn activate(
    _rl: RateLimited,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ActivateRequest>,
) -> AppResult<Json<ActivateResponse>> {
    validate_license_key(&req.key)?;
    validate_machine_id(&req.machine_id)?;

    let ip = extract_ip_from_headers(&headers, Some(addr));
    let ua = extract_user_agent(&headers);
    let machine_hash = crypto::hash_machine_id(&req.machine_id);

    info!(
        key = %req.key,
        machine_hash = %machine_hash,
        ip = ?ip,
        "[activate] request received"
    );

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    // Load license
    let license = db::get_license(&db, &req.key)?
        .ok_or_else(|| AppError::NotFound("License key not found".to_string()))?;

    // Reject if revoked or suspended
    match license.status.as_str() {
        "revoked" => return Err(AppError::Forbidden("License has been revoked".to_string())),
        "suspended" => return Err(AppError::Forbidden("License is suspended".to_string())),
        _ => {}
    }

    // Load pool
    let pool = db::get_pool(&db, &license.pool_id)?
        .ok_or_else(|| AppError::Internal("Pool not found".to_string()))?;

    let now = Utc::now().to_rfc3339();

    match pool.license_model.as_str() {
        "seat" => {
            activate_seat(&db, &license, &pool, &machine_hash, &req.email, &now)?;
        }
        "floating" => {
            activate_floating(&db, &license, &pool, &machine_hash, &req.key, &now)?;
        }
        "trial" => {
            // Trial activation — machine must not already have a trial
            if let Some(existing_hash) = &license.machine_id_hash {
                if existing_hash != &machine_hash {
                    return Err(AppError::Conflict(
                        "This trial key is already locked to another machine".to_string(),
                    ));
                }
            } else {
                activate_seat(&db, &license, &pool, &machine_hash, &req.email, &now)?;
            }
        }
        "subscription" | "perpetual" => {
            activate_seat(&db, &license, &pool, &machine_hash, &req.email, &now)?;
        }
        "oem" => {
            activate_seat(&db, &license, &pool, &machine_hash, &req.email, &now)?;
        }
        model => {
            return Err(AppError::BadRequest(format!(
                "Unknown license model: {}",
                model
            )));
        }
    }

    // Reload after update
    let updated = db::get_license(&db, &req.key)?
        .ok_or_else(|| AppError::Internal("License disappeared after activation".to_string()))?;

    // Build HMAC signature
    let issued_at = &now;
    let signature = crypto::build_validate_signature(
        &state.config.signing_secret,
        true,
        &pool.plan,
        updated.expires_at.as_deref(),
        issued_at,
    );

    // Audit
    db::insert_audit(
        &db,
        AuditParams {
            event: "license.activated",
            license_key: Some(&req.key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: Some(&machine_hash),
            ip: ip.as_deref(),
            user_agent: ua.as_deref(),
            admin_id: None,
            details: json!({
                "model": pool.license_model,
                "email": req.email,
            }),
        },
    )?;

    drop(db);

    // Fire webhook
    webhook::fire_event(
        Arc::clone(&state),
        "license.activated",
        Some(req.key.clone()),
        Some(license.org_id.clone()),
        json!({ "model": pool.license_model, "plan": pool.plan }),
    );

    info!(key = %req.key, "[activate] success");

    Ok(Json(ActivateResponse {
        ok: true,
        plan: pool.plan,
        features: pool.features,
        expires_at: updated.expires_at,
        maintenance_expires_at: updated.maintenance_expires_at,
        license_model: pool.license_model,
        signature,
    }))
}

fn activate_seat(
    db: &rusqlite::Connection,
    license: &License,
    pool: &Pool,
    machine_hash: &str,
    email: &Option<String>,
    now: &str,
) -> AppResult<()> {
    // Check if already activated on a different machine
    if let Some(existing_hash) = &license.machine_id_hash {
        if existing_hash != machine_hash {
            if license.transfer_count >= license.max_transfers {
                return Err(AppError::Conflict(
                    "License is locked to another machine and no transfers remain".to_string(),
                ));
            }
            // Allow transfer — increment counter, rebind
            warn!(
                key = %license.key,
                transfer_count = license.transfer_count + 1,
                "[activate] machine transfer"
            );
        } else if license.status == "active" {
            // Same machine, already active — idempotent OK
            return Ok(());
        }
    } else {
        // First activation — check pool capacity
        if pool.used_seats >= pool.total_seats {
            return Err(AppError::Conflict("Pool has no available seats".to_string()));
        }
    }

    // Calculate expiry
    let expires_at = pool.duration_days.map(|d| {
        (chrono::Utc::now() + chrono::Duration::days(d)).to_rfc3339()
    });
    let maintenance_expires_at = pool.maintenance_days.map(|d| {
        (chrono::Utc::now() + chrono::Duration::days(d)).to_rfc3339()
    });

    let is_transfer = license.machine_id_hash.is_some()
        && license.machine_id_hash.as_deref() != Some(machine_hash);

    db.execute(
        "UPDATE licenses SET status = 'active', machine_id_hash = ?1, email = COALESCE(?2, email),
         activated_at = ?3, expires_at = COALESCE(?4, expires_at),
         maintenance_expires_at = COALESCE(?5, maintenance_expires_at),
         transfer_count = transfer_count + ?6
         WHERE key = ?7",
        rusqlite::params![
            machine_hash,
            email.as_deref(),
            now,
            expires_at,
            maintenance_expires_at,
            if is_transfer { 1i64 } else { 0i64 },
            license.key,
        ],
    )?;

    // Only increment used_seats on first activation (not transfer)
    if license.machine_id_hash.is_none() {
        db.execute(
            "UPDATE pools SET used_seats = used_seats + 1 WHERE id = ?1",
            rusqlite::params![pool.id],
        )?;
    }

    Ok(())
}

fn activate_floating(
    db: &rusqlite::Connection,
    license: &License,
    pool: &Pool,
    machine_hash: &str,
    license_key: &str,
    now: &str,
) -> AppResult<()> {
    let concurrent_limit = pool.concurrent_limit.unwrap_or(pool.total_seats);

    // Check if this machine already has an active checkout (idempotent refresh)
    let existing_checkout: Option<String> = db
        .query_row(
            "SELECT id FROM floating_checkouts WHERE license_key = ?1 AND machine_id_hash = ?2",
            rusqlite::params![license_key, machine_hash],
            |r| r.get(0),
        )
        .ok();

    if let Some(checkout_id) = existing_checkout {
        // Refresh heartbeat
        let new_expires = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
        db.execute(
            "UPDATE floating_checkouts SET last_heartbeat_at = ?1, expires_at = ?2 WHERE id = ?3",
            rusqlite::params![now, new_expires, checkout_id],
        )?;
        return Ok(());
    }

    // Count active (non-expired) checkouts for this pool
    let now_str = chrono::Utc::now().to_rfc3339();
    let active_checkouts: i64 = db.query_row(
        "SELECT COUNT(*) FROM floating_checkouts fc
         JOIN licenses l ON l.key = fc.license_key
         WHERE l.pool_id = ?1 AND fc.expires_at > ?2",
        rusqlite::params![pool.id, now_str],
        |r| r.get(0),
    )?;

    if active_checkouts >= concurrent_limit {
        return Err(AppError::Conflict(
            "All floating license slots are in use".to_string(),
        ));
    }

    // Create checkout
    let checkout_id = uuid::Uuid::new_v4().to_string();
    let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();

    db.execute(
        "INSERT INTO floating_checkouts (id, license_key, machine_id_hash, checked_out_at, last_heartbeat_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
        rusqlite::params![checkout_id, license_key, machine_hash, now, expires_at],
    )?;

    // Update used_seats to current count + 1
    db.execute(
        "UPDATE pools SET used_seats = ?1 WHERE id = ?2",
        rusqlite::params![active_checkouts + 1, pool.id],
    )?;

    // Mark license active if not already
    if license.status != "active" {
        db.execute(
            "UPDATE licenses SET status = 'active', activated_at = ?1 WHERE key = ?2",
            rusqlite::params![now, license_key],
        )?;
    }

    Ok(())
}

// ─── POST /api/v1/license/validate ───────────────────────────────────────────

pub async fn validate(
    _rl: RateLimited,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ValidateRequest>,
) -> AppResult<Json<ValidateResponse>> {
    validate_license_key(&req.key)?;
    validate_machine_id(&req.machine_id)?;

    let ip = extract_ip_from_headers(&headers, Some(addr));
    let ua = extract_user_agent(&headers);
    let machine_hash = crypto::hash_machine_id(&req.machine_id);
    let issued_at = Utc::now().to_rfc3339();

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = match db::get_license(&db, &req.key)? {
        Some(l) => l,
        None => {
            let sig = crypto::build_validate_signature(
                &state.config.signing_secret,
                false,
                "unknown",
                None,
                &issued_at,
            );
            return Ok(Json(ValidateResponse {
                valid: false,
                reason: Some("key_not_found".to_string()),
                plan: None,
                features: None,
                expires_at: None,
                grace_until: None,
                signature: sig,
            }));
        }
    };

    let pool = db::get_pool(&db, &license.pool_id)?
        .ok_or_else(|| AppError::Internal("Pool not found".to_string()))?;

    let (valid, reason) = check_license_validity(&license, &pool, &machine_hash, &issued_at, &db);

    // Update last_validated_at (best-effort)
    let _ = db.execute(
        "UPDATE licenses SET last_validated_at = ?1 WHERE key = ?2",
        rusqlite::params![issued_at, req.key],
    );

    // For floating: refresh heartbeat
    if pool.license_model == "floating" && valid {
        let new_expires = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
        let _ = db.execute(
            "UPDATE floating_checkouts SET last_heartbeat_at = ?1, expires_at = ?2
             WHERE license_key = ?3 AND machine_id_hash = ?4",
            rusqlite::params![issued_at, new_expires, req.key, machine_hash],
        );
    }

    // Only log if status changed or machine changed (reduce noise)
    let should_audit = reason.is_some()
        || license.machine_id_hash.as_deref() != Some(&machine_hash)
        || license.last_validated_at.is_none();

    if should_audit {
        let _ = db::insert_audit(
            &db,
            AuditParams {
                event: "license.validated",
                license_key: Some(&req.key),
                org_id: Some(&license.org_id),
                pool_id: Some(&license.pool_id),
                machine_id_hash: Some(&machine_hash),
                ip: ip.as_deref(),
                user_agent: ua.as_deref(),
                admin_id: None,
                details: json!({ "valid": valid, "reason": reason }),
            },
        );
    }

    let grace_until = license.expires_at.as_ref().map(|exp| {
        chrono::DateTime::parse_from_rfc3339(exp)
            .ok()
            .map(|dt| {
                (dt.with_timezone(&chrono::Utc)
                    + chrono::Duration::hours(pool.grace_period_hours))
                .to_rfc3339()
            })
            .unwrap_or_default()
    });

    let signature = crypto::build_validate_signature(
        &state.config.signing_secret,
        valid,
        &pool.plan,
        license.expires_at.as_deref(),
        &issued_at,
    );

    Ok(Json(ValidateResponse {
        valid,
        reason,
        plan: Some(pool.plan),
        features: Some(pool.features),
        expires_at: license.expires_at,
        grace_until,
        signature,
    }))
}

fn check_license_validity(
    license: &License,
    pool: &Pool,
    machine_hash: &str,
    now: &str,
    db: &rusqlite::Connection,
) -> (bool, Option<String>) {
    match license.status.as_str() {
        "revoked" => return (false, Some("revoked".to_string())),
        "suspended" => return (false, Some("suspended".to_string())),
        "inactive" => return (false, Some("not_activated".to_string())),
        _ => {}
    }

    // Check machine binding
    if let Some(ref stored_hash) = license.machine_id_hash {
        if stored_hash != machine_hash && pool.license_model != "floating" {
            return (false, Some("machine_mismatch".to_string()));
        }
    }

    // Check floating: must have active checkout
    if pool.license_model == "floating" {
        let has_checkout: bool = db
            .query_row(
                "SELECT COUNT(*) FROM floating_checkouts WHERE license_key = ?1 AND machine_id_hash = ?2 AND expires_at > ?3",
                rusqlite::params![license.key, machine_hash, now],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_checkout {
            return (false, Some("no_floating_checkout".to_string()));
        }
    }

    // Check expiry
    if let Some(ref exp) = license.expires_at {
        if exp.as_str() < now {
            // Check grace period
            if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                let grace_end = exp_dt.with_timezone(&chrono::Utc)
                    + chrono::Duration::hours(pool.grace_period_hours);
                let now_dt = chrono::Utc::now();
                if now_dt <= grace_end {
                    return (true, Some("in_grace_period".to_string()));
                }
            }
            return (false, Some("expired".to_string()));
        }
    }

    (true, None)
}

// ─── POST /api/v1/license/deactivate ─────────────────────────────────────────

pub async fn deactivate(
    _rl: RateLimited,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<DeactivateRequest>,
) -> AppResult<Json<serde_json::Value>> {
    validate_license_key(&req.key)?;
    validate_machine_id(&req.machine_id)?;

    let ip = extract_ip_from_headers(&headers, Some(addr));
    let ua = extract_user_agent(&headers);
    let machine_hash = crypto::hash_machine_id(&req.machine_id);

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let license = db::get_license(&db, &req.key)?
        .ok_or_else(|| AppError::NotFound("License key not found".to_string()))?;

    let pool = db::get_pool(&db, &license.pool_id)?
        .ok_or_else(|| AppError::Internal("Pool not found".to_string()))?;

    // Verify machine binding
    match license.machine_id_hash.as_deref() {
        Some(h) if h != machine_hash => {
            return Err(AppError::Forbidden(
                "Machine ID does not match activated machine".to_string(),
            ));
        }
        None => {
            return Err(AppError::BadRequest(
                "License is not currently activated".to_string(),
            ));
        }
        _ => {}
    }

    if pool.license_model == "floating" {
        // Remove floating checkout
        db.execute(
            "DELETE FROM floating_checkouts WHERE license_key = ?1 AND machine_id_hash = ?2",
            rusqlite::params![req.key, machine_hash],
        )?;

        let active_checkouts: i64 = db.query_row(
            "SELECT COUNT(*) FROM floating_checkouts WHERE license_key = ?1",
            rusqlite::params![req.key],
            |r| r.get(0),
        )?;

        db.execute(
            "UPDATE pools SET used_seats = ?1 WHERE id = ?2",
            rusqlite::params![active_checkouts, pool.id],
        )?;
    } else {
        // Clear machine binding, increment transfer_count, set inactive
        db.execute(
            "UPDATE licenses SET machine_id_hash = NULL, status = 'inactive',
             transfer_count = transfer_count + 1 WHERE key = ?1",
            rusqlite::params![req.key],
        )?;

        db.execute(
            "UPDATE pools SET used_seats = MAX(0, used_seats - 1) WHERE id = ?1",
            rusqlite::params![pool.id],
        )?;
    }

    db::insert_audit(
        &db,
        AuditParams {
            event: "license.deactivated",
            license_key: Some(&req.key),
            org_id: Some(&license.org_id),
            pool_id: Some(&license.pool_id),
            machine_id_hash: Some(&machine_hash),
            ip: ip.as_deref(),
            user_agent: ua.as_deref(),
            admin_id: None,
            details: json!({ "model": pool.license_model }),
        },
    )?;

    info!(key = %req.key, machine_hash = %machine_hash, "[deactivate] success");

    Ok(Json(json!({ "ok": true })))
}

// ─── POST /api/v1/license/floating/heartbeat ─────────────────────────────────

pub async fn floating_heartbeat(
    _rl: RateLimited,
    State(state): State<Arc<AppState>>,
    Json(req): Json<HeartbeatRequest>,
) -> AppResult<Json<HeartbeatResponse>> {
    validate_license_key(&req.key)?;
    validate_machine_id(&req.machine_id)?;

    let machine_hash = crypto::hash_machine_id(&req.machine_id);
    let now = Utc::now().to_rfc3339();
    let new_expires = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let changed = db.execute(
        "UPDATE floating_checkouts SET last_heartbeat_at = ?1, expires_at = ?2
         WHERE license_key = ?3 AND machine_id_hash = ?4",
        rusqlite::params![now, new_expires, req.key, machine_hash],
    )?;

    if changed == 0 {
        return Err(AppError::NotFound(
            "No active floating checkout for this machine".to_string(),
        ));
    }

    tracing::debug!(key = %req.key, machine_hash = %machine_hash, "[heartbeat] refreshed");

    Ok(Json(HeartbeatResponse {
        ok: true,
        slot_expires_at: new_expires,
    }))
}

// ─── POST /api/v1/license/trial ───────────────────────────────────────────────

pub async fn start_trial(
    _rl: RateLimited,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TrialRequest>,
) -> AppResult<Json<ActivateResponse>> {
    validate_machine_id(&req.machine_id)?;

    let ip = extract_ip_from_headers(&headers, Some(addr));
    let ua = extract_user_agent(&headers);
    let machine_hash = crypto::hash_machine_id(&req.machine_id);
    let now = Utc::now().to_rfc3339();

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    // Check for existing trial on this machine
    let existing_trial: Option<String> = db
        .query_row(
            "SELECT key FROM licenses WHERE license_model = 'trial' AND machine_id_hash = ?1",
            rusqlite::params![machine_hash],
            |r| r.get(0),
        )
        .ok();

    if existing_trial.is_some() {
        return Err(AppError::Conflict(
            "A trial license already exists for this machine".to_string(),
        ));
    }

    // Find or create trial pool/org
    let trial_org_id = get_or_create_trial_org(&db, &now)?;
    let trial_pool_id = get_or_create_trial_pool(&db, &trial_org_id, state.config.trial_duration_days, &now)?;

    let key = crate::crypto::generate_license_key();
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::days(state.config.trial_duration_days))
            .to_rfc3339();

    let features = serde_json::json!(["meetings", "ai", "inbox"]);

    db.execute(
        "INSERT INTO licenses (key, pool_id, org_id, license_model, status, machine_id_hash, email,
                               activated_at, expires_at, created_at, metadata)
         VALUES (?1, ?2, ?3, 'trial', 'active', ?4, ?5, ?6, ?7, ?6, '{}')",
        rusqlite::params![
            key,
            trial_pool_id,
            trial_org_id,
            machine_hash,
            req.email.as_deref(),
            now,
            expires_at,
        ],
    )?;

    db.execute(
        "UPDATE pools SET used_seats = used_seats + 1 WHERE id = ?1",
        rusqlite::params![trial_pool_id],
    )?;

    let signature = crypto::build_validate_signature(
        &state.config.signing_secret,
        true,
        "trial",
        Some(&expires_at),
        &now,
    );

    db::insert_audit(
        &db,
        AuditParams {
            event: "trial.started",
            license_key: Some(&key),
            org_id: Some(&trial_org_id),
            pool_id: Some(&trial_pool_id),
            machine_id_hash: Some(&machine_hash),
            ip: ip.as_deref(),
            user_agent: ua.as_deref(),
            admin_id: None,
            details: json!({ "email": req.email }),
        },
    )?;

    drop(db);

    webhook::fire_event(
        Arc::clone(&state),
        "license.activated",
        Some(key.clone()),
        Some(trial_org_id),
        json!({ "model": "trial" }),
    );

    info!(key = %key, machine_hash = %machine_hash, "[trial] started");

    Ok(Json(ActivateResponse {
        ok: true,
        plan: "trial".to_string(),
        features,
        expires_at: Some(expires_at),
        maintenance_expires_at: None,
        license_model: "trial".to_string(),
        signature,
    }))
}

fn get_or_create_trial_org(db: &rusqlite::Connection, now: &str) -> AppResult<String> {
    // Use a single system trial org
    let existing: Option<String> = db
        .query_row(
            "SELECT id FROM organizations WHERE name = '__trial_system__'",
            [],
            |r| r.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO organizations (id, name, metadata, created_at) VALUES (?1, '__trial_system__', '{}', ?2)",
        rusqlite::params![id, now],
    )?;
    Ok(id)
}

fn get_or_create_trial_pool(
    db: &rusqlite::Connection,
    org_id: &str,
    duration_days: i64,
    now: &str,
) -> AppResult<String> {
    let existing: Option<String> = db
        .query_row(
            "SELECT id FROM pools WHERE org_id = ?1 AND license_model = 'trial'",
            rusqlite::params![org_id],
            |r| r.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO pools (id, org_id, name, license_model, total_seats, plan, duration_days, features, created_at, metadata)
         VALUES (?1, ?2, 'Trial Pool', 'trial', 999999, 'trial', ?3, '[\"meetings\",\"ai\",\"inbox\"]', ?4, '{}')",
        rusqlite::params![id, org_id, duration_days, now],
    )?;
    Ok(id)
}

// ─── GET /api/v1/health ───────────────────────────────────────────────────────

pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_status = match state.db.lock() {
        Ok(db) => {
            match db.query_row("SELECT 1", [], |r| r.get::<_, i64>(0)) {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("error: {}", e),
            }
        }
        Err(_) => "lock_error".to_string(),
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        db: db_status,
    })
}
