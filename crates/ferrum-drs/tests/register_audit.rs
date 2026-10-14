// SPDX-License-Identifier: BUSL-1.1
//! POST /register writes a residency row and fails closed when that insert fails.
//!
//! Seeded from evidence/2026-10/p7-claims-behavior: register created a DRS row
//! and wrote no audit row.

use axum::body::Body;
use ferrum_core::{FerrumPool, ResidencyAuditLog};
use ferrum_drs::ingest_api_v1_router;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::AppState;
use http::Request;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;

async fn pool() -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ferrum.db");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    (dir, pool)
}

fn state(pool: SqlitePool, objects: &std::path::Path) -> Arc<AppState> {
    let ferrum = FerrumPool::Sqlite(pool.clone());
    let local = ferrum_storage::LocalStorage::new(objects).unwrap();
    Arc::new(AppState {
        repo: Arc::new(DrsRepo::new(ferrum.clone(), "localhost".into())),
        storage: Some(Arc::new(local)),
        s3_presigner: None,
        provenance_store: None,
        crypt4gh_key_dir: None,
        crypt4gh_master_key_id: "node".into(),
        crypt4gh_decrypt_stream: false,
        ingest: Default::default(),
        object_storage_backend: "local".into(),
        outbreak: None,
        bandwidth: None,
        transfer_queue: None,
        residency_audit: Some(Arc::new(ResidencyAuditLog::new(ferrum))),
        denied_audit: None,
        background_gate: None,
        ads_introspect: None,
        solum_consent: None,
        ingest_require_auth: false,
        require_passport_for_bytes: false,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
    })
}

fn register_body(name: &str) -> String {
    format!(
        r#"{{"items":[{{"kind":"existing_object","storage_backend":"local","storage_key":"drs/{name}","size":4,"name":"{name}"}}]}}"#
    )
}

#[tokio::test]
async fn register_appends_data_uploaded_and_fails_closed_when_audit_is_gone() {
    let (dir, pool) = pool().await;
    let objects = dir.path().join("objects");
    let state = state(pool.clone(), &objects);
    let body = register_body("registered-with-audit");
    let app = ingest_api_v1_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    let audit_n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM residency_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit_n, 1);
    let verified = state
        .residency_audit
        .as_ref()
        .unwrap()
        .verify()
        .await
        .unwrap();
    assert!(verified.chain_valid);
    assert_eq!(verified.entry_count, 1);

    sqlx::query("DROP TABLE residency_audit")
        .execute(&pool)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drs_objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    let app = ingest_api_v1_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/json")
                .body(Body::from(register_body("registered-after-drop")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drs_objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        after, before,
        "a failed audit insert must not keep the new object"
    );
}
