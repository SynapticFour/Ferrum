// SPDX-License-Identifier: BUSL-1.1
//! HTTP API for the reference genome registry (`/api/v1/references`).

use crate::registry::ReferenceRegistry;
use crate::types::{LoadReferenceRequest, ReferenceGenome, RegisterReferenceRequest};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use ferrum_core::FerrumError;
use std::sync::Arc;

pub fn reference_api_v1_router(registry: Arc<ReferenceRegistry>) -> Router {
    Router::new()
        .route("/", get(list_references).post(register_reference))
        .route("/:id", get(get_reference))
        .route("/:id/load", put(load_reference))
        .with_state(registry)
}

async fn list_references(
    State(registry): State<Arc<ReferenceRegistry>>,
) -> Result<Json<Vec<ReferenceGenome>>, Response> {
    registry.list().await.map(Json).map_err(map_err)
}

async fn get_reference(
    State(registry): State<Arc<ReferenceRegistry>>,
    Path(id): Path<String>,
) -> Result<Json<ReferenceGenome>, Response> {
    match registry.get(&id).await.map_err(map_err)? {
        Some(r) => Ok(Json(r)),
        None => Err(not_found(&id)),
    }
}

async fn register_reference(
    State(registry): State<Arc<ReferenceRegistry>>,
    Json(req): Json<RegisterReferenceRequest>,
) -> Result<(StatusCode, Json<ReferenceGenome>), Response> {
    registry
        .register(&req)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(map_err)
}

async fn load_reference(
    State(registry): State<Arc<ReferenceRegistry>>,
    Path(id): Path<String>,
    Json(req): Json<LoadReferenceRequest>,
) -> Result<Json<ReferenceGenome>, Response> {
    registry
        .load_fasta(&id, &req)
        .await
        .map(Json)
        .map_err(map_err)
}

fn map_err(e: FerrumError) -> Response {
    let status = match &e {
        FerrumError::ValidationError(_) => StatusCode::BAD_REQUEST,
        FerrumError::NotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({
            "code": "error",
            "message": e.to_string(),
        })),
    )
        .into_response()
}

fn not_found(id: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "not_found",
            "message": format!("reference {id} not found"),
        })),
    )
        .into_response()
}
