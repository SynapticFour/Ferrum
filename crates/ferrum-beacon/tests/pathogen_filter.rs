// SPDX-License-Identifier: BUSL-1.1
//! Pathogen Beacon filters and human-genomics regression (SQLite fixtures).

use ferrum_beacon::router;
use ferrum_core::FerrumPool;
use http::{Method, Request, StatusCode};
use tower::ServiceExt;

async fn seeded_pool() -> FerrumPool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");

    sqlx::query(
        "INSERT INTO beacon_datasets (id, name, description, assembly_id) VALUES ('ds-hg38', 'Human', 'test', 'GRCh38')",
    )
    .execute(&pool)
    .await
    .expect("dataset");

    sqlx::query(
        "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
         VALUES ('ds-hg38', '1', 100000, 100000, 'A', 'G', 'SNV')",
    )
    .execute(&pool)
    .await
    .expect("variant");

    let repo = ferrum_beacon::repo::BeaconRepo::new(FerrumPool::Sqlite(pool.clone()));
    repo.insert_pathogen_annotation(
        "p-amr",
        "Klebsiella_pneumoniae",
        &["blaNDM-1".to_string()],
        None,
        Some(10.0),
        None,
        None,
    )
    .await
    .expect("pathogen");
    repo.insert_pathogen_annotation(
        "p-tb",
        "Mycobacterium_tuberculosis",
        &[],
        None,
        None,
        None,
        None,
    )
    .await
    .expect("tb");

    FerrumPool::Sqlite(pool)
}

fn beacon_query(body: serde_json::Value) -> Request<axum::body::Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/g_variants/query")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn test_pathogen_filter_amr() {
    let app = router(seeded_pool().await);
    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "organism": "Klebsiella_pneumoniae",
                "amrGene": "blaNDM-1",
                "requestedGranularity": "boolean"
            }
        }
    });
    let resp = app.oneshot(beacon_query(body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], true);
}

#[tokio::test]
async fn test_pathogen_filter_type_in_filters_array() {
    let app = router(seeded_pool().await);
    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "filters": [{
                "id": "PathoGenFilter",
                "organism": "Mycobacterium_tuberculosis"
            }],
            "requestParameters": { "requestedGranularity": "boolean" }
        }
    });
    let resp = app.oneshot(beacon_query(body)).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], true);
}

#[tokio::test]
async fn test_human_genomics_unaffected_with_pathogen_data_present() {
    let app = router(seeded_pool().await);

    let human = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "assemblyId": "GRCh38",
                "referenceName": "1",
                "start": 100000,
                "requestedGranularity": "boolean"
            }
        }
    });
    let human_resp = app.clone().oneshot(beacon_query(human)).await.unwrap();
    assert_eq!(human_resp.status(), StatusCode::OK);
    let hv: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(human_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(hv["response"]["exists"], true);

    let pathogen = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "organism": "Mycobacterium_tuberculosis",
                "requestedGranularity": "boolean"
            }
        }
    });
    let p_resp = app.oneshot(beacon_query(pathogen)).await.unwrap();
    let pv: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(p_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(pv["response"]["exists"], true);
}

#[tokio::test]
async fn test_human_query_without_pathogen_fields_unchanged_negative() {
    let app = router(seeded_pool().await);
    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "assemblyId": "GRCh38",
                "referenceName": "1",
                "start": 999999,
                "requestedGranularity": "boolean"
            }
        }
    });
    let resp = app.oneshot(beacon_query(body)).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], false);
}
