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
/// On a rename error the partial is left in place. Startup does not read or
/// delete `.ferrum-partial-*` files.
pub fn publish_staged_file(tmp: &Path, final_path: &Path) -> io::Result<()> {
    fs::rename(tmp, final_path)?;
    fsync_parent(final_path)
}
