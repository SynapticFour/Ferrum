//! Shared helpers for executors.

use std::path::Path;
use std::time::Duration;

/// Sanitize path for use in command (no parent dir escape).
#[allow(dead_code)]
pub fn sanitize_work_dir(p: &Path) -> Option<std::path::PathBuf> {
    let canonical = p.canonicalize().ok()?;
    if canonical
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return None;
    }
    Some(canonical)
}

/// Learned from Sapporo: cancel should attempt graceful shutdown first.
/// 1) Send SIGTERM (best-effort)
/// 2) Wait up to 30s
/// 3) Escalate to SIGKILL
pub async fn cancel_child_gracefully(mut child: tokio::process::Child) {
    // tokio::process::Child doesn't expose a SIGTERM-specific helper, so we send it
    // directly on Unix.
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let _ = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    }
    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(_)) => {}
        _ => {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}
