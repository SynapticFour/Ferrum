//! Lab Kit–oriented versioned ingest API (`/api/v1/ingest/*`).
//! Structured JSON errors: `code`, `message`, optional `details`.

use crate::error::DrsError;
use crate::ingest::{process_upload_from_spooled, ParsedMultipartUpload};
use crate::ingest_chunk::process_chunked_upload_from_parts;
use crate::metadata::{link_object_metadata_ref, provenance_destination, store_ferrum_meta_bundle};
use crate::state::AppState;
use crate::types::{ChecksumInput, CreateObjectRequest};
use crate::uri;
use axum::extract::{DefaultBodyLimit, Extension, Multipart, Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use ferrum_meta_connect::{parse_submission_document, MetaProfile};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;

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
            DrsError::Conflict(m) => Self {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: m,
                details: None,
            },
            DrsError::TransferQueued(m) => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "transfer_queued",
                message: m,
                details: None,
            },
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
        let mut res = (self.status, Json(body)).into_response();
        if self.code == "transfer_queued" {
            res.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("60"),
            );
        }
        res
    }
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    pub client_request_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Optional GISAID metadata applied to all registered objects unless overridden per item.
    #[serde(default)]
    pub gisaid_metadata: Option<serde_json::Value>,
    /// Optional ferrum-meta submission bundle (validated and stored; alias linked as metadata_ref).
    #[serde(default)]
    pub ferrum_meta: Option<serde_json::Value>,
    /// Optional profile hint when validating ferrum_meta: core, pathogen, h3africa.
    #[serde(default)]
    pub metadata_profile: Option<String>,
    /// Link existing stored submission by alias without inline bundle.
    #[serde(default)]
    pub metadata_ref: Option<String>,
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
        /// Optional ONT metadata (stored as `ont_metrics`; tags pathogen for Beacon).
        #[serde(default)]
        ont_metadata: Option<serde_json::Value>,
        /// Optional GISAID submission metadata (stored on `drs_objects.gisaid_metadata`).
        #[serde(default)]
        gisaid_metadata: Option<serde_json::Value>,
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
    ensure_ingest_allowed(&state, auth.as_ref())?;
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
    let metadata_ref = resolve_register_metadata_ref(state, body).await?;
    if let Some(ref gm) = body.gisaid_metadata {
        ferrum_core::validate_gisaid_metadata(gm)
            .map_err(|e| IngestApiError::validation(e.to_string()))?;
    }
    for item in &body.items {
        let gisaid_metadata = resolve_gisaid_metadata(body, item);
        if let Some(ref gm) = gisaid_metadata {
            ferrum_core::validate_gisaid_metadata(gm)
                .map_err(|e| IngestApiError::validation(e.to_string()))?;
        }
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
                    ont_metrics: None,
                    gisaid_metadata: gisaid_metadata.clone(),
                    metadata_ref: metadata_ref.clone(),
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
                ont_metadata,
                gisaid_metadata: item_gisaid,
            } => {
                if storage_backend.eq_ignore_ascii_case("url") {
                    return Err(IngestApiError::validation(
                        "use kind \"url\" for URL registration, not existing_object",
                    ));
                }
                if *size < 0 {
                    return Err(IngestApiError::validation("size must be >= 0"));
                }
                if ferrum_core::validate_object_key(storage_key).is_err() {
                    return Err(IngestApiError::validation(
                        "storage_key must be a relative object key without '..' or absolute paths",
                    ));
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
                    ont_metrics: ont_metadata.clone(),
                    gisaid_metadata: item_gisaid.clone().or(gisaid_metadata),
                    metadata_ref: metadata_ref.clone(),
                };
                state
                    .repo
                    .create_object_with_id(&req_create, Some(object_id.clone()))
                    .await
                    .map_err(|e| IngestApiError::internal(e.to_string()))?;
                if let Some(ref om) = ont_metadata {
                    apply_ont_side_effects(state, &object_id, om).await?;
                }
                let su = format!("drs://{}/{}", state.repo.hostname(), object_id);
                self_uris.push(su);
                object_ids.push(object_id);
            }
        }
    }
    Ok(())
}

fn resolve_gisaid_metadata(
    body: &RegisterRequest,
    item: &RegisterItem,
) -> Option<serde_json::Value> {
    match item {
        RegisterItem::Url { .. } => body.gisaid_metadata.clone(),
        RegisterItem::ExistingObject {
            gisaid_metadata, ..
        } => gisaid_metadata
            .clone()
            .or_else(|| body.gisaid_metadata.clone()),
    }
}

async fn resolve_register_metadata_ref(
    state: &AppState,
    body: &RegisterRequest,
) -> Result<Option<String>, IngestApiError> {
    if let Some(ref alias) = body.metadata_ref {
        if alias.trim().is_empty() {
            return Err(IngestApiError::validation(
                "metadata_ref must be non-empty".to_string(),
            ));
        }
        return Ok(Some(alias.clone()));
    }
    if let Some(ref bundle) = body.ferrum_meta {
        let profile = body
            .metadata_profile
            .as_deref()
            .and_then(MetaProfile::parse);
        let stored = store_ferrum_meta_bundle(&state.repo, bundle, profile)
            .await
            .map_err(|e| IngestApiError::validation(e.to_string()))?;
        return Ok(Some(stored.metadata_ref));
    }
    Ok(None)
}

fn parse_ferrum_meta_text(raw: &str) -> Result<serde_json::Value, IngestApiError> {
    let trimmed = raw.trim();
    let is_yaml = trimmed.starts_with("ferrum_meta_version:")
        || trimmed.starts_with("studies:")
        || trimmed.starts_with("---");
    parse_submission_document(trimmed, is_yaml)
        .map_err(|e| IngestApiError::validation(format!("invalid ferrum_meta: {e}")))
}

async fn append_ont_residency_audit(
    state: &AppState,
    object_id: &str,
    auth: Option<&ferrum_core::AuthClaims>,
    ont_req: &ferrum_ont::OntIngestRequest,
    metadata_ref: Option<&str>,
    bytes: i64,
) {
    if let Some(ref audit) = state.residency_audit {
        let requester = auth.and_then(|c| c.sub()).or(ont_req.collector.as_deref());
        let _ = audit
            .append(
                "data_uploaded",
                Some(object_id),
                requester,
                None,
                false,
                Some(bytes),
            )
            .await;
        if ont_req.collector.is_some()
            || ont_req.collected_at.is_some()
            || ont_req.location_label.is_some()
            || ont_req.latitude.is_some()
            || metadata_ref.is_some()
        {
            let destination = provenance_destination(
                metadata_ref,
                ont_req.collector.as_deref(),
                ont_req.collected_at.as_deref(),
                ont_req.location_label.as_deref(),
                ont_req.latitude,
                ont_req.longitude,
            );
            let _ = audit
                .append(
                    "collection_recorded",
                    Some(object_id),
                    requester,
                    Some(&destination),
                    false,
                    None,
                )
                .await;
        }
    }
}

pub async fn post_upload_chunk(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    match do_upload_chunk(state, auth, &mut multipart).await {
        Ok(j) => Json(j).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn do_upload_chunk(
    state: Arc<AppState>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    multipart: &mut Multipart,
) -> Result<crate::ingest_chunk::ChunkUploadResponse, IngestApiError> {
    ensure_ingest_allowed(&state, auth.as_ref())?;
    let max_bytes = state.ingest.effective_max_upload_bytes();
    let mut parsed = ParsedMultipartUpload::default();
    let mut chunk_spooled: Option<SpooledUpload> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| IngestApiError::validation(e.body_text()))?
    {
        match field.name().unwrap_or("") {
            "workspace_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.workspace_id = Some(t);
                    }
                }
            }
            "client_request_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.client_request_id = Some(t);
                    }
                }
            }
            "file" => {
                parsed.file_name = field.file_name().map(str::to_string);
                if let Some(mime) = field.content_type().map(|c| c.to_string()) {
                    parsed.mime_type = Some(mime);
                }
                chunk_spooled = Some(spool_multipart_field(field, max_bytes, 0).await?);
            }
            "name" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.explicit_name = Some(t);
                    }
                }
            }
            "encrypt" => {
                if let Ok(v) = field.text().await {
                    parsed.encrypt = Some(v.eq_ignore_ascii_case("true") || v == "1");
                }
            }
            "expected_sha256" => {
                if let Ok(v) = field.text().await {
                    parsed.expected_sha256 = Some(v.trim().to_string());
                }
            }
            "upload_token" | "resume_token" => {
                if let Ok(v) = field.text().await {
                    let t = v.trim().to_string();
                    if !t.is_empty() {
                        parsed.upload_token = Some(t);
                    }
                }
            }
            "chunk_offset" => {
                if let Ok(v) = field.text().await {
                    if let Ok(n) = v.trim().parse::<i64>() {
                        parsed.chunk_offset = Some(n);
                    }
                }
            }
            "total_bytes" => {
                if let Ok(v) = field.text().await {
                    if let Ok(n) = v.trim().parse::<i64>() {
                        parsed.total_bytes = Some(n);
                    }
                }
            }
            _ => {}
        }
    }

    let spooled =
        chunk_spooled.ok_or_else(|| IngestApiError::validation("no chunk data in multipart"))?;
    parsed.chunk_path = Some(spooled.path);

    let claims = auth.as_ref().map(|e| &e.0);
    process_chunked_upload_from_parts(state, claims, parsed)
        .await
        .map_err(IngestApiError::from_drs)
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
    ensure_ingest_allowed(&state, auth.as_ref())?;
    let max_bytes = state.ingest.effective_max_upload_bytes();
    let mut parsed = ParsedMultipartUpload::default();
    let mut spooled: Option<SpooledUpload> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| IngestApiError::validation(e.body_text()))?
    {
        let name_h = field.name().unwrap_or("").to_string();
        match name_h.as_str() {
            "workspace_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.workspace_id = Some(t);
                    }
                }
            }
            "client_request_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.client_request_id = Some(t);
                    }
                }
            }
            "file" => {
                parsed.file_name = field.file_name().map(str::to_string);
                if let Some(mime) = field.content_type().map(|c| c.to_string()) {
                    parsed.mime_type = Some(mime);
                }
                let chunk = spool_multipart_field(field, max_bytes, 0).await?;
                spooled = Some(chunk);
            }
            "name" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        parsed.explicit_name = Some(t);
                    }
                }
            }
            "encrypt" => {
                if let Ok(v) = field.text().await {
                    parsed.encrypt = Some(v.eq_ignore_ascii_case("true") || v == "1");
                }
            }
            "expected_sha256" => {
                if let Ok(v) = field.text().await {
                    parsed.expected_sha256 = Some(v.trim().to_string());
                }
            }
            _ => {}
        }
    }

    let spooled = spooled.ok_or_else(|| IngestApiError::validation("no file in multipart"))?;
    if spooled.size == 0 {
        return Err(IngestApiError::validation("no file in multipart"));
    }

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
    let spool_path = spooled.path.to_path_buf();
    let spool_size = spooled.size;
    match process_upload_from_spooled(Arc::clone(&state), claims, parsed, spool_path, spool_size)
        .await
    {
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

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> impl IntoResponse {
    let result = if let Some(sub) = auth.as_ref().and_then(|e| e.0.sub()) {
        state.repo.ingest_job_list_for_subject(sub, 50).await
    } else {
        state.repo.ingest_job_list_recent(50).await
    };
    match result {
        Ok(rows) => {
            let jobs: Vec<IngestJobResponse> = rows.iter().map(job_row_to_response).collect();
            Json(serde_json::json!({ "jobs": jobs })).into_response()
        }
        Err(e) => IngestApiError::internal(e.to_string()).into_response(),
    }
}

async fn apply_ont_side_effects(
    state: &AppState,
    object_id: &str,
    ont_metadata: &serde_json::Value,
) -> Result<(), IngestApiError> {
    if let Some(org) = ont_metadata.get("organism").and_then(|v| v.as_str()) {
        let qscore = ont_metadata
            .get("quality")
            .and_then(|q| q.get("mean_qscore"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        state
            .repo
            .insert_pathogen_annotation(object_id, org, &[], None, &[], qscore, None)
            .await
            .map_err(IngestApiError::from_drs)?;
    }
    Ok(())
}

async fn store_uploaded_object(
    state: &AppState,
    storage: &Arc<dyn ferrum_storage::ObjectStorage>,
    file_path: &std::path::Path,
    size: i64,
    mime_type: Option<String>,
    fields: ferrum_ont::OntCreateFields,
    metadata_ref: Option<String>,
) -> Result<(String, i64), IngestApiError> {
    let object_id = ulid::Ulid::new().to_string();
    let storage_key = format!("drs/{}", object_id);
    storage
        .put_file(&storage_key, file_path)
        .await
        .map_err(|e| IngestApiError::internal(e.to_string()))?;
    let create = CreateObjectRequest {
        name: fields.name,
        description: fields.description,
        mime_type: mime_type.or(fields.mime_type),
        size,
        checksums: vec![],
        aliases: None,
        storage_backend: fields.storage_backend,
        storage_key,
        is_encrypted: Some(false),
        workspace_id: None,
        ont_metrics: None,
        gisaid_metadata: None,
        metadata_ref,
    };
    state
        .repo
        .create_object_with_id(&create, Some(object_id.clone()))
        .await
        .map_err(IngestApiError::from_drs)?;
    state
        .repo
        .set_checksum_status(&object_id, "pending")
        .await
        .map_err(IngestApiError::from_drs)?;
    Ok((object_id, size))
}

struct SpooledUpload {
    path: tempfile::TempPath,
    size: u64,
    mime_type: Option<String>,
}

/// Stream a multipart file field to disk without buffering the entire payload in RAM.
async fn spool_multipart_field(
    mut field: axum::extract::multipart::Field<'_>,
    max_bytes: u64,
    bytes_already: u64,
) -> Result<SpooledUpload, IngestApiError> {
    let mime_type = field.content_type().map(|c| c.to_string());
    let mut temp = NamedTempFile::new().map_err(|e| IngestApiError::internal(e.to_string()))?;
    let mut written: u64 = 0;
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|e| {
            let detail = e.body_text();
            let msg = if detail.contains("failed to read stream") {
                "upload stream interrupted before the full file was received — retry the upload"
            } else {
                detail.as_str()
            };
            IngestApiError::validation(msg)
        })?;
        written += chunk.len() as u64;
        if bytes_already.saturating_add(written) > max_bytes {
            return Err(IngestApiError::validation(format!(
                "upload exceeds ingest.max_upload_bytes ({max_bytes})"
            )));
        }
        temp.as_file_mut()
            .write_all(&chunk)
            .map_err(|e| IngestApiError::internal(e.to_string()))?;
    }
    temp.as_file()
        .sync_all()
        .map_err(|e| IngestApiError::internal(e.to_string()))?;
    Ok(SpooledUpload {
        path: temp.into_temp_path(),
        size: written,
        mime_type,
    })
}

/// POST /api/v1/ingest/ont-metrics — update QC metrics from ont-qc WES workflow.
#[derive(Deserialize)]
pub struct OntMetricsUpdateRequest {
    pub drs_object_id: String,
    pub quality_metrics: ferrum_ont::OntQualityMetrics,
}

pub async fn post_ont_metrics(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OntMetricsUpdateRequest>,
) -> impl IntoResponse {
    match do_ont_metrics_update(state, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn do_ont_metrics_update(
    state: Arc<AppState>,
    body: OntMetricsUpdateRequest,
) -> Result<serde_json::Value, IngestApiError> {
    let mut metrics = serde_json::Map::new();
    metrics.insert(
        "quality".into(),
        serde_json::to_value(&body.quality_metrics).unwrap_or_default(),
    );
    metrics.insert(
        "updated_by".into(),
        serde_json::Value::String("ont-qc".into()),
    );
    let value = serde_json::Value::Object(metrics);
    let ok = state
        .repo
        .update_ont_metrics(&body.drs_object_id, &value)
        .await
        .map_err(IngestApiError::from_drs)?;
    if !ok {
        return Err(IngestApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("unknown drs_object_id {}", body.drs_object_id),
            details: None,
        });
    }
    Ok(json!({
        "drs_object_id": body.drs_object_id,
        "updated": true,
    }))
}

/// POST /api/v1/ingest/ont — multipart: `ont_metadata` (JSON) + `file` (binary).
pub async fn post_ont(
    State(state): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    match do_ont_ingest(state, &mut multipart, auth).await {
        Ok(j) => Json(j).into_response(),
        Err(e) => e.into_response(),
    }
}

fn ensure_ingest_allowed(
    state: &AppState,
    auth: Option<&Extension<ferrum_core::AuthClaims>>,
) -> Result<(), IngestApiError> {
    if !state.ingest_require_auth {
        return Ok(());
    }
    let Some(Extension(claims)) = auth else {
        return Err(IngestApiError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "ingest requires authentication".into(),
            details: None,
        });
    };
    if claims.can_ingest() {
        Ok(())
    } else {
        Err(IngestApiError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "ingest requires ferrum:collector or admin role".into(),
            details: None,
        })
    }
}

async fn do_ont_ingest(
    state: Arc<AppState>,
    multipart: &mut Multipart,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<serde_json::Value, IngestApiError> {
    ensure_ingest_allowed(&state, auth.as_ref())?;
    let storage = state
        .storage
        .clone()
        .ok_or_else(|| IngestApiError::not_configured("ingest not configured: no storage"))?;

    let max_bytes = state.ingest.effective_max_upload_bytes();

    let mut ont_metadata: Option<String> = None;
    let mut ferrum_meta_raw: Option<String> = None;
    let mut file_upload: Option<SpooledUpload> = None;
    let mut fastq_upload: Option<SpooledUpload> = None;
    let mut total_bytes: u64 = 0;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| IngestApiError::validation(e.to_string()))?
    {
        match field.name().unwrap_or("") {
            "ont_metadata" => {
                ont_metadata = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| IngestApiError::validation(e.to_string()))?,
                );
            }
            "ferrum_meta" => {
                ferrum_meta_raw = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| IngestApiError::validation(e.to_string()))?,
                );
            }
            "file" => {
                let spooled = spool_multipart_field(field, max_bytes, total_bytes).await?;
                total_bytes += spooled.size;
                file_upload = Some(spooled);
            }
            "fastq_file" => {
                let spooled = spool_multipart_field(field, max_bytes, total_bytes).await?;
                total_bytes += spooled.size;
                fastq_upload = Some(spooled);
            }
            _ => {}
        }
    }

    let meta_str = ont_metadata
        .ok_or_else(|| IngestApiError::validation("ont_metadata field is required".to_string()))?;
    let file_upload = file_upload
        .ok_or_else(|| IngestApiError::validation("file field is required".to_string()))?;
    if file_upload.size == 0 {
        return Err(IngestApiError::validation(
            "file field is required".to_string(),
        ));
    }

    let ont_req: ferrum_ont::OntIngestRequest = serde_json::from_str(&meta_str)
        .map_err(|e| IngestApiError::validation(format!("invalid ont_metadata JSON: {e}")))?;
    ferrum_ont::validate_ingest_request(&ont_req)
        .map_err(|e| IngestApiError::validation(e.to_string()))?;

    let metadata_ref = if let Some(ref raw) = ferrum_meta_raw {
        let bundle = parse_ferrum_meta_text(raw)?;
        let stored = store_ferrum_meta_bundle(&state.repo, &bundle, None)
            .await
            .map_err(|e| IngestApiError::validation(e.to_string()))?;
        Some(stored.metadata_ref)
    } else {
        None
    };
    let metadata_ref_for_objects = metadata_ref.clone();

    let backend = state.object_storage_backend.clone();
    let raw_fields =
        ferrum_ont::build_create_request(&ont_req, file_upload.size as i64, &backend, "pending");
    let file_mime = file_upload.mime_type;
    let (raw_id, raw_size) = store_uploaded_object(
        &state,
        &storage,
        file_upload.path.as_ref(),
        file_upload.size as i64,
        file_mime,
        raw_fields,
        metadata_ref_for_objects.clone(),
    )
    .await?;

    let mut members: Vec<(String, String, i64)> = vec![(raw_id.clone(), "raw".into(), raw_size)];
    if let Some(fq) = fastq_upload {
        if fq.size > 0 {
            let mut fastq_req = ont_req.clone();
            fastq_req.format = ferrum_ont::OntFormat::Fastq;
            fastq_req.dorado_basecalled = true;
            let fq_fields =
                ferrum_ont::build_create_request(&fastq_req, fq.size as i64, &backend, "pending");
            let (fq_id, fq_size) = store_uploaded_object(
                &state,
                &storage,
                fq.path.as_ref(),
                fq.size as i64,
                fq.mime_type.or(Some("application/x-fastq".into())),
                fq_fields,
                metadata_ref_for_objects.clone(),
            )
            .await?;
            members.push((fq_id, "fastq".into(), fq_size));
        }
    }

    let bundle_fields = ferrum_ont::build_create_request(
        &ont_req,
        members.iter().map(|(_, _, s)| s).sum(),
        &backend,
        "bundle",
    );
    let qscore = ont_req.quality_metrics.as_ref().map(|q| q.mean_qscore);

    let canonical_id = if members.len() > 1 {
        let bundle_id = ulid::Ulid::new().to_string();
        state
            .repo
            .create_ont_bundle(
                &bundle_id,
                bundle_fields.name,
                bundle_fields.description,
                bundle_fields.ont_metrics,
                metadata_ref_for_objects.clone(),
                &members,
            )
            .await
            .map_err(IngestApiError::from_drs)?;
        bundle_id
    } else {
        if let Some(ref metrics) = bundle_fields.ont_metrics {
            state
                .repo
                .update_ont_metrics(&raw_id, metrics)
                .await
                .map_err(IngestApiError::from_drs)?;
        }
        if let Some(ref mref) = metadata_ref_for_objects {
            link_object_metadata_ref(&state.repo, &raw_id, mref)
                .await
                .map_err(IngestApiError::from_drs)?;
        }
        raw_id
    };

    let total_bytes: i64 = members.iter().map(|(_, _, s)| s).sum();
    append_ont_residency_audit(
        &state,
        &canonical_id,
        auth.as_ref().map(|e| &e.0),
        &ont_req,
        metadata_ref.as_deref(),
        total_bytes,
    )
    .await;

    state
        .repo
        .insert_pathogen_annotation(
            &canonical_id,
            &ont_req.organism,
            &[],
            None,
            &[],
            qscore,
            None,
        )
        .await
        .map_err(IngestApiError::from_drs)?;

    let _ = auth;

    for (id, name, _) in &members {
        crate::pipeline_hooks::schedule_post_ingest_hooks(
            Arc::clone(&state),
            id.clone(),
            Some(name.clone()),
            Some("application/octet-stream".into()),
        );
    }

    let mut response = json!({
        "object_id": canonical_id,
        "drs_object_id": canonical_id,
        "self_uri": format!("drs://{}/{}", state.repo.hostname(), canonical_id),
        "organism": ont_req.organism,
        "format": ont_req.format,
        "bundle": members.len() > 1,
        "member_ids": members.iter().map(|(id, name, _)| json!({"name": name, "id": id})).collect::<Vec<_>>(),
        "size": total_bytes,
    });
    if let Some(mref) = metadata_ref {
        response["metadata_ref"] = json!(mref);
    }
    Ok(response)
}

/// Mount at `/api/v1/ingest` (gateway nests this router).
pub fn ingest_api_v1_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/register", post(post_register))
        .route("/upload", post(post_upload))
        .route("/upload/chunk", post(post_upload_chunk))
        .route("/ont", post(post_ont))
        .route("/ont-metrics", post(post_ont_metrics))
        .route("/jobs", get(list_jobs))
        .route("/jobs/:job_id", get(get_job))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
        .with_state(state)
}

pub fn ingest_api_v1_router_unconfigured() -> Router {
    async fn no() -> impl IntoResponse {
        IngestApiError::not_configured("DRS ingest not configured (no database state)")
    }
    Router::new()
        .route("/register", post(no))
        .route("/upload", post(no))
        .route("/upload/chunk", post(no))
        .route("/ont", post(no))
        .route("/ont-metrics", post(no))
        .route("/jobs", get(no))
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
