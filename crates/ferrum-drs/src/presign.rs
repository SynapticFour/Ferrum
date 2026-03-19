//! S3-compatible presigned URL generation (optional feature).

use crate::error::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Generates presigned GET URLs for S3-compatible storage.
/// Used when returning DRS access URLs for objects stored in S3/MinIO.
#[async_trait]
pub trait S3Presigner: Send + Sync {
    /// Produce a presigned GET URL for the given key.
    /// `range` is (start, end) inclusive for byte-range requests; None for full object.
    /// `expires_in` is the validity duration of the URL.
    async fn presign(
        &self,
        key: &str,
        range: Option<(u64, u64)>,
        expires_in: Duration,
    ) -> Result<String>;
}

/// No-op presigner when s3_signed is disabled (never used if state only sets presigner when feature on).
#[cfg(not(feature = "s3_signed"))]
pub fn create_presigner(
    _bucket: String,
    _region: String,
    _endpoint: Option<String>,
) -> Option<Arc<dyn S3Presigner>> {
    None
}

#[cfg(not(feature = "s3_signed"))]
use std::sync::Arc;

#[cfg(feature = "s3_signed")]
use std::sync::Arc;

#[cfg(feature = "s3_signed")]
mod aws_impl {
    use super::*;
    use aws_sdk_s3::presigning::PresigningConfig;
    use aws_sdk_s3::Client;
    use std::sync::Arc;

    pub struct AwsS3Presigner {
        client: Client,
        bucket: String,
    }

    impl AwsS3Presigner {
        pub fn new(client: Client, bucket: String) -> Self {
            Self { client, bucket }
        }
    }

    #[async_trait]
    impl S3Presigner for AwsS3Presigner {
        async fn presign(
            &self,
            key: &str,
            range: Option<(u64, u64)>,
            expires_in: Duration,
        ) -> Result<String> {
            let config = PresigningConfig::expires_in(expires_in)
                .map_err(|e| DrsError::Other(anyhow::anyhow!("presign config: {}", e)))?;
            let mut presign_op = self.client.get_object().bucket(&self.bucket).key(key);
            if let Some((start, end)) = range {
                presign_op = presign_op.range(format!("bytes={}-{}", start, end));
            }
            let uri = presign_op
                .presigned(config)
                .await
                .map_err(|e| DrsError::Other(anyhow::anyhow!("presign: {}", e)))?
                .uri()
                .to_string();
            Ok(uri)
        }
    }
}

#[cfg(feature = "s3_signed")]
pub use aws_impl::AwsS3Presigner;

/// Build an S3 presigner from shared config, bucket, region, and optional custom endpoint (MinIO).
/// Requires feature `s3_signed`. Call from app startup after loading `aws_config::defaults()`.
#[cfg(feature = "s3_signed")]
pub async fn create_presigner(
    bucket: String,
    region: String,
    endpoint: Option<String>,
) -> Option<Arc<dyn S3Presigner>> {
    use aws_config::BehaviorVersion;
    use aws_sdk_s3::config::Region;
    use aws_sdk_s3::Client;

    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let mut builder = aws_sdk_s3::config::Builder::from(&sdk_config).region(Region::new(region));
    if let Some(ep) = endpoint {
        builder = builder.endpoint_url(ep);
    }
    let client = Client::new(&builder.build());
    Some(Arc::new(AwsS3Presigner::new(client, bucket)))
}
