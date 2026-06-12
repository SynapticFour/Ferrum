//! Residency audit HTTP and event wiring tests.

use axum::http::{Method, Request, StatusCode};
use ferrum_core::{
    auth::{AuthClaims, PassportClaims},
    FerrumPool, OutbreakConfig, OutbreakPolicy, OutbreakService, ResidencyAuditLog,
};
use ferrum_drs::ingest::{process_upload_from_parts, ParsedMultipartUpload};
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::state::AppState;
use ferrum_gateway::audit::audit_router;
use ferrum_gateway::outbreak::outbreak_router;
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;
use tower::ServiceExt;

async fn audit_pool() -> (FerrumPool, Arc<ResidencyAuditLog>) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let fp = FerrumPool::Sqlite(pool);
    let audit = Arc::new(ResidencyAuditLog::new(fp.clone()));
    (fp, audit)
}

#[tokio::test]
async fn test_audit_delete_method_not_allowed() {
    let (_, audit) = audit_pool().await;
    let app = audit_router(audit);
    let req = Request::builder()
        .method(Method::DELETE)
        .uri("/residency")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_upload_writes_data_uploaded_event() {
    let (fp, audit) = audit_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path()).unwrap());
    let state = AppState {
        repo: Arc::new(DrsRepo::new(fp.clone(), "localhost".into())),
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
        residency_audit: Some(audit.clone()),
        background_gate: None,
    };
    process_upload_from_parts(
        Arc::new(state),
        None,
        ParsedMultipartUpload {
            file_name: Some("sample.fq".into()),
            data: b"ACGT".to_vec(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let entries = audit.query_range(None, None).await.unwrap().entries;
    assert!(entries.iter().any(|e| e.event_type == "data_uploaded"));
}

#[tokio::test]
async fn test_outbreak_activate_deactivate_audit_events() {
    let (fp, audit) = audit_pool().await;
    let service = Arc::new(OutbreakService::new(
        fp,
        OutbreakConfig {
            enabled: true,
            policies: vec![OutbreakPolicy {
                name: "test_policy".into(),
                trigger_pathogen: "Test_pathogen".into(),
                emergency_recipients: vec!["who.int".into()],
                access_level: "beacon_only".into(),
                gisaid_auto_package: false,
            }],
        },
    ));
    let app = outbreak_router(service, Some(audit.clone()));

    let claims = AuthClaims::Passport {
        claims: PassportClaims {
            sub: Some("activator@lab.org".into()),
            iss: Some("lab.org".into()),
            exp: None,
            iat: None,
            jti: None,
            ga4gh_passport_v1: None,
            scope: None,
            aud: None,
        },
        visas: vec![ferrum_core::VisaObject {
            r#type: "Role".into(),
            asserted: 0,
            value: "ferrum:outbreak_activator".into(),
            source: "lab.org".into(),
            conditions: None,
            by: None,
        }],
    };

    let activate = Request::builder()
        .method(Method::POST)
        .uri("/activate")
        .header("content-type", "application/json")
        .extension(claims.clone())
        .body(axum::body::Body::from(
            r#"{"policy":"test_policy","activated_by":"activator@lab.org"}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(activate).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let deactivate = Request::builder()
        .method(Method::POST)
        .uri("/deactivate")
        .header("content-type", "application/json")
        .extension(claims)
        .body(axum::body::Body::from(
            r#"{"policy":"test_policy","reason":"test complete"}"#,
        ))
        .unwrap();
    let resp = app.oneshot(deactivate).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let entries = audit.query_range(None, None).await.unwrap().entries;
    assert!(entries.iter().any(|e| e.event_type == "outbreak_activated"));
    assert!(entries
        .iter()
        .any(|e| e.event_type == "outbreak_deactivated"));
}
