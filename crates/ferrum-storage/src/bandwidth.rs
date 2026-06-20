//! Rolling bandwidth estimation from recent DRS transfers.

use ferrum_core::BandwidthConfig;
use std::collections::VecDeque;
use std::sync::Mutex;

const MAX_SAMPLES: usize = 10;

/// Ignore tiny DRS stream samples so localhost previews do not classify the link as VeryLow.
pub const MIN_BANDWIDTH_SAMPLE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthClass {
    High,
    Medium,
    Low,
    VeryLow,
}

impl BandwidthClass {
    pub fn chunk_size_bytes(self) -> u64 {
        match self {
            BandwidthClass::High => 64 * 1024 * 1024,
            BandwidthClass::Medium => 16 * 1024 * 1024,
            BandwidthClass::Low => 4 * 1024 * 1024,
            BandwidthClass::VeryLow => 512 * 1024,
        }
    }

    pub fn use_zstd_compression(self) -> bool {
        matches!(self, BandwidthClass::Low | BandwidthClass::VeryLow)
    }
}

#[derive(Debug, Clone)]
struct Sample {
    bytes: u64,
    duration_ms: u64,
}

pub struct BandwidthMonitor {
    config: BandwidthConfig,
    samples: Mutex<VecDeque<Sample>>,
    ema_bps: Mutex<f64>,
}

impl BandwidthMonitor {
    pub fn new(config: BandwidthConfig) -> Self {
        Self {
            config,
            samples: Mutex::new(VecDeque::with_capacity(MAX_SAMPLES)),
            ema_bps: Mutex::new(0.0),
        }
    }

    /// Record a completed transfer for rolling average / EMA.
    pub fn record_transfer(&self, bytes: u64, duration_ms: u64) {
        if bytes == 0 || duration_ms == 0 {
            return;
        }
        let bps = (bytes as f64 * 8000.0) / duration_ms as f64;
        {
            let mut ema = self.ema_bps.lock().expect("ema lock");
            if *ema <= 0.0 {
                *ema = bps;
            } else {
                *ema = 0.3 * bps + 0.7 * (*ema);
            }
        }
        let mut samples = self.samples.lock().expect("samples lock");
        samples.push_back(Sample { bytes, duration_ms });
        while samples.len() > MAX_SAMPLES {
            samples.pop_front();
        }
    }

    pub fn current_bandwidth_bps(&self) -> u64 {
        let ema = *self.ema_bps.lock().expect("ema lock");
        if ema > 0.0 {
            return ema as u64;
        }
        let samples = self.samples.lock().expect("samples lock");
        if samples.is_empty() {
            return self.config.high_bps;
        }
        let total_bytes: u64 = samples.iter().map(|s| s.bytes).sum();
        let total_ms: u64 = samples.iter().map(|s| s.duration_ms).sum();
        if total_ms == 0 {
            return self.config.high_bps;
        }
        ((total_bytes as f64 * 8000.0) / total_ms as f64) as u64
    }

    pub fn classify(&self) -> BandwidthClass {
        let bps = self.current_bandwidth_bps();
        if bps >= self.config.high_bps {
            BandwidthClass::High
        } else if bps >= self.config.medium_bps {
            BandwidthClass::Medium
        } else if bps >= self.config.low_bps {
            BandwidthClass::Low
        } else {
            BandwidthClass::VeryLow
        }
    }

    /// Inject mock speeds for tests (replaces samples with synthetic history).
    pub fn inject_mock_bps(&self, bps: u64) {
        *self.ema_bps.lock().expect("ema lock") = bps as f64;
        let mut samples = self.samples.lock().expect("samples lock");
        samples.clear();
        samples.push_back(Sample {
            bytes: bps / 8,
            duration_ms: 1000,
        });
    }
}
