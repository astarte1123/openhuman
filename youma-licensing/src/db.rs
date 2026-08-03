use crate::error::AppResult;
use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, info};

/// Initialize DB: enable WAL, run all DDL idempotently, seed superadmin.
pub fn init(conn: &Connection, admin_token_hash: &str) -> AppResult<()> {
    info!("[db] initializing database");

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch("PRAGMA busy_timeout=5000;")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            applied_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS admin_users (
            id            TEXT PRIMARY KEY,
            username      TEXT UNIQUE NOT NULL,
            token_hash    TEXT NOT NULL,
            role          TEXT NOT NULL DEFAULT 'admin',
            created_at    TEXT NOT NULL,
            last_seen_at  TEXT
        );

        CREATE TABLE IF NOT EXISTS organizations (
            id             TEXT PRIMARY KEY,
            name           TEXT NOT NULL,
            email          TEXT,
            country        TEXT,
            reseller       INTEGER DEFAULT 0,
            parent_org_id  TEXT REFERENCES organizations(id),
            metadata       TEXT NOT NULL DEFAULT '{}',
            created_at     TEXT NOT NULL,
            notes          TEXT
        );

        CREATE TABLE IF NOT EXISTS pools (
            id                  TEXT PRIMARY KEY,
            org_id              TEXT NOT NULL REFERENCES organizations(id),
            name                TEXT NOT NULL,
            license_model       TEXT NOT NULL DEFAULT 'seat',
            total_seats         INTEGER NOT NULL,
            used_seats          INTEGER NOT NULL DEFAULT 0,
            plan                TEXT NOT NULL DEFAULT 'pro',
            duration_days       INTEGER,
            maintenance_days    INTEGER,
            grace_period_hours  INTEGER NOT NULL DEFAULT 72,
            concurrent_limit    INTEGER,
            features            TEXT NOT NULL DEFAULT '[]',
            created_at          TEXT NOT NULL,
            created_by          TEXT,
            notes               TEXT,
            metadata            TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS licenses (
            key                     TEXT PRIMARY KEY,
            pool_id                 TEXT NOT NULL REFERENCES pools(id),
            org_id                  TEXT NOT NULL REFERENCES organizations(id),
            license_model           TEXT NOT NULL,
            status                  TEXT NOT NULL DEFAULT 'inactive',
            machine_id_hash         TEXT,
            machine_label           TEXT,
            email                   TEXT,
            activated_at            TEXT,
            expires_at              TEXT,
            maintenance_expires_at  TEXT,
            last_validated_at       TEXT,
            revoked_at              TEXT,
            revoke_reason           TEXT,
            transfer_count          INTEGER NOT NULL DEFAULT 0,
            max_transfers           INTEGER NOT NULL DEFAULT 3,
            created_at              TEXT NOT NULL,
            metadata                TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS floating_checkouts (
            id                TEXT PRIMARY KEY,
            license_key       TEXT NOT NULL REFERENCES licenses(key),
            machine_id_hash   TEXT NOT NULL,
            checked_out_at    TEXT NOT NULL,
            last_heartbeat_at TEXT NOT NULL,
            expires_at        TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id              TEXT PRIMARY KEY,
            ts              TEXT NOT NULL,
            event           TEXT NOT NULL,
            license_key     TEXT,
            org_id          TEXT,
            pool_id         TEXT,
            machine_id_hash TEXT,
            ip              TEXT,
            user_agent      TEXT,
            admin_id        TEXT,
            details         TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS webhooks (
            id               TEXT PRIMARY KEY,
            org_id           TEXT REFERENCES organizations(id),
            url              TEXT NOT NULL,
            events           TEXT NOT NULL DEFAULT '["license.activated","license.expired","license.suspended"]',
            secret           TEXT NOT NULL,
            active           INTEGER NOT NULL DEFAULT 1,
            created_at       TEXT NOT NULL,
            failure_count    INTEGER NOT NULL DEFAULT 0,
            last_delivery_at TEXT,
            last_status      INTEGER
        );

        CREATE TABLE IF NOT EXISTS webhook_deliveries (
            id              TEXT PRIMARY KEY,
            webhook_id      TEXT NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
            event           TEXT NOT NULL,
            payload         TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'pending',
            attempts        INTEGER NOT NULL DEFAULT 0,
            http_status     INTEGER,
            next_retry_at   TEXT,
            delivered_at    TEXT,
            created_at      TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_licenses_pool      ON licenses(pool_id);
        CREATE INDEX IF NOT EXISTS idx_licenses_org       ON licenses(org_id);
        CREATE INDEX IF NOT EXISTS idx_licenses_machine   ON licenses(machine_id_hash);
        CREATE INDEX IF NOT EXISTS idx_licenses_status    ON licenses(status);
        CREATE INDEX IF NOT EXISTS idx_audit_ts           ON audit_log(ts);
        CREATE INDEX IF NOT EXISTS idx_audit_license      ON audit_log(license_key);
        CREATE INDEX IF NOT EXISTS idx_floating_license   ON floating_checkouts(license_key);
        CREATE INDEX IF NOT EXISTS idx_floating_expires   ON floating_checkouts(expires_at);
        CREATE INDEX IF NOT EXISTS idx_wh_deliveries_pending ON webhook_deliveries(status, next_retry_at);
        CREATE INDEX IF NOT EXISTS idx_wh_deliveries_hook    ON webhook_deliveries(webhook_id);
        "#,
    )?;

    // Seed superadmin from ADMIN_TOKEN if not exists
    let admin_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM admin_users WHERE username = 'superadmin'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !admin_exists {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO admin_users (id, username, token_hash, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, "superadmin", admin_token_hash, "superadmin", now],
        )?;
        info!("[db] seeded superadmin user");
    }

    info!("[db] initialization complete");
    Ok(())
}

// ─── Org helpers ──────────────────────────────────────────────────────────────

pub fn get_org(conn: &Connection, id: &str) -> AppResult<Option<Organization>> {
    let result = conn
        .query_row(
            "SELECT id, name, email, country, reseller, parent_org_id, metadata, created_at, notes
             FROM organizations WHERE id = ?1",
            params![id],
            row_to_org,
        )
        .optional()?;
    Ok(result)
}

pub fn list_orgs(
    conn: &Connection,
    page: u32,
    limit: u32,
    q: Option<&str>,
) -> AppResult<(Vec<Organization>, i64)> {
    let offset = (page.saturating_sub(1)) * limit;
    let pattern = q.map(|s| format!("%{}%", s));

    let (rows, total) = if let Some(ref p) = pattern {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM organizations WHERE name LIKE ?1 OR email LIKE ?1",
            params![p],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, name, email, country, reseller, parent_org_id, metadata, created_at, notes
             FROM organizations WHERE name LIKE ?1 OR email LIKE ?1
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows: Vec<Organization> = stmt
            .query_map(params![p, limit, offset], row_to_org)?
            .filter_map(|r| r.ok())
            .collect();
        (rows, total)
    } else {
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM organizations", [], |r| r.get(0))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, email, country, reseller, parent_org_id, metadata, created_at, notes
             FROM organizations ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows: Vec<Organization> = stmt
            .query_map(params![limit, offset], row_to_org)?
            .filter_map(|r| r.ok())
            .collect();
        (rows, total)
    };

    Ok((rows, total))
}

fn row_to_org(row: &rusqlite::Row) -> rusqlite::Result<Organization> {
    let metadata_str: String = row.get(6)?;
    let metadata = serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
    Ok(Organization {
        id: row.get(0)?,
        name: row.get(1)?,
        email: row.get(2)?,
        country: row.get(3)?,
        reseller: row.get::<_, i64>(4)? != 0,
        parent_org_id: row.get(5)?,
        metadata,
        created_at: row.get(7)?,
        notes: row.get(8)?,
    })
}

// ─── Pool helpers ─────────────────────────────────────────────────────────────

pub fn get_pool(conn: &Connection, id: &str) -> AppResult<Option<Pool>> {
    let result = conn
        .query_row(
            "SELECT id, org_id, name, license_model, total_seats, used_seats, plan,
                    duration_days, maintenance_days, grace_period_hours, concurrent_limit,
                    features, created_at, created_by, notes, metadata
             FROM pools WHERE id = ?1",
            params![id],
            row_to_pool,
        )
        .optional()?;
    Ok(result)
}

pub fn list_pools(
    conn: &Connection,
    page: u32,
    limit: u32,
    org_id: Option<&str>,
    model: Option<&str>,
    plan: Option<&str>,
) -> AppResult<(Vec<Pool>, i64)> {
    let offset = (page.saturating_sub(1)) * limit;

    // Build dynamic WHERE clause
    let mut conditions = Vec::new();
    if org_id.is_some() {
        conditions.push("org_id = ?");
    }
    if model.is_some() {
        conditions.push("license_model = ?");
    }
    if plan.is_some() {
        conditions.push("plan = ?");
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Collect params dynamically
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(v) = org_id {
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = model {
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = plan {
        param_values.push(Box::new(v.to_string()));
    }

    let count_sql = format!("SELECT COUNT(*) FROM pools {}", where_clause);
    let select_sql = format!(
        "SELECT id, org_id, name, license_model, total_seats, used_seats, plan,
                duration_days, maintenance_days, grace_period_hours, concurrent_limit,
                features, created_at, created_by, notes, metadata
         FROM pools {} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|b| b.as_ref()).collect();

    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;
    let mut stmt = conn.prepare(&select_sql)?;
    let rows: Vec<Pool> = stmt
        .query_map(param_refs.as_slice(), row_to_pool)?
        .filter_map(|r| r.ok())
        .collect();

    Ok((rows, total))
}

fn row_to_pool(row: &rusqlite::Row) -> rusqlite::Result<Pool> {
    let features_str: String = row.get(11)?;
    let features = serde_json::from_str(&features_str).unwrap_or(serde_json::json!([]));
    let metadata_str: String = row.get(15)?;
    let metadata = serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
    Ok(Pool {
        id: row.get(0)?,
        org_id: row.get(1)?,
        name: row.get(2)?,
        license_model: row.get(3)?,
        total_seats: row.get(4)?,
        used_seats: row.get(5)?,
        plan: row.get(6)?,
        duration_days: row.get(7)?,
        maintenance_days: row.get(8)?,
        grace_period_hours: row.get(9)?,
        concurrent_limit: row.get(10)?,
        features,
        created_at: row.get(12)?,
        created_by: row.get(13)?,
        notes: row.get(14)?,
        metadata,
    })
}

// ─── License helpers ──────────────────────────────────────────────────────────

pub fn get_license(conn: &Connection, key: &str) -> AppResult<Option<License>> {
    let result = conn
        .query_row(
            "SELECT key, pool_id, org_id, license_model, status, machine_id_hash, machine_label,
                    email, activated_at, expires_at, maintenance_expires_at, last_validated_at,
                    revoked_at, revoke_reason, transfer_count, max_transfers, created_at, metadata
             FROM licenses WHERE key = ?1",
            params![key],
            row_to_license,
        )
        .optional()?;
    Ok(result)
}

pub fn list_licenses(
    conn: &Connection,
    page: u32,
    limit: u32,
    key_filter: Option<&str>,
    org: Option<&str>,
    status: Option<&str>,
    model: Option<&str>,
) -> AppResult<(Vec<License>, i64)> {
    let offset = (page.saturating_sub(1)) * limit;

    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<String> = Vec::new();

    if let Some(k) = key_filter {
        conditions.push("key LIKE ?".to_string());
        param_values.push(format!("%{}%", k));
    }
    if let Some(o) = org {
        conditions.push(
            "org_id IN (SELECT id FROM organizations WHERE lower(name) LIKE lower(?))".to_string(),
        );
        param_values.push(format!("%{}%", o));
    }
    if let Some(s) = status {
        conditions.push("status = ?".to_string());
        param_values.push(s.to_string());
    }
    if let Some(m) = model {
        conditions.push("license_model = ?".to_string());
        param_values.push(m.to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM licenses {}", where_clause);
    let select_sql = format!(
        "SELECT key, pool_id, org_id, license_model, status, machine_id_hash, machine_label,
                email, activated_at, expires_at, maintenance_expires_at, last_validated_at,
                revoked_at, revoke_reason, transfer_count, max_transfers, created_at, metadata
         FROM licenses {} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();

    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;
    let mut stmt = conn.prepare(&select_sql)?;
    let rows: Vec<License> = stmt
        .query_map(param_refs.as_slice(), row_to_license)?
        .filter_map(|r| r.ok())
        .collect();

    Ok((rows, total))
}

fn row_to_license(row: &rusqlite::Row) -> rusqlite::Result<License> {
    let metadata_str: String = row.get(17)?;
    let metadata = serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
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
}

// ─── Audit log helpers ────────────────────────────────────────────────────────

pub struct AuditParams<'a> {
    pub event: &'a str,
    pub license_key: Option<&'a str>,
    pub org_id: Option<&'a str>,
    pub pool_id: Option<&'a str>,
    pub machine_id_hash: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub admin_id: Option<&'a str>,
    pub details: serde_json::Value,
}

pub fn insert_audit(conn: &Connection, p: AuditParams) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let ts = chrono::Utc::now().to_rfc3339();
    let details_str = p.details.to_string();

    conn.execute(
        "INSERT INTO audit_log (id, ts, event, license_key, org_id, pool_id, machine_id_hash, ip, user_agent, admin_id, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id, ts, p.event, p.license_key, p.org_id, p.pool_id,
            p.machine_id_hash, p.ip, p.user_agent, p.admin_id, details_str
        ],
    )?;
    debug!("[db] audit event={} key={:?}", p.event, p.license_key);
    Ok(())
}

pub fn list_audit(
    conn: &Connection,
    page: u32,
    limit: u32,
    license_key: Option<&str>,
    org_id: Option<&str>,
    event: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<(Vec<AuditEntry>, i64)> {
    let offset = (page.saturating_sub(1)) * limit;
    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<String> = Vec::new();

    if let Some(k) = license_key {
        conditions.push("license_key = ?".to_string());
        param_values.push(k.to_string());
    }
    if let Some(o) = org_id {
        conditions.push("org_id = ?".to_string());
        param_values.push(o.to_string());
    }
    if let Some(e) = event {
        conditions.push("event = ?".to_string());
        param_values.push(e.to_string());
    }
    if let Some(f) = from {
        conditions.push("ts >= ?".to_string());
        param_values.push(f.to_string());
    }
    if let Some(t) = to {
        conditions.push("ts <= ?".to_string());
        param_values.push(t.to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM audit_log {}", where_clause);
    let select_sql = format!(
        "SELECT id, ts, event, license_key, org_id, pool_id, machine_id_hash, ip, user_agent, admin_id, details
         FROM audit_log {} ORDER BY ts DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();

    let total: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))?;
    let mut stmt = conn.prepare(&select_sql)?;
    let rows: Vec<AuditEntry> = stmt
        .query_map(param_refs.as_slice(), |row| {
            let details_str: String = row.get(10)?;
            let details = serde_json::from_str(&details_str).unwrap_or(serde_json::json!({}));
            Ok(AuditEntry {
                id: row.get(0)?,
                ts: row.get(1)?,
                event: row.get(2)?,
                license_key: row.get(3)?,
                org_id: row.get(4)?,
                pool_id: row.get(5)?,
                machine_id_hash: row.get(6)?,
                ip: row.get(7)?,
                user_agent: row.get(8)?,
                admin_id: row.get(9)?,
                details,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok((rows, total))
}

// ─── Webhook helpers ──────────────────────────────────────────────────────────

pub fn list_webhooks(conn: &Connection) -> AppResult<Vec<Webhook>> {
    let mut stmt = conn.prepare(
        "SELECT id, org_id, url, events, secret, active, created_at, failure_count, last_delivery_at, last_status
         FROM webhooks ORDER BY created_at DESC",
    )?;
    let rows: Vec<Webhook> = stmt
        .query_map([], |row| {
            let events_str: String = row.get(3)?;
            let events = serde_json::from_str(&events_str).unwrap_or(serde_json::json!([]));
            Ok(Webhook {
                id: row.get(0)?,
                org_id: row.get(1)?,
                url: row.get(2)?,
                events,
                secret: row.get(4)?,
                active: row.get::<_, i64>(5)? != 0,
                created_at: row.get(6)?,
                failure_count: row.get(7)?,
                last_delivery_at: row.get(8)?,
                last_status: row.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_webhook(conn: &Connection, id: &str) -> AppResult<Option<Webhook>> {
    let result = conn
        .query_row(
            "SELECT id, org_id, url, events, secret, active, created_at, failure_count, last_delivery_at, last_status
             FROM webhooks WHERE id = ?1",
            params![id],
            |row| {
                let events_str: String = row.get(3)?;
                let events = serde_json::from_str(&events_str).unwrap_or(serde_json::json!([]));
                Ok(Webhook {
                    id: row.get(0)?,
                    org_id: row.get(1)?,
                    url: row.get(2)?,
                    events,
                    secret: row.get(4)?,
                    active: row.get::<_, i64>(5)? != 0,
                    created_at: row.get(6)?,
                    failure_count: row.get(7)?,
                    last_delivery_at: row.get(8)?,
                    last_status: row.get(9)?,
                })
            },
        )
        .optional()?;
    Ok(result)
}

pub fn get_webhooks_for_event(conn: &Connection, event: &str) -> AppResult<Vec<Webhook>> {
    // Fetch all active webhooks and filter by event in Rust (SQLite JSON is limited)
    let all = list_webhooks(conn)?;
    Ok(all
        .into_iter()
        .filter(|w| {
            w.active
                && w.events
                    .as_array()
                    .map(|arr| arr.iter().any(|e| e.as_str() == Some(event)))
                    .unwrap_or(false)
        })
        .collect())
}

// ─── Admin user helpers ───────────────────────────────────────────────────────

pub fn find_admin_by_token_hash(conn: &Connection, hash: &str) -> AppResult<Option<AdminUser>> {
    let result = conn
        .query_row(
            "SELECT id, username, token_hash, role, created_at, last_seen_at
             FROM admin_users WHERE token_hash = ?1",
            params![hash],
            |row| {
                Ok(AdminUser {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    token_hash: row.get(2)?,
                    role: row.get(3)?,
                    created_at: row.get(4)?,
                    last_seen_at: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(result)
}

pub fn update_admin_last_seen(conn: &Connection, id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE admin_users SET last_seen_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

// ─── Dashboard stats ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct DashboardStats {
    pub total_orgs: i64,
    pub total_pools: i64,
    pub total_licenses: i64,
    pub active_licenses: i64,
    pub trial_licenses: i64,
    pub expired_licenses: i64,
    pub suspended_licenses: i64,
    pub revoked_licenses: i64,
    pub activations_today: i64,
    pub activations_week: i64,
    pub activations_month: i64,
    pub expiring_soon: i64,
    pub licenses_by_model: serde_json::Value,
    pub top_orgs: Vec<OrgUsage>,
}

#[derive(Debug, serde::Serialize)]
pub struct OrgUsage {
    pub org_id: String,
    pub org_name: String,
    pub license_count: i64,
    pub active_count: i64,
}

pub fn get_dashboard_stats(conn: &Connection) -> AppResult<DashboardStats> {
    let total_orgs: i64 =
        conn.query_row("SELECT COUNT(*) FROM organizations", [], |r| r.get(0))?;
    let total_pools: i64 = conn.query_row("SELECT COUNT(*) FROM pools", [], |r| r.get(0))?;
    let total_licenses: i64 =
        conn.query_row("SELECT COUNT(*) FROM licenses", [], |r| r.get(0))?;
    let active_licenses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE status = 'active'",
        [],
        |r| r.get(0),
    )?;
    let trial_licenses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE license_model = 'trial'",
        [],
        |r| r.get(0),
    )?;
    let expired_licenses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE status = 'expired'",
        [],
        |r| r.get(0),
    )?;
    let suspended_licenses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE status = 'suspended'",
        [],
        |r| r.get(0),
    )?;
    let revoked_licenses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE status = 'revoked'",
        [],
        |r| r.get(0),
    )?;
    let activations_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_log WHERE event = 'license.activated' AND ts >= date('now')",
        [],
        |r| r.get(0),
    )?;
    let activations_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_log WHERE event = 'license.activated' AND ts >= date('now', '-7 days')",
        [],
        |r| r.get(0),
    )?;
    let activations_month: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_log WHERE event = 'license.activated' AND ts >= date('now', '-30 days')",
        [],
        |r| r.get(0),
    )?;
    let expiring_soon: i64 = conn.query_row(
        "SELECT COUNT(*) FROM licenses WHERE status = 'active' AND expires_at IS NOT NULL AND expires_at <= datetime('now', '+3 days')",
        [],
        |r| r.get(0),
    )?;

    // Licenses by model
    let mut model_stmt = conn.prepare(
        "SELECT license_model, COUNT(*) FROM licenses GROUP BY license_model",
    )?;
    let model_map: serde_json::Map<String, serde_json::Value> = model_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
        .filter_map(|r| r.ok())
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect();

    // Top orgs by license count
    let mut org_stmt = conn.prepare(
        "SELECT l.org_id, o.name, COUNT(*) as total, SUM(CASE WHEN l.status='active' THEN 1 ELSE 0 END) as active
         FROM licenses l JOIN organizations o ON l.org_id = o.id
         GROUP BY l.org_id ORDER BY total DESC LIMIT 10",
    )?;
    let top_orgs: Vec<OrgUsage> = org_stmt
        .query_map([], |row| {
            Ok(OrgUsage {
                org_id: row.get(0)?,
                org_name: row.get(1)?,
                license_count: row.get(2)?,
                active_count: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(DashboardStats {
        total_orgs,
        total_pools,
        total_licenses,
        active_licenses,
        trial_licenses,
        expired_licenses,
        suspended_licenses,
        revoked_licenses,
        activations_today,
        activations_week,
        activations_month,
        expiring_soon,
        licenses_by_model: serde_json::Value::Object(model_map),
        top_orgs,
    })
}

// ─── Webhook delivery helpers ─────────────────────────────────────────────────

/// A pending or completed delivery record.
pub struct PendingDelivery {
    pub id: String,
    pub webhook_id: String,
    pub event: String,
    pub payload: String,
    pub attempts: i64,
}

/// Record a new delivery attempt (status=pending, attempts=0).
pub fn insert_webhook_delivery(
    conn: &Connection,
    id: &str,
    webhook_id: &str,
    event: &str,
    payload: &str,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO webhook_deliveries (id, webhook_id, event, payload, status, attempts, created_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5)",
        params![id, webhook_id, event, payload, now],
    )?;
    Ok(())
}

/// Fetch deliveries that are ready to be attempted (pending, retry window elapsed, < 5 attempts).
pub fn list_pending_deliveries(conn: &Connection, now: &str) -> AppResult<Vec<PendingDelivery>> {
    let mut stmt = conn.prepare(
        "SELECT id, webhook_id, event, payload, attempts
         FROM webhook_deliveries
         WHERE status = 'pending' AND attempts < 5
           AND (next_retry_at IS NULL OR next_retry_at <= ?1)",
    )?;
    let rows = stmt
        .query_map(params![now], |row| {
            Ok(PendingDelivery {
                id: row.get(0)?,
                webhook_id: row.get(1)?,
                event: row.get(2)?,
                payload: row.get(3)?,
                attempts: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Mark a delivery as successfully delivered.
pub fn mark_delivery_success(
    conn: &Connection,
    id: &str,
    http_status: i64,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE webhook_deliveries
         SET status = 'delivered', attempts = attempts + 1,
             delivered_at = ?1, http_status = ?2, next_retry_at = NULL
         WHERE id = ?3",
        params![now, http_status, id],
    )?;
    Ok(())
}

/// Mark a delivery attempt as failed; schedule next retry or abandon after 5 attempts.
pub fn mark_delivery_failed(
    conn: &Connection,
    id: &str,
    next_retry_at: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE webhook_deliveries
         SET attempts = attempts + 1, next_retry_at = ?1
         WHERE id = ?2",
        params![next_retry_at, id],
    )?;
    // Abandon after 5 attempts
    conn.execute(
        "UPDATE webhook_deliveries SET status = 'abandoned'
         WHERE id = ?1 AND attempts >= 5",
        params![id],
    )?;
    Ok(())
}
