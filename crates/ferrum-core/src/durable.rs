// SPDX-License-Identifier: BUSL-1.1
//! Publish a file by writing a sibling temporary, fsync, and rename.
//!
//! The final name is replaced only by `rename`, so a crash during the write
//! does not leave a 0-byte or truncated file at that name. A
//! `.ferrum-partial-*` sibling may remain. It is not the published file.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NONCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn partial_path(final_path: &Path) -> io::Result<PathBuf> {
    let parent = match final_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = final_path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let n = NONCE.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(
        ".{}.ferrum-partial-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        n
    );
    Ok(parent.join(tmp_name))
}

pub(crate) fn fsync_parent(final_path: &Path) -> io::Result<()> {
    let parent = match final_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let dir = File::open(parent)?;
    dir.sync_all()?;
    Ok(())
}

/// Write `bytes` to `final_path` via a temporary file in the same directory.
pub(crate) fn durable_write(final_path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = final_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp = partial_path(final_path)?;
    let write_result = (|| -> io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp, final_path) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    fsync_parent(final_path)
}

/// Publish a file that was already written and fsynced at `tmp`.
pub(crate) fn durable_publish(tmp: &Path, final_path: &Path) -> io::Result<()> {
    if let Err(err) = fs::rename(tmp, final_path) {
        let _ = fs::remove_file(tmp);
        return Err(err);
    }
    fsync_parent(final_path)
}

fn prepare_partial(final_path: &Path) -> io::Result<PathBuf> {
    if let Some(parent) = final_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    partial_path(final_path)
}

/// Copy `src` to a sibling partial of `final_path` and fsync it.
///
/// The final name is not created. The caller renames with
/// [`publish_staged_file`] after any database commit that must precede the
/// final name.
pub fn stage_file_beside(final_path: &Path, src: &Path) -> io::Result<PathBuf> {
    let tmp = prepare_partial(final_path)?;
    let copied = (|| -> io::Result<()> {
        fs::copy(src, &tmp)?;
        let file = File::open(&tmp)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = copied {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(tmp)
}

/// Write `bytes` to a sibling partial of `final_path` and fsync it.
///
/// The final name is not created. See [`stage_file_beside`].
pub fn stage_bytes_beside(final_path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let tmp = prepare_partial(final_path)?;
    let written = (|| -> io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = written {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(tmp)
}

/// Rename a staged partial onto `final_path` and fsync the directory.
///
/// On a rename error the partial is left in place. Startup removes a
/// `.ferrum-partial-*` file only when it is older than one hour.
pub fn publish_staged_file(tmp: &Path, final_path: &Path) -> io::Result<()> {
    fs::rename(tmp, final_path)?;
    fsync_parent(final_path)
}

/// Partials younger than this stay, including a write that is still open.
pub const STALE_PARTIAL_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

fn is_partial_name(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().contains(".ferrum-partial-")
}

/// Remove `.ferrum-partial-*` files in `dir` whose modified time is older than
/// `max_age`. Other names are left in place. When `recursive` is false, only
/// the directory itself is scanned.
///
/// Each removal is logged. A missing directory removes nothing.
pub fn sweep_stale_partials(
    dir: &Path,
    max_age: std::time::Duration,
    recursive: bool,
) -> io::Result<Vec<PathBuf>> {
    sweep_stale_partials_at(dir, max_age, recursive, std::time::SystemTime::now())
}

fn sweep_stale_partials_at(
    dir: &Path,
    max_age: std::time::Duration,
    recursive: bool,
    now: std::time::SystemTime,
) -> io::Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut removed = Vec::new();
    sweep_dir(dir, max_age, recursive, now, &mut removed)?;
    Ok(removed)
}

fn sweep_dir(
    dir: &Path,
    max_age: std::time::Duration,
    recursive: bool,
    now: std::time::SystemTime,
    removed: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if recursive {
                sweep_dir(&entry.path(), max_age, true, now, removed)?;
            }
            continue;
        }
        if !file_type.is_file() || !is_partial_name(&entry.file_name()) {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        let age = match now.duration_since(modified) {
            Ok(age) => age,
            Err(_) => continue,
        };
        if age <= max_age {
            continue;
        }
        let path = entry.path();
        fs::remove_file(&path)?;
        tracing::info!(path = %path.display(), "removed stale .ferrum-partial write");
        removed.push(path);
    }
    Ok(())
}

#[cfg(test)]
mod partial_sweep_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn write_aged(path: &Path, age: Duration, now: SystemTime) {
        fs::write(path, b"partial").unwrap();
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(now.checked_sub(age).expect("age fits in the clock"))
            .unwrap();
    }

    #[test]
    fn removes_only_old_partials() {
        let root = std::env::temp_dir().join(format!(
            "ferrum-partial-sweep-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        let now = SystemTime::now();
        let old = root.join(".blob.ferrum-partial-1-1");
        let young = root.join(".blob.ferrum-partial-1-2");
        let nested_old = root.join("nested").join(".blob.ferrum-partial-9-1");
        let plain = root.join("object.bin");
        let decoy = root.join("ferrum-partial-note.txt");
        write_aged(&old, Duration::from_secs(3601), now);
        write_aged(&young, Duration::from_secs(60), now);
        write_aged(&nested_old, Duration::from_secs(7200), now);
        write_aged(&plain, Duration::from_secs(86_400), now);
        write_aged(&decoy, Duration::from_secs(86_400), now);

        let removed = sweep_stale_partials_at(&root, STALE_PARTIAL_MAX_AGE, true, now).unwrap();
        assert!(removed.iter().any(|path| path == &old));
        assert!(removed.iter().any(|path| path == &nested_old));
        assert_eq!(removed.len(), 2);
        assert!(!old.exists());
        assert!(young.exists());
        assert!(!nested_old.exists());
        assert!(plain.exists());
        assert!(decoy.exists());

        let shallow = sweep_stale_partials_at(&root, STALE_PARTIAL_MAX_AGE, false, now).unwrap();
        assert!(shallow.is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
