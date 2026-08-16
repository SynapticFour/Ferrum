// SPDX-License-Identifier: BUSL-1.1
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
use std::sync::Arc;
use tower::ServiceExt;

async fn spawn_mock_ads(active: bool, introspect_calls: Arc<AtomicUsize>) -> String {
    introspect_calls.store(0, Ordering::SeqCst);
    let calls = introspect_calls.clone();
    let app = Router::new()
        .route(
            "/ads/v1/introspect",
            post(move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Json(json!({ "active": active }))
                }
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

async fn spawn_mock_drs(drs_calls: Arc<AtomicUsize>) -> String {
    drs_calls.store(0, Ordering::SeqCst);
    let calls = drs_calls.clone();
    let app = Router::new().route(
        "/ga4gh/drs/v1/objects/:id",
        get(move || {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({ "id": "peer-obj", "name": "peer" }))
            }
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
    let introspect_calls = Arc::new(AtomicUsize::new(0));
    let drs_calls = Arc::new(AtomicUsize::new(0));
    let ads = spawn_mock_ads(false, introspect_calls.clone()).await;
    let drs = spawn_mock_drs(drs_calls.clone()).await;
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
    assert!(introspect_calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(drs_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn federated_drs_proxy_forwards_when_introspect_active() {
    let introspect_calls = Arc::new(AtomicUsize::new(0));
    let drs_calls = Arc::new(AtomicUsize::new(0));
    let ads = spawn_mock_ads(true, introspect_calls.clone()).await;
    let drs = spawn_mock_drs(drs_calls.clone()).await;
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
    assert!(introspect_calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(drs_calls.load(Ordering::SeqCst), 1);
}
