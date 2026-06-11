//! Reference registry seeded entries and load API.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use ferrum_reference::{reference_api_v1_router, LoadReferenceRequest, ReferenceRegistry};
use ferrum_core::FerrumPool;
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn test_registry_seeded_entries() {
    let pool = memory_pool().await;
    let registry = ReferenceRegistry::new(FerrumPool::Sqlite(pool));
    let all = registry.list().await.unwrap();
    assert_eq!(all.len(), 6);
    let ids: Vec<_> = all.iter().map(|r| r.id.as_str()).collect();
    for expected in [
        "GRCh38",
        "T2T-CHM13",
        "H3Africa_v1",
        "AWI-GEN_panel",
        "Pf3D7_v3",
        "MTB_H37Rv",
    ] {
        assert!(ids.contains(&expected), "missing {expected}");
    }
    let default = registry.default_reference().await.unwrap().unwrap();
    assert_eq!(default.id, "GRCh38");
    assert!(default.is_default);
}

#[tokio::test]
async fn test_load_reference_via_ingest() {
    let pool = memory_pool().await;
    sqlx::query(
        "INSERT INTO drs_objects (id, size, created_time, updated_time) VALUES ('fasta-stub', 10, datetime('now'), datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let registry = Arc::new(ReferenceRegistry::new(FerrumPool::Sqlite(pool.clone())));
    let app = reference_api_v1_router(registry);

    let body = serde_json::to_string(&LoadReferenceRequest {
        fasta_drs_id: "fasta-stub".into(),
        index_drs_id: None,
    })
    .unwrap();
    let req = Request::builder()
        .method(Method::PUT)
        .uri("/Pf3D7_v3/load")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["fasta_drs_id"], "fasta-stub");
    assert_eq!(v["id"], "Pf3D7_v3");
}
