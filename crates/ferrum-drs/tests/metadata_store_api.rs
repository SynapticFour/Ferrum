//! Metadata Store API (M1/M2) — PUT/GET/list, versions, attach/detach.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::types::CreateObjectRequest;
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

fn sample_bundle(title: &str) -> Value {
    json!({
        "ferrum_meta_version": "0.1.0",
        "studies": [{"alias": "s1", "title": title, "description": "d", "type": "PATHOGEN_SURVEILLANCE", "data_use_conditions": ["DUO:0000007"]}],
        "individuals": [{"alias": "i1", "consent_type": "RESEARCH"}],
        "samples": [{"alias": "sa1", "individual_alias": "i1", "collection_date": "2026-01-01", "collection_site": "site"}],
        "experiments": [{"alias": "e1", "sample_alias": "sa1", "pathogen_organism": "Plasmodium_falciparum"}],
        "files": [{"alias": "f1"}],
        "datasets": [{"alias": "dataset_path001", "title": title, "file_aliases": ["f1"]}]
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

    let bundle = sample_bundle("t");
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
    assert_eq!(
        put_res.headers().get("etag").and_then(|v| v.to_str().ok()),
        Some("\"1\"")
    );

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
    assert_eq!(body["version"], 1);
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
    assert_eq!(body["items"][0]["version"], 1);
}

#[tokio::test]
async fn metadata_store_versioning_and_if_match() {
    let pool = sqlite_pool().await;
    let state = enabled_state(pool);
    let app = metadata_api_router(Arc::new(state));
    let alias = "dataset_path001";

    let v1 = sample_bundle("one");
    let put1 = Request::builder()
        .method("PUT")
        .uri(format!("/submissions/{alias}?profile=pathogen"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&v1).unwrap()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(put1).await.unwrap().status(),
        StatusCode::OK
    );

    let v2 = sample_bundle("two");
    let put2 = Request::builder()
        .method("PUT")
        .uri(format!("/submissions/{alias}?profile=pathogen"))
        .header("content-type", "application/json")
        .header("if-match", "\"1\"")
        .body(Body::from(serde_json::to_vec(&v2).unwrap()))
        .unwrap();
    let put2_res = app.clone().oneshot(put2).await.unwrap();
    assert_eq!(put2_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(put2_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["version"], 2);
    assert_eq!(body["unchanged"], false);

    let conflict = Request::builder()
        .method("PUT")
        .uri(format!("/submissions/{alias}?profile=pathogen"))
        .header("content-type", "application/json")
        .header("if-match", "\"1\"")
        .body(Body::from(
            serde_json::to_vec(&sample_bundle("three")).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(conflict).await.unwrap().status(),
        StatusCode::CONFLICT
    );

    let versions = Request::builder()
        .method("GET")
        .uri(format!("/submissions/{alias}/versions"))
        .body(Body::empty())
        .unwrap();
    let versions_res = app.clone().oneshot(versions).await.unwrap();
    assert_eq!(versions_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(versions_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["count"].as_u64().unwrap() >= 2);

    let get_v1 = Request::builder()
        .method("GET")
        .uri(format!("/submissions/{alias}/versions/1"))
        .body(Body::empty())
        .unwrap();
    let get_v1_res = app.oneshot(get_v1).await.unwrap();
    assert_eq!(get_v1_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(get_v1_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["version"], 1);
    assert_eq!(body["document"]["datasets"][0]["title"], "one");
}

#[tokio::test]
async fn metadata_store_attach_detach_object() {
    let pool = sqlite_pool().await;
    let repo = DrsRepo::new(pool.clone(), "localhost".into());
    let bundle = sample_bundle("attach");
    let alias = submission_alias(&bundle).unwrap();
    repo.upsert_metadata_submission(&alias, "pathogen", &bundle.to_string(), None)
        .await
        .unwrap();
    let object_id = repo
        .create_object(&CreateObjectRequest {
            name: Some("seq".into()),
            description: None,
            mime_type: Some("application/x-fastq".into()),
            size: 8,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "drs/seq".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        })
        .await
        .unwrap();

    let state = enabled_state(pool);
    let app = metadata_api_router(Arc::new(state));

    let attach = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{object_id}/metadata_ref"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "metadata_ref": alias })).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(attach).await.unwrap().status(),
        StatusCode::OK
    );

    let detach = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{object_id}/metadata_ref"))
        .header("content-type", "application/json")
        .body(Body::from(br#"{"metadata_ref":null}"#.as_slice()))
        .unwrap();
    assert_eq!(app.oneshot(detach).await.unwrap().status(), StatusCode::OK);
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
    let bundle = sample_bundle("post");

    let post = Request::builder()
        .method("POST")
        .uri("/submissions?profile=pathogen")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&bundle).unwrap()))
        .unwrap();
    let res = app.oneshot(post).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}
