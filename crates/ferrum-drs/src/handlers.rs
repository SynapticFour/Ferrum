//! DRS HTTP handlers.

use crate::error::{DrsError, Result};
use crate::state::AppState;
use crate::types::*;
use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use tokio::io::AsyncReadExt;
use ferrum_core::{ServiceInfo, ServiceType, Organization};
use std::sync::Arc;
use utoipa::ToSchema;

/// GA4GH service-info (DRS).
#[utoipa::path(get, path = "/service-info", responses((status = 200, body = ferrum_core::ServiceInfo)))]
pub async fn get_service_info() -> Json<ServiceInfo> {
    Json(ServiceInfo {
        id: "ferrum-drs".to_string(),
        name: "Ferrum DRS".to_string(),
        service_type: ServiceType {
            group: "org.ga4gh".to_string(),
            artifact: "drs".to_string(),
            version: "1.4.0".to_string(),
        },
        description: Some("GA4GH Data Repository Service 1.4".to_string()),
        organization: Some(Organization {
            name: "Ferrum".to_string(),
            url: None,
        }),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Get DRS object by id, alias, or drs://hostname/id URI. Use ?expand=true for bundle contents.
#[utoipa::path(
    get,
    path = "/objects/{object_id}",
    params(ExpandQuery),
    responses((status = 200, body = DrsObject, description = "Object metadata and access methods"), (status = 404, description = "Not found"))
)]
pub async fn get_object(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    Query(params): Query<ExpandQuery>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<DrsObject>> {
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| DrsError::Forbidden("authentication required for this dataset".into()))?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }
    let obj = state
        .repo
        .get_object(&canonical, params.expand.unwrap_or(false))
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let _ = state.repo.log_access(&canonical, None, "GET", 200, client_ip.as_deref()).await;
    Ok(Json(obj))
}

/// Query params for GET /objects/{object_id}. expand=true returns bundle contents recursively.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, ToSchema)]
pub struct ExpandQuery {
    /// If true, expand bundle contents (and nested bundles).
    pub expand: Option<bool>,
}

/// OPTIONS /objects/{object_id}: authorization discovery (DRS 1.4). Returns 204 when not supported.
#[utoipa::path(
    get,
    path = "/objects/{object_id}",
    responses((status = 204, description = "Authorizations not supported"))
)]
pub async fn options_object() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// Get access URL for an access_id (e.g. presigned URL).
#[utoipa::path(
    get,
    path = "/objects/{object_id}/access/{access_id}",
    responses((status = 200, body = AccessUrl), (status = 404, description = "Not found"))
)]
pub async fn get_access(
    State(state): State<Arc<AppState>>,
    Path((object_id, access_id)): Path<(String, String)>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<AccessUrl>> {
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| DrsError::Forbidden("authentication required for this dataset".into()))?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }
    let mut url = state
        .repo
        .get_access_url(&canonical, &access_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("access_id not found: {}", access_id)))?;

    let range = parse_range_header(headers.get("range"));
    let storage_ref = state.repo.get_storage_ref(&canonical).await?;
    if let (Some((backend, key, _)), Some(presigner)) = (storage_ref, state.s3_presigner.as_ref()) {
        if backend.eq_ignore_ascii_case("s3") || backend.eq_ignore_ascii_case("minio") {
            let expires = std::time::Duration::from_secs(3600);
            match presigner.presign(key.as_str(), range, expires).await {
                Ok(presigned) => url.url = presigned,
                Err(e) => tracing::warn!(?e, "presign failed, returning placeholder URL"),
            }
        }
    }

    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let _ = state.repo.log_access(&canonical, Some(access_id.as_str()), "GET", 200, client_ip.as_deref()).await;
    Ok(Json(url))
}

/// GET /objects/{object_id}/view — serve object body as HTML for browser viewing. Only for mime_type text/html; same auth as normal access.
#[utoipa::path(
    get,
    path = "/objects/{object_id}/view",
    responses((status = 200, description = "HTML body"), (status = 400, description = "Not text/html"), (status = 404), (status = 501, description = "View not supported (e.g. encrypted or remote storage)"))
)]
pub async fn get_object_view(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> crate::error::Result<axum::response::Response> {
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| DrsError::Forbidden("authentication required for this dataset".into()))?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }
    let obj = state
        .repo
        .get_object(&canonical, false)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let mime = obj.mime_type.as_deref().unwrap_or("").trim().to_lowercase();
    if mime != "text/html" {
        return Err(DrsError::Validation("view only allowed for mime_type text/html".into()));
    }
    let storage_ref = state
        .repo
        .get_storage_ref(&canonical)
        .await?
        .ok_or_else(|| DrsError::NotFound("no storage reference".into()))?;
    let (backend, key, is_encrypted) = storage_ref;
    if is_encrypted {
        return Err(DrsError::Other(anyhow::anyhow!(
            "view not available for encrypted objects; use access URL with Crypt4GH"
        )));
    }
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| DrsError::Other(anyhow::anyhow!("storage not configured")))?;
    if !backend.eq_ignore_ascii_case("local") {
        return Err(DrsError::Other(anyhow::anyhow!(
            "view only supported for local storage"
        )));
    }
    let mut reader = storage.get(&key).await.map_err(|e| DrsError::Other(e.into()))?;
    let mut body = Vec::new();
    reader.read_to_end(&mut body).await.map_err(|e| DrsError::Other(e.into()))?;
    let _ = state.repo.log_access(&canonical, None, "GET/view", 200, None).await;
    let res = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", HeaderValue::from_static("text/html; charset=utf-8"))
        .body(axum::body::Body::from(body))
        .map_err(|e| DrsError::Other(e.into()))?;
    Ok(res)
}

/// Parse Range header (e.g. "bytes=0-1023") into (start, end) inclusive. Returns None if missing or invalid.
fn parse_range_header(value: Option<&axum::http::HeaderValue>) -> Option<(u64, u64)> {
    let s = value?.to_str().ok()?.strip_prefix("bytes=")?;
    let (start, end) = s.split_once('-')?;
    let start: u64 = start.parse().ok()?;
    let end: u64 = end.parse().ok()?;
    if start <= end { Some((start, end)) } else { None }
}

/// Create object (admin).
#[utoipa::path(
    post,
    path = "/objects",
    request_body = CreateObjectRequest,
    responses((status = 200, body = CreatedResponse))
)]
pub async fn post_object(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateObjectRequest>,
) -> Result<Json<CreatedResponse>> {
    let id = state.repo.create_object(&req).await?;
    Ok(Json(CreatedResponse { id }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct CreatedResponse {
    pub id: String,
}

/// Update object (admin).
#[utoipa::path(
    put,
    path = "/objects/{object_id}",
    request_body = UpdateObjectRequest,
    responses((status = 200, body = UpdatedResponse), (status = 404, description = "Not found"))
)]
pub async fn put_object(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    Json(req): Json<UpdateObjectRequest>,
) -> Result<Json<UpdatedResponse>> {
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let updated = state.repo.update_object(&canonical, &req).await?;
    Ok(Json(UpdatedResponse { updated }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct UpdatedResponse {
    pub updated: bool,
}

/// Delete object (admin).
#[utoipa::path(
    delete,
    path = "/objects/{object_id}",
    responses((status = 200, body = DeletedResponse), (status = 404, description = "Not found"))
)]
pub async fn delete_object(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
) -> Result<Json<DeletedResponse>> {
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let deleted = state.repo.delete_object(&canonical).await?;
    Ok(Json(DeletedResponse { deleted }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct DeletedResponse {
    pub deleted: bool,
}

/// List objects (admin) with pagination and filters.
#[utoipa::path(
    get,
    path = "/objects",
    responses((status = 200, body = [DrsObject]))
)]
pub async fn list_objects(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListObjectsQuery>,
) -> Result<Json<Vec<DrsObject>>> {
    let limit = q.limit.unwrap_or(100).min(1000);
    let offset = q.offset.unwrap_or(0);
    let list = state
        .repo
        .list_objects(
            limit,
            offset,
            q.mime_type.as_deref(),
            q.min_size,
            q.max_size,
        )
        .await?;
    Ok(Json(list))
}

/// Query params for GET /objects/{object_id}/provenance
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, ToSchema)]
pub struct ProvenanceQuery {
    /// upstream | downstream | both
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default = "default_depth")]
    pub depth: Option<u32>,
}

fn default_direction() -> String {
    "upstream".to_string()
}
fn default_depth() -> Option<u32> {
    Some(10)
}

/// GET /objects/{object_id}/provenance — lineage graph (upstream/downstream/both).
#[utoipa::path(
    get,
    path = "/objects/{object_id}/provenance",
    params(ProvenanceQuery),
    responses((status = 200, description = "Provenance graph"), (status = 404, description = "Not found"), (status = 503, description = "Provenance not configured"))
)]
pub async fn get_object_provenance(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    Query(q): Query<ProvenanceQuery>,
) -> Result<Json<ProvenanceResponse>> {
    let store = state
        .provenance_store
        .as_ref()
        .ok_or_else(|| DrsError::Other(anyhow::anyhow!("provenance not configured")))?;
    let canonical = state
        .repo
        .resolve_id_or_uri(&object_id)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let depth = q.depth.unwrap_or(10).min(20).max(1);
    let graph = match q.direction.as_str() {
        "downstream" => store.downstream(&canonical, depth).await?,
        "both" => store.both(&canonical, depth).await?,
        _ => store.upstream(&canonical, depth).await?,
    };
    Ok(Json(ProvenanceResponse {
        object_id: canonical,
        direction: q.direction.clone(),
        graph: ProvenanceGraphResponse {
            nodes: graph.nodes.clone(),
            edges: graph.edges.clone(),
            mermaid: graph.to_mermaid(),
        },
    }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct ProvenanceResponse {
    pub object_id: String,
    pub direction: String,
    pub graph: ProvenanceGraphResponse,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ProvenanceGraphResponse {
    pub nodes: Vec<ferrum_core::ProvenanceNode>,
    pub edges: Vec<ferrum_core::ProvenanceEdge>,
    pub mermaid: String,
}
