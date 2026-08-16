// SPDX-License-Identifier: BUSL-1.1
//! Solum sidecar consent status client (H2.1 Teeth).
//!
//! When configured, Ferrum polls `GET /v1/consent/status` before bound DRS byte
//! access and WES submit. Only `status=granted` allows; otherwise fail-closed.

use crate::config::SolumConfig;
use serde::Deserialize;
use thiserror::Error;

/// Metadata / tag key for Solum clinical subject (must match Solum `solum_subject_id`).
pub const SOLUM_SUBJECT_METADATA_KEY: &str = "solum_subject";
/// Metadata / tag key for Solum purpose binding.
pub const SOLUM_PURPOSE_METADATA_KEY: &str = "solum_purpose";

/// Header expected by the Solum sidecar token middleware.
pub const SOLUM_SIDECAR_TOKEN_HEADER: &str = "X-Solum-Sidecar-Token";

/// Outcome of a Solum consent status check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolumConsentStatus {
    Granted,
    Revoked,
    Unknown,
}

/// Errors talking to Solum or interpreting status.
#[derive(Debug, Error)]
pub enum SolumConsentError {
    #[error("solum consent denied: status={0}")]
    Denied(String),
    #[error("solum consent check failed: {0}")]
    Unavailable(String),
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
}

/// HTTP client for Solum `GET /v1/consent/status`.
#[derive(Clone)]
pub struct SolumConsentClient {
    base_url: String,
    token: String,
    http: reqwest::Client,
    default_subject: Option<String>,
    default_purpose: Option<String>,
}

impl SolumConsentClient {
    /// Build from `[solum]` config. Returns `None` when `base_url` is unset/empty
    /// or when no sidecar token can be resolved (feature stays off).
    pub fn from_config(cfg: &SolumConfig) -> Option<Self> {
        let base = cfg
            .base_url
            .as_ref()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())?;
        let token = cfg
            .sidecar_token
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("SOLUM_SIDECAR_TOKEN")
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })?;
        let timeout = std::time::Duration::from_secs(cfg.timeout_secs.max(1));
        let http = reqwest::Client::builder().timeout(timeout).build().ok()?;
        Some(Self {
            base_url: base,
            token,
            http,
            default_subject: nonempty_opt(cfg.default_subject.clone()),
            default_purpose: nonempty_opt(cfg.default_purpose.clone()),
        })
    }

    /// Resolve `(subject, purpose)` from explicit values falling back to defaults.
    /// Returns `None` when either side is still missing (skip Solum check).
    pub fn resolve_binding(
        &self,
        subject: Option<&str>,
        purpose: Option<&str>,
    ) -> Option<(String, String)> {
        let subject = subject
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| self.default_subject.clone())?;
        let purpose = purpose
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| self.default_purpose.clone())?;
        Some((subject, purpose))
    }

    /// Binding from DRS object metadata key/value pairs.
    pub fn binding_from_metadata(&self, metadata: &[(String, String)]) -> Option<(String, String)> {
        let subject = metadata
            .iter()
            .find(|(k, _)| k == SOLUM_SUBJECT_METADATA_KEY)
            .map(|(_, v)| v.as_str());
        let purpose = metadata
            .iter()
            .find(|(k, _)| k == SOLUM_PURPOSE_METADATA_KEY)
            .map(|(_, v)| v.as_str());
        self.resolve_binding(subject, purpose)
    }

    /// Binding from WES run `tags` JSON object.
    pub fn binding_from_tags(&self, tags: &serde_json::Value) -> Option<(String, String)> {
        let subject = tags
            .get(SOLUM_SUBJECT_METADATA_KEY)
            .and_then(|v| v.as_str());
        let purpose = tags
            .get(SOLUM_PURPOSE_METADATA_KEY)
            .and_then(|v| v.as_str());
        self.resolve_binding(subject, purpose)
    }

    /// Query Solum status for a concrete pair.
    pub async fn query_status(
        &self,
        subject: &str,
        purpose: &str,
    ) -> Result<SolumConsentStatus, SolumConsentError> {
        let url = format!("{}/v1/consent/status", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header(SOLUM_SIDECAR_TOKEN_HEADER, &self.token)
            .query(&[("subject", subject), ("purpose", purpose)])
            .send()
            .await
            .map_err(|e| SolumConsentError::Unavailable(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SolumConsentError::Unavailable(format!(
                "HTTP {status}: {body}"
            )));
        }
        let body: StatusResponse = resp
            .json()
            .await
            .map_err(|e| SolumConsentError::Unavailable(e.to_string()))?;
        Ok(match body.status.trim().to_ascii_lowercase().as_str() {
            "granted" => SolumConsentStatus::Granted,
            "revoked" => SolumConsentStatus::Revoked,
            _ => SolumConsentStatus::Unknown,
        })
    }

    /// Fail-closed require `granted` for the pair.
    pub async fn require_granted(
        &self,
        subject: &str,
        purpose: &str,
    ) -> Result<(), SolumConsentError> {
        match self.query_status(subject, purpose).await? {
            SolumConsentStatus::Granted => Ok(()),
            SolumConsentStatus::Revoked => Err(SolumConsentError::Denied("revoked".into())),
            SolumConsentStatus::Unknown => Err(SolumConsentError::Denied("unknown".into())),
        }
    }
}

/// If client and binding are present, require Solum `granted`; otherwise no-op.
pub async fn enforce_solum_consent(
    client: Option<&std::sync::Arc<SolumConsentClient>>,
    subject: Option<&str>,
    purpose: Option<&str>,
) -> Result<(), SolumConsentError> {
    let Some(client) = client else {
        return Ok(());
    };
    let Some((subject, purpose)) = client.resolve_binding(subject, purpose) else {
        return Ok(());
    };
    client.require_granted(&subject, &purpose).await
}

fn nonempty_opt(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg(base: &str) -> SolumConfig {
        SolumConfig {
            base_url: Some(base.to_string()),
            sidecar_token: Some("test-token".into()),
            default_subject: Some("patient/demo".into()),
            default_purpose: Some("secondary_use_hdab".into()),
            timeout_secs: 5,
        }
    }

    #[test]
    fn subject_bridge_metadata_keys_match_solum_adr() {
        // Keep in lockstep with Solum ADR 0003 / sidecar subject-link.
        assert_eq!(SOLUM_SUBJECT_METADATA_KEY, "solum_subject");
        assert_eq!(SOLUM_PURPOSE_METADATA_KEY, "solum_purpose");
        let client = SolumConsentClient::from_config(&cfg("http://127.0.0.1:9")).unwrap();
        let meta = vec![
            (SOLUM_SUBJECT_METADATA_KEY.into(), "bridge-patient-1".into()),
            (SOLUM_PURPOSE_METADATA_KEY.into(), "care_provision".into()),
        ];
        let (s, p) = client.binding_from_metadata(&meta).unwrap();
        assert_eq!(s, "bridge-patient-1");
        assert_eq!(p, "care_provision");
    }

    #[test]
    fn resolve_binding_prefers_explicit_over_defaults() {
        let client = SolumConsentClient::from_config(&cfg("http://127.0.0.1:9")).unwrap();
        let (s, p) = client
            .resolve_binding(Some("patient/x"), Some("care_provision"))
            .unwrap();
        assert_eq!(s, "patient/x");
        assert_eq!(p, "care_provision");
    }

    #[test]
    fn resolve_binding_uses_defaults_when_partial() {
        let client = SolumConsentClient::from_config(&cfg("http://127.0.0.1:9")).unwrap();
        let (s, p) = client.resolve_binding(None, None).unwrap();
        assert_eq!(s, "patient/demo");
        assert_eq!(p, "secondary_use_hdab");
    }

    #[test]
    fn from_config_none_without_url() {
        let mut c = cfg("http://x");
        c.base_url = None;
        assert!(SolumConsentClient::from_config(&c).is_none());
    }

    #[tokio::test]
    async fn require_granted_ok_and_denied() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/consent/status"))
            .and(query_param("subject", "patient/demo"))
            .and(query_param("purpose", "secondary_use_hdab"))
            .and(header(SOLUM_SIDECAR_TOKEN_HEADER, "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "granted"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        let client = SolumConsentClient::from_config(&cfg(&server.uri())).unwrap();
        client
            .require_granted("patient/demo", "secondary_use_hdab")
            .await
            .expect("granted");

        Mock::given(method("GET"))
            .and(path("/v1/consent/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "revoked"
            })))
            .mount(&server)
            .await;

        let err = client
            .require_granted("patient/demo", "secondary_use_hdab")
            .await
            .expect_err("revoked");
        assert!(matches!(err, SolumConsentError::Denied(_)));
    }

    #[tokio::test]
    async fn fail_closed_on_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/consent/status"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = SolumConsentClient::from_config(&cfg(&server.uri())).unwrap();
        let err = client
            .require_granted("patient/demo", "secondary_use_hdab")
            .await
            .expect_err("unavailable");
        assert!(matches!(err, SolumConsentError::Unavailable(_)));
    }

    #[tokio::test]
    async fn enforce_skips_without_client() {
        enforce_solum_consent(None, Some("a"), Some("b"))
            .await
            .expect("skip");
    }
}
