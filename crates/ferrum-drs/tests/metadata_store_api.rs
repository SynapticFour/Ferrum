//! Metadata Store API (M1) — PUT/GET/list over metadata_submissions.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::{metadata_api_router, metadata_api_router_disabled, AppState};
use ferrum_meta_connect::{submission_alias, validate_submission, MetaProfile};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn sqlite_pool() -> FerrumPool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    FerrumPool::Sqlite(pool)
}

fn sample_bundle() -> Value {
    json!({
        "ferrum_meta_version": "0.1.0",
        "studies": [{"alias": "s1", "title": "t", "description": "d", "type": "PATHOGEN_SURVEILLANCE", "data_use_conditions": ["DUO:0000007"]}],
        "individuals": [{"alias": "i1", "consent_type": "RESEARCH"}],
        "samples": [{"alias": "sa1", "individual_alias": "i1", "collection_date": "2026-01-01", "collection_site": "site"}],
        "experiments": [{"alias": "e1", "sample_alias": "sa1", "pathogen_organism": "Plasmodium_falciparum"}],
        "files": [{"alias": "f1"}],
        "datasets": [{"alias": "dataset_path001", "title": "t", "file_aliases": ["f1"]}]
    })
}

fn enabled_state(pool: FerrumPool) -> AppState {
    AppState {
        repo: Arc::new(DrsRepo::new(pool, "localhost".into())),
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
        ingest_require_auth: false,
        metadata_store_enabled: true,
        pipeline: Default::default(),
    }
}

#[tokio::test]
async fn metadata_store_put_get_list_roundtrip() {
    let pool = sqlite_pool().await;
    let state = enabled_state(pool);
    let app = metadata_api_router(Arc::new(state));

    let bundle = sample_bundle();
    let report = validate_submission(&bundle, Some(MetaProfile::Pathogen));
    assert!(report.valid, "{report}");
    let alias = submission_alias(&bundle).expect("alias");

    let put = Request::builder()
        .method("PUT")
        .uri(format!("/submissions/{alias}?profile=pathogen"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&bundle).unwrap()))
        .unwrap();
    let put_res = app.clone().oneshot(put).await.unwrap();
    assert_eq!(put_res.status(), StatusCode::OK);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/submissions/{alias}"))
        .body(Body::empty())
        .unwrap();
    let get_res = app.clone().oneshot(get).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(get_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["alias"], alias);
    assert_eq!(body["profile"], "pathogen");
    assert_eq!(body["document"]["datasets"][0]["alias"], alias);

    let list = Request::builder()
        .method("GET")
        .uri("/submissions?profile=pathogen")
        .body(Body::empty())
        .unwrap();
    let list_res = app.oneshot(list).await.unwrap();
    assert_eq!(list_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["count"], 1);
    assert_eq!(body["items"][0]["alias"], alias);
}

#[tokio::test]
async fn metadata_store_disabled_returns_501() {
    let app = metadata_api_router_disabled();
    let req = Request::builder()
        .method("GET")
        .uri("/submissions")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn metadata_store_post_creates() {
    let pool = sqlite_pool().await;
    let state = enabled_state(pool);
    let app = metadata_api_router(Arc::new(state));
    let bundle = sample_bundle();

    let post = Request::builder()
        .method("POST")
        .uri("/submissions?profile=pathogen")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&bundle).unwrap()))
        .unwrap();
    let res = app.oneshot(post).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}
