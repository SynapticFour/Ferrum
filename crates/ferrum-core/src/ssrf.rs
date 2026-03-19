//! A10: SSRF protection — SafeHttpClient and URL validation.

use crate::error::{FerrumError, Result};
use reqwest::Client;
use std::net::IpAddr;
use std::sync::Arc;
use url::Url;

/// Blocked hostnames (metadata, internal).
const BLOCKED_HOSTS: &[&str] = &[
    "169.254.169.254",
    "metadata.google.internal",
    "metadata",
    "localhost",
    "127.0.0.1",
    "::1",
];

/// Policy for SSRF checks.
#[derive(Clone)]
pub struct SsrfPolicy {
    pub allow_private_networks: bool,
    pub allowed_schemes: Vec<String>,
    pub blocked_hosts: Vec<String>,
}

impl Default for SsrfPolicy {
    fn default() -> Self {
        Self {
            allow_private_networks: false,
            allowed_schemes: vec!["https".to_string()],
            blocked_hosts: BLOCKED_HOSTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Returns true if the IP is private/loopback/link-local.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

/// Validate URL against SSRF policy (no network call). Use before storing or fetching.
pub fn validate_url_ssrf(url: &str, policy: &SsrfPolicy) -> Result<()> {
    let parsed =
        Url::parse(url).map_err(|_| FerrumError::ValidationError("invalid URL".to_string()))?;
    let scheme = parsed.scheme().to_lowercase();
    if !policy
        .allowed_schemes
        .iter()
        .any(|s| s.to_lowercase() == scheme)
    {
        return Err(FerrumError::SsrfBlocked("scheme not allowed".to_string()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| FerrumError::ValidationError("URL has no host".to_string()))?;
    let host_lower = host.to_lowercase();
    for blocked in &policy.blocked_hosts {
        if host_lower == *blocked || host_lower.ends_with(&format!(".{}", blocked)) {
            return Err(FerrumError::SsrfBlocked("blocked host".to_string()));
        }
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !policy.allow_private_networks && is_private_ip(&ip) {
            return Err(FerrumError::SsrfBlocked(
                "private network not allowed".to_string(),
            ));
        }
    }
    Ok(())
}

/// HTTP client that validates URLs against SSRF policy before requests.
pub struct SafeHttpClient {
    inner: Client,
    policy: Arc<SsrfPolicy>,
}

impl SafeHttpClient {
    pub fn new(policy: SsrfPolicy) -> Self {
        Self {
            inner: Client::builder().build().unwrap_or_else(|_| Client::new()),
            policy: Arc::new(policy),
        }
    }

    pub async fn get(&self, url: &str) -> Result<reqwest::Response> {
        validate_url_ssrf(url, &self.policy)?;
        if let Some(host) = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
        {
            for blocked in &self.policy.blocked_hosts {
                if host.eq_ignore_ascii_case(blocked) {
                    return Err(FerrumError::SsrfBlocked("blocked host".to_string()));
                }
            }
        }
        self.inner
            .get(url)
            .send()
            .await
            .map_err(|e| FerrumError::Internal(e.into()))
    }

    /// Validates URL then returns a request builder. Caller must use it immediately.
    pub fn post_builder(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        validate_url_ssrf(url, &self.policy)?;
        Ok(self.inner.post(url))
    }

    pub async fn post_json<T: serde::Serialize + ?Sized>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        validate_url_ssrf(url, &self.policy)?;
        self.inner
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| FerrumError::Internal(e.into()))
    }
}
