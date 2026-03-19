use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WorkspaceError>;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for WorkspaceError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            WorkspaceError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            WorkspaceError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            WorkspaceError::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            WorkspaceError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            WorkspaceError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            WorkspaceError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}
