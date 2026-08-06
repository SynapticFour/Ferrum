//! H2.1 Teeth: Solum consent gates DRS byte access when binding resolves.

use ferrum_core::{auth::AuthClaims, FerrumPool, SolumConfig, SolumConsentClient};
use ferrum_drs::access::check_object_byte_access;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::state::AppState;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn base_state() -> (AppState, tempfile::TempDir, Arc<DrsRepo>) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let fp = FerrumPool::Sqlite(pool);
    let repo = Arc::new(DrsRepo::new(fp, "localhost".into()));
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).expect("storage"));
    storage.put_bytes("drs/obj-teeth", b"DATA").await.unwrap();
    repo.create_object_with_id(
        &CreateObjectRequest {
            name: Some("teeth.bin".into()),
            description: None,
            mime_type: Some("application/octet-stream".into()),
            size: 4,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/obj-teeth".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        },
        Some("obj-teeth".into()),
    )
    .await
    .expect("create");

    let state = AppState {
        repo: repo.clone(),
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
    (state, tmp, repo)
}

fn jwt() -> AuthClaims {
    AuthClaims::Jwt {
        sub: "tester".into(),
        iss: None,
        exp: 0,
        jti: None,
        scope: None,
        raw_token: None,
    }
}

#[tokio::test]
async fn unbound_object_skips_solum() {
    let (state, _tmp, _repo) = base_state().await;
    check_object_byte_access(&state, "obj-teeth", Some(&jwt()), None)
        .await
        .expect("unbound ok");
}

#[tokio::test]
async fn granted_allows_bound_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/consent/status"))
        .and(query_param("subject", "patient/demo"))
        .and(query_param("purpose", "secondary_use_hdab"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "granted"
        })))
        .mount(&server)
        .await;

    let (mut state, _tmp, repo) = base_state().await;
    repo.set_metadata("obj-teeth", "solum_subject", "patient/demo")
        .await
        .unwrap();
    repo.set_metadata("obj-teeth", "solum_purpose", "secondary_use_hdab")
        .await
        .unwrap();
    let client = SolumConsentClient::from_config(&SolumConfig {
        base_url: Some(server.uri()),
        sidecar_token: Some("tok".into()),
        default_subject: None,
        default_purpose: None,
        timeout_secs: 5,
    })
    .unwrap();
    state.solum_consent = Some(Arc::new(client));

    check_object_byte_access(&state, "obj-teeth", Some(&jwt()), None)
        .await
        .expect("granted");
}

#[tokio::test]
async fn revoked_denies_bound_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/consent/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "revoked"
        })))
        .mount(&server)
        .await;

    let (mut state, _tmp, repo) = base_state().await;
    repo.set_metadata("obj-teeth", "solum_subject", "patient/demo")
        .await
        .unwrap();
    repo.set_metadata("obj-teeth", "solum_purpose", "secondary_use_hdab")
        .await
        .unwrap();
    let client = SolumConsentClient::from_config(&SolumConfig {
        base_url: Some(server.uri()),
        sidecar_token: Some("tok".into()),
        default_subject: None,
        default_purpose: None,
        timeout_secs: 5,
    })
    .unwrap();
    state.solum_consent = Some(Arc::new(client));

    let err = check_object_byte_access(&state, "obj-teeth", Some(&jwt()), None)
        .await
        .expect_err("revoked");
    assert!(err.to_string().contains("solum consent"));
}

#[tokio::test]
async fn defaults_arm_check_without_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/consent/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "unknown"
        })))
        .mount(&server)
        .await;

    let (mut state, _tmp, _repo) = base_state().await;
    let client = SolumConsentClient::from_config(&SolumConfig {
        base_url: Some(server.uri()),
        sidecar_token: Some("tok".into()),
        default_subject: Some("patient/demo".into()),
        default_purpose: Some("secondary_use_hdab".into()),
        timeout_secs: 5,
    })
    .unwrap();
    state.solum_consent = Some(Arc::new(client));

    let err = check_object_byte_access(&state, "obj-teeth", Some(&jwt()), None)
        .await
        .expect_err("unknown deny");
    assert!(err.to_string().contains("solum consent"));
}
