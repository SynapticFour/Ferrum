//! htsget protocol errors (JSON body with `htsget.error` / `htsget.message`).

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub const HTSGET_JSON: &str = "application/vnd.ga4gh.htsget.v1.0.0+json; charset=utf-8";

/// Machine-readable htsget error + HTTP status.
pub fn htsget_error_response(status: StatusCode, error: &'static str, message: impl Into<String>) -> Response {
    let body = json!({
        "htsget": {
            "error": error,
            "message": message.into(),
        }
    });
    (
        status,
        [(header::CONTENT_TYPE, axum::http::HeaderValue::from_static("application/json"))],
        Json(body),
    )
        .into_response()
}
