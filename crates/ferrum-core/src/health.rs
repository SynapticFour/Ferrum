//! Health check handler for readiness/liveness.

use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: Option<String>,
}

/// Returns a router with GET /health and GET /ready.
pub fn health_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
    })
}

async fn ready_handler() -> Json<HealthResponse> {
    // TODO: check database connectivity when pool is in state
    Json(HealthResponse {
        status: "ready".to_string(),
        version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
    })
}
