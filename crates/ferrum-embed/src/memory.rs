// SPDX-License-Identifier: BUSL-1.1
//! Resident-set memory cap for laptop deployments (protect SQLite from OOM).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Result of evaluating resident set size against the configured cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCapLevel {
    Normal,
    Approaching,
    Exceeded,
}

/// Shared memory cap state checked by gateway middleware.
#[derive(Clone)]
pub struct MemoryCapState {
    limit_mb: u64,
    over_limit: Arc<AtomicBool>,
}

impl MemoryCapState {
    pub fn new(limit_mb: u64) -> Self {
        Self {
            limit_mb,
            over_limit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn limit_mb(&self) -> u64 {
        self.limit_mb
    }

    pub fn is_over_limit(&self) -> bool {
        self.over_limit.load(Ordering::Relaxed)
    }

    /// Evaluate RSS (MB) against the cap; log warnings at 80% and when exceeded.
    pub fn update_from_rss_mb(&self, rss_mb: u64) -> MemoryCapLevel {
        let warn_threshold = self.limit_mb.saturating_mul(80) / 100;
        if rss_mb >= self.limit_mb {
            self.over_limit.store(true, Ordering::Relaxed);
            tracing::warn!(
                rss_mb,
                limit_mb = self.limit_mb,
                "memory cap exceeded — refusing new connections"
            );
            MemoryCapLevel::Exceeded
        } else if rss_mb >= warn_threshold {
            tracing::warn!(
                rss_mb,
                limit_mb = self.limit_mb,
                "approaching memory cap (80% threshold)"
            );
            self.over_limit.store(false, Ordering::Relaxed);
            MemoryCapLevel::Approaching
        } else {
            self.over_limit.store(false, Ordering::Relaxed);
            MemoryCapLevel::Normal
        }
    }

    /// Read `/proc/self/status` VmRSS on Linux; log warning when approaching limit.
    pub fn check_and_update(&self) -> Option<u64> {
        let rss_kb = read_rss_kb()?;
        let rss_mb = rss_kb / 1024;
        self.update_from_rss_mb(rss_mb);
        Some(rss_mb)
    }
}

/// Axum middleware guard token (keeps monitor task alive).
pub struct MemoryCapGuard {
    _state: MemoryCapState,
}

impl MemoryCapGuard {
    pub fn spawn_monitor(state: MemoryCapState) -> Self {
        let monitor = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                monitor.check_and_update();
            }
        });
        Self { _state: state }
    }
}

#[cfg(target_os = "linux")]
fn read_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(kb) = line.strip_prefix("VmRSS:") {
            let kb = kb.trim().trim_end_matches(" kB").parse().ok()?;
            return Some(kb);
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_rss_kb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_from_rss_mb_thresholds() {
        let state = MemoryCapState::new(512);
        assert_eq!(state.update_from_rss_mb(400), MemoryCapLevel::Normal);
        assert!(!state.is_over_limit());

        assert_eq!(state.update_from_rss_mb(410), MemoryCapLevel::Approaching);
        assert!(!state.is_over_limit());

        assert_eq!(state.update_from_rss_mb(512), MemoryCapLevel::Exceeded);
        assert!(state.is_over_limit());
    }
}
