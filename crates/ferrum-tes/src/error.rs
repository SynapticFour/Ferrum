//! TES errors.

use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, TesError>;

#[derive(Error, Debug)]
pub enum TesError {
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
}

impl IntoResponse for TesError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            TesError::NotFound(_) => (axum::http::StatusCode::NOT_FOUND, self.to_string()),
            TesError::Validation(_) => (axum::http::StatusCode::BAD_REQUEST, self.to_string()),
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                self.to_string(),
            ),
        };
        (status, axum::Json(serde_json::json!({ "msg": msg }))).into_response()
    }
}
