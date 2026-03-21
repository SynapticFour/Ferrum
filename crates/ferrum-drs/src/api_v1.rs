//! Lab Kit–oriented versioned ingest API (`/api/v1/ingest/*`).
//! Structured JSON errors: `code`, `message`, optional `details`.

use crate::error::DrsError;
use crate::ingest::{parse_multipart_upload, process_upload_from_parts};
use crate::state::AppState;
use crate::types::{ChecksumInput, CreateObjectRequest};
use crate::uri;
use axum::extract::{Extension, Multipart, Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Serialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

pub struct IngestApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Option<serde_json::Value>,
}

impl IngestApiError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "validation_error",
            message: msg.into(),
            details: None,
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: msg.into(),
            details: None,
        }
    }

    pub fn not_configured(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "not_configured",
            message: msg.into(),
            details: None,
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: msg.into(),
            details: None,
        }
    }

    pub fn from_drs(e: DrsError) -> Self {
        match e {
            DrsError::NotFound(m) => Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: m,
                details: None,
            },
            DrsError::Forbidden(m) => Self::forbidden(m),
            DrsError::Validation(m) => Self::validation(m),
            DrsError::Database(se) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "database_error",
                message: se.to_string(),
                details: None,
            },
            DrsError::Other(o) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "internal_error",
                message: o.to_string(),
                details: None,
            },
        }
    }

    fn as_json_value(&self) -> serde_json::Value {
        serde_json::to_value(ApiErrorBody {
            code: self.code.to_string(),
            message: self.message.clone(),
            details: self.details.clone(),
        })
        .unwrap_or_else(|_| json!({}))
    }
}

impl IntoResponse for IngestApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            code: self.code.to_string(),
            message: self.message,
            details: self.details,
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    pub client_request_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub items: Vec<RegisterItem>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegisterItem {
    Url {
        url: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        mime_type: Option<String>,
        #[serde(default)]
        derived_from: Option<Vec<String>>,
    },
    ExistingObject {
        storage_backend: String,
        storage_key: String,
        size: i64,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        mime_type: Option<String>,
        #[serde(default)]
        is_encrypted: Option<bool>,
        #[serde(default)]
        checksums: Option<Vec<ChecksumInput>>,
    },
}

#[derive(Serialize)]
pub struct IngestJobResponse {
    pub job_id: String,
    pub status: String,
    pub job_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

fn job_row_to_response(row: &crate::repo::DrsIngestJobRow) -> IngestJobResponse {
    IngestJobResponse {
        job_id: row.id.clone(),
        status: row.status.clone(),
        job_type: row.job_type.clone(),
        result: row.result_json.clone(),
        error: row.error_json.clone(),
    }
}

fn is_unique_violation(e: &DrsError) -> bool {
    match e {
        DrsError::Database(se) => se
            .as_database_error()
            .and_then(|d| d.code())
            .map(|c| c.as_ref() == "23505")
            .unwrap_or(false),
        _ => false,
    }
}

pub async fn post_register(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    match do_register(state, auth, body).await {
        Ok(j) => Json(j).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn do_register(
    state: Arc<AppState>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    body: RegisterRequest,
) -> Result<IngestJobResponse, IngestApiError> {
    if body.items.is_empty() {
        return Err(IngestApiError::validation("items must be non-empty"));
    }
    if let Some(ref cid) = body.client_request_id {
        if let Ok(Some(existing)) = state.repo.ingest_job_by_client_request_id(cid).await {
            return Ok(job_row_to_response(&existing));
        }
    }

    if let Some(ref ws_id) = body.workspace_id {
        let claims = auth
            .as_ref()
            .and_then(|e| e.0.sub())
            .ok_or_else(|| IngestApiError::forbidden("workspace_id requires authentication"))?;
        let ok = ferrum_core::is_workspace_editor_or_owner(state.repo.pool(), ws_id, claims)
            .await
            .map_err(|e| IngestApiError::internal(e.to_string()))?;
        if !ok {
            return Err(IngestApiError::forbidden("not a workspace editor or owner"));
        }
    }

    let job_id = ulid::Ulid::new().to_string();
    let client_id = body.client_request_id.as_deref();
    if let Err(e) = state
        .repo
        .ingest_job_insert(&job_id, client_id, "register", "running")
        .await
    {
        if is_unique_violation(&e) {
            if let Some(cid) = client_id {
                if let Ok(Some(row)) = state.repo.ingest_job_by_client_request_id(cid).await {
                    return Ok(job_row_to_response(&row));
                }
            }
        }
        return Err(IngestApiError::internal(e.to_string()));
    }

    let mut object_ids = Vec::new();
    let mut self_uris = Vec::new();
    let r = process_register_items(&state, &body, &mut object_ids, &mut self_uris).await;
    match r {
        Ok(()) => {
            let result = json!({
                "object_ids": object_ids,
                "self_uris": self_uris,
            });
            state
                .repo
                .ingest_job_finish_success(&job_id, &result)
                .await
                .map_err(|e| IngestApiError::internal(e.to_string()))?;
            Ok(IngestJobResponse {
                job_id,
                status: "succeeded".into(),
                job_type: "register".into(),
                result: Some(result),
                error: None,
            })
        }
        Err(e) => {
            let err_body = e.as_json_value();
            let _ = state
                .repo
                .ingest_job_finish_failed(&job_id, &err_body)
                .await;
            Err(e)
        }
    }
}

async fn process_register_items(
    state: &AppState,
    body: &RegisterRequest,
    object_ids: &mut Vec<String>,
    self_uris: &mut Vec<String>,
) -> Result<(), IngestApiError> {
    let policy = ferrum_core::SsrfPolicy::default();
    for item in &body.items {
        match item {
            RegisterItem::Url {
                url,
                name,
                mime_type,
                derived_from,
            } => {
                ferrum_core::validate_url_ssrf(url, &policy)
                    .map_err(|e| IngestApiError::validation(e.to_string()))?;
                if let Some(ref n) = name {
                    ferrum_core::validate_drs_name(n)
                        .map_err(|e| IngestApiError::validation(e.to_string()))?;
                }
                let object_id = ulid::Ulid::new().to_string();
                let req_create = CreateObjectRequest {
                    name: name.clone().or_else(|| Some(url.clone())),
                    description: Some(format!("External URL: {}", url)),
                    mime_type: mime_type.clone(),
                    size: 0,
                    checksums: vec![],
                    aliases: None,
                    storage_backend: "url".to_string(),
                    storage_key: url.clone(),
                    is_encrypted: Some(false),
                    workspace_id: body.workspace_id.clone(),
                };
                state
                    .repo
                    .create_object_with_id(&req_create, Some(object_id.clone()))
                    .await
                    .map_err(|e| IngestApiError::internal(e.to_string()))?;

                if let Some(ref store) = state.provenance_store {
                    if let Some(ref uris) = derived_from {
                        for uri in uris {
                            if let Some((_host, from_id)) = uri::parse_drs_uri(uri) {
                                if let Ok(Some(canonical)) =
                                    state.repo.resolve_id_or_uri(&from_id).await
                                {
                                    let _ = store.record_derived_from(&canonical, &object_id).await;
                                }
                            }
                        }
                    }
                }
                let su = format!("drs://{}/{}", state.repo.hostname(), object_id);
                self_uris.push(su);
                object_ids.push(object_id);
            }
            RegisterItem::ExistingObject {
                storage_backend,
                storage_key,
                size,
                name,
                description,
                mime_type,
                is_encrypted,
                checksums,
            } => {
                if storage_backend.eq_ignore_ascii_case("url") {
                    return Err(IngestApiError::validation(
                        "use kind \"url\" for URL registration, not existing_object",
                    ));
                }
                if *size < 0 {
                    return Err(IngestApiError::validation("size must be >= 0"));
                }
                let object_id = ulid::Ulid::new().to_string();
                let ch: Vec<ChecksumInput> = checksums.clone().unwrap_or_default();
                let obj_name = name
                    .clone()
                    .or_else(|| Some(storage_key.clone()))
                    .or_else(|| Some(object_id.clone()));
                let req_create = CreateObjectRequest {
                    name: obj_name,
                    description: description.clone(),
                    mime_type: mime_type.clone(),
                    size: *size,
                    checksums: ch,
                    aliases: None,
                    storage_backend: storage_backend.clone(),
                    storage_key: storage_key.clone(),
                    is_encrypted: Some(is_encrypted.unwrap_or(false)),
                    workspace_id: body.workspace_id.clone(),
                };
                state
                    .repo
                    .create_object_with_id(&req_create, Some(object_id.clone()))
                    .await
                    .map_err(|e| IngestApiError::internal(e.to_string()))?;
                let su = format!("drs://{}/{}", state.repo.hostname(), object_id);
                self_uris.push(su);
                object_ids.push(object_id);
            }
        }
    }
    Ok(())
}

pub async fn post_upload(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    match do_upload(state, auth, &mut multipart).await {
        Ok(j) => Json(j).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn do_upload(
    state: Arc<AppState>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    multipart: &mut Multipart,
) -> Result<IngestJobResponse, IngestApiError> {
    let parsed = parse_multipart_upload(multipart)
        .await
        .map_err(IngestApiError::from_drs)?;

    if let Some(ref cid) = parsed.client_request_id {
        if let Ok(Some(existing)) = state.repo.ingest_job_by_client_request_id(cid).await {
            return Ok(job_row_to_response(&existing));
        }
    }

    let job_id = ulid::Ulid::new().to_string();
    let client_id = parsed.client_request_id.as_deref();
    if let Err(e) = state
        .repo
        .ingest_job_insert(&job_id, client_id, "upload", "running")
        .await
    {
        if is_unique_violation(&e) {
            if let Some(cid) = client_id {
                if let Ok(Some(row)) = state.repo.ingest_job_by_client_request_id(cid).await {
                    return Ok(job_row_to_response(&row));
                }
            }
        }
        return Err(IngestApiError::internal(e.to_string()));
    }

    let claims = auth.as_ref().map(|e| &e.0);
    match process_upload_from_parts(Arc::clone(&state), claims, parsed).await {
        Ok(upload) => {
            let result = json!({
                "object_ids": vec![upload.id.clone()],
                "self_uris": vec![format!("drs://{}/{}", state.repo.hostname(), upload.id)],
                "size": upload.size,
            });
            state
                .repo
                .ingest_job_finish_success(&job_id, &result)
                .await
                .map_err(|e| IngestApiError::internal(e.to_string()))?;
            Ok(IngestJobResponse {
                job_id,
                status: "succeeded".into(),
                job_type: "upload".into(),
                result: Some(result),
                error: None,
            })
        }
        Err(drs_err) => {
            let api_err = IngestApiError::from_drs(drs_err);
            let err_body = api_err.as_json_value();
            let _ = state
                .repo
                .ingest_job_finish_failed(&job_id, &err_body)
                .await;
            Err(api_err)
        }
    }
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    AxPath(job_id): AxPath<String>,
) -> impl IntoResponse {
    match state.repo.ingest_job_get(&job_id).await {
        Ok(Some(row)) => Json(job_row_to_response(&row)).into_response(),
        Ok(None) => IngestApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "unknown job_id".into(),
            details: None,
        }
        .into_response(),
        Err(e) => IngestApiError::internal(e.to_string()).into_response(),
    }
}

/// Mount at `/api/v1/ingest` (gateway nests this router).
pub fn ingest_api_v1_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/register", post(post_register))
        .route("/upload", post(post_upload))
        .route("/jobs/:job_id", get(get_job))
        .with_state(state)
}

pub fn ingest_api_v1_router_unconfigured() -> Router {
    async fn no() -> impl IntoResponse {
        IngestApiError::not_configured("DRS ingest not configured (no database state)")
    }
    Router::new()
        .route("/register", post(no))
        .route("/upload", post(no))
        .route("/jobs/:job_id", get(no))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_body_json_shape() {
        let b = ApiErrorBody {
            code: "validation_error".into(),
            message: "items must be non-empty".into(),
            details: Some(json!({"hint": "add items"})),
        };
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["code"], "validation_error");
        assert_eq!(v["message"], "items must be non-empty");
        assert!(v.get("details").is_some());
    }

    #[test]
    fn register_request_deserializes_url_and_existing_object() {
        let j = r#"{
            "client_request_id": "c1",
            "items": [
                {"kind": "url", "url": "https://example.com/x"},
                {"kind": "existing_object", "storage_backend": "s3", "storage_key": "b/k", "size": 0, "name": "n"}
            ]
        }"#;
        let r: RegisterRequest = serde_json::from_str(j).unwrap();
        assert_eq!(r.client_request_id.as_deref(), Some("c1"));
        assert_eq!(r.items.len(), 2);
    }
}
