//! ADS token introspection client (ga4gh-infra integration).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from ADS introspection.
#[derive(Debug, Error)]
pub enum AdsIntrospectError {
    #[error("ADS URL not configured")]
    MissingUrl,
    #[error("ADS API key not configured")]
    MissingApiKey,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ADS returned HTTP {status}: {body}")]
    AdsHttp { status: u16, body: String },
    #[error("invalid ADS response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Serialize)]
struct IntrospectBody {
    token: String,
    resource: String,
    action: Option<String>,
    dataset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntrospectResponse {
    active: bool,
}

/// HTTP client for `POST /ads/v1/introspect`.
#[derive(Clone)]
pub struct AdsIntrospectClient {
    ads_base: String,
    api_key: String,
    http: reqwest::Client,
}

impl AdsIntrospectClient {
    /// Build from ADS base URL (`…/ads/v1`) and API key env var name.
    pub fn from_env(ads_base: &str, api_key_env: &str) -> Result<Self, AdsIntrospectError> {
        let base = ads_base.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(AdsIntrospectError::MissingUrl);
        }
        let api_key = std::env::var(api_key_env).map_err(|_| AdsIntrospectError::MissingApiKey)?;
        Ok(Self {
            ads_base: if base.ends_with("/ads/v1") {
                base
            } else if base.ends_with("/ads") {
                format!("{base}/v1")
            } else {
                format!("{base}/ads/v1")
            },
            api_key,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(AdsIntrospectError::Http)?,
        })
    }

    /// Returns true when ADS reports an active grant for the dataset/resource.
    pub async fn is_dataset_access_active(
        &self,
        passport_jwt: &str,
        resource: &str,
        dataset_id: &str,
    ) -> Result<bool, AdsIntrospectError> {
        self.introspect_at_base(&self.ads_base, passport_jwt, resource, dataset_id)
            .await
    }

    /// Introspect against an explicit ADS base URL (for federated nodes).
    pub async fn introspect_at_base(
        &self,
        ads_base: &str,
        passport_jwt: &str,
        resource: &str,
        dataset_id: &str,
    ) -> Result<bool, AdsIntrospectError> {
        let base = ads_base.trim().trim_end_matches('/');
        let base = if base.ends_with("/ads/v1") {
            base.to_string()
        } else if base.ends_with("/ads") {
            format!("{base}/v1")
        } else {
            format!("{base}/ads/v1")
        };
        let url = format!("{base}/introspect");
        let resp = self
            .http
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&IntrospectBody {
                token: passport_jwt.to_string(),
                resource: resource.to_string(),
                action: Some("read".to_string()),
                dataset_id: Some(dataset_id.to_string()),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AdsIntrospectError::AdsHttp {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let body: IntrospectResponse = resp
            .json()
            .await
            .map_err(|e| AdsIntrospectError::InvalidResponse(e.to_string()))?;
        Ok(body.active)
    }
}
