// SPDX-License-Identifier: BUSL-1.1
//! Federated Beacon integration tests.

use axum::{routing::post, Json, Router};
use ferrum_beacon::router_with_services;
use ferrum_core::config::{AggregateStrategy, FederationConfig, FerrumPeerConfig};
use ferrum_core::FerrumPool;
use ferrum_federation::FederationClient;
use ferrum_federation::FederationRuntime;
use http::{Method, Request, StatusCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

static PEER_CALLS: AtomicUsize = AtomicUsize::new(0);

async fn seed_variant(
    pool: &sqlx::SqlitePool,
    chr: &str,
    start: i64,
    reference: &str,
    alternate: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO beacon_datasets (id, name, description, assembly_id)
         VALUES ('default', 'default', 'default', 'GRCh38')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
         VALUES ('default', $1, $2, $2, $3, $4, 'SNV')",
    )
    .bind(chr)
    .bind(start)
    .bind(reference)
    .bind(alternate)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_federation_disabled_by_default() {
    PEER_CALLS.store(0, Ordering::SeqCst);
    let peer_app = Router::new().route(
        "/ga4gh/beacon/v2/g_variants/query",
        post(|| async {
            PEER_CALLS.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({"response": {"exists": true}}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener, peer_app).await.unwrap();
    });

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let app = router_with_services(FerrumPool::Sqlite(pool), None, None, None, None);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/g_variants?referenceName=1&start=1000&referenceBases=A&alternateBases=T")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(PEER_CALLS.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_peer_timeout_non_fatal() {
    let federation = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![FerrumPeerConfig {
                name: "dead-peer".into(),
                beacon_endpoint: "http://127.0.0.1:1/ga4gh/beacon/v2".into(),
                public_key: None,
                timeout_ms: 200,
                service_token: None,
            }],
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::Union,
            peer_requests_per_minute: 100,
        })
        .unwrap(),
    );
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    seed_variant(&pool, "1", 1000, "A", "T").await;
    let app = router_with_services(
        FerrumPool::Sqlite(pool),
        None,
        Some(Arc::new(federation)),
        None,
        None,
    );
    let req = Request::builder()
        .method(Method::GET)
        .uri("/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(v["meta"]["warnings"].is_array());
}

#[tokio::test]
async fn test_federated_beacon_union() {
    let peer_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = peer_listener.local_addr().unwrap();
    let peer_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&peer_pool)
        .await
        .unwrap();
    seed_variant(&peer_pool, "1", 3000, "G", "C").await;
    let peer_app = router_with_services(FerrumPool::Sqlite(peer_pool), None, None, None, None);
    tokio::spawn(async move {
        axum::serve(peer_listener, peer_app).await.unwrap();
    });

    let local_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&local_pool)
        .await
        .unwrap();
    seed_variant(&local_pool, "1", 1000, "A", "T").await;

    let peer_url = format!("http://{}/ga4gh/beacon/v2", peer_addr);
    let federation = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![FerrumPeerConfig {
                name: "remote".into(),
                beacon_endpoint: peer_url,
                public_key: None,
                timeout_ms: 3000,
                service_token: None,
            }],
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::Union,
            peer_requests_per_minute: 100,
        })
        .unwrap(),
    );
    let app = router_with_services(
        FerrumPool::Sqlite(local_pool),
        None,
        Some(Arc::new(federation)),
        None,
        None,
    );

    let req = Request::builder()
        .method(Method::GET)
        .uri("/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], true);
}

#[test]
fn test_federation_intersection_strategy() {
    use ferrum_core::config::{AggregateStrategy, FederationConfig};
    use ferrum_federation::{FederationClient, FederationRuntime, PeerQueryResult};

    let client = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![],
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::Intersection,
            peer_requests_per_minute: 100,
        })
        .unwrap(),
    );
    let peers = vec![
        PeerQueryResult {
            peer_name: "p1".into(),
            exists: Some(true),
            count: Some(10),
            error: None,
        },
        PeerQueryResult {
            peer_name: "p2".into(),
            exists: Some(false),
            count: Some(5),
            error: None,
        },
    ];
    let (exists, count, _) = client.aggregate(Some(true), Some(8), &peers);
    assert_eq!(exists, Some(false));
    assert_eq!(count, Some(5));
}

#[test]
fn test_federation_local_first_strategy() {
    use ferrum_core::config::{AggregateStrategy, FederationConfig};
    use ferrum_federation::{FederationClient, FederationRuntime, PeerQueryResult};

    let client = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![],
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::LocalFirst,
            peer_requests_per_minute: 100,
        })
        .unwrap(),
    );
    let peers = vec![PeerQueryResult {
        peer_name: "p1".into(),
        exists: Some(true),
        count: Some(100),
        error: None,
    }];
    let (exists, count, _) = client.aggregate(Some(true), Some(3), &peers);
    assert_eq!(exists, Some(true));
    assert_eq!(count, Some(3));
}

#[tokio::test]
async fn test_federation_peer_rate_limiting() {
    use ferrum_core::config::{AggregateStrategy, FederationConfig, FerrumPeerConfig};
    use ferrum_federation::FederationClient;
    use ferrum_federation::FederationRuntime;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_route = Arc::clone(&calls);
    let peer_app = Router::new().route(
        "/ga4gh/beacon/g_variants/query",
        post(move || {
            let calls = Arc::clone(&calls_for_route);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(serde_json::json!({"response": {"exists": true}}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, peer_app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let federation = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![FerrumPeerConfig {
                name: "limited-peer".into(),
                beacon_endpoint: format!("http://{}/ga4gh/beacon/v2", addr),
                public_key: None,
                timeout_ms: 3000,
                service_token: None,
            }],
            fan_out_parallel: false,
            aggregate_strategy: AggregateStrategy::Union,
            peer_requests_per_minute: 1,
        })
        .unwrap(),
    );

    let envelope = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": { "requestParameters": { "referenceName": "1", "start": 1 } }
    });
    let first = federation.query_peers(&envelope).await;
    assert_eq!(first.len(), 1);
    assert!(
        first[0].error.is_none(),
        "first peer query failed: {:?}",
        first[0].error
    );

    let second = federation.query_peers(&envelope).await;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].error.as_deref(), Some("rate limit exceeded"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_federated_beacon_intersection_e2e() {
    let peer_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = peer_listener.local_addr().unwrap();
    let peer_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&peer_pool)
        .await
        .unwrap();
    seed_variant(&peer_pool, "1", 1000, "A", "T").await;
    let peer_app = Router::new().route(
        "/ga4gh/beacon/g_variants/query",
        post(|| async { Json(serde_json::json!({"response": {"exists": true}})) }),
    );
    tokio::spawn(async move {
        axum::serve(peer_listener, peer_app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let local_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&local_pool)
        .await
        .unwrap();
    seed_variant(&local_pool, "1", 3000, "G", "C").await;

    let peer_url = format!("http://{}/ga4gh/beacon/v2", peer_addr);
    let federation = FederationClient::new(
        FederationRuntime::from_config(&FederationConfig {
            enabled: true,
            peers: vec![FerrumPeerConfig {
                name: "remote".into(),
                beacon_endpoint: peer_url,
                public_key: None,
                timeout_ms: 3000,
                service_token: None,
            }],
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::Intersection,
            peer_requests_per_minute: 100,
        })
        .unwrap(),
    );
    let app = router_with_services(
        FerrumPool::Sqlite(local_pool),
        None,
        Some(Arc::new(federation)),
        None,
        None,
    );

    let req = Request::builder()
        .method(Method::GET)
        .uri("/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["response"]["exists"], false);
}
