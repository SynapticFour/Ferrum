//! Health check handler for readiness/liveness.

use crate::clock::{clock_status, DEFAULT_MAX_SKEW_SECS, DEFAULT_NTP_HOST};
use crate::disk::DiskSpaceStatus;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

static DATA_PATH: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
static CLOCK_CFG: OnceLock<RwLock<(String, i64)>> = OnceLock::new();

fn data_path_store() -> &'static RwLock<Option<PathBuf>> {
    DATA_PATH.get_or_init(|| RwLock::new(None))
}

fn clock_cfg_store() -> &'static RwLock<(String, i64)> {
    CLOCK_CFG.get_or_init(|| RwLock::new((DEFAULT_NTP_HOST.to_string(), DEFAULT_MAX_SKEW_SECS)))
}

/// Register the primary data directory (objects/SQLite) for disk-space reporting on `/health`.
pub fn set_health_data_path(path: PathBuf) {
    if let Ok(mut guard) = data_path_store().write() {
        *guard = Some(path);
    }
}

/// Configure NTP host and max skew threshold for clock reporting on `/health`.
pub fn set_health_clock_config(ntp_host: String, max_skew_secs: i64) {
    if let Ok(mut guard) = clock_cfg_store().write() {
        *guard = (ntp_host, max_skew_secs);
    }
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<DiskSpaceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock: Option<crate::clock::ClockStatus>,
}

/// Returns a router with GET /health and GET /ready.
pub fn health_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
}

async fn health_handler() -> Json<HealthResponse> {
    let disk = data_path_store()
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .and_then(|p| crate::disk::disk_space_status(&p, 10.0));

    let (ntp_host, max_skew) = clock_cfg_store()
        .read()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| (DEFAULT_NTP_HOST.to_string(), DEFAULT_MAX_SKEW_SECS));
    let clock = Some(clock_status(&ntp_host, max_skew));

    let mut status = if disk.as_ref().is_some_and(|d| d.warn_low_space) {
        "degraded".to_string()
    } else {
        "ok".to_string()
    };
    if clock.as_ref().is_some_and(|c| c.warn_skew) {
        status = "degraded".to_string();
    }

    Json(HealthResponse {
        status,
        version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
        disk,
        clock,
    })
}

async fn ready_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ready".to_string(),
        version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
        disk: None,
        clock: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_includes_disk_when_path_set() {
        let tmp = tempfile::tempdir().unwrap();
        set_health_data_path(tmp.path().to_path_buf());
        let app = health_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("disk").is_some());
        assert!(json.get("clock").is_some());
    }
}
