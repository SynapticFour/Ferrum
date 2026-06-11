//! Power source detection and Ferrum operating modes for solar/battery deployments.

use crate::config::PowerConfig;
use async_trait::async_trait;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    Ac,
    Battery,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerLevel {
    Full,
    Low,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FerrumPowerMode {
    HighPerformance,
    LowPower,
    Emergency,
}

#[async_trait]
pub trait PowerMonitor: Send + Sync {
    fn source(&self) -> PowerSource;
    fn level(&self) -> PowerLevel;
    fn battery_percent(&self) -> Option<u8>;
}

pub struct LinuxPowerMonitor;

impl LinuxPowerMonitor {
    pub fn new() -> Self {
        Self
    }

    fn read_sysfs_battery() -> Option<(PowerSource, u8)> {
        let base = Path::new("/sys/class/power_supply");
        if !base.exists() {
            return None;
        }
        let entries = std::fs::read_dir(base).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let type_path = path.join("type");
            let type_str = std::fs::read_to_string(&type_path).ok()?;
            if !type_str.trim().eq_ignore_ascii_case("Battery") {
                continue;
            }
            let status = std::fs::read_to_string(path.join("status"))
                .unwrap_or_default()
                .to_lowercase();
            let capacity = std::fs::read_to_string(path.join("capacity"))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(100);
            let source = if status.contains("discharg") {
                PowerSource::Battery
            } else if status.contains("charg") || status.contains("full") {
                PowerSource::Ac
            } else {
                PowerSource::Unknown
            };
            return Some((source, capacity));
        }
        None
    }
}

impl Default for LinuxPowerMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PowerMonitor for LinuxPowerMonitor {
    fn source(&self) -> PowerSource {
        Self::read_sysfs_battery()
            .map(|(s, _)| s)
            .unwrap_or(PowerSource::Unknown)
    }

    fn level(&self) -> PowerLevel {
        Self::read_sysfs_battery()
            .map(|(_, pct)| classify_level(pct))
            .unwrap_or(PowerLevel::Full)
    }

    fn battery_percent(&self) -> Option<u8> {
        Self::read_sysfs_battery().map(|(_, pct)| pct)
    }
}

pub struct AcpiPowerMonitor;

#[async_trait]
impl PowerMonitor for AcpiPowerMonitor {
    fn source(&self) -> PowerSource {
        LinuxPowerMonitor::read_sysfs_battery()
            .map(|(s, _)| s)
            .unwrap_or(PowerSource::Unknown)
    }

    fn level(&self) -> PowerLevel {
        LinuxPowerMonitor::read_sysfs_battery()
            .map(|(_, pct)| classify_level(pct))
            .unwrap_or(PowerLevel::Full)
    }

    fn battery_percent(&self) -> Option<u8> {
        LinuxPowerMonitor::read_sysfs_battery().map(|(_, pct)| pct)
    }
}

pub struct MacOsPowerMonitor;

impl MacOsPowerMonitor {
    fn parse_pmset() -> Option<(PowerSource, u8)> {
        let output = Command::new("pmset").arg("-g").arg("batt").output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let lower = text.to_lowercase();
        let source = if lower.contains("battery power") {
            PowerSource::Battery
        } else if lower.contains("ac power") {
            PowerSource::Ac
        } else {
            PowerSource::Unknown
        };
        let pct = text
            .split('%')
            .next()
            .and_then(|s| s.split_whitespace().last())
            .and_then(|n| n.trim_end_matches(';').parse::<u8>().ok())
            .unwrap_or(100);
        Some((source, pct))
    }
}

#[async_trait]
impl PowerMonitor for MacOsPowerMonitor {
    fn source(&self) -> PowerSource {
        Self::parse_pmset()
            .map(|(s, _)| s)
            .unwrap_or(PowerSource::Unknown)
    }

    fn level(&self) -> PowerLevel {
        Self::parse_pmset()
            .map(|(_, pct)| classify_level(pct))
            .unwrap_or(PowerLevel::Full)
    }

    fn battery_percent(&self) -> Option<u8> {
        Self::parse_pmset().map(|(_, pct)| pct)
    }
}

pub struct StubPowerMonitor {
    pub source: PowerSource,
    pub level: PowerLevel,
    pub battery_percent: Option<u8>,
}

#[async_trait]
impl PowerMonitor for StubPowerMonitor {
    fn source(&self) -> PowerSource {
        self.source
    }

    fn level(&self) -> PowerLevel {
        self.level
    }

    fn battery_percent(&self) -> Option<u8> {
        self.battery_percent
    }
}

fn classify_level(pct: u8) -> PowerLevel {
    if pct < 10 {
        PowerLevel::Critical
    } else if pct < 50 {
        PowerLevel::Low
    } else {
        PowerLevel::Full
    }
}

pub fn default_power_monitor() -> Arc<dyn PowerMonitor> {
    if cfg!(target_os = "macos") {
        Arc::new(MacOsPowerMonitor)
    } else if cfg!(target_os = "linux") {
        Arc::new(LinuxPowerMonitor::new())
    } else {
        Arc::new(StubPowerMonitor {
            source: PowerSource::Unknown,
            level: PowerLevel::Full,
            battery_percent: None,
        })
    }
}

pub fn resolve_power_mode(config: &PowerConfig, monitor: &dyn PowerMonitor) -> FerrumPowerMode {
    if let Ok(mode) = std::env::var("FERRUM_POWER_MODE") {
        return match mode.trim().to_lowercase().as_str() {
            "low_power" | "low-power" => FerrumPowerMode::LowPower,
            "emergency" => FerrumPowerMode::Emergency,
            _ => FerrumPowerMode::HighPerformance,
        };
    }
    if !config.enabled {
        return FerrumPowerMode::HighPerformance;
    }
    let pct = monitor.battery_percent().unwrap_or(100);
    if monitor.source() == PowerSource::Ac {
        return FerrumPowerMode::HighPerformance;
    }
    if pct <= config.emergency_threshold {
        FerrumPowerMode::Emergency
    } else if pct <= config.low_power_threshold {
        FerrumPowerMode::LowPower
    } else {
        FerrumPowerMode::HighPerformance
    }
}

pub fn max_concurrent_requests(mode: FerrumPowerMode) -> usize {
    match mode {
        FerrumPowerMode::HighPerformance => 256,
        FerrumPowerMode::LowPower => 4,
        FerrumPowerMode::Emergency => 0,
    }
}

/// Whether background indexing, checksum jobs, and transfer-queue drains may run.
pub fn allows_background_work(mode: FerrumPowerMode) -> bool {
    !matches!(mode, FerrumPowerMode::LowPower | FerrumPowerMode::Emergency)
}

/// Shared gate updated by the gateway power watcher; DRS uses it to pause background work.
pub struct BackgroundWorkGate {
    mode: std::sync::atomic::AtomicU8,
}

impl BackgroundWorkGate {
    pub fn new(mode: FerrumPowerMode) -> Self {
        Self {
            mode: std::sync::atomic::AtomicU8::new(mode as u8),
        }
    }

    pub fn set_mode(&self, mode: FerrumPowerMode) {
        self.mode
            .store(mode as u8, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn current_mode(&self) -> FerrumPowerMode {
        match self.mode.load(std::sync::atomic::Ordering::SeqCst) {
            1 => FerrumPowerMode::LowPower,
            2 => FerrumPowerMode::Emergency,
            _ => FerrumPowerMode::HighPerformance,
        }
    }

    pub fn allows_background_work(&self) -> bool {
        allows_background_work(self.current_mode())
    }
}

impl Default for BackgroundWorkGate {
    fn default() -> Self {
        Self::new(FerrumPowerMode::HighPerformance)
    }
}

pub fn checkpoint_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(|h| {
        std::path::PathBuf::from(h)
            .join(".ferrum")
            .join("CHECKPOINT")
    })
}

pub async fn write_emergency_checkpoint(last_tx_id: Option<i64>) -> std::io::Result<()> {
    let path = checkpoint_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir for checkpoint")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "last_transaction_id": last_tx_id,
        "message": "FERRUM_EMERGENCY_SHUTDOWN: checkpoint written",
    });
    std::fs::write(&path, serde_json::to_string_pretty(&payload).unwrap())?;
    eprintln!(
        "FERRUM_EMERGENCY_SHUTDOWN: checkpoint written to {}",
        path.display()
    );
    Ok(())
}
