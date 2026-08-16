// SPDX-License-Identifier: BUSL-1.1
//! Filesystem free-space probes for Edge / field health checks.

use serde::Serialize;
use std::path::Path;

/// Free-space snapshot for a data directory (objects + SQLite parent).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiskSpaceStatus {
    pub path: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub free_percent: f32,
    /// True when free space is below the configured warn threshold.
    pub warn_low_space: bool,
}

/// Probe available disk space for `path`. Returns `None` if the path is missing or unsupported.
pub fn disk_space_status(path: &Path, warn_below_percent: f32) -> Option<DiskSpaceStatus> {
    let canonical = path.canonicalize().ok()?;
    let (total, free) = platform_free_space(&canonical)?;
    if total == 0 {
        return None;
    }
    let free_percent = (free as f64 / total as f64 * 100.0) as f32;
    Some(DiskSpaceStatus {
        path: canonical.display().to_string(),
        total_bytes: total,
        free_bytes: free,
        free_percent,
        warn_low_space: free_percent < warn_below_percent,
    })
}

#[cfg(unix)]
fn platform_free_space(path: &Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c_path` is a valid NUL-terminated C string from `CString`; `stat` is zeroed
    // and used only if `statvfs` returns 0, which fills the struct.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return None;
    }
    // libc field widths differ (Linux `u64` vs macOS `fsblkcnt_t` / `c_ulong`).
    #[allow(clippy::unnecessary_cast)]
    let total = stat.f_blocks as u64 * stat.f_frsize as u64;
    #[allow(clippy::unnecessary_cast)]
    let free = stat.f_bavail as u64 * stat.f_frsize as u64;
    Some((total, free))
}

#[cfg(not(unix))]
fn platform_free_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn disk_space_for_temp_dir() {
        let tmp = env::temp_dir();
        let status = disk_space_status(&tmp, 5.0).expect("temp dir stat");
        assert!(status.total_bytes > 0);
        assert!(status.free_bytes <= status.total_bytes);
        assert!(status.free_percent >= 0.0 && status.free_percent <= 100.0);
    }
}
