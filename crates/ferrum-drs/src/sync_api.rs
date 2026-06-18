//! HTTP API for field sync queue (`/api/v1/sync/*`).

use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use ferrum_core::{enqueue_all_local, enqueue_object, list_queue_items, FerrumConfig, SyncConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct EnqueueQuery {
    pub object_id: Option<String>,
    #[serde(default)]
    pub all_local: bool,
    pub target: Option<String>,
}

#[derive(Serialize)]
pub struct SyncStatusResponse {
    items: Vec<ferrum_core::SyncQueueItem>,
    count: usize,
}

#[derive(Serialize)]
pub struct EnqueueResponse {
    enqueued: usize,
    items: Vec<ferrum_core::SyncQueueItem>,
}

fn sync_policy_from_env() -> SyncConfig {
    FerrumConfig::load().map(|c| c.sync).unwrap_or_default()
}

fn resolve_target(target: Option<String>) -> Result<String, (StatusCode, String)> {
    target
        .or_else(|| {
            FerrumConfig::load()
                .ok()
                .and_then(|c| c.sync.default_target_url)
        })
        .map(|t| ferrum_core::normalize_target_url(&t))
        .ok_or((
            StatusCode::BAD_REQUEST,
            "target required (query param or [sync] default_target_url)".into(),
        ))
}

pub async fn get_sync_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SyncStatusResponse>, (StatusCode, String)> {
    let items = list_queue_items(state.repo.pool(), None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let count = items.len();
    Ok(Json(SyncStatusResponse { items, count }))
}

pub async fn post_sync_enqueue(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EnqueueQuery>,
) -> Result<Json<EnqueueResponse>, (StatusCode, String)> {
    let target = resolve_target(q.target)?;
    let policy = sync_policy_from_env();
    let pool = state.repo.pool();

    if q.all_local {
        let items = enqueue_all_local(pool, &target, &policy)
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        let enqueued = items.len();
        return Ok(Json(EnqueueResponse { enqueued, items }));
    }
    let oid = q.object_id.ok_or((
        StatusCode::BAD_REQUEST,
        "object_id or all_local=true required".into(),
    ))?;
    let item = enqueue_object(pool, &oid, &target, &policy)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(EnqueueResponse {
        enqueued: 1,
        items: vec![item],
    }))
}

pub fn sync_api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/status", get(get_sync_status))
        .route("/enqueue", post(post_sync_enqueue))
        .with_state(state)
}

pub fn sync_api_router_unconfigured() -> Router {
    async fn no() -> impl IntoResponse {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "code": "not_configured",
                "message": "sync API requires DRS database state"
            })),
        )
    }
    Router::new()
        .route("/status", get(no))
        .route("/enqueue", post(no))
}
