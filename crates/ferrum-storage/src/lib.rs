// SPDX-License-Identifier: BUSL-1.1
//! Object storage backends: [`ObjectStorage`], [`LocalStorage`], [`S3Storage`].

mod bandwidth;
mod local;
#[cfg(feature = "opendal")]
mod opendal_store;
mod parts;
mod s3;
mod transfer_queue;

pub use bandwidth::{BandwidthClass, BandwidthMonitor, MIN_BANDWIDTH_SAMPLE_BYTES};
pub use local::LocalStorage;
#[cfg(feature = "opendal")]
pub use opendal_store::OpenDalStorage;
pub use parts::{split_file_part_ranges, split_into_part_ranges};
pub use s3::S3Storage;
pub use transfer_queue::{QueuedTransfer, TransferDirection, TransferQueue};

use async_trait::async_trait;
use ferrum_core::error::Result;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Object storage backend: put_bytes, get, delete, exists, size.
/// Only [`ObjectStorage::put_bytes`] is used (no generic put) so the trait is object-safe for `Arc<dyn ObjectStorage>`.
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()>;

    /// Store object bytes from a local file path (streaming; avoids loading entire file into RAM).
    async fn put_file(&self, key: &str, path: &std::path::Path) -> Result<()> {
        let data = tokio::fs::read(path).await.map_err(|e| {
            ferrum_core::FerrumError::StorageError(anyhow::anyhow!("put_file read: {e}"))
        })?;
        self.put_bytes(key, &data).await
    }

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>>;

    async fn delete(&self, key: &str) -> Result<()>;

    async fn exists(&self, key: &str) -> Result<bool>;

    async fn size(&self, key: &str) -> Result<u64>;

    /// Append bytes to an object (creates the key when missing).
    async fn append_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let mut buf = if self.exists(key).await? {
            let mut reader = self.get(key).await?;
            let mut existing = Vec::new();
            reader.read_to_end(&mut existing).await.map_err(|e| {
                ferrum_core::FerrumError::StorageError(anyhow::anyhow!("append read: {e}"))
            })?;
            existing
        } else {
            Vec::new()
        };
        buf.extend_from_slice(data);
        self.put_bytes(key, &buf).await
    }
}
