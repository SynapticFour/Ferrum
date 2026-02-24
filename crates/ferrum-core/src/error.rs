//! Error types for Ferrum using thiserror, with axum IntoResponse.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, FerrumError>;

#[derive(Error, Debug)]
pub enum FerrumError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),

    #[error("storage error: {0}")]
    StorageError(#[source] anyhow::Error),

    #[error("encryption error: {0}")]
    EncryptionError(#[source] anyhow::Error),

    #[error("workflow error: {0}")]
    WorkflowError(String),

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: Option<String>,
}

impl IntoResponse for FerrumError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            FerrumError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            FerrumError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            FerrumError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            FerrumError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            FerrumError::DatabaseError(_) | FerrumError::MigrationError(_) | FerrumError::StorageError(_) | FerrumError::EncryptionError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            FerrumError::WorkflowError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            FerrumError::ValidationError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            FerrumError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string()),
            FerrumError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };
        (
            status,
            Json(ErrorBody {
                error: status.as_str().to_string(),
                message: Some(message),
            }),
        )
            .into_response()
    }
}
