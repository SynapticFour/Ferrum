use axum::response::IntoResponse;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BeaconError>;

#[derive(Error, Debug)]
pub enum BeaconError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("validation: {0}")]
    Validation(String),
}

impl IntoResponse for BeaconError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            BeaconError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            BeaconError::Validation(_) => axum::http::StatusCode::BAD_REQUEST,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            axum::Json(serde_json::json!({ "msg": self.to_string() })),
        )
            .into_response()
    }
}
