use thiserror::Error;

#[derive(Debug, Error)]
pub enum YoumaError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("encryption error: {0}")]
    Encryption(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<YoumaError> for String {
    fn from(e: YoumaError) -> String {
        e.to_string()
    }
}

pub type YoumaResult<T> = Result<T, YoumaError>;
