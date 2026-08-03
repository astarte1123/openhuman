use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use std::sync::Arc;

use crate::{
    crypto,
    db,
    error::AppError,
    models::AdminUser,
    AppState,
};

/// Axum extractor: validates `Authorization: Bearer <token>` against admin_users table.
pub struct AuthenticatedAdmin(pub AdminUser);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthenticatedAdmin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid Authorization format".to_string()))?
            .trim();

        if token.is_empty() {
            return Err(AppError::Unauthorized("Empty bearer token".to_string()));
        }

        let token_hash = crypto::hash_token(token);

        let db = state
            .db
            .lock()
            .map_err(|_| AppError::Internal("DB lock poisoned".to_string()))?;

        let admin = db::find_admin_by_token_hash(&db, &token_hash)?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".to_string()))?;

        // Update last_seen_at (best-effort, don't fail the request)
        let _ = db::update_admin_last_seen(&db, &admin.id);

        tracing::debug!(
            admin_id = %admin.id,
            username = %admin.username,
            role = %admin.role,
            "[auth] authenticated admin request"
        );

        Ok(AuthenticatedAdmin(admin))
    }
}

/// Extractor that requires superadmin role.
#[allow(dead_code)]
pub struct SuperAdmin(pub AdminUser);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for SuperAdmin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let AuthenticatedAdmin(admin) = AuthenticatedAdmin::from_request_parts(parts, state).await?;

        if admin.role != "superadmin" {
            return Err(AppError::Forbidden(
                "Superadmin role required".to_string(),
            ));
        }

        Ok(SuperAdmin(admin))
    }
}
