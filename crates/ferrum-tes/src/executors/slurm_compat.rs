//! SLURM / glibc process-spawn performance hint.
//!
//! Lesson: `std::process::Command` / `tokio::process::Command` on old glibc may use slow `fork()`.
//! Source: Rust users forum + HPC cluster experience (glibc &lt; 2.24).
//! Reason: warn operators when sbatch/squeue/sacct latency may be dominated by spawn, not SLURM.
// Note: glibc version comes from `gnu_get_libc_version`, not `/proc/version` (kernel string).

use std::sync::Once;

static GLIBC_WARN_ONCE: Once = Once::new();

/// Log once if we detect GNU libc older than 2.24 (may imply slow `fork`-based spawns).
pub fn warn_old_glibc_process_spawn_if_needed() {
    GLIBC_WARN_ONCE.call_once(|| {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            if let Some((major, minor)) = gnu_libc_version() {
                if major < 2 || (major == 2 && minor < 24) {
                    tracing::warn!(
                        glibc_major = major,
                        glibc_minor = minor,
                        "glibc < 2.24: process spawning may fall back to slow fork(); \
                         SLURM CLI (sbatch/squeue/sacct) can be much slower on some clusters"
                    );
                }
            }
        }
    });
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn gnu_libc_version() -> Option<(u32, u32)> {
    // SAFETY: gnu_get_libc_version returns a static NUL-terminated string.
    unsafe {
        let p = libc::gnu_get_libc_version();
        if p.is_null() {
            return None;
        }
        let s = std::ffi::CStr::from_ptr(p).to_string_lossy();
        parse_glibc_version(&s)
    }
}

// Used by Linux+GNU runtime path and by unit tests on all targets.
#[cfg(any(test, all(target_os = "linux", target_env = "gnu")))]
fn parse_glibc_version(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    let (maj_s, min_s) = s.split_once('.')?;
    let major: u32 = maj_s.parse().ok()?;
    let minor: u32 = min_s
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::parse_glibc_version;

    #[test]
    fn parses_typical_glibc_strings() {
        assert_eq!(parse_glibc_version("2.35"), Some((2, 35)));
        assert_eq!(parse_glibc_version("2.23"), Some((2, 23)));
        assert_eq!(parse_glibc_version("2.31-0ubuntu9"), Some((2, 31)));
    }
}
