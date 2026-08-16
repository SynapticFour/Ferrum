// SPDX-License-Identifier: BUSL-1.1
//! Beacon pathogen query reference genome metadata.

use axum::http::{Method, Request, StatusCode};
use ferrum_beacon::router_with_services;
use ferrum_core::FerrumPool;
use ferrum_reference::ReferenceRegistry;
use http::header;
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn test_pathogen_reference_beacon_integration() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO beacon_datasets (id, name, description, assembly_id) VALUES ('default', 'Default', 'test', 'GRCh38')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO drs_objects (id, size, created_time, updated_time) VALUES ('pf-seq', 100, datetime('now'), datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pathogen_annotations (id, dataset_id, drs_object_id, organism)
         VALUES ('pa1', 'default', 'pf-seq', 'Plasmodium_falciparum')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let fp = FerrumPool::Sqlite(pool);
    let registry = Arc::new(ReferenceRegistry::new(fp.clone()));
    let app = router_with_services(fp, None, None, None, Some(registry));

    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "organism": "Plasmodium_falciparum",
                "requestedGranularity": "boolean"
            }
        }
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/g_variants/query")
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["meta"]["referenceGenome"], "Pf3D7_v3");
    assert_eq!(v["response"]["exists"], true);
}
