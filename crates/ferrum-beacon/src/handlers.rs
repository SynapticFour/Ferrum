//! Beacon v2 handlers.

use crate::error::Result;
use crate::repo::BeaconRepo;
use axum::extract::{Extension, Query, State};
use axum::Json;
use ferrum_core::OutbreakService;
use ferrum_core::ResidencyAuditLog;
use ferrum_federation::FederationClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// GA4GH Beacon v2 response meta schema URL.
pub const BEACON_RESPONSE_META_SCHEMA: &str =
    "https://raw.githubusercontent.com/ga4gh-beacon/beacon-v2/main/framework/json/schemas/beaconResponseMeta.json";

fn beacon_meta_base() -> serde_json::Value {
    serde_json::json!({
        "$schema": BEACON_RESPONSE_META_SCHEMA,
        "requestedSchemas": [],
        "apiVersion": "v2.0"
    })
}

fn pathogen_filtering_terms() -> serde_json::Value {
    serde_json::json!([
        {
            "id": "PathoGenFilter",
            "label": "Pathogen surveillance filter",
            "scope": "beacon",
            "query": "organism,amrGene,serotype,minQscore"
        }
    ])
}

pub struct AppState {
    pub repo: Arc<BeaconRepo>,
    pub outbreak: Option<Arc<OutbreakService>>,
    pub federation: Option<Arc<FederationClient>>,
    pub residency_audit: Option<Arc<ResidencyAuditLog>>,
    pub reference_registry: Option<Arc<ferrum_reference::ReferenceRegistry>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BeaconFilter {
    pub id: String,
    /// PathoGenFilter extension fields (when `id` is `PathoGenFilter`).
    #[serde(default)]
    pub organism: Option<String>,
    #[serde(rename = "amrGene", alias = "amr_gene", default)]
    pub amr_gene: Option<String>,
    #[serde(default)]
    pub serotype: Option<String>,
    #[serde(rename = "minQscore", alias = "min_qscore", default)]
    pub min_qscore: Option<f32>,
}

/// Beacon v2 encodes OR in `query.filters` as nested arrays.
///
/// A filter item can be either:
/// - `{ "id": "..." }` (single filter)
/// - `[{ "id": "..." }, { "id": "..." }]` (OR group; any of the nested filters)
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum BeaconFilterExpr {
    Single(BeaconFilter),
    OrGroup(Vec<BeaconFilter>),
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
    #[serde(rename = "filters")]
    pub filters: Option<Vec<BeaconFilterExpr>>,
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
    /// Multi-pathogen filter: NCBI taxonomy ID or free text organism name.
    #[serde(default)]
    pub organism: Option<String>,
    /// AMR gene symbol filter (e.g. blaNDM-1).
    #[serde(rename = "amrGene", default)]
    pub amr_gene: Option<String>,
    #[serde(default)]
    pub serotype: Option<String>,
    #[serde(rename = "minQscore", default)]
    pub min_qscore: Option<f32>,
}

fn envelope_to_variant_query_with_filters(
    envelope: BeaconQueryEnvelope,
) -> (
    VariantQueryRequest,
    Vec<BeaconFilterExpr>,
    PathogenFilterParams,
) {
    let BeaconQueryEnvelope { query, .. } = envelope;
    let filters = query.filters.unwrap_or_default();
    let p = query.request_parameters;
    let pathogen = PathogenFilterParams {
        organism: p.organism.clone(),
        amr_gene: p.amr_gene.clone(),
        serotype: p.serotype.clone(),
        min_qscore: p.min_qscore,
    };
    (
        VariantQueryRequest {
            assembly_id: p.assembly_id,
            reference_name: p.reference_name,
            start: p.start,
            end: p.end,
            reference_bases: p.reference_bases,
            alternate_bases: p.alternate_bases,
            granularity: p.requested_granularity,
        },
        filters,
        pathogen,
    )
}

#[derive(Debug, Clone, Default)]
pub struct PathogenFilterParams {
    pub organism: Option<String>,
    pub amr_gene: Option<String>,
    pub serotype: Option<String>,
    pub min_qscore: Option<f32>,
}

#[utoipa::path(get, path = "/service-info", responses((status = 200)))]
pub async fn get_service_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "meta": beacon_meta_base(),
        "id": "ferrum-beacon",
        "name": "Ferrum Beacon v2",
        "version": env!("CARGO_PKG_VERSION"),
        "filteringTerms": pathogen_filtering_terms(),
    }))
}

#[utoipa::path(get, path = "/info", responses((status = 200, body = BeaconInfoResponse)))]
pub async fn get_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "meta": beacon_meta_base(),
        "id": "ferrum-beacon",
        "name": "Ferrum Beacon",
        "api_version": "v2.0",
        "filteringTerms": pathogen_filtering_terms(),
    }))
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(envelope): Json<BeaconQueryEnvelope>,
) -> Result<Json<VariantQueryResponse>> {
    let _ = &envelope.meta;
    if let Some(ref audit) = state.residency_audit {
        let requester = auth.as_ref().and_then(|a| a.0.sub());
        audit
            .append_warn("beacon_query", None, requester, None, false, None)
            .await;
    }
    let (body, filters_exprs, pathogen_base) = envelope_to_variant_query_with_filters(envelope);
    let pathogen = crate::pathogen::merge_pathogen_params(pathogen_base, &filters_exprs);

    if crate::pathogen::has_pathogen_params(&pathogen) {
        if let (Some(ref outbreak), Some(claims)) = (&state.outbreak, auth.as_ref().map(|e| &e.0)) {
            if let (Some(recipient), Some(organism)) =
                (claims.recipient_identity(), pathogen.organism.as_deref())
            {
                if outbreak
                    .emergency_beacon_access(recipient, organism)
                    .await
                    .unwrap_or(false)
                {
                    let active = outbreak.active_policies().await.unwrap_or_default();
                    for policy in active {
                        let _ = outbreak
                            .audit_beacon_query(
                                &policy,
                                claims.sub().unwrap_or("unknown"),
                                recipient,
                                organism,
                                "pathogen_filter",
                            )
                            .await;
                    }
                }
            }
        }
        return Ok(Json(
            run_pathogen_query(
                &state,
                pathogen.organism.as_deref(),
                pathogen.amr_gene.as_deref(),
                pathogen.serotype.as_deref(),
                pathogen.min_qscore,
                body.granularity.as_deref(),
            )
            .await?,
        ));
    }

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
                // Beacon conformance: invalid/unknown `assemblyId` must be treated as
                // a client error (400), not as a missing resource (404).
                crate::error::BeaconError::Validation(format!("invalid assembly_id '{aid}'"))
            })?,
        None => "default".to_string(),
    };

    let chromosome = sanitized.reference_name;
    let start = sanitized.start;
    let end = sanitized.end;

    // HelixTest v2 supplies referenceBases/alternateBases. We sanitize them before any DB
    // interaction (EGA lesson: reject injection vectors early).
    let reference = crate::query::sanitize::sanitize_bases(body.reference_bases.as_deref())?;
    let alternate = crate::query::sanitize::sanitize_bases(body.alternate_bases.as_deref())?;
    let reference_ref = reference.as_deref();
    let alternate_ref = alternate.as_deref();

    match parse_granularity(body.granularity.as_deref())? {
        VariantGranularity::Boolean => {
            let exists = if filters_exprs.is_empty() {
                state
                    .repo
                    .variant_exists(
                        &dataset_id,
                        &chromosome,
                        start,
                        end,
                        reference_ref,
                        alternate_ref,
                    )
                    .await?
            } else {
                // OR semantics from Beacon:
                // - top-level filter items are AND-ed
                // - nested arrays inside `filters` represent OR alternatives
                use std::collections::HashSet;

                let mut current: Option<HashSet<i64>> = None;
                for expr in filters_exprs {
                    let expr_ids: Vec<i64> = match expr {
                        BeaconFilterExpr::Single(f) => {
                            let fid = crate::query::sanitize::sanitize_filter_id(&f.id)?;
                            let fid_up = fid.to_ascii_uppercase();

                            if matches!(fid_up.as_str(), "A" | "C" | "G" | "T" | "N") {
                                // Approximation: nucleotide filters match either reference or alternate.
                                let ids_ref = state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        Some(fid_up.as_str()),
                                        alternate_ref,
                                        None,
                                    )
                                    .await?;
                                let ids_alt = state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        reference_ref,
                                        Some(fid_up.as_str()),
                                        None,
                                    )
                                    .await?;
                                ids_ref.into_iter().chain(ids_alt).collect()
                            } else {
                                // Default mapping: treat filter id as variant_type selector.
                                state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        reference_ref,
                                        alternate_ref,
                                        Some(fid_up.as_str()),
                                    )
                                    .await?
                            }
                        }
                        BeaconFilterExpr::OrGroup(group) => {
                            // OR within group: union of alternatives.
                            let mut out: Vec<i64> = Vec::new();
                            for f in group {
                                let fid = crate::query::sanitize::sanitize_filter_id(&f.id)?;
                                let fid_up = fid.to_ascii_uppercase();

                                if matches!(fid_up.as_str(), "A" | "C" | "G" | "T" | "N") {
                                    let ids_ref = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            Some(fid_up.as_str()),
                                            alternate_ref,
                                            None,
                                        )
                                        .await?;
                                    let ids_alt = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            reference_ref,
                                            Some(fid_up.as_str()),
                                            None,
                                        )
                                        .await?;
                                    out.extend(ids_ref);
                                    out.extend(ids_alt);
                                } else {
                                    let ids = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            reference_ref,
                                            alternate_ref,
                                            Some(fid_up.as_str()),
                                        )
                                        .await?;
                                    out.extend(ids);
                                }
                            }
                            out
                        }
                    };

                    let set: HashSet<i64> = expr_ids.into_iter().collect();
                    current = Some(match current {
                        None => set,
                        Some(prev) => prev.intersection(&set).copied().collect(),
                    });
                }

                current.map(|s| !s.is_empty()).unwrap_or(false)
            };
            Ok(Json(VariantQueryResponse {
                meta: beacon_meta_base(),
                response: VariantQueryResult {
                    exists: Some(exists),
                    count: None,
                },
            }))
        }
        VariantGranularity::Count => {
            let count = if filters_exprs.is_empty() {
                state
                    .repo
                    .variant_count(
                        &dataset_id,
                        &chromosome,
                        start,
                        end,
                        reference_ref,
                        alternate_ref,
                    )
                    .await?
            } else {
                use std::collections::HashSet;
                let mut current: Option<HashSet<i64>> = None;

                for expr in filters_exprs {
                    let expr_ids: Vec<i64> = match expr {
                        BeaconFilterExpr::Single(f) => {
                            let fid = crate::query::sanitize::sanitize_filter_id(&f.id)?;
                            let fid_up = fid.to_ascii_uppercase();

                            if matches!(fid_up.as_str(), "A" | "C" | "G" | "T" | "N") {
                                let ids_ref = state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        Some(fid_up.as_str()),
                                        alternate_ref,
                                        None,
                                    )
                                    .await?;
                                let ids_alt = state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        reference_ref,
                                        Some(fid_up.as_str()),
                                        None,
                                    )
                                    .await?;
                                ids_ref.into_iter().chain(ids_alt).collect()
                            } else {
                                state
                                    .repo
                                    .variant_match_ids(
                                        &dataset_id,
                                        &chromosome,
                                        start,
                                        end,
                                        reference_ref,
                                        alternate_ref,
                                        Some(fid_up.as_str()),
                                    )
                                    .await?
                            }
                        }
                        BeaconFilterExpr::OrGroup(group) => {
                            let mut out: Vec<i64> = Vec::new();
                            for f in group {
                                let fid = crate::query::sanitize::sanitize_filter_id(&f.id)?;
                                let fid_up = fid.to_ascii_uppercase();

                                if matches!(fid_up.as_str(), "A" | "C" | "G" | "T" | "N") {
                                    let ids_ref = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            Some(fid_up.as_str()),
                                            alternate_ref,
                                            None,
                                        )
                                        .await?;
                                    let ids_alt = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            reference_ref,
                                            Some(fid_up.as_str()),
                                            None,
                                        )
                                        .await?;
                                    out.extend(ids_ref);
                                    out.extend(ids_alt);
                                } else {
                                    let ids = state
                                        .repo
                                        .variant_match_ids(
                                            &dataset_id,
                                            &chromosome,
                                            start,
                                            end,
                                            reference_ref,
                                            alternate_ref,
                                            Some(fid_up.as_str()),
                                        )
                                        .await?;
                                    out.extend(ids);
                                }
                            }
                            out
                        }
                    };

                    let set: HashSet<i64> = expr_ids.into_iter().collect();
                    current = Some(match current {
                        None => set,
                        Some(prev) => prev.intersection(&set).copied().collect(),
                    });
                }

                current.map(|s| s.len() as i64).unwrap_or(0)
            };
            Ok(Json(VariantQueryResponse {
                meta: beacon_meta_base(),
                response: VariantQueryResult {
                    exists: None,
                    count: Some(count),
                },
            }))
        }
    }
}

async fn run_pathogen_query(
    state: &AppState,
    organism: Option<&str>,
    amr_gene: Option<&str>,
    serotype: Option<&str>,
    min_qscore: Option<f32>,
    granularity: Option<&str>,
) -> Result<VariantQueryResponse> {
    let meta = beacon_meta_with_reference(state, None, organism).await;
    match parse_granularity(granularity)? {
        VariantGranularity::Boolean => {
            let exists = state
                .repo
                .pathogen_exists(organism, amr_gene, serotype, min_qscore)
                .await?;
            Ok(VariantQueryResponse {
                meta,
                response: VariantQueryResult {
                    exists: Some(exists),
                    count: None,
                },
            })
        }
        VariantGranularity::Count => {
            let count = state
                .repo
                .pathogen_count(organism, amr_gene, serotype, min_qscore)
                .await?;
            Ok(VariantQueryResponse {
                meta,
                response: VariantQueryResult {
                    exists: None,
                    count: Some(count),
                },
            })
        }
    }
}

async fn beacon_meta_with_reference(
    state: &AppState,
    assembly_id: Option<&str>,
    organism: Option<&str>,
) -> serde_json::Value {
    let mut meta = beacon_meta_base();
    let Some(ref registry) = state.reference_registry else {
        return meta;
    };
    let ref_id = if let Some(aid) = assembly_id {
        registry.get(aid).await.ok().flatten().map(|r| r.id)
    } else if let Some(org) = organism {
        registry
            .list()
            .await
            .ok()
            .and_then(|all| all.into_iter().find(|r| r.organism == org).map(|r| r.id))
    } else {
        None
    };
    if let Some(id) = ref_id {
        meta["referenceGenome"] = serde_json::json!(id);
    }
    meta
}

#[derive(Debug, Deserialize)]
pub struct GetVariantsQuery {
    #[serde(default)]
    pub federate: Option<bool>,
    #[serde(rename = "assemblyId")]
    pub assembly_id: Option<String>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    #[serde(rename = "referenceBases")]
    pub reference_bases: Option<String>,
    #[serde(rename = "alternateBases")]
    pub alternate_bases: Option<String>,
    #[serde(rename = "requestedGranularity")]
    pub requested_granularity: Option<String>,
    pub organism: Option<String>,
    #[serde(rename = "amrGene")]
    pub amr_gene: Option<String>,
    pub serotype: Option<String>,
    #[serde(rename = "minQscore")]
    pub min_qscore: Option<f32>,
}

#[utoipa::path(get, path = "/g_variants", responses((status = 200, body = VariantQueryResponse)))]
pub async fn get_g_variants(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GetVariantsQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<VariantQueryResponse>> {
    let federate = q.federate.unwrap_or(false);
    let requester = auth.as_ref().and_then(|a| a.0.sub());

    if let Some(ref audit) = state.residency_audit {
        audit
            .append_warn("beacon_query", None, requester, None, false, None)
            .await;
    }

    let (local_exists, local_count) =
        local_variant_result(&state, &q, q.requested_granularity.as_deref()).await?;

    let params = ferrum_federation::BeaconQueryParams {
        assembly_id: q.assembly_id.clone(),
        reference_name: q.reference_name.clone(),
        start: q.start,
        end: q.end.or(q.start),
        reference_bases: q.reference_bases.clone(),
        alternate_bases: q.alternate_bases.clone(),
        granularity: q.requested_granularity.clone(),
        organism: q.organism.clone(),
        amr_gene: q.amr_gene.clone(),
        serotype: q.serotype.clone(),
        min_qscore: q.min_qscore,
    };

    Ok(Json(
        crate::federation::maybe_federate_get(
            &state,
            params,
            federate,
            local_exists,
            local_count,
            requester,
        )
        .await,
    ))
}

async fn local_variant_result(
    state: &AppState,
    q: &GetVariantsQuery,
    granularity: Option<&str>,
) -> Result<(Option<bool>, Option<i64>)> {
    let pathogen = PathogenFilterParams {
        organism: q.organism.clone(),
        amr_gene: q.amr_gene.clone(),
        serotype: q.serotype.clone(),
        min_qscore: q.min_qscore,
    };
    if crate::pathogen::has_pathogen_params(&pathogen) {
        let resp = run_pathogen_query(
            state,
            pathogen.organism.as_deref(),
            pathogen.amr_gene.as_deref(),
            pathogen.serotype.as_deref(),
            pathogen.min_qscore,
            granularity,
        )
        .await?;
        return Ok((resp.response.exists, resp.response.count));
    }

    let end = q.end.or(q.start);
    let sanitized = crate::query::sanitize::sanitize_query_params(
        q.assembly_id.as_deref(),
        q.reference_name.as_deref(),
        q.start,
        end,
    )?;
    let dataset_id = match sanitized.assembly_id.as_deref() {
        Some(aid) => state
            .repo
            .dataset_id_for_assembly(aid)
            .await?
            .ok_or_else(|| {
                crate::error::BeaconError::Validation(format!("invalid assembly_id '{aid}'"))
            })?,
        None => "default".to_string(),
    };
    let reference = crate::query::sanitize::sanitize_bases(q.reference_bases.as_deref())?;
    let alternate = crate::query::sanitize::sanitize_bases(q.alternate_bases.as_deref())?;

    match parse_granularity(granularity)? {
        VariantGranularity::Boolean => {
            let exists = state
                .repo
                .variant_exists(
                    &dataset_id,
                    &sanitized.reference_name,
                    sanitized.start,
                    sanitized.end,
                    reference.as_deref(),
                    alternate.as_deref(),
                )
                .await?;
            Ok((Some(exists), None))
        }
        VariantGranularity::Count => {
            let count = state
                .repo
                .variant_count(
                    &dataset_id,
                    &sanitized.reference_name,
                    sanitized.start,
                    sanitized.end,
                    reference.as_deref(),
                    alternate.as_deref(),
                )
                .await?;
            Ok((None, Some(count)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_granularity_defaults_to_boolean() {
        assert_eq!(
            parse_granularity(None).unwrap(),
            VariantGranularity::Boolean
        );
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
        "meta": beacon_meta_base(),
        "response": { "individuals": [] }
    }))
}

#[utoipa::path(post, path = "/biosamples/query", responses((status = 200)))]
pub async fn query_biosamples() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "meta": beacon_meta_base(),
        "response": { "biosamples": [] }
    }))
}
