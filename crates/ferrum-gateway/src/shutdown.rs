//! Graceful shutdown coordinator for long-running streaming transfers.
//!
//! Lesson 9: in production (Kubernetes pod eviction, SLURM preemption) Ferrum must:
//! - reject new transfers with `503 Service Unavailable`
//! - drain existing transfers for a bounded time window
//! - avoid abruptly terminating 500GB streams
//!
//! Note: we intentionally track "active transfers" per request body lifetime in the gateway
//! middleware (see `src/lib.rs`).

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ShutdownCoordinator {
    token: CancellationToken,
    active_transfers: Arc<AtomicU64>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            active_transfers: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn register_transfer(&self) -> TransferGuard {
        self.active_transfers.fetch_add(1, Ordering::SeqCst);
        Arc::new(TransferGuardInner {
            active_transfers: Arc::clone(&self.active_transfers),
        })
    }

    /// Trigger shutdown and wait up to `drain_timeout` for in-flight transfers.
    pub async fn shutdown(&self, drain_timeout: Duration) {
        self.token.cancel();
        let start = Instant::now();
        while self.active_transfers.load(Ordering::SeqCst) > 0 {
            if start.elapsed() >= drain_timeout {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }
}

pub type TransferGuard = Arc<TransferGuardInner>;

pub struct TransferGuardInner {
    active_transfers: Arc<AtomicU64>,
}

impl Drop for TransferGuardInner {
    fn drop(&mut self) {
        self.active_transfers.fetch_sub(1, Ordering::SeqCst);
    }
}
