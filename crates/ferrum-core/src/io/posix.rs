//! Dedicated Rayon pool for blocking POSIX-style I/O.
//!
//! Lesson: `spawn_blocking` for POSIX file I/O — use a separate pool from Tokio's default.
//! Source: Tokio documentation + production HPC deployments (many concurrent CRAM/BAM reads).
//! Reason: Tokio's async `fs` still runs blocking syscalls on a shared pool; a dedicated pool
//! isolates heavy POSIX workloads from other `spawn_blocking` users.

use rayon::ThreadPoolBuilder;
use std::sync::OnceLock;
use thiserror::Error;
use tokio::sync::oneshot;

/// Error when the POSIX I/O pool cannot return a result (e.g. shutdown).
#[derive(Debug, Error)]
#[error("POSIX I/O worker pool channel closed")]
pub struct PosixPoolError;

static POSIX_IO_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn pool() -> &'static rayon::ThreadPool {
    POSIX_IO_POOL.get_or_init(|| {
        let n = std::env::var("FERRUM_POSIX_IO_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(32)
            .max(1);
        ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(|i| format!("ferrum-posix-io-{i}"))
            .build()
            .expect("POSIX I/O threadpool initialization failed")
    })
}

/// Run `f` on the dedicated POSIX I/O Rayon pool (not Tokio's `spawn_blocking` pool).
///
/// Prefer this for **synchronous** `std::fs` / libc-heavy work when many tasks do I/O at once.
pub async fn spawn_blocking<F, R>(f: F) -> Result<R, PosixPoolError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    pool().spawn(move || {
        let _ = tx.send(f());
    });
    rx.await.map_err(|_| PosixPoolError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_blocking_runs_on_pool() {
        let v = spawn_blocking(|| 42u8).await.expect("pool");
        assert_eq!(v, 42);
    }
}
