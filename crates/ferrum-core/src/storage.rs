//! Storage abstraction: ObjectStorage trait, LocalStorage and S3Storage.

use crate::error::{FerrumError, Result};
use crate::io::posix;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use bytes::Bytes;
use std::path::PathBuf;
use tokio::io::AsyncRead;

/// Object storage backend: put_bytes, get, delete, exists, size.
/// Only [ObjectStorage::put_bytes] is used (no generic put) so the trait is object-safe for `Arc<dyn ObjectStorage>`.
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()>;

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>>;

    async fn delete(&self, key: &str) -> Result<()>;

    async fn exists(&self, key: &str) -> Result<bool>;

    async fn size(&self, key: &str) -> Result<u64>;
}

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

fn path_for_local(base_path: &std::path::Path, key: &str) -> Result<PathBuf> {
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
        // Lesson: POSIX blocking I/O on dedicated pool (not Tokio blocking pool).
        // Source: TB-scale / HPC concurrent read patterns.
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

/// S3-compatible storage (AWS S3, MinIO, etc.).
pub struct S3Storage {
    client: S3Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(
        endpoint: Option<&str>,
        region: Option<&str>,
        bucket: &str,
        access_key_id: Option<&str>,
        secret_access_key: Option<&str>,
    ) -> Result<Self> {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let (Some(ak), Some(sk)) = (access_key_id, secret_access_key) {
            loader =
                loader.credentials_provider(Credentials::new(ak, sk, None, None, "ferrum-storage"));
        }
        let base = loader.load().await;
        let mut s3_builder = aws_sdk_s3::config::Builder::from(&base);
        if let Some(ep) = endpoint {
            s3_builder = s3_builder.endpoint_url(ep).force_path_style(true);
        }
        if let Some(reg) = region {
            s3_builder = s3_builder.region(aws_config::Region::new(reg.to_string()));
        }
        let client = S3Client::from_conf(s3_builder.build());
        Ok(S3Storage {
            client,
            bucket: bucket.to_string(),
        })
    }

    pub fn from_config(
        cfg: &crate::config::StorageConfig,
    ) -> impl std::future::Future<Output = Result<Self>> + Send {
        let endpoint = cfg.s3_endpoint.clone();
        let region = cfg
            .s3_region
            .clone()
            .unwrap_or_else(|| "us-east-1".to_string());
        let bucket = cfg
            .s3_bucket
            .clone()
            .unwrap_or_else(|| "ferrum".to_string());
        let access = cfg.s3_access_key_id.clone();
        let secret = cfg.s3_secret_access_key.clone();
        async move {
            Self::new(
                endpoint.as_deref(),
                Some(&region),
                &bucket,
                access.as_deref(),
                secret.as_deref(),
            )
            .await
        }
    }
}

#[async_trait]
impl ObjectStorage for S3Storage {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let body = ByteStream::from(Bytes::from(data.to_vec()));
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                let inner = e.into_service_error();
                if inner.is_no_such_key() {
                    FerrumError::NotFound(key.to_string())
                } else {
                    FerrumError::StorageError(inner.into())
                }
            })?;
        let reader = out.body.into_async_read();
        Ok(Box::new(reader))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let inner = e.into_service_error();
                if inner.is_not_found() {
                    Ok(false)
                } else {
                    Err(FerrumError::StorageError(inner.into()))
                }
            }
        }
    }

    async fn size(&self, key: &str) -> Result<u64> {
        let out = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                let inner = e.into_service_error();
                if inner.is_not_found() {
                    FerrumError::NotFound(key.to_string())
                } else {
                    FerrumError::StorageError(inner.into())
                }
            })?;
        Ok(out.content_length().unwrap_or(0) as u64)
    }
}
