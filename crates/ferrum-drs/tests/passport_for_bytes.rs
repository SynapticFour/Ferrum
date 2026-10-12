// SPDX-License-Identifier: BUSL-1.1
//! ADR-030: `require_passport_for_bytes` gates byte paths for unbound objects only.

use ferrum_core::{auth::AuthClaims, FerrumPool};
use ferrum_drs::access::{check_object_byte_access, check_object_metadata_access};
use ferrum_drs::error::DrsError;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::state::AppState;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;

async fn unbound_object_state(require_passport_for_bytes: bool) -> (AppState, tempfile::TempDir) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = Arc::new(DrsRepo::new(FerrumPool::Sqlite(pool), "localhost".into()));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).expect("storage"));
    storage.put_bytes("drs/open-obj", b"OPEN").await.unwrap();
    repo.create_object_with_id(
        &CreateObjectRequest {
            name: Some("seed".into()),
            description: None,
            mime_type: Some("application/octet-stream".into()),
            size: 4,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/open-obj".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        },
        Some("open-obj".into()),
    )
    .await
    .expect("create");

    let state = AppState {
        repo,
        storage: Some(storage as Arc<dyn ObjectStorage>),
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
        residency_audit: None,
        background_gate: None,
        ads_introspect: None,
        solum_consent: None,
        ingest_require_auth: false,
        require_passport_for_bytes,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
    };
    (state, tmp)
}

fn jwt_claims(sub: &str) -> AuthClaims {
    AuthClaims::Jwt {
        sub: sub.to_string(),
        iss: None,
        exp: 0,
        jti: None,
        scope: None,
        raw_token: None,
    }
}

#[tokio::test]
async fn flag_off_anonymous_bytes_allowed_for_unbound_object() {
    let (state, _tmp) = unbound_object_state(false).await;
    check_object_byte_access(&state, "open-obj", None, None)
        .await
        .expect("default behaviour unchanged");
}

#[tokio::test]
async fn flag_on_rejects_anonymous_bytes_with_unauthorized() {
    let (state, _tmp) = unbound_object_state(true).await;
    let err = check_object_byte_access(&state, "open-obj", None, None)
        .await
        .expect_err("anonymous stream/access must be rejected");
    assert!(
        matches!(err, DrsError::Unauthorized(_)),
        "expected Unauthorized (401), got: {err}"
    );
}

#[tokio::test]
async fn flag_on_allows_bytes_with_bearer() {
    let (state, _tmp) = unbound_object_state(true).await;
    check_object_byte_access(&state, "open-obj", Some(&jwt_claims("user@lab")), None)
        .await
        .expect("bearer allowed");
}

#[tokio::test]
async fn flag_on_keeps_anonymous_metadata_for_unbound_object() {
    let (state, _tmp) = unbound_object_state(true).await;
    check_object_metadata_access(&state, "open-obj", None, None)
        .await
        .expect("catalog/metadata stays anonymous for unbound objects");
}
