//! Per-peer request budget (default 10 req/min).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct PeerRateLimiter {
    budget_per_minute: u32,
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl PeerRateLimiter {
    pub fn new(budget_per_minute: u32) -> Self {
        Self {
            budget_per_minute: budget_per_minute.max(1),
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if the request is allowed under the peer budget.
    pub fn allow(&self, peer_name: &str) -> bool {
        let mut map = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let entry = map.entry(peer_name.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        if entry.1 >= self.budget_per_minute {
            return false;
        }
        entry.1 += 1;
        true
    }
}
