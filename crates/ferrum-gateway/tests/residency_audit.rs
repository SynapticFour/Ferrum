// SPDX-License-Identifier: BUSL-1.1
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

/// Serialise tests that mutate `FERRUM_AUTH__REQUIRE_AUTH` (process-global).
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct RestoreRequireAuth(Option<String>);

impl Drop for RestoreRequireAuth {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", value),
            None => std::env::remove_var("FERRUM_AUTH__REQUIRE_AUTH"),
        }
    }
}

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
    let app = audit_router(audit, None);
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
    // Audit event wiring, not the bearer gate. ADR-027 rejects anonymous ingest
    // when require_auth is on, and the runtime default is on.
    let _env = ENV_LOCK.lock().await;
    let prev_auth = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
    let _restore_auth = RestoreRequireAuth(prev_auth);
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
        denied_audit: None,
        background_gate: None,
        ads_introspect: None,
        solum_consent: None,
        ingest_require_auth: false,
        require_passport_for_bytes: false,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
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
        raw_token: None,
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

#[tokio::test]
async fn test_rls_residency_audit() {
    let (_, audit) = audit_pool().await;
    audit
        .append(
            "data_uploaded",
            Some("obj-a"),
            Some("inst-a"),
            None,
            false,
            None,
        )
        .await
        .unwrap();
    audit
        .append(
            "data_uploaded",
            Some("obj-b"),
            Some("inst-b"),
            None,
            false,
            None,
        )
        .await
        .unwrap();

    let inst_a = audit
        .query_range_for_requester(None, None, Some("inst-a"), false)
        .await
        .unwrap();
    assert_eq!(inst_a.entries.len(), 1);
    assert_eq!(inst_a.entries[0].requester.as_deref(), Some("inst-a"));

    let admin = audit
        .query_range_for_requester(None, None, Some("inst-a"), true)
        .await
        .unwrap();
    assert!(admin.entries.len() >= 2);
}

fn get_residency_request(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
async fn test_residency_get_without_auth_is_401_when_require_auth_on() {
    let _env = ENV_LOCK.lock().await;
    let prev_auth = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "true");
    let _restore_auth = RestoreRequireAuth(prev_auth);

    let (_, audit) = audit_pool().await;
    let app = audit_router(audit, None);

    let resp = app
        .clone()
        .oneshot(get_residency_request("/residency"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "unauthorized");

    // ADR-031: verify stays unauthenticated.
    let resp = app
        .clone()
        .oneshot(get_residency_request("/residency/verify"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // With a bearer-derived claim the list is served.
    let claims = AuthClaims::Jwt {
        sub: "auditor@lab.org".into(),
        iss: Some("lab.org".into()),
        exp: i64::MAX,
        jti: None,
        scope: None,
        raw_token: None,
    };
    let mut req = get_residency_request("/residency");
    req.extensions_mut().insert(claims);
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_residency_get_anonymous_ok_when_require_auth_off() {
    let _env = ENV_LOCK.lock().await;
    let prev_auth = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
    let _restore_auth = RestoreRequireAuth(prev_auth);

    let (_, audit) = audit_pool().await;
    let resp = audit_router(audit, None)
        .oneshot(get_residency_request("/residency"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_authorization_denied_event_minimizes_ip_and_stores_no_token() {
    let (_, audit) = audit_pool().await;
    let sink = ferrum_core::DeniedAuditSink::new(
        audit.clone(),
        &ferrum_core::AuditConfig {
            anonymous_client_ip: "truncated".into(),
            denied_events_per_minute: 1,
        },
    );
    assert!(
        sink.record(
            ferrum_core::DeniedReason::InvalidToken,
            None,
            None,
            Some("203.0.113.77"),
        )
        .await
    );
    // Second append in the same minute is rate limited.
    assert!(
        !sink
            .record(
                ferrum_core::DeniedReason::InvalidToken,
                None,
                None,
                Some("203.0.113.78"),
            )
            .await
    );

    let result = audit.query_range(None, None).await.unwrap();
    let denied: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.event_type == "authorization_denied")
        .collect();
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].destination.as_deref(), Some("203.0.113.0/24"));
    assert_eq!(denied[0].drs_id.as_deref(), Some("reason=invalid_token"));
    assert!(denied[0].requester.is_none());
    assert!(!denied[0].data_left_node);
    assert!(result.chain_valid);
}

#[tokio::test]
async fn test_authorization_denied_default_omits_ip() {
    let (_, audit) = audit_pool().await;
    let sink = ferrum_core::DeniedAuditSink::new(audit.clone(), &Default::default());
    assert!(
        sink.record(
            ferrum_core::DeniedReason::MissingBearer,
            None,
            None,
            Some("198.51.100.5"),
        )
        .await
    );
    let entries = audit.query_range(None, None).await.unwrap().entries;
    let e = entries
        .iter()
        .find(|e| e.event_type == "authorization_denied")
        .unwrap();
    assert!(e.destination.is_none());
}
