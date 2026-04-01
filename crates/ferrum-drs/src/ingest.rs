//! Ingest API: file upload, URL registration, batch.

use crate::error::{DrsError, Result};
use crate::state::AppState;
use crate::types::{CreateObjectRequest, IngestBatchItem, IngestBatchRequest, IngestUrlRequest};
use axum::{
    extract::{Extension, Multipart, State},
    Json,
};
use ferrum_crypt4gh::KeyStore;
use sha2::{Digest, Sha256, Sha512};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use utoipa::ToSchema;

/// Parsed multipart fields for file ingest (GA4GH `/ingest/file` and `/api/v1/ingest/upload`).
#[derive(Debug, Default)]
pub struct ParsedMultipartUpload {
    pub file_name: Option<String>,
    pub explicit_name: Option<String>,
    pub mime_type: Option<String>,
    /// When absent, callers should apply [`ferrum_core::IngestConfig::default_encrypt_upload`].
    pub encrypt: Option<bool>,
    pub expected_sha256: Option<String>,
    pub workspace_id: Option<String>,
    pub client_request_id: Option<String>,
    pub data: Vec<u8>,
}

pub async fn parse_multipart_upload(multipart: &mut Multipart) -> Result<ParsedMultipartUpload> {
    let mut out = ParsedMultipartUpload::default();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| DrsError::Other(e.into()))?
    {
        let name_h = field.name().unwrap_or("").to_string();
        match name_h.as_str() {
            "workspace_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        out.workspace_id = Some(t);
                    }
                }
            }
            "client_request_id" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        out.client_request_id = Some(t);
                    }
                }
            }
            "file" => {
                out.file_name = field.file_name().map(str::to_string);
                if let Some(mime) = field.content_type().map(|c| c.to_string()) {
                    out.mime_type = Some(mime);
                }
                let buf = field.bytes().await.map_err(|e| DrsError::Other(e.into()))?;
                out.data = buf.to_vec();
            }
            "name" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        out.explicit_name = Some(t);
                    }
                }
            }
            "encrypt" => {
                if let Ok(v) = field.text().await {
                    out.encrypt = Some(v.eq_ignore_ascii_case("true") || v == "1");
                }
            }
            "expected_sha256" => {
                if let Ok(v) = field.text().await {
                    out.expected_sha256 = Some(v.trim().to_string());
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Store bytes (optionally Crypt4GH-encrypted with Ferrum node public key), create DRS object, async checksums.
pub async fn process_upload_from_parts(
    state: Arc<AppState>,
    auth: Option<&ferrum_core::AuthClaims>,
    parsed: ParsedMultipartUpload,
) -> Result<IngestFileResponse> {
    let storage = state
        .storage
        .clone()
        .ok_or_else(|| DrsError::Validation("ingest not configured: no storage".into()))?;
    let object_name = parsed.explicit_name.or(parsed.file_name);
    if parsed.data.is_empty() {
        return Err(DrsError::Validation("no file in multipart".into()));
    }
    let max_bytes = state.ingest.effective_max_upload_bytes();
    if parsed.data.len() as u64 > max_bytes {
        return Err(DrsError::Validation(format!(
            "upload exceeds ingest.max_upload_bytes ({max_bytes})"
        )));
    }
    let mut data = parsed.data;
    let encrypt = parsed
        .encrypt
        .unwrap_or(state.ingest.default_encrypt_upload);
    if let Some(ref expected) = parsed.expected_sha256 {
        let sha256 = hex::encode(Sha256::digest(&data));
        if expected.to_lowercase() != sha256 {
            return Err(DrsError::Validation(format!(
                "checksum mismatch: expected sha-256 {}",
                expected
            )));
        }
    }
    if let Some(ref ws_id) = parsed.workspace_id {
        let sub = auth
            .and_then(|c| c.sub())
            .ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let ok = ferrum_core::is_workspace_editor_or_owner(state.repo.pool(), ws_id, sub)
            .await
            .map_err(|e| DrsError::Other(e.into()))?;
        if !ok {
            return Err(DrsError::Forbidden(
                "not a workspace editor or owner".into(),
            ));
        }
    }
    if encrypt {
        let key_dir = state.crypt4gh_key_dir.as_ref().ok_or_else(|| {
            DrsError::Validation(
                "encrypt=true requires FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR (or [encryption].crypt4gh_key_dir)"
                    .into(),
            )
        })?;
        let ks = ferrum_crypt4gh::LocalKeyStore::new(key_dir.as_path());
        let key_id = state.crypt4gh_master_key_id.clone();
        let pk = ks
            .get_public_key_bytes(&key_id)
            .await
            .map_err(|e| DrsError::Validation(format!("crypt4gh key store: {e}")))?;
        let pubkey = pk.ok_or_else(|| {
            DrsError::Validation(format!(
                "no public key for crypt4gh_master_key_id={key_id} under CRYPT4GH_KEY_DIR"
            ))
        })?;
        let plaintext = std::mem::take(&mut data);
        data = tokio::task::spawn_blocking(move || {
            ferrum_crypt4gh::encrypt_bytes_for_pubkey(&pubkey, &plaintext)
        })
        .await
        .map_err(|e| DrsError::Other(e.into()))?
        .map_err(|e| DrsError::Validation(format!("crypt4gh encrypt: {e}")))?;
    }
    let size = data.len() as i64;
    let object_id = ulid::Ulid::new().to_string();
    let storage_key = format!("drs/{}", object_id);
    storage
        .put_bytes(&storage_key, &data)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;
    let backend = state.object_storage_backend.clone();
    let req = CreateObjectRequest {
        name: object_name.or_else(|| Some(storage_key.clone())),
        description: None,
        mime_type: parsed.mime_type,
        size,
        checksums: vec![],
        aliases: None,
        storage_backend: backend,
        storage_key: storage_key.clone(),
        is_encrypted: Some(encrypt),
        workspace_id: parsed.workspace_id,
    };
    state
        .repo
        .create_object_with_id(&req, Some(object_id.clone()))
        .await?;

    state
        .repo
        .set_checksum_status(&object_id, "pending")
        .await?;

    let repo = Arc::clone(&state.repo);
    let storage_key_bg = storage_key.clone();
    let object_id_bg = object_id.clone();
    let storage_bg = Arc::clone(&storage);
    tokio::spawn(async move {
        let mut reader = match storage_bg.get(&storage_key_bg).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(object_id = %object_id_bg, error = %e, "checksum compute: failed to open storage object");
                let _ = repo
                    .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                    .await;
                return;
            }
        };

        let mut buf = vec![0u8; 64 * 1024];
        let mut sha256 = Sha256::new();
        let mut sha512 = Sha512::new();
        let mut md5_hasher = md5::Context::new();

        loop {
            let n = match reader.read(&mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::error!(object_id = %object_id_bg, error = %e, "checksum compute: read failed");
                    let _ = repo
                        .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                        .await;
                    return;
                }
            };
            if n == 0 {
                break;
            }
            sha256.update(&buf[..n]);
            sha512.update(&buf[..n]);
            md5_hasher.consume(&buf[..n]);
        }

        let sha256_hex = hex::encode(sha256.finalize());
        let sha512_hex = hex::encode(sha512.finalize());
        let md5_hex = format!("{:x}", md5_hasher.compute());

        let checksum_pairs = vec![
            ("sha-256", sha256_hex.as_str()),
            ("sha-512", sha512_hex.as_str()),
            ("md5", md5_hex.as_str()),
        ];

        if let Err(e) = repo.upsert_checksums(&object_id_bg, &checksum_pairs).await {
            tracing::error!(object_id = %object_id_bg, error = %e, "checksum compute: db upsert failed");
            let _ = repo
                .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                .await;
            return;
        }

        let _ = repo.set_checksum_status(&object_id_bg, "computed").await;
    });

    Ok(IngestFileResponse {
        id: object_id,
        size,
        checksums: vec![],
    })
}

/// Multipart file upload; computes checksums, stores file, creates DRS object. Optional encrypt=true.
#[utoipa::path(
    post,
    path = "/ingest/file",
    responses((status = 200, body = IngestFileResponse))
)]
pub async fn ingest_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<IngestFileResponse>> {
    let parsed = parse_multipart_upload(&mut multipart).await?;
    let claims = auth.as_ref().map(|e| &e.0);
    let res = process_upload_from_parts(state, claims, parsed).await?;
    Ok(Json(res))
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<IngestUrlResponse>> {
    if let Some(ref ws_id) = req.workspace_id {
        let claims =
            auth.ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let sub = claims
            .0
            .sub()
            .ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let ok = ferrum_core::is_workspace_editor_or_owner(state.repo.pool(), ws_id, sub)
            .await
            .map_err(|e| DrsError::Other(e.into()))?;
        if !ok {
            return Err(DrsError::Forbidden(
                "not a workspace editor or owner".into(),
            ));
        }
    }
    let policy = ferrum_core::SsrfPolicy::default();
    ferrum_core::validate_url_ssrf(&req.url, &policy)
        .map_err(|e| DrsError::Validation(e.to_string()))?;
    if let Some(ref name) = req.name {
        ferrum_core::validate_drs_name(name).map_err(|e| DrsError::Validation(e.to_string()))?;
    }
    if let Some(ref aliases) = req.aliases {
        for a in aliases {
            ferrum_core::validate_drs_name(a).map_err(|e| DrsError::Validation(e.to_string()))?;
        }
    }
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
        workspace_id: req.workspace_id,
    };
    state
        .repo
        .create_object_with_id(&req_create, Some(object_id.clone()))
        .await?;
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<IngestBatchResponse>> {
    if let Some(ref ws_id) = req.workspace_id {
        let claims =
            auth.ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let sub = claims
            .0
            .sub()
            .ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let ok = ferrum_core::is_workspace_editor_or_owner(state.repo.pool(), ws_id, sub)
            .await
            .map_err(|e| DrsError::Other(e.into()))?;
        if !ok {
            return Err(DrsError::Forbidden(
                "not a workspace editor or owner".into(),
            ));
        }
    }
    let mut ids = Vec::new();
    let policy = ferrum_core::SsrfPolicy::default();
    for item in req.items {
        match item {
            IngestBatchItem::Url {
                url,
                name,
                mime_type,
                derived_from,
            } => {
                ferrum_core::validate_url_ssrf(&url, &policy)
                    .map_err(|e| DrsError::Validation(e.to_string()))?;
                if let Some(ref n) = name {
                    ferrum_core::validate_drs_name(n)
                        .map_err(|e| DrsError::Validation(e.to_string()))?;
                }
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
                    workspace_id: req.workspace_id.clone(),
                };
                let id = state.repo.create_object(&create).await?;
                if let Some(ref store) = state.provenance_store {
                    if let Some(ref uris) = derived_from {
                        for uri in uris {
                            if let Some((_host, from_id)) = crate::uri::parse_drs_uri(uri) {
                                if let Ok(Some(canonical)) =
                                    state.repo.resolve_id_or_uri(&from_id).await
                                {
                                    let _ = store.record_derived_from(&canonical, &id).await;
                                }
                            }
                        }
                    }
                }
                ids.push(id);
            }
            IngestBatchItem::Path {
                path,
                name,
                derived_from,
            } => {
                if let Some(ref n) = name {
                    ferrum_core::validate_drs_name(n)
                        .map_err(|e| DrsError::Validation(e.to_string()))?;
                }
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
                    return Err(DrsError::Validation(format!(
                        "empty object at path: {}",
                        path
                    )));
                }
                let size = data.len() as i64;
                let object_id = ulid::Ulid::new().to_string();
                let storage_key = format!("drs/{}", object_id);
                storage
                    .put_bytes(&storage_key, &data)
                    .await
                    .map_err(|e| DrsError::Other(e.into()))?;
                let create = CreateObjectRequest {
                    name: name.or(Some(path)),
                    description: None,
                    mime_type: None,
                    size,
                    // Async checksum computation (see ingest_file for details).
                    checksums: vec![],
                    aliases: None,
                    storage_backend: state.object_storage_backend.clone(),
                    storage_key: storage_key.clone(),
                    is_encrypted: Some(false),
                    workspace_id: req.workspace_id.clone(),
                };
                let id = state
                    .repo
                    .create_object_with_id(&create, Some(object_id))
                    .await?;

                state.repo.set_checksum_status(&id, "pending").await?;

                // Background task to compute checksums by streaming from storage.
                let repo = Arc::clone(&state.repo);
                let storage_key_bg = storage_key.clone();
                let object_id_bg = id.clone();
                let storage_bg = Arc::clone(&storage);
                tokio::spawn(async move {
                    let mut reader = match storage_bg.get(&storage_key_bg).await {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(object_id = %object_id_bg, error = %e, "batch checksum compute: failed to open storage object");
                            let _ = repo
                                .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                                .await;
                            return;
                        }
                    };

                    let mut buf = vec![0u8; 64 * 1024];
                    let mut sha256 = Sha256::new();
                    let mut sha512 = Sha512::new();
                    let mut md5_hasher = md5::Context::new();

                    loop {
                        let n = match reader.read(&mut buf).await {
                            Ok(n) => n,
                            Err(e) => {
                                tracing::error!(object_id = %object_id_bg, error = %e, "batch checksum compute: read failed");
                                let _ = repo
                                    .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                                    .await;
                                return;
                            }
                        };
                        if n == 0 {
                            break;
                        }
                        sha256.update(&buf[..n]);
                        sha512.update(&buf[..n]);
                        md5_hasher.consume(&buf[..n]);
                    }

                    let sha256_hex = hex::encode(sha256.finalize());
                    let sha512_hex = hex::encode(sha512.finalize());
                    let md5_hex = format!("{:x}", md5_hasher.compute());

                    let checksum_pairs = vec![
                        ("sha-256", sha256_hex.as_str()),
                        ("sha-512", sha512_hex.as_str()),
                        ("md5", md5_hex.as_str()),
                    ];

                    if let Err(e) = repo.upsert_checksums(&object_id_bg, &checksum_pairs).await {
                        tracing::error!(object_id = %object_id_bg, error = %e, "batch checksum compute: db upsert failed");
                        let _ = repo
                            .set_checksum_status(&object_id_bg, &format!("failed:{e}"))
                            .await;
                        return;
                    }

                    let _ = repo.set_checksum_status(&object_id_bg, "computed").await;
                });
                if let Some(ref store) = state.provenance_store {
                    if let Some(ref uris) = derived_from {
                        for uri in uris {
                            if let Some((_host, from_id)) = crate::uri::parse_drs_uri(uri) {
                                if let Ok(Some(canonical)) =
                                    state.repo.resolve_id_or_uri(&from_id).await
                                {
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

/// BAM helpers using noodles' lazy record iterator (feature `bam-lazy-ingest`).
#[cfg(feature = "bam-lazy-ingest")]
pub mod bam {
    //! Lesson 6: `Reader::records()` yields lazily decoded records that avoid full CIGAR / sequence /
    //! quality materialization when only positions (or other header fields) are needed — typically
    //! ~1.5× faster for index-style scans. Source: noodles documentation and community benchmarks.
    //!
    //! **Caveat:** This path does **not** validate every field of each record; use full validation
    //! paths when you need complete structural guarantees.

    use std::io::{self, Read};

    use noodles_bam::io::Reader;

    /// Returns 1-based alignment start positions for records that are aligned.
    pub fn scan_alignment_start_positions<R: Read>(reader: R) -> io::Result<Vec<usize>> {
        let mut reader = Reader::new(reader);
        reader.read_header()?;
        let mut positions = Vec::new();
        for result in reader.records() {
            let record = result?;
            if let Some(pos) = record.alignment_start().transpose()? {
                positions.push(usize::from(pos));
            }
        }
        Ok(positions)
    }
}
