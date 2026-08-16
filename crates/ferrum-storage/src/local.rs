// SPDX-License-Identifier: BUSL-1.1
//! Local filesystem storage.

use crate::ObjectStorage;
use async_trait::async_trait;
use ferrum_core::error::{FerrumError, Result};
use ferrum_core::io::posix;
use std::path::PathBuf;
use tokio::io::AsyncRead;

/// Local filesystem storage.
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        std::fs::create_dir_all(&base_path).map_err(|e| FerrumError::StorageError(e.into()))?;
        Ok(Self { base_path })
    }

    fn path_for(&self, key: &str) -> Result<PathBuf> {
        path_for_local(&self.base_path, key)
    }
}

pub(crate) fn path_for_local(base_path: &std::path::Path, key: &str) -> Result<PathBuf> {
    ferrum_core::validate_object_key(key)?;
    Ok(base_path.join(key))
}

#[async_trait]
impl ObjectStorage for LocalStorage {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let base_path = self.base_path.clone();
        let key = key.to_string();
        let data = data.to_vec();
        posix::spawn_blocking(move || {
            let path = path_for_local(&base_path, &key).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key: path escape")
            })?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &data)?;
            Ok::<(), std::io::Error>(())
        })
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                FerrumError::ValidationError(e.to_string())
            } else {
                FerrumError::StorageError(e.into())
            }
        })?;
        Ok(())
    }

    async fn put_file(&self, key: &str, src: &std::path::Path) -> Result<()> {
        let dest = self.path_for(key)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| FerrumError::StorageError(anyhow::anyhow!("put_file mkdir: {e}")))?;
        }
        tokio::fs::copy(src, &dest)
            .await
            .map_err(|e| FerrumError::StorageError(anyhow::anyhow!("put_file copy: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        let path = self.path_for(key)?;
        // Zero-copy streaming path: File → BufReader → AsyncRead (Tokio uses efficient
        // OS reads on Linux; DRS handlers chunk at 64 KiB — see docs/PERFORMANCE.md).
        let file = tokio::fs::File::open(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FerrumError::NotFound(key.to_string())
            } else {
                FerrumError::StorageError(e.into())
            }
        })?;
        Ok(Box::new(tokio::io::BufReader::new(file)))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let base_path = self.base_path.clone();
        let key = key.to_string();
        posix::spawn_blocking(move || {
            let path = path_for_local(&base_path, &key).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key: path escape")
            })?;
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            Ok::<(), std::io::Error>(())
        })
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                FerrumError::ValidationError(e.to_string())
            } else {
                FerrumError::StorageError(e.into())
            }
        })?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let base_path = self.base_path.clone();
        let key = key.to_string();
        let exists = posix::spawn_blocking(move || {
            let path = path_for_local(&base_path, &key).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key: path escape")
            })?;
            Ok::<bool, std::io::Error>(path.exists())
        })
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                FerrumError::ValidationError(e.to_string())
            } else {
                FerrumError::StorageError(e.into())
            }
        })?;
        Ok(exists)
    }

    async fn size(&self, key: &str) -> Result<u64> {
        let base_path = self.base_path.clone();
        let key_owned = key.to_string();
        let len = posix::spawn_blocking(move || {
            let path = path_for_local(&base_path, &key_owned).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key: path escape")
            })?;
            let meta = std::fs::metadata(&path)?;
            Ok::<u64, std::io::Error>(meta.len())
        })
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FerrumError::NotFound(key.to_string()),
            std::io::ErrorKind::InvalidInput => FerrumError::ValidationError(e.to_string()),
            _ => FerrumError::StorageError(e.into()),
        })?;
        Ok(len)
    }

    async fn append_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.path_for(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| FerrumError::StorageError(anyhow::anyhow!("append mkdir: {e}")))?;
        }
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        file.write_all(data)
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        file.sync_all()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::path_for_local;
    use std::path::Path;

    #[test]
    fn path_for_local_rejects_parent_dir() {
        let base = Path::new("/tmp/ferrum-store");
        assert!(path_for_local(base, "../etc/passwd").is_err());
        assert!(path_for_local(base, "drs/../../secret").is_err());
        assert!(path_for_local(base, "/etc/passwd").is_err());
    }

    #[test]
    fn path_for_local_allows_nested_key() {
        let base = Path::new("/tmp/ferrum-store");
        let p = path_for_local(base, "drs/01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        assert!(p.starts_with(base));
        assert!(p.ends_with("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
    }
}
