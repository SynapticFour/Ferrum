//! Workspace-scoped private DRS access (no ADS until publish).

use ferrum_core::{auth::AuthClaims, FerrumPool};
use ferrum_drs::access::check_object_byte_access;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::state::AppState;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;

async fn workspace_drs_state() -> (AppState, tempfile::TempDir) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    sqlx::query(
        "CREATE TABLE workspaces (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            owner_sub TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            is_archived INTEGER NOT NULL DEFAULT 0,
            settings TEXT NOT NULL DEFAULT '{}'
        )",
    )
    .execute(&pool)
    .await
    .expect("workspaces");
    sqlx::query(
        "CREATE TABLE workspace_members (
            workspace_id TEXT NOT NULL,
            sub TEXT NOT NULL,
            role TEXT NOT NULL,
            invited_by TEXT NOT NULL,
            PRIMARY KEY (workspace_id, sub)
        )",
    )
    .execute(&pool)
    .await
    .expect("members");
    sqlx::query(
        "INSERT INTO workspaces (id, name, owner_sub, slug) VALUES ('ws-1', 'Lab', 'owner@lab', 'lab')",
    )
    .execute(&pool)
    .await
    .expect("ws");
    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, sub, role, invited_by) VALUES ('ws-1', 'member@lab', 'viewer', 'owner@lab')",
    )
    .execute(&pool)
    .await
    .expect("member");

    let fp = FerrumPool::Sqlite(pool);
    let repo = Arc::new(DrsRepo::new(fp, "localhost".into()));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).expect("storage"));
    storage.put_bytes("drs/ws-obj", b"PRIVATE").await.unwrap();
    repo.create_object_with_id(
        &CreateObjectRequest {
            name: Some("private-seq".into()),
            description: None,
            mime_type: Some("application/octet-stream".into()),
            size: 7,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/ws-obj".into(),
            is_encrypted: Some(false),
            workspace_id: Some("ws-1".into()),
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        },
        Some("ws-obj".into()),
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
async fn workspace_member_can_access_private_object() {
    let (state, _tmp) = workspace_drs_state().await;
    check_object_byte_access(&state, "ws-obj", Some(&jwt_claims("member@lab")), None)
        .await
        .expect("member allowed");
}

#[tokio::test]
async fn non_member_denied_private_object() {
    let (state, _tmp) = workspace_drs_state().await;
    let err = check_object_byte_access(&state, "ws-obj", Some(&jwt_claims("outsider@other")), None)
        .await
        .expect_err("outsider blocked");
    assert!(err.to_string().contains("workspace"));
}

#[tokio::test]
async fn unauthenticated_denied_private_object() {
    let (state, _tmp) = workspace_drs_state().await;
    let err = check_object_byte_access(&state, "ws-obj", None, None)
        .await
        .expect_err("auth required");
    assert!(err.to_string().contains("authentication"));
}
