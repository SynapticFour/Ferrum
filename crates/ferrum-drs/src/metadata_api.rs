//! Optional Metadata Store HTTP API (`/api/v1/metadata/*`).
//!
//! Opt-in via `[metadata_store] enabled = true` / `FERRUM_METADATA_STORE__ENABLED=true`.
//! Stores and retrieves ferrum-meta submission documents already held in
//! `metadata_submissions` (same table as ingest-time binding).
//!
//! M2: versioning (`If-Match` / `expected_version`), version history, DRS attach/detach.

use crate::metadata::{
    link_object_metadata_ref, store_ferrum_meta_bundle_versioned, unlink_object_metadata_ref,
};
use crate::state::AppState;
use axum::extract::{Extension, Path as AxPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use ferrum_meta_connect::{submission_alias, MetaProfile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Serialize)]
struct ApiErrorBody {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Option<Value>,
}

impl ApiError {
    fn validation(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "validation_error",
            message: msg.into(),
            details: None,
        }
    }

    fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: msg.into(),
            details: None,
        }
    }

    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: msg.into(),
            details: None,
        }
    }

    fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: msg.into(),
            details: None,
        }
    }

    fn not_enabled() -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            code: "not_enabled",
            message: "Metadata Store API disabled ([metadata_store] enabled = false)".into(),
            details: Some(json!({
                "hint": "Set FERRUM_METADATA_STORE__ENABLED=true or [metadata_store] enabled = true"
            })),
        }
    }

    fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: msg.into(),
            details: None,
        }
    }

    fn from_drs(e: crate::error::DrsError) -> Self {
        match e {
            crate::error::DrsError::NotFound(m) => Self::not_found(m),
            crate::error::DrsError::Forbidden(m) => Self::forbidden(m),
            crate::error::DrsError::Validation(m) => Self::validation(m),
            crate::error::DrsError::Conflict(m) => Self::conflict(m),
            crate::error::DrsError::TransferQueued(m) => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "transfer_queued",
                message: m,
                details: None,
            },
            crate::error::DrsError::Database(se) => Self::internal(se.to_string()),
            crate::error::DrsError::Other(o) => Self::internal(o.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            code: self.code.into(),
            message: self.message,
            details: self.details,
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub profile: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct PutQuery {
    pub profile: Option<String>,
    pub expected_version: Option<i64>,
}

#[derive(Serialize)]
pub struct SubmissionResponse {
    pub alias: String,
    pub profile: String,
    pub version: i64,
    pub content_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_time: Option<String>,
    pub document: Value,
}

#[derive(Serialize)]
pub struct ListItem {
    pub alias: String,
    pub profile: String,
    pub version: i64,
    pub content_sha256: String,
    pub created_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_time: Option<String>,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub items: Vec<ListItem>,
    pub count: usize,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize)]
pub struct StoreResponse {
    pub alias: String,
    pub profile: String,
    pub version: i64,
    pub content_sha256: String,
    pub stored: bool,
    pub unchanged: bool,
}

#[derive(Serialize)]
pub struct VersionListItem {
    pub alias: String,
    pub version: i64,
    pub profile: String,
    pub content_sha256: String,
    pub created_time: String,
    pub is_current: bool,
}

#[derive(Serialize)]
pub struct VersionListResponse {
    pub alias: String,
    pub items: Vec<VersionListItem>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct VersionDocumentResponse {
    pub alias: String,
    pub version: i64,
    pub profile: String,
    pub content_sha256: String,
    pub created_time: String,
    pub is_current: bool,
    pub document: Value,
}

#[derive(Deserialize)]
pub struct AttachBody {
    /// Dataset/study alias to attach, or `null` to detach.
    pub metadata_ref: Option<String>,
}

#[derive(Serialize)]
pub struct AttachResponse {
    pub object_id: String,
    pub metadata_ref: Option<String>,
}

fn ensure_store_enabled(state: &AppState) -> Result<(), ApiError> {
    if state.metadata_store_enabled {
        Ok(())
    } else {
        Err(ApiError::not_enabled())
    }
}

fn ensure_write_allowed(
    state: &AppState,
    auth: Option<&Extension<ferrum_core::AuthClaims>>,
) -> Result<(), ApiError> {
    if !state.ingest_require_auth {
        return Ok(());
    }
    let Some(Extension(claims)) = auth else {
        return Err(ApiError::forbidden(
            "Metadata Store write requires authentication",
        ));
    };
    if claims.can_ingest() {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Metadata Store write requires ferrum:collector or admin role",
        ))
    }
}

fn ensure_read_allowed(
    state: &AppState,
    auth: Option<&Extension<ferrum_core::AuthClaims>>,
) -> Result<(), ApiError> {
    if !state.ingest_require_auth {
        return Ok(());
    }
    let Some(Extension(claims)) = auth else {
        return Err(ApiError::forbidden(
            "Metadata Store read requires authentication",
        ));
    };
    if claims.can_analyze() {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Metadata Store read requires ferrum:analyst, ferrum:collector, or admin role",
        ))
    }
}

fn parse_profile(raw: Option<&str>) -> Result<Option<MetaProfile>, ApiError> {
    match raw {
        None => Ok(None),
        Some(s) => MetaProfile::parse(s)
            .map(Some)
            .ok_or_else(|| ApiError::validation(format!("unknown metadata profile: {s}"))),
    }
}

fn parse_expected_version(headers: &HeaderMap, query: Option<i64>) -> Option<i64> {
    if query.is_some() {
        return query;
    }
    headers
        .get(axum::http::header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().trim_matches('"'))
        .and_then(|s| s.parse::<i64>().ok())
}

fn with_etag<T: Serialize>(status: StatusCode, version: i64, body: T) -> Response {
    let mut res = (status, Json(body)).into_response();
    if let Ok(v) = HeaderValue::from_str(&format!("\"{version}\"")) {
        res.headers_mut().insert(axum::http::header::ETAG, v);
    }
    res
}

async fn put_submission(
    State(state): State<Arc<AppState>>,
    AxPath(alias): AxPath<String>,
    Query(q): Query<PutQuery>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(document): Json<Value>,
) -> Result<Response, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_write_allowed(&state, auth.as_ref())?;

    let doc_alias = submission_alias(&document)
        .ok_or_else(|| ApiError::validation("ferrum-meta submission has no dataset/study alias"))?;
    if doc_alias != alias {
        return Err(ApiError::validation(format!(
            "path alias '{alias}' does not match document alias '{doc_alias}'"
        )));
    }

    let profile = parse_profile(q.profile.as_deref())?;
    let expected = parse_expected_version(&headers, q.expected_version);
    let stored = store_ferrum_meta_bundle_versioned(&state.repo, &document, profile, expected)
        .await
        .map_err(ApiError::from_drs)?;

    Ok(with_etag(
        StatusCode::OK,
        stored.version,
        StoreResponse {
            alias: stored.metadata_ref,
            profile: stored.profile.as_str().to_string(),
            version: stored.version,
            content_sha256: stored.content_sha256,
            stored: true,
            unchanged: stored.unchanged,
        },
    ))
}

async fn post_submission(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PutQuery>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(document): Json<Value>,
) -> Result<Response, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_write_allowed(&state, auth.as_ref())?;

    let profile = parse_profile(q.profile.as_deref())?;
    let expected = parse_expected_version(&headers, q.expected_version);
    let stored = store_ferrum_meta_bundle_versioned(&state.repo, &document, profile, expected)
        .await
        .map_err(ApiError::from_drs)?;

    let status = if stored.unchanged {
        StatusCode::OK
    } else if stored.version == 1 {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok(with_etag(
        status,
        stored.version,
        StoreResponse {
            alias: stored.metadata_ref,
            profile: stored.profile.as_str().to_string(),
            version: stored.version,
            content_sha256: stored.content_sha256,
            stored: true,
            unchanged: stored.unchanged,
        },
    ))
}

async fn get_submission(
    State(state): State<Arc<AppState>>,
    AxPath(alias): AxPath<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Response, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_read_allowed(&state, auth.as_ref())?;

    let row = state
        .repo
        .get_metadata_submission(&alias)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found(format!("metadata submission '{alias}' not found")))?;

    let document: Value = serde_json::from_str(&row.document)
        .map_err(|e| ApiError::internal(format!("stored document is not JSON: {e}")))?;

    Ok(with_etag(
        StatusCode::OK,
        row.version,
        SubmissionResponse {
            alias: row.alias,
            profile: row.profile,
            version: row.version,
            content_sha256: row.content_sha256,
            updated_time: row.updated_time,
            document,
        },
    ))
}

async fn list_submissions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<ListResponse>, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_read_allowed(&state, auth.as_ref())?;

    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let items = state
        .repo
        .list_metadata_submissions(q.profile.as_deref(), limit, offset)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let count = items.len();
    Ok(Json(ListResponse {
        items: items
            .into_iter()
            .map(|i| ListItem {
                alias: i.alias,
                profile: i.profile,
                version: i.version,
                content_sha256: i.content_sha256,
                created_time: i.created_time,
                updated_time: i.updated_time,
            })
            .collect(),
        count,
        limit,
        offset,
    }))
}

async fn list_versions(
    State(state): State<Arc<AppState>>,
    AxPath(alias): AxPath<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<VersionListResponse>, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_read_allowed(&state, auth.as_ref())?;

    if state
        .repo
        .get_metadata_submission(&alias)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .is_none()
    {
        return Err(ApiError::not_found(format!(
            "metadata submission '{alias}' not found"
        )));
    }

    let items = state
        .repo
        .list_metadata_submission_versions(&alias)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let count = items.len();
    Ok(Json(VersionListResponse {
        alias,
        items: items
            .into_iter()
            .map(|i| VersionListItem {
                alias: i.alias,
                version: i.version,
                profile: i.profile,
                content_sha256: i.content_sha256,
                created_time: i.created_time,
                is_current: i.is_current,
            })
            .collect(),
        count,
    }))
}

async fn get_version(
    State(state): State<Arc<AppState>>,
    AxPath((alias, version)): AxPath<(String, i64)>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Response, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_read_allowed(&state, auth.as_ref())?;

    let row = state
        .repo
        .get_metadata_submission_version(&alias, version)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| {
            ApiError::not_found(format!(
                "metadata submission '{alias}' version {version} not found"
            ))
        })?;

    let document: Value = serde_json::from_str(&row.document)
        .map_err(|e| ApiError::internal(format!("stored document is not JSON: {e}")))?;

    Ok(with_etag(
        StatusCode::OK,
        row.version,
        VersionDocumentResponse {
            alias: row.alias,
            version: row.version,
            profile: row.profile,
            content_sha256: row.content_sha256,
            created_time: row.created_time,
            is_current: row.is_current,
            document,
        },
    ))
}

async fn put_object_metadata_ref(
    State(state): State<Arc<AppState>>,
    AxPath(object_id): AxPath<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(body): Json<AttachBody>,
) -> Result<Json<AttachResponse>, ApiError> {
    ensure_store_enabled(&state)?;
    ensure_write_allowed(&state, auth.as_ref())?;

    match body.metadata_ref {
        Some(alias) => {
            if alias.trim().is_empty() {
                return Err(ApiError::validation("metadata_ref must be non-empty"));
            }
            if state
                .repo
                .get_metadata_submission(&alias)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?
                .is_none()
            {
                return Err(ApiError::not_found(format!(
                    "metadata submission '{alias}' not found"
                )));
            }
            link_object_metadata_ref(&state.repo, &object_id, &alias)
                .await
                .map_err(ApiError::from_drs)?;
            Ok(Json(AttachResponse {
                object_id,
                metadata_ref: Some(alias),
            }))
        }
        None => {
            unlink_object_metadata_ref(&state.repo, &object_id)
                .await
                .map_err(ApiError::from_drs)?;
            Ok(Json(AttachResponse {
                object_id,
                metadata_ref: None,
            }))
        }
    }
}

/// Mount at `/api/v1/metadata` when `[metadata_store] enabled = true`.
pub fn metadata_api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/submissions", get(list_submissions).post(post_submission))
        .route(
            "/submissions/:alias",
            get(get_submission).put(put_submission),
        )
        .route("/submissions/:alias/versions", get(list_versions))
        .route("/submissions/:alias/versions/:version", get(get_version))
        .route(
            "/objects/:object_id/metadata_ref",
            axum::routing::put(put_object_metadata_ref),
        )
        .with_state(state)
}

/// Mount when Metadata Store is disabled — always 501.
pub fn metadata_api_router_disabled() -> Router {
    async fn disabled() -> impl IntoResponse {
        ApiError::not_enabled()
    }
    Router::new()
        .route("/submissions", get(disabled).post(disabled))
        .route("/submissions/:alias", get(disabled).put(disabled))
        .route("/submissions/:alias/versions", get(disabled))
        .route("/submissions/:alias/versions/:version", get(disabled))
        .route(
            "/objects/:object_id/metadata_ref",
            axum::routing::put(disabled),
        )
}
