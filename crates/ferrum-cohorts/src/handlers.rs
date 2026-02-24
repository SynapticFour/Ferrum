//! HTTP handlers for Cohort API.

use crate::error::{CohortError, Result};
use crate::query::CohortQuery;
use crate::state::AppState;
use crate::types::*;
use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use utoipa::ToSchema;
use ulid::Ulid;

/// Prefer JWT/Passport sub; fallback to x-owner-sub for backward compat (spoofable — gateway should enforce auth).
fn owner_sub_from_request(auth: Option<&Extension<ferrum_core::AuthClaims>>, headers: &HeaderMap) -> String {
    auth.and_then(|c| c.sub().map(String::from))
        .or_else(|| {
            headers
                .get("x-owner-sub")
                .or_else(|| headers.get("x-passport-sub"))
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        })
        .unwrap_or_else(|| "anonymous".to_string())
}

#[derive(Debug, Deserialize)]
pub struct ListCohortsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListCohortsResponse {
    pub cohorts: Vec<CohortSummary>,
    pub next_offset: Option<i64>,
}

pub async fn list_cohorts(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Query(q): Query<ListCohortsQuery>,
) -> Result<Json<ListCohortsResponse>> {
    let caller_sub = auth.as_ref().and_then(|c| c.sub());
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = q.offset.unwrap_or(0);
    let rows = state
        .repo
        .list_cohorts(caller_sub, q.tag.as_deref(), limit + 1, offset)
        .await?;
    let has_more = rows.len() as i64 > limit;
    let cohorts: Vec<CohortSummary> = rows
        .into_iter()
        .take(limit as usize)
        .map(
            |(id, name, description, owner_sub, workspace_id, version, is_frozen, sample_count, tags, created_at, updated_at)| {
                let tags_vec = tags
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                CohortSummary {
                    id,
                    name,
                    description,
                    owner_sub,
                    workspace_id,
                    version,
                    is_frozen,
                    sample_count,
                    tags: tags_vec,
                    created_at,
                    updated_at,
                }
            },
        )
        .collect();
    Ok(Json(ListCohortsResponse {
        next_offset: if has_more { Some(offset + limit) } else { None },
        cohorts,
    }))
}

pub async fn create_cohort(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    headers: HeaderMap,
    Json(req): Json<CreateCohortRequest>,
) -> Result<Json<CohortDetail>> {
    let owner = owner_sub_from_request(auth.as_ref(), &headers);
    let id = Ulid::new().to_string();
    let tags = serde_json::to_value(req.tags).unwrap_or(json!([]));
    let filter_criteria = req.filter_criteria;
    state
        .repo
        .create_cohort(
            &id,
            &req.name,
            req.description.as_deref(),
            &owner,
            None,
            &tags,
            &filter_criteria,
        )
        .await?;
    let row = state.repo.get_cohort(&id).await?.expect("just created");
    let (_, name, description, owner_sub, workspace_id, version, is_frozen, sample_count, tags_val, filter_criteria_val, created_at, updated_at) = row;
    let tags_vec = tags_val
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(Json(CohortDetail {
        id,
        name,
        description,
        owner_sub,
        workspace_id,
        version,
        is_frozen,
        sample_count,
        tags: tags_vec,
        filter_criteria: filter_criteria_val,
        created_at,
        updated_at,
    }))
}

pub async fn get_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<CohortDetail>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let row = state
        .repo
        .get_cohort(&id)
        .await?
        .ok_or_else(|| CohortError::NotFound(format!("cohort {}", id)))?;
    let (id, name, description, owner_sub, workspace_id, version, is_frozen, sample_count, tags, filter_criteria, created_at, updated_at) = row;
    let tags_vec = tags
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(Json(CohortDetail {
        id,
        name,
        description,
        owner_sub,
        workspace_id,
        version,
        is_frozen,
        sample_count,
        tags: tags_vec,
        filter_criteria,
        created_at,
        updated_at,
    }))
}

pub async fn update_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(req): Json<UpdateCohortRequest>,
) -> Result<Json<CohortDetail>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let tags_opt = req.tags.as_ref().map(|t| serde_json::to_value(t).unwrap_or(json!([])));
    state
        .repo
        .update_cohort(
            &id,
            req.name.as_deref(),
            req.description.as_deref(),
            tags_opt.as_ref(),
            req.filter_criteria.as_ref(),
        )
        .await?;
    get_cohort(State(state), Path(id), auth).await
}

pub async fn delete_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    state.repo.delete_cohort(&id).await?;
    Ok(Json(json!({ "deleted": id })))
}

pub async fn freeze_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<CohortDetail>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    state.repo.freeze_cohort(&id).await?;
    get_cohort(State(state), Path(id), auth).await
}

#[derive(Debug, Deserialize)]
pub struct CloneCohortRequest {
    pub name: Option<String>,
}

pub async fn clone_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    headers: HeaderMap,
    Json(req): Json<CloneCohortRequest>,
) -> Result<Json<CohortDetail>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let owner = owner_sub_from_request(auth.as_ref(), &headers);
    let row = state.repo.get_cohort(&id).await?.ok_or_else(|| CohortError::NotFound(format!("cohort {}", id)))?;
    let (_, name, description, _owner, workspace_id, _v, _frozen, _count, tags, filter_criteria, _, _) = row;
    let new_id = Ulid::new().to_string();
    let new_name = req.name.unwrap_or_else(|| format!("{} (copy)", name));
    state
        .repo
        .create_cohort(
            &new_id,
            &new_name,
            description.as_deref(),
            &owner,
            workspace_id.as_deref(),
            &tags,
            &filter_criteria,
        )
        .await?;
    let samples = state.repo.samples_for_cohort(&id).await?;
    for (sample_id, drs_object_ids, phenotype) in samples {
        let sid = Ulid::new().to_string();
        state.repo.add_sample(&sid, &new_id, &sample_id, &drs_object_ids, &phenotype, &owner).await?;
    }
    get_cohort(State(state), Path(new_id), auth).await
}

// --- Samples ---
#[derive(Debug, Deserialize)]
pub struct ListSamplesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListSamplesResponse {
    pub samples: Vec<CohortSample>,
    pub next_offset: Option<i64>,
}

fn sample_from_row(
    id: String,
    cohort_id: String,
    sample_id: String,
    drs_object_ids: serde_json::Value,
    phenotype: serde_json::Value,
    added_at: chrono::DateTime<Utc>,
    added_by: String,
) -> CohortSample {
    let drs_ids = drs_object_ids
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    CohortSample {
        id,
        cohort_id,
        sample_id,
        drs_object_ids: drs_ids,
        phenotype,
        added_at,
        added_by,
    }
}

pub async fn list_samples(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Query(q): Query<ListSamplesQuery>,
) -> Result<Json<ListSamplesResponse>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);
    let rows = state.repo.list_samples(&id, limit + 1, offset).await?;
    let has_more = rows.len() as i64 > limit;
    let samples: Vec<CohortSample> = rows
        .into_iter()
        .take(limit as usize)
        .map(|(cid, cohort_id, sample_id, drs_object_ids, phenotype, added_at, added_by)| {
            sample_from_row(cid, cohort_id, sample_id, drs_object_ids, phenotype, added_at, added_by)
        })
        .collect();
    Ok(Json(ListSamplesResponse {
        next_offset: if has_more { Some(offset + limit) } else { None },
        samples,
    }))
}

pub async fn add_samples(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    headers: HeaderMap,
    Json(req): Json<AddSamplesBatchRequest>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let owner = owner_sub_from_request(auth.as_ref(), &headers);
    let cohort = state.repo.get_cohort(&id).await?.ok_or_else(|| CohortError::NotFound(format!("cohort {}", id)))?;
    let (_, _, _, _, _, _, frozen, _, _, _, _, _) = cohort;
    if frozen {
        return Err(CohortError::Validation("cohort is frozen".into()));
    }
    let mut added = 0;
    for s in req.samples {
        let sid = Ulid::new().to_string();
        let drs_val = serde_json::to_value(s.drs_object_ids).unwrap_or(json!([]));
        state
            .repo
            .add_sample(&sid, &id, &s.sample_id, &drs_val, &s.phenotype, &owner)
            .await?;
        added += 1;
    }
    Ok(Json(json!({ "added": added })))
}

pub async fn get_sample(
    State(state): State<Arc<AppState>>,
    Path((id, sample_id)): Path<(String, String)>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<CohortSample>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let row = state
        .repo
        .get_sample(&id, &sample_id)
        .await?
        .ok_or_else(|| CohortError::NotFound(format!("sample {} in cohort {}", sample_id, id)))?;
    let (cid, cohort_id, sid, drs_object_ids, phenotype, added_at, added_by) = row;
    Ok(Json(sample_from_row(
        cid, cohort_id, sid, drs_object_ids, phenotype, added_at, added_by,
    )))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSampleRequest {
    pub drs_object_ids: Option<Vec<String>>,
    pub phenotype: Option<serde_json::Value>,
}

pub async fn update_sample(
    State(state): State<Arc<AppState>>,
    Path((id, sample_id)): Path<(String, String)>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(req): Json<UpdateSampleRequest>,
) -> Result<Json<CohortSample>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let drs_opt = req.drs_object_ids.map(|v| serde_json::to_value(v).unwrap_or(json!([])));
    state
        .repo
        .update_sample(&id, &sample_id, drs_opt.as_ref(), req.phenotype.as_ref())
        .await?;
    get_sample(State(state), Path((id, sample_id)), auth).await
}

pub async fn remove_sample(
    State(state): State<Arc<AppState>>,
    Path((id, sample_id)): Path<(String, String)>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    state.repo.remove_sample(&id, &sample_id).await?;
    Ok(Json(json!({ "removed": sample_id })))
}

// --- Phenotype schema ---
pub async fn get_schema(State(state): State<Arc<AppState>>) -> Result<Json<Vec<PhenotypeSchemaField>>> {
    let rows = state.repo.list_phenotype_schema().await?;
    let schema: Vec<PhenotypeSchemaField> = rows
        .into_iter()
        .map(
            |(id, field_name, display_name, field_type, ontology, required, description)| PhenotypeSchemaField {
                id,
                field_name,
                display_name,
                field_type,
                ontology,
                required,
                description,
            },
        )
        .collect();
    Ok(Json(schema))
}

// --- Query ---
pub async fn query_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(q): Json<CohortQuery>,
) -> Result<Json<QueryResult>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let result = q.execute(&id, state.repo.pool()).await?;
    Ok(Json(result))
}

// --- Stats ---
pub async fn cohort_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<CohortStats>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let samples = state.repo.samples_for_cohort(&id).await?;
    let sample_count = samples.len();
    let mut field_filled: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut sex_distribution = std::collections::HashMap::new();
    let total_data_size_bytes: i64 = 0;
    let data_type_breakdown: std::collections::HashMap<String, DataTypeStat> = std::collections::HashMap::new();
    for (_, _drs_ids, phenotype) in &samples {
        if let Some(obj) = phenotype.as_object() {
            for (k, v) in obj {
                let filled = !v.is_null()
                    && !v.as_str().map(|s| s.is_empty()).unwrap_or(true)
                    && !v.as_f64().map(|f| f.to_string() == "0" && v.as_i64() == Some(0)).unwrap_or(false);
                if filled {
                    *field_filled.entry(k.clone()).or_insert(0) += 1;
                }
                if k == "sex" {
                    if let Some(s) = v.as_str() {
                        *sex_distribution.entry(s.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let n = sample_count.max(1) as f64;
    let phenotype_completeness: std::collections::HashMap<String, f64> =
        field_filled.into_iter().map(|(k, count)| (k, count as f64 / n)).collect();
    Ok(Json(CohortStats {
        cohort_id: id,
        sample_count,
        total_data_size_bytes,
        data_type_breakdown,
        phenotype_completeness,
        sex_distribution,
    }))
}

// --- Versions ---
pub async fn list_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<Vec<CohortVersionInfo>>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let rows = state.repo.list_versions(&id).await?;
    let versions: Vec<CohortVersionInfo> = rows
        .into_iter()
        .map(|(vid, version, created_at, created_by, note)| CohortVersionInfo {
            id: vid,
            cohort_id: id.clone(),
            version,
            created_at,
            created_by,
            note,
        })
        .collect();
    Ok(Json(versions))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExportResponse {
    pub format: String,
    pub content: String,
}

pub async fn export_cohort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    if !state.repo.cohort_accessible_by(&id, sub).await? {
        return Err(CohortError::NotFound(format!("cohort {}", id)));
    }
    let _format = q.get("format").map(|s| s.as_str()).unwrap_or("json");
    let cohort = state.repo.get_cohort(&id).await?.ok_or_else(|| CohortError::NotFound(format!("cohort {}", id)))?;
    let (cid, name, description, _, _, version, is_frozen, sample_count, tags, filter_criteria, created_at, updated_at) = cohort;
    let samples = state.repo.samples_for_cohort(&id).await?;
    let samples_json: Vec<serde_json::Value> = samples
        .into_iter()
        .map(|(sample_id, drs_object_ids, phenotype)| {
            json!({
                "sample_id": sample_id,
                "drs_object_ids": drs_object_ids,
                "phenotype": phenotype
            })
        })
        .collect();
    let payload = json!({
        "id": cid,
        "name": name,
        "description": description,
        "version": version,
        "is_frozen": is_frozen,
        "sample_count": sample_count,
        "tags": tags,
        "filter_criteria": filter_criteria,
        "created_at": created_at,
        "updated_at": updated_at,
        "samples": samples_json
    });
    Ok(Json(payload))
}
