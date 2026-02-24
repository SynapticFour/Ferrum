//! Cohort service errors.

use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CohortError>;

#[derive(Error, Debug)]
pub enum CohortError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for CohortError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            CohortError::NotFound(_) => (axum::http::StatusCode::NOT_FOUND, self.to_string()),
            CohortError::Validation(_) => (axum::http::StatusCode::BAD_REQUEST, self.to_string()),
            CohortError::Forbidden(_) => (axum::http::StatusCode::FORBIDDEN, self.to_string()),
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                self.to_string(),
            ),
        };
        (
            status,
            axum::Json(serde_json::json!({ "error": msg })),
        )
            .into_response()
    }
}

/// Wrapper so that Result<Json<T>, CohortError> implements IntoResponse.
pub struct CohortJsonResult<T>(pub std::result::Result<axum::Json<T>, CohortError>);

impl<T: serde::Serialize> axum::response::IntoResponse for CohortJsonResult<T> {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Ok(j) => j.into_response(),
            Err(e) => e.into_response(),
        }
    }
}
