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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantGranularity {
    Boolean,
    Count,
}

fn parse_granularity(granularity: Option<&str>) -> Result<VariantGranularity> {
    let g = granularity
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "boolean".to_string());

    match g.as_str() {
        "boolean" => Ok(VariantGranularity::Boolean),
        "count" => Ok(VariantGranularity::Count),
        "record" => Err(crate::error::BeaconError::Validation(
            "record granularity is not supported".into(),
        )),
        other => Err(crate::error::BeaconError::Validation(format!(
            "invalid granularity '{other}' (expected boolean|count|record)"
        ))),
    }
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
    /// Beacon v2 granularity selector.
    /// Supported here: `boolean` and `count`.
    /// `record` is rejected (Ferrum Beacon currently does not serve records).
    pub granularity: Option<String>,
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
    let sanitized = crate::query::sanitize::sanitize_query_params(
        body.assembly_id.as_deref(),
        body.reference_name.as_deref(),
        body.start,
        body.end,
    )?;

    let dataset_id = match sanitized.assembly_id.as_deref() {
        Some(aid) => state
            .repo
            .dataset_id_for_assembly(aid)
            .await?
            .ok_or_else(|| {
                crate::error::BeaconError::NotFound(format!(
                    "no dataset for assembly_id '{aid}'"
                ))
            })?,
        None => "default".to_string(),
    };

    let chromosome = sanitized.reference_name;
    let start = sanitized.start;
    let end = sanitized.end;

    match parse_granularity(body.granularity.as_deref())? {
        VariantGranularity::Boolean => {
            let exists = state
                .repo
                .variant_exists(&dataset_id, &chromosome, start, end)
                .await?;
            Ok(Json(VariantQueryResponse {
                meta: serde_json::json!({ "requestedSchemas": [], "apiVersion": "v2.0" }),
                response: VariantQueryResult {
                    exists: Some(exists),
                    count: None,
                },
            }))
        }
        VariantGranularity::Count => {
            let count = state
                .repo
                .variant_count(&dataset_id, &chromosome, start, end)
                .await?;
            Ok(Json(VariantQueryResponse {
                meta: serde_json::json!({ "requestedSchemas": [], "apiVersion": "v2.0" }),
                response: VariantQueryResult { exists: None, count: Some(count) },
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_granularity_defaults_to_boolean() {
        assert_eq!(parse_granularity(None).unwrap(), VariantGranularity::Boolean);
    }

    #[test]
    fn test_parse_granularity_count() {
        assert_eq!(
            parse_granularity(Some("count")).unwrap(),
            VariantGranularity::Count
        );
    }

    #[test]
    fn test_parse_granularity_record_rejected() {
        assert!(parse_granularity(Some("record")).is_err());
    }
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
