// SPDX-License-Identifier: BUSL-1.1
//! Priority transfer queue for bandwidth-constrained deployments.

use crate::bandwidth::{BandwidthClass, BandwidthMonitor};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct QueuedTransfer {
    pub object_id: String,
    pub size_bytes: u64,
    pub direction: TransferDirection,
    pub enqueued_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Download,
    Upload,
}

pub struct TransferQueue {
    queue: Mutex<VecDeque<QueuedTransfer>>,
    schedule_secs: u64,
    last_drain: Mutex<Instant>,
}

impl TransferQueue {
    pub fn new(schedule_secs: u64) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            schedule_secs,
            last_drain: Mutex::new(Instant::now()),
        }
    }

    /// Small payloads bypass the queue; large transfers may be deferred on VeryLow bandwidth.
    pub fn should_queue(&self, size_bytes: u64, bandwidth: &BandwidthMonitor) -> bool {
        if size_bytes <= 1024 {
            return false;
        }
        size_bytes > 10 * 1024 * 1024 && bandwidth.classify() == BandwidthClass::VeryLow
    }

    pub fn enqueue(&self, object_id: String, size_bytes: u64, direction: TransferDirection) {
        self.queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(QueuedTransfer {
                object_id,
                size_bytes,
                direction,
                enqueued_at: Instant::now(),
            });
    }

    pub fn len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn drain_if_ready(&self, bandwidth: &BandwidthMonitor) -> Vec<QueuedTransfer> {
        let mut last = self.last_drain.lock().unwrap_or_else(|e| e.into_inner());
        let class = bandwidth.classify();
        let due = last.elapsed() >= Duration::from_secs(self.schedule_secs);
        if !due && class == BandwidthClass::VeryLow {
            return Vec::new();
        }
        *last = Instant::now();
        let mut q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.drain(..).collect()
    }
}
