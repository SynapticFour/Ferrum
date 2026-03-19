use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, TrsError>;

#[derive(Error, Debug)]
pub enum TrsError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for TrsError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            TrsError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            TrsError::Validation(_) => axum::http::StatusCode::BAD_REQUEST,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            axum::Json(serde_json::json!({ "msg": self.to_string() })),
        )
            .into_response()
    }
}
