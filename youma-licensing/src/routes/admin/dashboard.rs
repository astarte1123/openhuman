use axum::{extract::State, Json};
use std::sync::Arc;

use crate::{
    auth::AuthenticatedAdmin,
    db,
    error::{AppError, AppResult},
    AppState,
};

pub async fn get_dashboard(
    _admin: AuthenticatedAdmin,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<db::DashboardStats>> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Internal("DB lock".to_string()))?;

    let stats = db::get_dashboard_stats(&db)?;
    Ok(Json(stats))
}
