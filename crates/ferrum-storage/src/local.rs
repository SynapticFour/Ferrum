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
    let path = base_path.join(key);
    if path.strip_prefix(base_path).is_err() {
        return Err(FerrumError::ValidationError(
            "invalid key: path escape".to_string(),
        ));
    }
    Ok(path)
}

#[async_trait]
impl ObjectStorage for LocalStorage {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let base_path = self.base_path.clone();
        let key = key.to_string();
        let data = data.to_vec();
        posix::spawn_blocking(move || {
            let path = base_path.join(&key);
            if path.strip_prefix(&base_path).is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid key: path escape",
                ));
            }
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

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        let path = self.path_for(key)?;
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
            let path = base_path.join(&key);
            if path.strip_prefix(&base_path).is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid key: path escape",
                ));
            }
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
            let path = base_path.join(&key);
            if path.strip_prefix(&base_path).is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid key: path escape",
                ));
            }
            Ok(path.exists())
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
            let path = base_path.join(&key_owned);
            if path.strip_prefix(&base_path).is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid key: path escape",
                ));
            }
            let meta = std::fs::metadata(&path)?;
            Ok(meta.len())
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
}
