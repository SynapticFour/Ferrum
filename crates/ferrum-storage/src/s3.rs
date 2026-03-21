//! S3-compatible storage with multipart upload for large payloads.

use crate::parts::split_into_part_ranges;
use crate::ObjectStorage;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client as S3Client;
use aws_smithy_types::byte_stream::Length;
use bytes::Bytes;
use ferrum_core::config::StorageConfig;
use ferrum_core::error::{FerrumError, Result};

use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Threshold: below this size, use single `put_object`.
const MIN_MULTIPART_BYTES: usize = 5 * 1024 * 1024;
/// S3 minimum part size for multipart (except last part).
const PART_SIZE: usize = 5 * 1024 * 1024;
const MAX_IN_FLIGHT_PARTS: usize = 16;
const UPLOAD_CONCURRENCY: usize = 4;

/// Path-based upload: below this size use a single ranged `put_object` read (no full-file RAM buffer).
/// Lesson 2: community threshold ~8 MiB before multipart; avoids `EntityTooSmall` on naive splits.
/// Source: aws-sdk-rust discussions + S3 multipart minimum part rules.
const FILE_PUT_THRESHOLD: u64 = 8 * 1024 * 1024;
/// 64 MiB parts balance throughput vs part count for TB objects.
const FILE_PART_SIZE: u64 = 64 * 1024 * 1024;
const FILE_UPLOAD_CONCURRENCY: usize = 8;
const FILE_MAX_IN_FLIGHT: usize = 16;

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
        cfg: &StorageConfig,
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

    /// Upload a local file using ranged reads — suitable for TB-scale objects (no `Vec` of full file).
    pub async fn put_file(&self, key: &str, path: &Path) -> Result<()> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        let size = meta.len();

        if size == 0 {
            let body = ByteStream::from(Bytes::new());
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(body)
                .send()
                .await
                .map_err(|e| FerrumError::StorageError(e.into()))?;
            return Ok(());
        }

        if size < FILE_PUT_THRESHOLD {
            let body = ByteStream::read_from()
                .path(path)
                .offset(0)
                .length(Length::Exact(size))
                .build()
                .await
                .map_err(|e| FerrumError::StorageError(e.into()))?;
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(body)
                .send()
                .await
                .map_err(|e| FerrumError::StorageError(e.into()))?;
            return Ok(());
        }

        self.put_file_multipart(key, path, size).await
    }

    async fn put_file_multipart(&self, key: &str, path: &Path, size: u64) -> Result<()> {
        let ranges = crate::parts::split_file_part_ranges(size, FILE_PART_SIZE);
        let create = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        let upload_id = create
            .upload_id()
            .ok_or_else(|| {
                FerrumError::StorageError(anyhow::anyhow!(
                    "create_multipart_upload: missing upload_id"
                ))
            })?
            .to_string();

        let path_buf = path.to_path_buf();
        let sem_if = Arc::new(Semaphore::new(FILE_MAX_IN_FLIGHT));
        let sem_up = Arc::new(Semaphore::new(FILE_UPLOAD_CONCURRENCY));
        let mut join_set: JoinSet<std::result::Result<(i32, String), FerrumError>> = JoinSet::new();

        for (idx, (start, end)) in ranges.iter().enumerate() {
            let part_number = (idx + 1) as i32;
            let len = end - start;
            if len == 0 {
                continue;
            }
            let permit_if = sem_if
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| FerrumError::StorageError(e.into()))?;
            let part_path = path_buf.clone();
            let client = self.client.clone();
            let bucket = self.bucket.clone();
            let key_owned = key.to_string();
            let upload_id_owned = upload_id.clone();
            let sem_up = sem_up.clone();
            let off = *start;
            join_set.spawn(async move {
                let _up = sem_up
                    .acquire_owned()
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
                let body = ByteStream::read_from()
                    .path(part_path.as_path())
                    .offset(off)
                    .length(Length::Exact(len))
                    .build()
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
                let out = client
                    .upload_part()
                    .bucket(&bucket)
                    .key(&key_owned)
                    .upload_id(&upload_id_owned)
                    .part_number(part_number)
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
                let etag = out
                    .e_tag()
                    .ok_or_else(|| {
                        FerrumError::StorageError(anyhow::anyhow!("upload_part: missing e_tag"))
                    })?
                    .to_string();
                drop(_up);
                drop(permit_if);
                Ok((part_number, etag))
            });
        }

        let mut completed: Vec<(i32, String)> = Vec::with_capacity(ranges.len());
        let mut first_err: Option<FerrumError> = None;

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok(Ok(pair)) => completed.push(pair),
                Ok(Err(e)) => {
                    first_err = Some(e);
                    break;
                }
                Err(e) => {
                    first_err = Some(FerrumError::StorageError(e.into()));
                    break;
                }
            }
        }

        if let Some(e) = first_err {
            join_set.abort_all();
            while join_set.join_next().await.is_some() {}
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(&upload_id)
                .send()
                .await;
            return Err(e);
        }

        completed.sort_by_key(|(n, _)| *n);
        let parts: Vec<CompletedPart> = completed
            .into_iter()
            .map(|(n, etag)| CompletedPart::builder().e_tag(etag).part_number(n).build())
            .collect();

        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(&upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;

        Ok(())
    }

    async fn put_object_single(&self, key: &str, data: &[u8]) -> Result<()> {
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

    async fn put_bytes_multipart(&self, key: &str, data: &[u8]) -> Result<()> {
        let ranges = split_into_part_ranges(data.len(), PART_SIZE);
        let create = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        let upload_id = create
            .upload_id()
            .ok_or_else(|| {
                FerrumError::StorageError(anyhow::anyhow!(
                    "create_multipart_upload: missing upload_id"
                ))
            })?
            .to_string();

        let sem_if = Arc::new(Semaphore::new(MAX_IN_FLIGHT_PARTS));
        let sem_up = Arc::new(Semaphore::new(UPLOAD_CONCURRENCY));
        let mut join_set: JoinSet<std::result::Result<(i32, String), FerrumError>> = JoinSet::new();

        for (idx, (start, end)) in ranges.iter().enumerate() {
            let part_number = (idx + 1) as i32;
            let permit_if = sem_if
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| FerrumError::StorageError(e.into()))?;
            let chunk = Bytes::copy_from_slice(&data[*start..*end]);
            let client = self.client.clone();
            let bucket = self.bucket.clone();
            let key_owned = key.to_string();
            let upload_id_owned = upload_id.clone();
            let sem_up = sem_up.clone();
            join_set.spawn(async move {
                let _up = sem_up
                    .acquire_owned()
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
                let body = ByteStream::from(chunk);
                let out = client
                    .upload_part()
                    .bucket(&bucket)
                    .key(&key_owned)
                    .upload_id(&upload_id_owned)
                    .part_number(part_number)
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
                let etag = out
                    .e_tag()
                    .ok_or_else(|| {
                        FerrumError::StorageError(anyhow::anyhow!("upload_part: missing e_tag"))
                    })?
                    .to_string();
                drop(_up);
                drop(permit_if);
                Ok((part_number, etag))
            });
        }

        let mut completed: Vec<(i32, String)> = Vec::with_capacity(ranges.len());
        let mut first_err: Option<FerrumError> = None;

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok(Ok(pair)) => completed.push(pair),
                Ok(Err(e)) => {
                    first_err = Some(e);
                    break;
                }
                Err(e) => {
                    first_err = Some(FerrumError::StorageError(e.into()));
                    break;
                }
            }
        }

        if let Some(e) = first_err {
            join_set.abort_all();
            while join_set.join_next().await.is_some() {}
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(&upload_id)
                .send()
                .await;
            return Err(e);
        }

        completed.sort_by_key(|(n, _)| *n);
        let parts: Vec<CompletedPart> = completed
            .into_iter()
            .map(|(n, etag)| CompletedPart::builder().e_tag(etag).part_number(n).build())
            .collect();

        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(&upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;

        Ok(())
    }
}

#[async_trait]
impl ObjectStorage for S3Storage {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        if data.len() < MIN_MULTIPART_BYTES {
            self.put_object_single(key, data).await
        } else {
            self.put_bytes_multipart(key, data).await
        }
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
