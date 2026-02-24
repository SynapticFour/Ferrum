//! Shared helpers for executors.

use std::path::Path;

/// Sanitize path for use in command (no parent dir escape).
#[allow(dead_code)]
pub fn sanitize_work_dir(p: &Path) -> Option<std::path::PathBuf> {
    let canonical = p.canonicalize().ok()?;
    if canonical.components().any(|c| c == std::path::Component::ParentDir) {
        return None;
    }
    Some(canonical)
}
