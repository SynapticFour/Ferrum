//! Host platform detection for Laptop Mode startup logging and optional tuning.

use serde::Serialize;

/// Detected host characteristics (best-effort, no extra dependencies).
#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub cpu_model: Option<String>,
    pub ram_total_mb: Option<u64>,
    pub laptop_build: bool,
}

/// Read host OS/arch/RAM/CPU for startup diagnostics.
pub fn detect_platform(laptop_build: bool) -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_model: read_cpu_model(),
        ram_total_mb: read_ram_total_mb(),
        laptop_build,
    }
}

/// Log platform summary at gateway startup (Laptop Mode).
pub fn log_platform_startup(laptop_build: bool) {
    let info = detect_platform(laptop_build);
    tracing::info!(
        os = %info.os,
        arch = %info.arch,
        cpu = info.cpu_model.as_deref().unwrap_or("unknown"),
        ram_total_mb = ?info.ram_total_mb,
        laptop_build = info.laptop_build,
        "Ferrum platform detected"
    );
}

/// Suggest `[africa] max_memory_mb` when unset: ~80% of physical RAM (Linux/macOS).
pub fn suggested_memory_cap_mb() -> Option<u64> {
    read_ram_total_mb().map(|mb| mb.saturating_mul(80) / 100)
}

fn read_ram_total_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in meminfo.lines() {
            if let Some(kb) = line.strip_prefix("MemTotal:") {
                let kb: u64 = kb.trim().trim_end_matches(" kB").parse().ok()?;
                return Some(kb / 1024);
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let out = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !out.status.success() {
            None
        } else {
            let bytes: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
            Some(bytes / 1024 / 1024)
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

fn read_cpu_model() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in cpuinfo.lines() {
            if let Some(model) = line.strip_prefix("model name") {
                if let Some((_k, v)) = model.split_once(':') {
                    return Some(v.trim().to_string());
                }
            }
            if let Some(model) = line.strip_prefix("Model") {
                if let Some((_k, v)) = model.split_once(':') {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let out = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_platform_has_os_and_arch() {
        let info = detect_platform(true);
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(info.laptop_build);
    }
}
