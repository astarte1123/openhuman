use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    TooManyRequests,
    Internal(String),
    Database(rusqlite::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::TooManyRequests => write!(f, "Too many requests"),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AppError::Database(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Too many requests".to_string(),
            ),
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Internal server error".to_string(),
                )
            }
            AppError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    "Database error".to_string(),
                )
            }
        };

        let body = json!({
            "error": message,
            "code": code,
        });

        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
