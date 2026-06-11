//! ONT ingest round-trip and Beacon pathogen filter integration tests.

use ferrum_beacon::router;
use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_ont::{OntFormat, OntIngestRequest, OntQualityMetrics};
use ferrum_storage::{LocalStorage, ObjectStorage};
use http::{Method, Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

async fn sqlite_pool() -> FerrumPool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate embed schema");
    FerrumPool::Sqlite(pool)
}

#[tokio::test]
async fn test_pod5_ingest_round_trip() {
    let pool = sqlite_pool().await;
    let storage =
        Arc::new(LocalStorage::new(tempfile::tempdir().unwrap().path()).expect("local storage"));
    let repo = Arc::new(DrsRepo::new(pool.clone(), "localhost".into()));
    let pod5 = ferrum_ont::synthetic_pod5_bytes("sample-A");
    let ont_req = OntIngestRequest {
        format: OntFormat::Pod5,
        source_path: "/dev/null".into(),
        run_id: "run-001".into(),
        sample_id: "sample-A".into(),
        organism: "Plasmodium_falciparum".into(),
        dorado_basecalled: false,
        quality_metrics: Some(OntQualityMetrics {
            mean_qscore: 11.0,
            read_count: 500,
            n50: 12000,
            read_length_histogram: vec![(1000, 10)],
        }),
    };
    ferrum_ont::validate_ingest_request(&ont_req).expect("valid");

    let object_id = ulid::Ulid::new().to_string();
    let storage_key = format!("drs/{object_id}");
    storage.put_bytes(&storage_key, &pod5).await.expect("put");

    let fields =
        ferrum_ont::build_create_request(&ont_req, pod5.len() as i64, "local", &storage_key);
    let create = CreateObjectRequest {
        name: fields.name,
        description: fields.description,
        mime_type: fields.mime_type,
        size: fields.size,
        checksums: vec![],
        aliases: None,
        storage_backend: fields.storage_backend,
        storage_key,
        is_encrypted: Some(false),
        workspace_id: None,
        ont_metrics: fields.ont_metrics.clone(),
    };
    repo.create_object_with_id(&create, Some(object_id.clone()))
        .await
        .expect("create");

    repo.insert_pathogen_annotation(
        &object_id,
        &ont_req.organism,
        &[],
        None,
        &[],
        Some(11.0),
        None,
    )
    .await
    .expect("pathogen annotation");

    let row: (Option<String>,) =
        sqlx::query_as("SELECT ont_metrics FROM drs_objects WHERE id = $1")
            .bind(&object_id)
            .fetch_one(repo.pool().as_sqlite().expect("sqlite"))
            .await
            .expect("fetch ont_metrics");

    let metrics = row.0.expect("ont_metrics set");
    assert!(metrics.contains("Plasmodium_falciparum"));
    assert!(metrics.contains("sample-A"));

    let obj = repo
        .get_object(&object_id, false)
        .await
        .expect("get_object")
        .expect("object");
    assert!(obj.ont_metrics.is_some());
}

#[tokio::test]
async fn test_ont_beacon_filter() {
    let pool = sqlite_pool().await;
    let repo = Arc::new(ferrum_beacon::repo::BeaconRepo::new(pool.clone()));

    repo.insert_pathogen_annotation("pa1", "Plasmodium_falciparum", &[], None, None, None, None)
        .await
        .expect("malaria");
    repo.insert_pathogen_annotation(
        "pa2",
        "Mycobacterium_tuberculosis",
        &[],
        None,
        None,
        None,
        None,
    )
    .await
    .expect("tb");

    let app = router(pool);
    let query = |organism: &str| {
        serde_json::json!({
            "meta": { "apiVersion": "v2.0.0" },
            "query": {
                "requestParameters": {
                    "organism": organism,
                    "requestedGranularity": "boolean"
                }
            }
        })
    };

    let malaria_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/g_variants/query")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&query("Plasmodium_falciparum")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malaria_resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(malaria_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["response"]["exists"], true);

    let tb_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/g_variants/query")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&query("Mycobacterium_tuberculosis")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(tb_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["response"]["exists"], true);

    let miss_resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/g_variants/query")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&query("Vibrio_cholerae")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(miss_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["response"]["exists"], false);
}
