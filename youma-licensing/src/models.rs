use serde::{Deserialize, Serialize};

// ─── Organization ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub country: Option<String>,
    pub reseller: bool,
    pub parent_org_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub notes: Option<String>,
}

// ─── Pool ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub license_model: String,
    pub total_seats: i64,
    pub used_seats: i64,
    pub plan: String,
    pub duration_days: Option<i64>,
    pub maintenance_days: Option<i64>,
    pub grace_period_hours: i64,
    pub concurrent_limit: Option<i64>,
    pub features: serde_json::Value,
    pub created_at: String,
    pub created_by: Option<String>,
    pub notes: Option<String>,
    pub metadata: serde_json::Value,
}

// ─── License ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub key: String,
    pub pool_id: String,
    pub org_id: String,
    pub license_model: String,
    pub status: String,
    pub machine_id_hash: Option<String>,
    pub machine_label: Option<String>,
    pub email: Option<String>,
    pub activated_at: Option<String>,
    pub expires_at: Option<String>,
    pub maintenance_expires_at: Option<String>,
    pub last_validated_at: Option<String>,
    pub revoked_at: Option<String>,
    pub revoke_reason: Option<String>,
    pub transfer_count: i64,
    pub max_transfers: i64,
    pub created_at: String,
    pub metadata: serde_json::Value,
}

// ─── Floating Checkout ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FloatingCheckout {
    pub id: String,
    pub license_key: String,
    pub machine_id_hash: String,
    pub checked_out_at: String,
    pub last_heartbeat_at: String,
    pub expires_at: String,
}

// ─── Audit Log ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub ts: String,
    pub event: String,
    pub license_key: Option<String>,
    pub org_id: Option<String>,
    pub pool_id: Option<String>,
    pub machine_id_hash: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub admin_id: Option<String>,
    pub details: serde_json::Value,
}

// ─── Admin User ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUser {
    pub id: String,
    pub username: String,
    pub token_hash: String,
    pub role: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
}

// ─── Webhook ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub org_id: Option<String>,
    pub url: String,
    pub events: serde_json::Value,
    pub secret: String,
    pub active: bool,
    pub created_at: String,
    pub failure_count: i64,
    pub last_delivery_at: Option<String>,
    pub last_status: Option<i64>,
}

// ─── Webhook Delivery ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_id: String,
    pub event: String,
    pub status: String,
    pub attempts: i64,
    pub http_status: Option<i64>,
    pub next_retry_at: Option<String>,
    pub delivered_at: Option<String>,
    pub created_at: String,
}

// ─── Request/Response DTOs ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ActivateRequest {
    pub key: String,
    pub machine_id: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ActivateResponse {
    pub ok: bool,
    pub plan: String,
    pub features: serde_json::Value,
    pub expires_at: Option<String>,
    pub maintenance_expires_at: Option<String>,
    pub license_model: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub key: String,
    pub machine_id: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub reason: Option<String>,
    pub plan: Option<String>,
    pub features: Option<serde_json::Value>,
    pub expires_at: Option<String>,
    pub grace_until: Option<String>,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
pub struct DeactivateRequest {
    pub key: String,
    pub machine_id: String,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub key: String,
    pub machine_id: String,
}

#[derive(Debug, Serialize)]
pub struct HeartbeatResponse {
    pub ok: bool,
    pub slot_expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TrialRequest {
    pub machine_id: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub db: String,
}

// ─── Admin request types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub email: Option<String>,
    pub country: Option<String>,
    pub reseller: Option<bool>,
    pub parent_org_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub country: Option<String>,
    pub reseller: Option<bool>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePoolRequest {
    pub org_id: String,
    pub name: String,
    pub license_model: String,
    pub total_seats: i64,
    pub plan: Option<String>,
    pub duration_days: Option<i64>,
    pub maintenance_days: Option<i64>,
    pub grace_period_hours: Option<i64>,
    pub concurrent_limit: Option<i64>,
    pub features: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateKeysRequest {
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct SuspendRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ExtendRequest {
    pub days: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetMachineLabelRequest {
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct SetMaxTransfersRequest {
    pub max: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub org_id: Option<String>,
    pub url: String,
    pub events: Option<serde_json::Value>,
    pub secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub url: Option<String>,
    pub events: Option<serde_json::Value>,
    pub active: Option<bool>,
}

// ─── Pagination ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseFilterParams {
    pub key: Option<String>,
    pub org: Option<String>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct PoolFilterParams {
    pub org_id: Option<String>,
    pub model: Option<String>,
    pub plan: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct AuditFilterParams {
    pub license_key: Option<String>,
    pub org_id: Option<String>,
    pub event: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct PagedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub pages: u32,
}

impl<T: Serialize> PagedResponse<T> {
    pub fn new(data: Vec<T>, total: i64, page: u32, limit: u32) -> Self {
        let pages = if limit == 0 {
            1
        } else {
            ((total as u32 + limit - 1) / limit).max(1)
        };
        Self {
            data,
            total,
            page,
            limit,
            pages,
        }
    }
}
