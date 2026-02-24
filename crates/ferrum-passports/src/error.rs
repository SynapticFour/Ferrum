use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PassportError>;

#[derive(Error, Debug)]
pub enum PassportError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("jwt: {0}")]
    Jwt(String),
}

impl IntoResponse for PassportError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            PassportError::Validation(_) => axum::http::StatusCode::BAD_REQUEST,
            PassportError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            PassportError::Unauthorized(_) => axum::http::StatusCode::UNAUTHORIZED,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            axum::Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
