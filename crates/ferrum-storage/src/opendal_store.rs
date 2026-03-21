//! OpenDAL-backed object storage (optional `opendal` feature).
//!
//! Lesson 7: one async API for many backends (S3, POSIX, GCS, …).
//! Source: Apache OpenDAL — production use in Databend, GreptimeDB, and similar TB-scale Rust systems.
//!
//! **Note:** [`OpenDalStorage::get`] currently buffers the full object in memory via [`opendal::Operator::read`].
//! For multi‑TB reads prefer the native [`crate::S3Storage`] streaming path until we wire OpenDAL's streaming reader.

use crate::ObjectStorage;
use async_trait::async_trait;
use ferrum_core::error::{FerrumError, Result};
use opendal::Operator;
use std::io::Cursor;
use std::path::Path;
use tokio::io::AsyncRead;

fn map_err(e: opendal::Error) -> FerrumError {
    FerrumError::StorageError(anyhow::Error::from(e))
}

/// Storage backed by any OpenDAL-supported service (build the [`Operator`] yourself).
pub struct OpenDalStorage {
    op: Operator,
}

impl OpenDalStorage {
    pub fn new(op: Operator) -> Self {
        Self { op }
    }

    /// POSIX / NFS mount style backend: `root` is the base directory for keys.
    pub fn from_local_dir(root: impl AsRef<Path>) -> Result<Self> {
        use opendal::services::Fs;
        let root = root.as_ref().to_str().ok_or_else(|| {
            FerrumError::ValidationError("OpenDAL fs root path must be valid UTF-8".into())
        })?;
        let op = Operator::new(Fs::default().root(root))
            .map_err(map_err)?
            .finish();
        Ok(Self { op })
    }
}

#[async_trait]
impl ObjectStorage for OpenDalStorage {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        self.op.write(key, data.to_vec()).await.map_err(map_err)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        let buf = self.op.read(key).await.map_err(map_err)?;
        Ok(Box::new(Cursor::new(buf.to_vec())))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.op.delete(key).await.map_err(map_err)?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        match self.op.stat(key).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(map_err(e)),
        }
    }

    async fn size(&self, key: &str) -> Result<u64> {
        let m = self.op.stat(key).await.map_err(map_err)?;
        Ok(m.content_length())
    }
}
