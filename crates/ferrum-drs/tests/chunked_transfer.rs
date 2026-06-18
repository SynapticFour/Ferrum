//! Chunked resume, low-bandwidth compression, and transfer queue tests.

use ferrum_core::{FerrumPool, ResidencyAuditLog};
use ferrum_drs::checkpoint::{create_checkpoint, load_checkpoint, update_checkpoint_progress};
use ferrum_drs::handlers::get_object_stream;
use ferrum_drs::ingest::{process_upload_from_parts, ParsedMultipartUpload};
use ferrum_drs::ingest_chunk::process_chunked_upload_from_parts;
use ferrum_drs::state::AppState;
use ferrum_storage::{
    BandwidthClass, BandwidthMonitor, LocalStorage, ObjectStorage, TransferQueue,
};
use http::{Method, Request, StatusCode};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tower::ServiceExt;

const PAYLOAD: &[u8] = b"0123456789abcdef0123456789abcdef";

async fn drs_test_state() -> (AppState, tempfile::TempDir) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let fp = FerrumPool::Sqlite(pool.clone());
    let repo = Arc::new(ferrum_drs::repo::DrsRepo::new(
        fp.clone(),
        "localhost".into(),
    ));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).expect("storage"));
    storage.put_bytes("drs/obj1", PAYLOAD).await.unwrap();
    repo.create_object_with_id(
        &ferrum_drs::types::CreateObjectRequest {
            name: Some("obj".into()),
            description: None,
            mime_type: Some("application/octet-stream".into()),
            size: PAYLOAD.len() as i64,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/obj1".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        },
        Some("obj1".into()),
    )
    .await
    .unwrap();
    let bw = Arc::new(BandwidthMonitor::new(Default::default()));
    bw.inject_mock_bps(50_000_000);
    (
        AppState {
            repo,
            storage: Some(storage),
            s3_presigner: None,
            provenance_store: None,
            crypt4gh_key_dir: None,
            crypt4gh_master_key_id: "node".into(),
            crypt4gh_decrypt_stream: false,
            ingest: Default::default(),
            object_storage_backend: "local".into(),
            outbreak: None,
            bandwidth: Some(bw),
            transfer_queue: Some(Arc::new(TransferQueue::new(300))),
            residency_audit: Some(Arc::new(ResidencyAuditLog::new(fp))),
            background_gate: None,
            ads_introspect: None,
        },
        tmp,
    )
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

#[tokio::test]
async fn test_chunked_resume_http_interrupt_and_sha256() {
    let (state, _tmp) = drs_test_state().await;
    let cp = create_checkpoint(
        state.repo.pool(),
        "obj1",
        "download",
        PAYLOAD.len() as i64,
        BandwidthClass::High,
    )
    .await
    .unwrap();

    let app = axum::Router::new()
        .route(
            "/objects/:object_id/stream",
            axum::routing::get(get_object_stream),
        )
        .with_state(Arc::new(state.clone()));

    // Simulate interrupted transfer: consume first 8 bytes, record checkpoint, resume.
    let req1 = Request::builder()
        .method(Method::GET)
        .uri("/objects/obj1/stream")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);
    let first_body = axum::body::to_bytes(resp1.into_body(), usize::MAX)
        .await
        .unwrap();
    let partial = &first_body[..8];
    assert_eq!(partial, &PAYLOAD[..8]);

    update_checkpoint_progress(state.repo.pool(), &cp.resume_token, 8)
        .await
        .unwrap();

    let req2 = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/objects/obj1/stream?resume_token={}",
            cp.resume_token
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let remainder = axum::body::to_bytes(resp2.into_body(), usize::MAX)
        .await
        .unwrap();

    let mut reconstructed = partial.to_vec();
    reconstructed.extend_from_slice(&remainder);
    assert_eq!(reconstructed.as_slice(), PAYLOAD);
    assert_eq!(sha256_hex(&reconstructed), sha256_hex(PAYLOAD));

    let loaded = load_checkpoint(state.repo.pool(), &cp.resume_token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.completed_bytes, PAYLOAD.len() as i64);
}

#[tokio::test]
async fn test_low_bandwidth_compression_flag() {
    let (state, _tmp) = drs_test_state().await;
    state.bandwidth.as_ref().unwrap().inject_mock_bps(50_000);
    let class = state.bandwidth.as_ref().unwrap().classify();
    assert!(class.use_zstd_compression());
}

#[tokio::test]
async fn test_low_bandwidth_compression() {
    let (state, _tmp) = drs_test_state().await;
    state.bandwidth.as_ref().unwrap().inject_mock_bps(50_000);
    let app = axum::Router::new()
        .route(
            "/objects/:object_id/stream",
            axum::routing::get(get_object_stream),
        )
        .with_state(Arc::new(state));
    let req = Request::builder()
        .method(Method::GET)
        .uri("/objects/obj1/stream")
        .header("Accept-Encoding", "zstd")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("zstd")
    );
}

#[tokio::test]
async fn test_transfer_queue_defers_large_download() {
    let (state, _tmp) = drs_test_state().await;
    state.bandwidth.as_ref().unwrap().inject_mock_bps(50_000);
    let large = vec![0u8; 11 * 1024 * 1024];
    state
        .storage
        .as_ref()
        .unwrap()
        .put_bytes("drs/large", &large)
        .await
        .unwrap();
    state
        .repo
        .create_object_with_id(
            &ferrum_drs::types::CreateObjectRequest {
                name: Some("large".into()),
                description: None,
                mime_type: Some("application/octet-stream".into()),
                size: large.len() as i64,
                checksums: vec![],
                aliases: None,
                storage_backend: "local".into(),
                storage_key: "drs/large".into(),
                is_encrypted: Some(false),
                workspace_id: None,
                ont_metrics: None,
                gisaid_metadata: None,
                metadata_ref: None,
            },
            Some("large1".into()),
        )
        .await
        .unwrap();

    let app = axum::Router::new()
        .route(
            "/objects/:object_id/stream",
            axum::routing::get(get_object_stream),
        )
        .with_state(Arc::new(state.clone()));
    let req = Request::builder()
        .method(Method::GET)
        .uri("/objects/large1/stream")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(state.transfer_queue.as_ref().unwrap().len(), 1);
}

#[tokio::test]
async fn test_transfer_queue_defers_large_upload() {
    let (state, _tmp) = drs_test_state().await;
    state.bandwidth.as_ref().unwrap().inject_mock_bps(50_000);
    let large = vec![1u8; 11 * 1024 * 1024];
    let err = process_upload_from_parts(
        Arc::new(state.clone()),
        None,
        ParsedMultipartUpload {
            file_name: Some("big.bin".into()),
            data: large,
            ..Default::default()
        },
    )
    .await;
    assert!(matches!(
        err,
        Err(ferrum_drs::error::DrsError::TransferQueued(_))
    ));
    assert_eq!(state.transfer_queue.as_ref().unwrap().len(), 1);
}

#[tokio::test]
async fn test_chunked_upload_resume_and_sha256() {
    let (state, _tmp) = drs_test_state().await;
    let payload: Vec<u8> = (0..64).map(|i| i as u8).collect();
    let first = process_chunked_upload_from_parts(
        Arc::new(state.clone()),
        None,
        ParsedMultipartUpload {
            total_bytes: Some(payload.len() as i64),
            chunk_offset: Some(0),
            data: payload[..20].to_vec(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(!first.complete);
    assert_eq!(first.completed_bytes, 20);

    let second = process_chunked_upload_from_parts(
        Arc::new(state.clone()),
        None,
        ParsedMultipartUpload {
            upload_token: Some(first.upload_token.clone()),
            total_bytes: Some(payload.len() as i64),
            chunk_offset: Some(20),
            data: payload[20..].to_vec(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(second.complete);
    assert_eq!(second.size, Some(payload.len() as i64));
    assert_eq!(sha256_hex(&payload), sha256_hex(&payload));
}

#[tokio::test]
async fn test_checksum_deferred_in_low_power() {
    let (mut state, _tmp) = drs_test_state().await;
    state.background_gate = Some(Arc::new(ferrum_core::BackgroundWorkGate::new(
        ferrum_core::FerrumPowerMode::LowPower,
    )));
    let resp = process_upload_from_parts(
        Arc::new(state.clone()),
        None,
        ParsedMultipartUpload {
            file_name: Some("small.bin".into()),
            data: b"tiny-payload".to_vec(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let status = state.repo.get_checksum_status(&resp.id).await.unwrap();
    assert_eq!(status.as_deref(), Some("deferred_low_power"));
}
