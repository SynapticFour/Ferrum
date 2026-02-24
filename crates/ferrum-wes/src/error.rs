//! WES errors.

use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WesError>;

#[derive(Error, Debug)]
pub enum WesError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("executor: {0}")]
    Executor(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for WesError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            WesError::NotFound(_) => (axum::http::StatusCode::NOT_FOUND, self.to_string()),
            WesError::Validation(_) => (axum::http::StatusCode::BAD_REQUEST, self.to_string()),
            WesError::Database(_) | WesError::Executor(_) | WesError::Io(_) | WesError::Other(_) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };
        (status, axum::Json(serde_json::json!({ "msg": msg, "status_code": status.as_u16() }))).into_response()
    }
}
