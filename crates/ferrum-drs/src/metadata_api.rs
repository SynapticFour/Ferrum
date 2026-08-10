//! Optional Metadata Store HTTP API (`/api/v1/metadata/*`).
//!
//! Opt-in via `[metadata_store] enabled = true` / `FERRUM_METADATA_STORE__ENABLED=true`.
//! Stores and retrieves ferrum-meta submission documents already held in
//! `metadata_submissions` (same table as ingest-time binding).

use crate::metadata::store_ferrum_meta_bundle;
use crate::state::AppState;
use axum::extract::{Extension, Path as AxPath, Query, State};
use axum::http::StatusCode;
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
}

#[derive(Serialize)]
pub struct SubmissionResponse {
    pub alias: String,
    pub profile: String,
    pub document: Value,
}

#[derive(Serialize)]
pub struct ListItem {
    pub alias: String,
    pub profile: String,
    pub created_time: String,
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
    pub stored: bool,
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

async fn put_submission(
    State(state): State<Arc<AppState>>,
    AxPath(alias): AxPath<String>,
    Query(q): Query<PutQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(document): Json<Value>,
) -> Result<Json<StoreResponse>, ApiError> {
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
    let stored = store_ferrum_meta_bundle(&state.repo, &document, profile)
        .await
        .map_err(|e| ApiError::validation(e.to_string()))?;

    Ok(Json(StoreResponse {
        alias: stored.metadata_ref,
        profile: stored.profile.as_str().to_string(),
        stored: true,
    }))
}

async fn post_submission(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PutQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(document): Json<Value>,
) -> Result<(StatusCode, Json<StoreResponse>), ApiError> {
    ensure_store_enabled(&state)?;
    ensure_write_allowed(&state, auth.as_ref())?;

    let profile = parse_profile(q.profile.as_deref())?;
    let stored = store_ferrum_meta_bundle(&state.repo, &document, profile)
        .await
        .map_err(|e| ApiError::validation(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(StoreResponse {
            alias: stored.metadata_ref,
            profile: stored.profile.as_str().to_string(),
            stored: true,
        }),
    ))
}

async fn get_submission(
    State(state): State<Arc<AppState>>,
    AxPath(alias): AxPath<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<SubmissionResponse>, ApiError> {
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

    Ok(Json(SubmissionResponse {
        alias: row.alias,
        profile: row.profile,
        document,
    }))
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
                created_time: i.created_time,
            })
            .collect(),
        count,
        limit,
        offset,
    }))
}

/// Mount at `/api/v1/metadata` when `[metadata_store] enabled = true`.
pub fn metadata_api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/submissions", get(list_submissions).post(post_submission))
        .route(
            "/submissions/:alias",
            get(get_submission).put(put_submission),
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
}
