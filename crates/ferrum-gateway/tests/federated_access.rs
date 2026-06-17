//! Federated ADS introspect gate + DRS/WES proxy smoke tests.

use axum::{
    routing::{get, post},
    Json, Router,
};
use ferrum_core::FerrumConfig;
use ferrum_gateway::access::access_router;
use http::{Method, Request, StatusCode};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

static INTROSPECT_CALLS: AtomicUsize = AtomicUsize::new(0);
static DRS_CALLS: AtomicUsize = AtomicUsize::new(0);

async fn spawn_mock_ads(active: bool) -> String {
    INTROSPECT_CALLS.store(0, Ordering::SeqCst);
    let app = Router::new()
        .route(
            "/ads/v1/introspect",
            post(move || async move {
                INTROSPECT_CALLS.fetch_add(1, Ordering::SeqCst);
                Json(json!({ "active": active }))
            }),
        )
        .route(
            "/ads/v1/me/grants",
            get(|| async { Json(json!({ "grants": [] })) }),
        )
        .route(
            "/ads/v1/catalog/datasets",
            get(|| async { Json(json!({ "datasets": [] })) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    base
}

async fn spawn_mock_drs() -> String {
    DRS_CALLS.store(0, Ordering::SeqCst);
    let app = Router::new().route(
        "/ga4gh/drs/v1/objects/:id",
        get(|| async {
            DRS_CALLS.fetch_add(1, Ordering::SeqCst);
            Json(json!({ "id": "peer-obj", "name": "peer" }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    base
}

fn test_config(ads_url: &str) -> FerrumConfig {
    std::env::set_var("ADS_DAC_API_KEY", "test-key");
    serde_json::from_str(&format!(
        r#"{{
            "database": {{ "url": "sqlite::memory:" }},
            "auth": {{ "ads_url": "{ads_url}/ads/v1" }},
            "discovery": {{ "enabled": false }}
        }}"#
    ))
    .expect("test config")
}

#[tokio::test]
async fn federated_drs_proxy_blocks_when_introspect_inactive() {
    let ads = spawn_mock_ads(false).await;
    let drs = spawn_mock_drs().await;
    let app = access_router(&test_config(&ads));
    let uri = format!(
        "/federated/drs/objects/peer-obj?base_url={}/ga4gh/drs/v1&ads_base_url={}/ads/v1&dataset_id=550e8400-e29b-41d4-a716-446655440000",
        drs, ads
    );
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Authorization", "Bearer test-passport")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(INTROSPECT_CALLS.load(Ordering::SeqCst) >= 1);
    assert_eq!(DRS_CALLS.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn federated_drs_proxy_forwards_when_introspect_active() {
    let ads = spawn_mock_ads(true).await;
    let drs = spawn_mock_drs().await;
    let app = access_router(&test_config(&ads));
    let uri = format!(
        "/federated/drs/objects/peer-obj?base_url={}/ga4gh/drs/v1&ads_base_url={}/ads/v1&dataset_id=550e8400-e29b-41d4-a716-446655440000",
        drs, ads
    );
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Authorization", "Bearer test-passport")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(INTROSPECT_CALLS.load(Ordering::SeqCst) >= 1);
    assert_eq!(DRS_CALLS.load(Ordering::SeqCst), 1);
}
