//! Outbreak download enforcement on DRS stream/access.

use ferrum_core::{
    auth::{AuthClaims, PassportClaims},
    ActivateRequest, ApproveDownloadRequest, OutbreakConfig, OutbreakPolicy, OutbreakService,
    FerrumPool,
};
use ferrum_drs::access::check_object_byte_access;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::state::AppState;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;

async fn outbreak_drs_state() -> AppState {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let fp = FerrumPool::Sqlite(pool);
    let repo = Arc::new(DrsRepo::new(fp.clone(), "localhost".into()));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).expect("storage"));

    let object_id = "obj-mpox-1".to_string();
    storage
        .put_bytes("drs/obj-mpox-1", b"SEQ")
        .await
        .expect("put");
    repo.create_object_with_id(
        &CreateObjectRequest {
            name: Some("mpox-seq".into()),
            description: None,
            mime_type: Some("application/x-fastq".into()),
            size: 3,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/obj-mpox-1".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
        },
        Some(object_id.clone()),
    )
    .await
    .expect("create");

    repo.insert_pathogen_annotation(
        &object_id,
        "Monkeypox_virus",
        &[],
        None,
        &[],
        None,
        None,
    )
    .await
    .expect("anno");

    let outbreak = Arc::new(OutbreakService::new(
        fp,
        OutbreakConfig {
            enabled: true,
            policies: vec![OutbreakPolicy {
                name: "mpox_who_emergency".into(),
                trigger_pathogen: "Monkeypox_virus".into(),
                emergency_recipients: vec!["who.int".into()],
                access_level: "beacon_only".into(),
                gisaid_auto_package: true,
            }],
        },
    ));
    outbreak
        .activate(&ActivateRequest {
            policy: "mpox_who_emergency".into(),
            activated_by: "ops@lab.org".into(),
        })
        .await
        .expect("activate");

    AppState {
        repo,
        storage: Some(storage as Arc<dyn ObjectStorage>),
        s3_presigner: None,
        provenance_store: None,
        crypt4gh_key_dir: None,
        crypt4gh_master_key_id: "node".into(),
        crypt4gh_decrypt_stream: false,
        ingest: Default::default(),
        object_storage_backend: "local".into(),
        outbreak: Some(outbreak),
        bandwidth: None,
        transfer_queue: None,
        residency_audit: None,
        background_gate: None,
    }
}

fn who_claims() -> AuthClaims {
    AuthClaims::Passport {
        claims: PassportClaims {
            sub: Some("who-user".into()),
            iss: Some("who.int".into()),
            exp: None,
            iat: None,
            jti: None,
            ga4gh_passport_v1: None,
            scope: None,
            aud: None,
        },
        visas: vec![],
    }
}

#[tokio::test]
async fn test_outbreak_download_requires_approval() {
    let state = outbreak_drs_state().await;
    let err = check_object_byte_access(&state, "obj-mpox-1", Some(&who_claims()))
        .await
        .expect_err("should block");
    assert!(err.to_string().contains("approve-download"));
}

#[tokio::test]
async fn test_outbreak_download_allowed_after_approval() {
    let state = outbreak_drs_state().await;
    let outbreak = state.outbreak.as_ref().unwrap();
    outbreak
        .approve_download(
            "mpox_who_emergency",
            "obj-mpox-1",
            &ApproveDownloadRequest {
                recipient: "who.int".into(),
                approved_by: "dac@lab.org".into(),
            },
        )
        .await
        .expect("approve");
    check_object_byte_access(&state, "obj-mpox-1", Some(&who_claims()))
        .await
        .expect("allowed after approval");
}
