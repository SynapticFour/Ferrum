//! Beacon v2 handlers.

use crate::error::Result;
use crate::repo::BeaconRepo;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

pub struct AppState {
    pub repo: Arc<BeaconRepo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BeaconInfoResponse {
    pub id: String,
    pub name: String,
    pub api_version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VariantQueryRequest {
    pub assembly_id: Option<String>,
    pub reference_name: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VariantQueryResponse {
    pub meta: serde_json::Value,
    pub response: VariantQueryResult,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VariantQueryResult {
    pub exists: Option<bool>,
    pub count: Option<i64>,
}

#[utoipa::path(get, path = "/service-info", responses((status = 200)))]
pub async fn get_service_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": "ferrum-beacon",
        "name": "Ferrum Beacon v2",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[utoipa::path(get, path = "/info", responses((status = 200, body = BeaconInfoResponse)))]
pub async fn get_info() -> Json<BeaconInfoResponse> {
    Json(BeaconInfoResponse {
        id: "ferrum-beacon".to_string(),
        name: "Ferrum Beacon".to_string(),
        api_version: "v2.0".to_string(),
    })
}

#[utoipa::path(get, path = "/map", responses((status = 200)))]
pub async fn get_map(State(_state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    let map = serde_json::json!({
        "endpointSets": {},
        "entryTypes": {
            "g_variants": { "open": true, "id": "g_variants" },
            "individuals": { "open": true, "id": "individuals" },
            "biosamples": { "open": true, "id": "biosamples" }
        }
    });
    Ok(Json(map))
}

#[utoipa::path(post, path = "/g_variants/query", request_body = VariantQueryRequest, responses((status = 200, body = VariantQueryResponse)))]
pub async fn query_variants(
    State(state): State<Arc<AppState>>,
    Json(body): Json<VariantQueryRequest>,
) -> Result<Json<VariantQueryResponse>> {
    let dataset_id = "default";
    let chromosome = body.reference_name.as_deref().unwrap_or("1");
    let start = body.start.unwrap_or(0);
    let end = body.end.unwrap_or(999999999);
    let exists = state.repo.variant_exists(dataset_id, chromosome, start, end).await?;
    Ok(Json(VariantQueryResponse {
        meta: serde_json::json!({ "requestedSchemas": [], "apiVersion": "v2.0" }),
        response: VariantQueryResult {
            exists: Some(exists),
            count: None,
        },
    }))
}

#[utoipa::path(post, path = "/individuals/query", responses((status = 200)))]
pub async fn query_individuals() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "meta": { "apiVersion": "v2.0" },
        "response": { "individuals": [] }
    }))
}

#[utoipa::path(post, path = "/biosamples/query", responses((status = 200)))]
pub async fn query_biosamples() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "meta": { "apiVersion": "v2.0" },
        "response": { "biosamples": [] }
    }))
}
