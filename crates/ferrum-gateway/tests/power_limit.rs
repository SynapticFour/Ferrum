//! Power middleware tests.

use axum::{routing::get, Router};
use ferrum_core::{FerrumPowerMode, PowerConfig, PowerLevel, PowerSource, StubPowerMonitor};
use ferrum_gateway::power::{power_limit_middleware, PowerState};
use http::{Method, Request, StatusCode};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower::ServiceExt;

fn power_state_with_mode(mode: FerrumPowerMode) -> Arc<tokio::sync::Mutex<PowerState>> {
    let monitor = Arc::new(StubPowerMonitor {
        source: PowerSource::Battery,
        level: PowerLevel::Critical,
        battery_percent: Some(5),
    });
    Arc::new(tokio::sync::Mutex::new(PowerState {
        mode,
        semaphore: Arc::new(Semaphore::new(ferrum_core::max_concurrent_requests(mode))),
        monitor,
        config: PowerConfig {
            enabled: true,
            low_power_threshold: 50,
            emergency_threshold: 10,
        },
    }))
}

#[tokio::test]
async fn test_power_middleware_emergency_rejects_requests() {
    let power = power_state_with_mode(FerrumPowerMode::Emergency);
    let app = Router::new()
        .route("/ok", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn({
            let power = Arc::clone(&power);
            move |req, next| {
                let power = Arc::clone(&power);
                async move { power_limit_middleware(power, req, next).await }
            }
        }));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/ok")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_power_middleware_uses_low_power_permit_budget() {
    let power = power_state_with_mode(FerrumPowerMode::LowPower);
    let sem = {
        let guard = power.lock().await;
        assert_eq!(guard.mode, FerrumPowerMode::LowPower);
        Arc::clone(&guard.semaphore)
    };
    let mut guards = Vec::new();
    for _ in 0..4 {
        guards.push(sem.clone().acquire_owned().await.expect("permit"));
    }
    assert!(sem.try_acquire().is_err());
}
