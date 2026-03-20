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
    /// Beacon v2 query: referenceBases for exact match (HelixTest uses it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_bases: Option<String>,
    /// Beacon v2 query: alternateBases for exact match (HelixTest uses it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternate_bases: Option<String>,
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

// Learned from HelixTest: Beacon v2 `/query` payload is wrapped.
// HelixTest sends:
// { "meta": { "apiVersion": "v2.0.0" }, "query": { "requestParameters": {...} } }
#[derive(Debug, Deserialize)]
pub struct BeaconQueryEnvelope {
    pub meta: serde_json::Value,
    pub query: BeaconQuery,
}

#[derive(Debug, Deserialize)]
pub struct BeaconQuery {
    #[serde(rename = "requestParameters")]
    pub request_parameters: BeaconRequestParameters,
}

#[derive(Debug, Deserialize)]
pub struct BeaconRequestParameters {
    #[serde(rename = "assemblyId")]
    pub assembly_id: Option<String>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    #[serde(rename = "start")]
    pub start: Option<i64>,
    // HelixTest v2 currently only sends `start` for SNV-style existence checks.
    // For our minimal support, we treat missing `end` as `end = start`.
    #[serde(rename = "end")]
    pub end: Option<i64>,
    #[serde(rename = "referenceBases")]
    pub reference_bases: Option<String>,
    #[serde(rename = "alternateBases")]
    pub alternate_bases: Option<String>,
    /// Beacon v2 requested granularity (e.g. "count"). For completeness.
    #[serde(rename = "requestedGranularity")]
    pub requested_granularity: Option<String>,
}

fn envelope_to_variant_query(envelope: BeaconQueryEnvelope) -> VariantQueryRequest {
    let p = envelope.query.request_parameters;
    VariantQueryRequest {
        assembly_id: p.assembly_id,
        reference_name: p.reference_name,
        start: p.start,
        end: p.end,
        reference_bases: p.reference_bases,
        alternate_bases: p.alternate_bases,
        granularity: p.requested_granularity,
    }
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
    Json(envelope): Json<BeaconQueryEnvelope>,
) -> Result<Json<VariantQueryResponse>> {
    // `meta` is currently informational only; HelixTest validates shape, not usage.
    let _ = &envelope.meta;
    let body = envelope_to_variant_query(envelope);
    let end = body.end.or(body.start);
    let sanitized = crate::query::sanitize::sanitize_query_params(
        body.assembly_id.as_deref(),
        body.reference_name.as_deref(),
        body.start,
        end,
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

    // HelixTest v2 supplies referenceBases/alternateBases. Our DB supports exact matching,
    // but we also allow missing values (e.g. Level0/Level1 reachability tests).
    let reference = body.reference_bases.as_deref();
    let alternate = body.alternate_bases.as_deref();

    match parse_granularity(body.granularity.as_deref())? {
        VariantGranularity::Boolean => {
            let exists = state
                .repo
                .variant_exists(&dataset_id, &chromosome, start, end, reference, alternate)
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
                .variant_count(&dataset_id, &chromosome, start, end, reference, alternate)
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
