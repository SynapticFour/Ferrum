//! DRS-specific errors.

use axum::response::IntoResponse;
use ferrum_core::FerrumError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DrsError>;

#[derive(Error, Debug)]
pub enum DrsError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl From<DrsError> for FerrumError {
    fn from(e: DrsError) -> Self {
        match e {
            DrsError::NotFound(s) => FerrumError::NotFound(s),
            DrsError::Forbidden(s) => FerrumError::Forbidden(s),
            DrsError::Validation(s) => FerrumError::ValidationError(s),
            DrsError::Database(se) => FerrumError::DatabaseError(se),
            DrsError::Other(o) => FerrumError::Internal(o),
        }
    }
}

impl From<FerrumError> for DrsError {
    fn from(e: FerrumError) -> Self {
        DrsError::Other(anyhow::anyhow!("{}", e))
    }
}

impl From<DrsError> for axum::response::Response {
    fn from(e: DrsError) -> Self {
        FerrumError::from(e).into_response()
    }
}

impl IntoResponse for DrsError {
    fn into_response(self) -> axum::response::Response {
        axum::response::Response::from(self)
    }
}

/// Wrapper so that `Result<Json<T>, DrsError>` can implement `IntoResponse` (orphan rule).
pub struct JsonResult<T>(pub std::result::Result<axum::Json<T>, DrsError>);

impl<T> IntoResponse for JsonResult<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Ok(j) => j.into_response(),
            Err(e) => axum::response::Response::from(e),
        }
    }
}

/// Wrapper for GET /objects/{id}/view which returns a raw Response (HTML body).
pub struct ViewResult(pub std::result::Result<axum::response::Response, DrsError>);

impl IntoResponse for ViewResult {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Ok(r) => r,
            Err(e) => axum::response::Response::from(e),
        }
    }
}

/// Wrapper for GET /objects/{id}/stream (binary / decrypted stream).
pub struct StreamResult(pub std::result::Result<axum::response::Response, DrsError>);

impl IntoResponse for StreamResult {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Ok(r) => r,
            Err(e) => axum::response::Response::from(e),
        }
    }
}
