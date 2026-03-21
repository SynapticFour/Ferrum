//! Object storage backends: [`ObjectStorage`], [`LocalStorage`], [`S3Storage`].

mod local;
mod parts;
mod s3;

pub use local::LocalStorage;
pub use parts::split_into_part_ranges;
pub use s3::S3Storage;

use async_trait::async_trait;
use ferrum_core::error::Result;
use tokio::io::AsyncRead;

/// Object storage backend: put_bytes, get, delete, exists, size.
/// Only [`ObjectStorage::put_bytes`] is used (no generic put) so the trait is object-safe for `Arc<dyn ObjectStorage>`.
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()>;

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>>;

    async fn delete(&self, key: &str) -> Result<()>;

    async fn exists(&self, key: &str) -> Result<bool>;

    async fn size(&self, key: &str) -> Result<u64>;
}
