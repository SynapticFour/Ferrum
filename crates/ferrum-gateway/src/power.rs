//! Solar/battery power mode middleware and emergency shutdown watcher.

use ferrum_core::{
    default_power_monitor, last_transaction_id, max_concurrent_requests, resolve_power_mode,
    write_emergency_checkpoint, FerrumPool, FerrumPowerMode, PowerConfig, PowerMonitor,
};
use axum::response::IntoResponse;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

pub struct PowerState {
    pub mode: FerrumPowerMode,
    pub semaphore: Arc<Semaphore>,
    pub monitor: Arc<dyn PowerMonitor>,
    pub config: PowerConfig,
}

impl PowerState {
    pub fn new(config: PowerConfig) -> Self {
        let monitor = default_power_monitor();
        let mode = resolve_power_mode(&config, monitor.as_ref());
        let permits = max_concurrent_requests(mode);
        Self {
            mode,
            semaphore: Arc::new(Semaphore::new(permits)),
            monitor,
            config,
        }
    }

    pub fn refresh(&mut self) {
        self.mode = resolve_power_mode(&self.config, self.monitor.as_ref());
        let permits = max_concurrent_requests(self.mode);
        self.semaphore = Arc::new(Semaphore::new(permits));
    }

    pub fn current_mode(&self) -> FerrumPowerMode {
        self.mode
    }
}

pub fn spawn_power_watcher(
    state: Arc<tokio::sync::Mutex<PowerState>>,
    pool: Option<FerrumPool>,
    background_gate: Option<Arc<ferrum_core::BackgroundWorkGate>>,
) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(15)).await;
            {
                let mut guard = state.lock().await;
                guard.refresh();
                if let Some(ref gate) = background_gate {
                    gate.set_mode(guard.current_mode());
                }
                if guard.mode == FerrumPowerMode::Emergency {
                    if let Some(ref p) = pool {
                        let tx = last_transaction_id(p).await.ok().flatten();
                        let _ = write_emergency_checkpoint(tx).await;
                    } else {
                        let _ = write_emergency_checkpoint(None).await;
                    }
                    sleep(Duration::from_secs(30)).await;
                    std::process::exit(0);
                }
            }
        }
    });
}

static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

pub async fn power_limit_middleware(
    state: Arc<tokio::sync::Mutex<PowerState>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mode = {
        let guard = state.lock().await;
        guard.mode
    };
    if mode == FerrumPowerMode::Emergency {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Ferrum emergency power mode: refusing new connections",
        )
            .into_response();
    }
    let sem = {
        let guard = state.lock().await;
        Arc::clone(&guard.semaphore)
    };
    let _permit = match sem.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "power-limited concurrency exceeded",
            )
                .into_response();
        }
    };
    IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
    let resp = next.run(req).await;
    IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    resp
}
