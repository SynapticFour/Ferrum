//! DRS HTTP handlers.

use crate::error::{DrsError, Result};
use crate::state::AppState;
use crate::types::*;
use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use ferrum_core::{FerrumError, Organization, ServiceInfo, ServiceType};
use ferrum_crypt4gh::{stream_decrypt, KeyStore, LocalKeyStore};
use futures_util::stream::StreamExt;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
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
) -> Result<Response> {
    tracing::info!(object_id = %object_id, "DRS get_object");
    let resolved = state.repo.resolve_id_or_uri(&object_id).await?;
    tracing::info!(object_id = %object_id, resolved = ?resolved, "DRS resolve_id_or_uri");
    let canonical =
        resolved.ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }
    let obj = state
        .repo
        .get_object(&canonical, params.expand.unwrap_or(false))
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;

    // Async checksum model: when metadata is `pending`, DRS must not leak partially computed checksums.
    // Learned from Broad Terra production behavior (DRS scaling issues).
    let mut obj = obj;
    if state.repo.get_checksum_status(&canonical).await?.as_deref() == Some("pending") {
        obj.checksums = vec![];
        let mut res = Json(obj).into_response();
        res.headers_mut().insert(
            "X-Ferrum-Checksum-Status",
            HeaderValue::from_static("pending"),
        );
        return Ok(res);
    }

    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let _ = state
        .repo
        .log_access(&canonical, None, "GET", 200, client_ip.as_deref())
        .await;
    Ok(Json(obj).into_response())
}

/// Query params for GET /objects/{object_id}. expand=true returns bundle contents recursively.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, ToSchema)]
pub struct ExpandQuery {
    /// If true, expand bundle contents (and nested bundles).
    pub expand: Option<bool>,
}

/// Query params for GET /objects/{object_id}/contents.
/// Returns direct bundle members with cursor pagination.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, ToSchema)]
pub struct BundleContentsQuery {
    /// Opaque cursor (base64) for pagination.
    pub page_token: Option<String>,
    /// Max items per page.
    pub page_size: Option<u32>,
}

/// Paginated list of direct bundle members.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct BundleContentsPage {
    pub contents: Vec<ContentsObject>,
    pub next_page_token: Option<String>,
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

/// GET /objects/{object_id}/contents — list direct bundle contents with cursor pagination.
#[utoipa::path(
    get,
    path = "/objects/{object_id}/contents",
    params(BundleContentsQuery),
    responses((status = 200, body = BundleContentsPage, description = "Bundle contents page"), (status = 404, description = "Not found"))
)]
pub async fn list_bundle_contents(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    Query(params): Query<BundleContentsQuery>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<BundleContentsPage>> {
    tracing::info!(object_id = %object_id, "DRS list_bundle_contents");
    let resolved = state.repo.resolve_id_or_uri(&object_id).await?;
    let canonical =
        resolved.ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;

    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }

    let page_size = params.page_size.unwrap_or(100);
    let (contents, next_page_token) = state
        .repo
        .list_bundle_contents_page(&canonical, params.page_token.as_deref(), page_size)
        .await?;

    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let _ = state
        .repo
        .log_access(&canonical, None, "GET/contents", 200, client_ip.as_deref())
        .await;

    Ok(Json(BundleContentsPage {
        contents,
        next_page_token,
    }))
}

/// Get access URL for an access_id (e.g. presigned URL).
///
/// Returns **`application/json`** body [`AccessUrl`]: a **URL string** (`url`), optional `headers`,
/// optional `expires_at` — **not** the object bytes. For raw octets use **`GET .../objects/{id}/stream`**
/// (binary stream, see that operation).
///
/// **Storage:** `access_url` in the database may be a JSON string or `{"url":"…"}`; both are accepted.
///
/// **S3 / MinIO:** When a presigner is configured, `url` is replaced with a **presigned** HTTPS URL
/// when possible; if presigning fails, the stored URL is returned and a warning is logged (fallback).
#[utoipa::path(
    get,
    path = "/objects/{object_id}/access/{access_id}",
    responses((
        status = 200,
        body = AccessUrl,
        description = "JSON AccessUrl (url, optional headers, optional expires_at); not object bytes"
    ), (status = 404, description = "Not found"))
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
        let claims = auth.as_ref().ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
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
            let expires_secs: i64 = 3600;
            url.expires_at =
                Some((chrono::Utc::now() + chrono::Duration::seconds(expires_secs)).to_rfc3339());
            let expires = std::time::Duration::from_secs(expires_secs as u64);
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
    let _ = state
        .repo
        .log_access(
            &canonical,
            Some(access_id.as_str()),
            "GET",
            200,
            client_ip.as_deref(),
        )
        .await;
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
        let claims = auth.as_ref().ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
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
        return Err(DrsError::Validation(
            "view only allowed for mime_type text/html".into(),
        ));
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
    let mut reader = storage
        .get(&key)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;
    // A08: Verify stored sha-256 on serve; on mismatch return 500 and log.
    if let Some(expected) = obj
        .checksums
        .iter()
        .find(|c| c.r#type.eq_ignore_ascii_case("sha-256"))
        .map(|c| c.checksum.as_str())
    {
        let actual = hex::encode(Sha256::digest(&body));
        if !expected.eq_ignore_ascii_case(&actual) {
            tracing::error!(object_id = %canonical, "checksum_mismatch on serve");
            return Err(DrsError::Other(anyhow::anyhow!(
                "checksum verification failed"
            )));
        }
    }
    let _ = state
        .repo
        .log_access(&canonical, None, "GET/view", 200, None)
        .await;
    let res = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(
            "Content-Type",
            HeaderValue::from_static("text/html; charset=utf-8"),
        )
        .body(axum::body::Body::from(body))
        .map_err(|e| DrsError::Other(e.into()))?;
    Ok(res)
}

/// AsyncWrite adapter: decrypted Crypt4GH bytes into a bounded channel for HTTP streaming.
///
/// Lesson 4: bounded channels provide backpressure and prevent OOM on slow/dead clients.
/// Source: production postmortems (OOM due to unbounded buffering) + Tokio mpsc bounded design.
struct BoundedBodyWriter {
    tx: mpsc::Sender<Bytes>,
    pending_send: Option<
        Pin<
            Box<
                dyn Future<Output = std::result::Result<(), mpsc::error::SendError<Bytes>>>
                    + Send
                    + 'static,
            >,
        >,
    >,
    pending_timeout: Option<Pin<Box<tokio::time::Sleep>>>,
    pending_len: usize,
    send_timeout: Duration,
    /// Optional bytes accepted from decrypt and handed to the HTTP channel (observability).
    byte_counter: Option<Arc<AtomicU64>>,
}

impl AsyncWrite for BoundedBodyWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.as_mut().get_mut();

        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // If a previous send is still in-flight, poll it. `AsyncWrite::write_all` ensures
        // we won't see a new `buf` until the previous one is complete.
        if this.pending_send.is_none() {
            let chunk = Bytes::copy_from_slice(buf);
            this.pending_len = buf.len();
            let tx = this.tx.clone();
            this.pending_send = Some(Box::pin(async move { tx.send(chunk).await }));
            this.pending_timeout = Some(Box::pin(tokio::time::sleep(this.send_timeout)));
        }

        // Timeout: if the client doesn't drain the HTTP stream fast enough, the bounded
        // channel stays full and we fail fast to avoid resource leaks.
        if let Some(t) = this.pending_timeout.as_mut() {
            if t.as_mut().poll(cx).is_ready() {
                this.pending_send.take();
                this.pending_timeout.take();
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "client timeout while sending response bytes",
                )));
            }
        }

        let fut = this
            .pending_send
            .as_mut()
            .expect("pending_send must exist when poll_write is called");

        match fut.as_mut().poll(cx) {
            Poll::Ready(Ok(())) => {
                this.pending_send.take();
                this.pending_timeout.take();
                if let Some(c) = &this.byte_counter {
                    c.fetch_add(this.pending_len as u64, Ordering::Relaxed);
                }
                Poll::Ready(Ok(this.pending_len))
            }
            Poll::Ready(Err(_)) => {
                this.pending_send.take();
                this.pending_timeout.take();
                Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "stream receiver dropped",
                )))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// GET /objects/{object_id}/stream — stream object bytes. Unencrypted objects pass through from
/// storage; Crypt4GH at-rest objects are decrypted server-side when `crypt4gh_decrypt_stream` is
/// enabled and `crypt4gh_key_dir` contains the node key (`{crypt4gh_master_key_id}.sec`).
///
/// Response is a **binary octet stream** (`Content-Type` from object `mime_type` or
/// `application/octet-stream`), **not** JSON. Contrast with **`GET .../access/{access_id}`**, which
/// returns JSON with a **URL** to fetch bytes (and may use presigned URLs for S3/MinIO).
///
/// **Ferrum extension header:** `X-Ferrum-DRS-Stream-Path: plaintext | crypt4gh_decrypt` identifies
/// the server path for benchmarks (not a GA4GH standard field). Structured logs: target
/// `ferrum_drs::stream`, events `drs.stream.started` / `drs.stream.finished`.
#[utoipa::path(
    get,
    path = "/objects/{object_id}/stream",
    responses(
        (status = 200, description = "Binary body: object bytes (plaintext after Crypt4GH decrypt when applicable); not JSON. Response may include X-Ferrum-DRS-Stream-Path."),
        (status = 404, description = "Not found"),
        (status = 501, description = "Stream not supported for this storage backend")
    )
)]
pub async fn get_object_stream(
    State(state): State<Arc<AppState>>,
    Path(object_id): Path<String>,
    headers: HeaderMap,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Response> {
    tracing::info!(object_id = %object_id, "DRS get_object_stream");
    let resolved = state.repo.resolve_id_or_uri(&object_id).await?;
    let canonical =
        resolved.ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    if let Some(dataset_id) = state.repo.get_dataset_id(&canonical).await? {
        let claims = auth.as_ref().ok_or_else(|| {
            DrsError::Forbidden("authentication required for this dataset".into())
        })?;
        if !claims.has_dataset_grant(&dataset_id) && !claims.is_admin() {
            return Err(DrsError::Forbidden("dataset access not granted".into()));
        }
    }

    let storage = state.storage.as_ref().ok_or_else(|| {
        DrsError::Validation("object streaming requires configured storage (S3/local)".into())
    })?;

    let storage_ref = state
        .repo
        .get_storage_ref(&canonical)
        .await?
        .ok_or_else(|| DrsError::NotFound("no storage reference for object".into()))?;

    let (backend, key, is_encrypted) = storage_ref;
    let key = key.trim().trim_start_matches('/').to_string();
    let backend_lower = backend.to_lowercase();
    if !(backend_lower == "local" || backend_lower == "s3" || backend_lower == "minio") {
        return Err(DrsError::Validation(format!(
            "stream not supported for storage backend '{}'; use local, s3, or minio",
            backend
        )));
    }

    let obj = state
        .repo
        .get_object(&canonical, false)
        .await?
        .ok_or_else(|| DrsError::NotFound(format!("object not found: {}", object_id)))?;
    let mime = obj
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    let reader = storage
        .get(key.as_str())
        .await
        .map_err(|e| match e {
            FerrumError::NotFound(msg) => DrsError::NotFound(format!(
                "storage object missing (backend={} key={}): {msg}",
                backend_lower, key
            )),
            other => DrsError::Other(other.into()),
        })?;

    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let _ = state
        .repo
        .log_access(&canonical, None, "GET/stream", 200, client_ip.as_deref())
        .await;

    tracing::info!(
        target: "ferrum_drs::stream",
        object_id = %canonical,
        encrypted = is_encrypted,
        declared_size = obj.size,
        storage_backend = %backend_lower,
        event = "drs.stream.started",
    );

    if !is_encrypted {
        // Lesson 4: bounded channel between storage read and HTTP body — backpressure on slow clients.
        // Source: Zellij/OneUptime postmortems; unbounded buffering can OOM on TB-scale streams.
        const STORAGE_TO_HTTP_CAP: usize = 8;
        const READ_CHUNK: usize = 64 * 1024;
        const STORAGE_READ_TIMEOUT: Duration = Duration::from_secs(120);

        let (tx, rx) = mpsc::channel::<Bytes>(STORAGE_TO_HTTP_CAP);
        let bytes_from_storage = Arc::new(AtomicU64::new(0));
        let bytes_counter = bytes_from_storage.clone();
        let oid_plain = canonical.clone();
        tokio::spawn(async move {
            let mut reader = reader;
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                let read_fut = reader.read(&mut buf);
                let n = match tokio::time::timeout(STORAGE_READ_TIMEOUT, read_fut).await {
                    Ok(Ok(0)) => break,
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "plaintext stream read from storage failed");
                        break;
                    }
                    Err(_) => {
                        tracing::error!("plaintext stream read from storage timed out");
                        break;
                    }
                };
                bytes_counter.fetch_add(n as u64, Ordering::Relaxed);
                if tx.send(Bytes::copy_from_slice(&buf[..n])).await.is_err() {
                    break;
                }
            }
            tracing::info!(
                target: "ferrum_drs::stream",
                object_id = %oid_plain,
                event = "drs.stream.finished",
                stream_path = "plaintext",
                bytes_from_storage = bytes_counter.load(Ordering::Relaxed),
            );
        });

        let stream = ReceiverStream::new(rx).map(|b| Ok::<_, std::io::Error>(b));
        let body = Body::from_stream(stream);
        return Response::builder()
            .status(StatusCode::OK)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_str(mime)
                    .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
            )
            .header(
                HeaderName::from_static("x-ferrum-drs-stream-path"),
                HeaderValue::from_static("plaintext"),
            )
            .body(body)
            .map_err(|e| DrsError::Other(e.into()));
    }

    if !state.crypt4gh_decrypt_stream {
        return Err(DrsError::Validation(
            "object is Crypt4GH-encrypted; plaintext streaming is disabled (encryption.crypt4gh_decrypt_stream=false)"
                .into(),
        ));
    }

    let key_dir = state
        .crypt4gh_key_dir
        .as_ref()
        .ok_or_else(|| {
            DrsError::Validation(
                "Crypt4GH encrypted object: set encryption.crypt4gh_key_dir (or FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR) to the directory containing the node .sec key"
                    .into(),
            )
        })?;

    let ks = LocalKeyStore::new(key_dir.as_path());
    let keys = ks
        .get_private_key(&state.crypt4gh_master_key_id)
        .await
        .map_err(|e| DrsError::Other(anyhow::anyhow!("Crypt4GH key store: {}", e)))?
        .ok_or_else(|| {
            DrsError::Validation(format!(
                "no Crypt4GH private key for id '{}' under {:?}",
                state.crypt4gh_master_key_id, key_dir
            ))
        })?;

    const CLIENT_SEND_TIMEOUT: Duration = Duration::from_secs(30);
    const DECRYPT_TO_HTTP_CHANNEL_CAPACITY: usize = 4;

    let (tx, rx) = mpsc::channel::<Bytes>(DECRYPT_TO_HTTP_CHANNEL_CAPACITY);
    let bytes_to_client = Arc::new(AtomicU64::new(0));
    let bytes_counter = bytes_to_client.clone();
    let writer = BoundedBodyWriter {
        tx,
        pending_send: None,
        pending_timeout: None,
        pending_len: 0,
        send_timeout: CLIENT_SEND_TIMEOUT,
        byte_counter: Some(bytes_counter),
    };
    let oid_for_log = canonical.clone();
    tokio::spawn(async move {
        let r = stream_decrypt(&keys, reader, writer, None).await;
        let n = bytes_to_client.load(Ordering::Relaxed);
        tracing::info!(
            target: "ferrum_drs::stream",
            object_id = %oid_for_log,
            event = "drs.stream.finished",
            stream_path = "crypt4gh_decrypt",
            bytes_to_client = n,
            decrypt_ok = r.is_ok(),
        );
        if let Err(e) = r {
            tracing::error!(object_id = %oid_for_log, error = %e, "Crypt4GH stream_decrypt failed");
        }
    });

    let stream = ReceiverStream::new(rx).map(|b| Ok::<_, std::io::Error>(b));
    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(mime)
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        )
        .header(
            HeaderName::from_static("x-ferrum-drs-stream-path"),
            HeaderValue::from_static("crypt4gh_decrypt"),
        )
        .body(body)
        .map_err(|e| DrsError::Other(e.into()))
}

/// Parse Range header (e.g. "bytes=0-1023") into (start, end) inclusive. Returns None if missing or invalid.
fn parse_range_header(value: Option<&axum::http::HeaderValue>) -> Option<(u64, u64)> {
    let s = value?.to_str().ok()?.strip_prefix("bytes=")?;
    let (start, end) = s.split_once('-')?;
    let start: u64 = start.parse().ok()?;
    let end: u64 = end.parse().ok()?;
    if start <= end {
        Some((start, end))
    } else {
        None
    }
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<CreatedResponse>> {
    if let Some(ref ws_id) = req.workspace_id {
        let sub = auth
            .as_ref()
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
    if let Some(ref name) = req.name {
        ferrum_core::validate_drs_name(name).map_err(|e| DrsError::Validation(e.to_string()))?;
    }
    if let Some(ref aliases) = req.aliases {
        for a in aliases {
            ferrum_core::validate_drs_name(a).map_err(|e| DrsError::Validation(e.to_string()))?;
        }
    }
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
    if let Some(ref name) = req.name {
        ferrum_core::validate_drs_name(name).map_err(|e| DrsError::Validation(e.to_string()))?;
    }
    if let Some(ref aliases) = req.aliases {
        for a in aliases {
            ferrum_core::validate_drs_name(a).map_err(|e| DrsError::Validation(e.to_string()))?;
        }
    }
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<Vec<DrsObject>>> {
    let workspace_id = if let Some(ref ws_id) = q.workspace_id {
        let sub = auth
            .as_ref()
            .and_then(|c| c.sub())
            .ok_or_else(|| DrsError::Forbidden("workspace_id requires authentication".into()))?;
        let is_member = ferrum_core::get_workspace_member_role(state.repo.pool(), ws_id, sub)
            .await
            .map_err(|e| DrsError::Other(e.into()))?
            .is_some();
        if !is_member {
            return Err(DrsError::Forbidden("not a member of this workspace".into()));
        }
        q.workspace_id.as_deref()
    } else {
        None
    };
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
            workspace_id,
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
    let depth = q.depth.unwrap_or(10).clamp(1, 20);
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
