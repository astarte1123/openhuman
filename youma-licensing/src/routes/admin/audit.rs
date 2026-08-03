use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

use crate::{
    auth::AuthenticatedAdmin,
    db,
    error::{AppError, AppResult},
    models::*,
    AppState,
};

pub async fn list_audit(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditFilterParams>,
) -> AppResult<Json<PagedResponse<AuditEntry>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let (entries, total) = db::list_audit(
        &db,
        page,
        limit,
        params.license_key.as_deref(),
        params.org_id.as_deref(),
        params.event.as_deref(),
        params.from.as_deref(),
        params.to.as_deref(),
    )?;

    Ok(Json(PagedResponse::new(entries, total, page, limit)))
}
