// SPDX-License-Identifier: BUSL-1.1
//! Anonymous DRS create/update/delete is rejected when require_auth is on.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::AppState;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;

fn auth_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn app() -> axum::Router {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = Arc::new(DrsRepo::new(FerrumPool::Sqlite(pool), "localhost".into()));
    let state = AppState {
        repo,
        storage: None,
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
        ingest_require_auth: true,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
    };
    ferrum_drs::router(state)
}

fn json_req(method: &str, uri: &str, body: &'static str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

const CREATE: &str = r#"{"name":"anonymous-probe","size":1,"checksums":[{"type":"sha256","checksum":"aa"}],"storage_backend":"local","storage_key":"probe"}"#;

#[tokio::test]
async fn anonymous_object_mutations_are_401_when_require_auth_is_true() {
    let _guard = auth_lock().lock().await;
    let prev = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "true");
    let app = app().await;
    for (method, uri, body) in [
        ("POST", "/objects", CREATE),
        ("PUT", "/objects/missing", r#"{"name":"x"}"#),
        ("DELETE", "/objects/missing", ""),
        (
            "POST",
            "/ingest/url",
            r#"{"url":"https://example.invalid/giab.bed"}"#,
        ),
        ("POST", "/ingest/batch", r#"{"items":[]}"#),
    ] {
        let res = app
            .clone()
            .oneshot(json_req(method, uri, body))
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {uri} must reject a missing bearer token"
        );
    }
    match prev {
        Some(v) => std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", v),
        None => std::env::remove_var("FERRUM_AUTH__REQUIRE_AUTH"),
    }
}

#[tokio::test]
async fn anonymous_post_is_not_401_when_require_auth_is_false() {
    let _guard = auth_lock().lock().await;
    let prev = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
    std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_req("POST", "/objects", CREATE))
        .await
        .unwrap();
    assert_ne!(res.status(), StatusCode::UNAUTHORIZED);
    match prev {
        Some(v) => std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", v),
        None => std::env::remove_var("FERRUM_AUTH__REQUIRE_AUTH"),
    }
}
