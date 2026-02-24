//! Ingest API: file upload, URL registration, batch.

use crate::error::{DrsError, Result};
use crate::state::AppState;
use crate::types::{ChecksumInput, CreateObjectRequest, IngestBatchItem, IngestBatchRequest, IngestUrlRequest};
use axum::{
    extract::{Multipart, State},
    Json,
};
use sha2::{Digest, Sha256, Sha512};
use std::sync::Arc;
use utoipa::ToSchema;

/// Multipart file upload; computes checksums, stores file, creates DRS object. Optional encrypt=true.
#[utoipa::path(
    post,
    path = "/ingest/file",
    responses((status = 200, body = IngestFileResponse))
)]
pub async fn ingest_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<IngestFileResponse>> {
    let storage = state.storage.as_ref().ok_or_else(|| DrsError::Validation("ingest not configured: no storage".into()))?;
    let mut name = None;
    let mut explicit_name = None::<String>;
    let mut mime_type = None;
    let mut encrypt = false;
    let mut expected_sha256 = None::<String>;
    let mut data = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| DrsError::Other(e.into()))? {
        let name_h = field.name().unwrap_or("").to_string();
        match name_h.as_str() {
            "file" => {
                name = field.file_name().map(str::to_string);
                if let Some(mime) = field.content_type().map(|c| c.to_string()) {
                    mime_type = Some(mime);
                }
                let buf = field.bytes().await.map_err(|e| DrsError::Other(e.into()))?;
                data = buf.to_vec();
            }
            "name" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        explicit_name = Some(t);
                    }
                }
            }
            "encrypt" => {
                if let Ok(v) = field.text().await {
                    encrypt = v.eq_ignore_ascii_case("true") || v == "1";
                }
            }
            "expected_sha256" => {
                if let Ok(v) = field.text().await {
                    expected_sha256 = Some(v.trim().to_string());
                }
            }
            _ => {}
        }
    }
    let object_name = explicit_name.or(name);
    if data.is_empty() {
        return Err(DrsError::Validation("no file in multipart".into()));
    }
    let size = data.len() as i64;
    let sha256 = format!("{:x}", Sha256::digest(&data));
    let sha512 = format!("{:x}", Sha512::digest(&data));
    let md5_hex = format!("{:x}", md5::compute(&data));
    if let Some(ref expected) = expected_sha256 {
        if expected.to_lowercase() != sha256 {
            return Err(DrsError::Validation(format!(
                "checksum mismatch: expected sha-256 {}",
                expected
            )));
        }
    }
    let object_id = ulid::Ulid::new().to_string();
    let storage_key = format!("drs/{}", object_id);
    storage.put_bytes(&storage_key, &data).await.map_err(|e| DrsError::Other(e.into()))?;
    let checksums = vec![
        ChecksumInput { r#type: "sha-256".to_string(), checksum: sha256.clone() },
        ChecksumInput { r#type: "sha-512".to_string(), checksum: sha512 },
        ChecksumInput { r#type: "md5".to_string(), checksum: md5_hex },
    ];
    let req = CreateObjectRequest {
        name: object_name.or_else(|| Some(storage_key.clone())),
        description: None,
        mime_type,
        size,
        checksums: checksums.clone(),
        aliases: None,
        storage_backend: "local".to_string(),
        storage_key: storage_key.clone(),
        is_encrypted: Some(encrypt),
    };
    state.repo.create_object_with_id(&req, Some(object_id.clone())).await?;
    Ok(Json(IngestFileResponse {
        id: object_id,
        size,
        checksums: checksums.iter().map(|c| ferrum_core::Checksum { r#type: c.r#type.clone(), checksum: c.checksum.clone() }).collect(),
    }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct IngestFileResponse {
    pub id: String,
    pub size: i64,
    pub checksums: Vec<ferrum_core::Checksum>,
}

/// Register external URL as DRS object (no local copy).
#[utoipa::path(
    post,
    path = "/ingest/url",
    request_body = IngestUrlRequest,
    responses((status = 200, body = IngestUrlResponse))
)]
pub async fn ingest_url(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IngestUrlRequest>,
) -> Result<Json<IngestUrlResponse>> {
    let object_id = ulid::Ulid::new().to_string();
    let req_create = CreateObjectRequest {
        name: req.name.or_else(|| Some(req.url.clone())),
        description: Some(format!("External URL: {}", req.url)),
        mime_type: req.mime_type,
        size: 0,
        checksums: vec![],
        aliases: req.aliases,
        storage_backend: "url".to_string(),
        storage_key: req.url,
        is_encrypted: Some(false),
    };
    state.repo.create_object_with_id(&req_create, Some(object_id.clone())).await?;
    if let Some(ref store) = state.provenance_store {
        if let Some(ref uris) = req.derived_from {
            for uri in uris {
                if let Some((_host, from_id)) = crate::uri::parse_drs_uri(uri) {
                    if let Ok(Some(canonical)) = state.repo.resolve_id_or_uri(&from_id).await {
                        let _ = store.record_derived_from(&canonical, &object_id).await;
                    }
                }
            }
        }
    }
    Ok(Json(IngestUrlResponse { id: object_id }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct IngestUrlResponse {
    pub id: String,
}

/// Bulk register URLs (and optionally paths) as DRS objects.
#[utoipa::path(
    post,
    path = "/ingest/batch",
    request_body = IngestBatchRequest,
    responses((status = 200, body = IngestBatchResponse))
)]
pub async fn ingest_batch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IngestBatchRequest>,
) -> Result<Json<IngestBatchResponse>> {
    let mut ids = Vec::new();
    for item in req.items {
        match item {
            IngestBatchItem::Url { url, name, mime_type, derived_from } => {
                let create = CreateObjectRequest {
                    name: name.or_else(|| Some(url.clone())),
                    description: Some(format!("External: {}", url)),
                    mime_type,
                    size: 0,
                    checksums: vec![],
                    aliases: None,
                    storage_backend: "url".to_string(),
                    storage_key: url,
                    is_encrypted: Some(false),
                };
                let id = state.repo.create_object(&create).await?;
                if let Some(ref store) = state.provenance_store {
                    if let Some(ref uris) = derived_from {
                        for uri in uris {
                            if let Some((_host, from_id)) = crate::uri::parse_drs_uri(uri) {
                                if let Ok(Some(canonical)) = state.repo.resolve_id_or_uri(&from_id).await {
                                    let _ = store.record_derived_from(&canonical, &id).await;
                                }
                            }
                        }
                    }
                }
                ids.push(id);
            }
            IngestBatchItem::Path { path, name, derived_from } => {
                let storage = state
                    .storage
                    .as_ref()
                    .ok_or_else(|| DrsError::Validation("batch path requires storage".into()))?;
                let mut reader = storage
                    .get(&path)
                    .await
                    .map_err(|e| DrsError::Other(e.into()))?;
                let mut data = Vec::new();
                tokio::io::AsyncReadExt::read_to_end(&mut *reader, &mut data)
                    .await
                    .map_err(|e| DrsError::Other(e.into()))?;
                if data.is_empty() {
                    return Err(DrsError::Validation(format!("empty object at path: {}", path)));
                }
                let size = data.len() as i64;
                let sha256 = format!("{:x}", Sha256::digest(&data));
                let sha512 = format!("{:x}", Sha512::digest(&data));
                let md5_hex = format!("{:x}", md5::compute(&data));
                let object_id = ulid::Ulid::new().to_string();
                let storage_key = format!("drs/{}", object_id);
                storage.put_bytes(&storage_key, &data).await.map_err(|e| DrsError::Other(e.into()))?;
                let checksums = vec![
                    ChecksumInput { r#type: "sha-256".to_string(), checksum: sha256 },
                    ChecksumInput { r#type: "sha-512".to_string(), checksum: sha512 },
                    ChecksumInput { r#type: "md5".to_string(), checksum: md5_hex },
                ];
                let create = CreateObjectRequest {
                    name: name.or_else(|| Some(path)),
                    description: None,
                    mime_type: None,
                    size,
                    checksums: checksums.clone(),
                    aliases: None,
                    storage_backend: "local".to_string(),
                    storage_key: storage_key.clone(),
                    is_encrypted: Some(false),
                };
                let id = state.repo.create_object_with_id(&create, Some(object_id)).await?;
                if let Some(ref store) = state.provenance_store {
                    if let Some(ref uris) = derived_from {
                        for uri in uris {
                            if let Some((_host, from_id)) = crate::uri::parse_drs_uri(uri) {
                                if let Ok(Some(canonical)) = state.repo.resolve_id_or_uri(&from_id).await {
                                    let _ = store.record_derived_from(&canonical, &id).await;
                                }
                            }
                        }
                    }
                }
                ids.push(id);
            }
        }
    }
    Ok(Json(IngestBatchResponse { ids }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct IngestBatchResponse {
    pub ids: Vec<String>,
}
