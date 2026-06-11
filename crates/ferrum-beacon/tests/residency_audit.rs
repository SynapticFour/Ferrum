//! Beacon POST query residency audit wiring.

use axum::http::{Method, Request, StatusCode};
use ferrum_beacon::router_with_services;
use ferrum_core::{FerrumPool, ResidencyAuditLog};
use http::header;
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn test_post_query_logs_beacon_query_residency() {
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
    let app = router_with_services(fp, None, None, Some(audit.clone()), None);

    let body = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": {
            "requestParameters": {
                "referenceName": "1",
                "start": 999999,
                "referenceBases": "A",
                "alternateBases": "T"
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

    let entries = audit.query_range(None, None).await.unwrap().entries;
    assert!(entries.iter().any(|e| e.event_type == "beacon_query"));
}
