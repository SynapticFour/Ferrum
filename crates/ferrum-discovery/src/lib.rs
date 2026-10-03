// SPDX-License-Identifier: BUSL-1.1

//! GA4GH Service Registry client for Ferrum gateway integration.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ferrum_core::{AuthConfig, DiscoveryConfig, FerrumConfig};

mod service_info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use service_info::{ServiceInfo, ServiceOrganization, ServiceType};
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors from service registry operations.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("service registry URL not configured")]
    MissingRegistryUrl,
    #[error("registration API key not configured")]
    MissingApiKey,
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("registry returned HTTP {status}: {body}")]
    RegistryHttp { status: u16, body: String },
    #[error("invalid registry response: {0}")]
    InvalidResponse(String),
}

/// Registered GA4GH service entry (flattened ServiceInfo + URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredService {
    #[serde(flatten)]
    pub info: ServiceInfo,
    pub url: String,
}

/// GA4GH artifact names used for service discovery.
pub const ARTIFACT_DRS: &str = "drsservice";
pub const ARTIFACT_BEACON: &str = "beacon";
pub const ARTIFACT_HTSGET: &str = "htsget";
pub const ARTIFACT_WES: &str = "wes";
pub const ARTIFACT_TES: &str = "tes";
pub const ARTIFACT_TRS: &str = "tool-registry";
pub const ARTIFACT_ADS: &str = "access-decision-service";

/// Resolved base URLs for cross-service calls (registry → fallbacks → local gateway).
#[derive(Debug, Clone)]
pub struct ResolvedServiceUrls {
    pub gateway_base: String,
    pub tes: Option<String>,
    pub trs: Option<String>,
    pub ads: Option<String>,
    pub drs: Option<String>,
    pub wes: Option<String>,
}

impl ResolvedServiceUrls {
    /// Local Ferrum paths when no external registry is configured.
    pub fn local_defaults(gateway_base: &str) -> Self {
        let base = gateway_base.trim_end_matches('/');
        Self {
            gateway_base: base.to_string(),
            tes: local_service_url(base, ARTIFACT_TES),
            trs: local_service_url(base, ARTIFACT_TRS),
            ads: None,
            drs: local_service_url(base, ARTIFACT_DRS),
            wes: local_service_url(base, ARTIFACT_WES),
        }
    }
}

/// Map a GA4GH service artifact to a path under the Ferrum gateway base URL.
pub fn local_service_url(gateway_base: &str, artifact: &str) -> Option<String> {
    let base = gateway_base.trim_end_matches('/');
    let path = match artifact {
        ARTIFACT_DRS => "/ga4gh/drs/v1",
        ARTIFACT_BEACON => "/ga4gh/beacon/v2",
        ARTIFACT_HTSGET => "/ga4gh/htsget/v1",
        ARTIFACT_WES => "/ga4gh/wes/v1",
        ARTIFACT_TES => "/ga4gh/tes/v1",
        ARTIFACT_TRS => "/ga4gh/trs/v2",
        _ => return None,
    };
    Some(format!("{base}{path}"))
}

/// Resolve service URLs for gateway startup (TES, TRS register, ADS proxy).
pub async fn resolve_service_urls(
    config: &FerrumConfig,
    gateway_base: &str,
) -> ResolvedServiceUrls {
    let base = gateway_base.trim_end_matches('/').to_string();
    let mut resolved = ResolvedServiceUrls::local_defaults(&base);

    if let Some(ads) = resolve_ads_url(&config.auth, &config.discovery, &base).await {
        resolved.ads = Some(ads);
    }

    if config.discovery.enabled {
        if let Ok(client) = ServiceRegistryClient::from_config(&config.discovery) {
            if let Some(tes) = client.resolve_artifact(ARTIFACT_TES).await {
                resolved.tes = Some(tes);
            }
            if let Some(trs) = client.resolve_artifact(ARTIFACT_TRS).await {
                resolved.trs = Some(trs);
            }
            if let Some(drs) = client.resolve_artifact(ARTIFACT_DRS).await {
                resolved.drs = Some(drs);
            } else if resolved.drs.is_none() {
                resolved.drs = local_service_url(&base, ARTIFACT_DRS);
            }
            if let Some(wes) = client.resolve_artifact(ARTIFACT_WES).await {
                resolved.wes = Some(wes);
            } else if resolved.wes.is_none() {
                resolved.wes = local_service_url(&base, ARTIFACT_WES);
            }
            if resolved.ads.is_none() {
                if let Some(ads) = client.resolve_artifact(ARTIFACT_ADS).await {
                    resolved.ads = Some(normalize_ads_base(&ads));
                }
            }
        }
    }

    resolved
}

/// ADS base URL (`…/ads/v1`) from config, registry, or co-located broker host.
pub async fn resolve_ads_url(
    auth: &AuthConfig,
    discovery: &DiscoveryConfig,
    gateway_base: &str,
) -> Option<String> {
    if let Some(url) = auth
        .ads_url
        .as_ref()
        .map(|u| normalize_ads_base(u.trim()))
        .filter(|u| !u.is_empty())
    {
        return Some(url);
    }

    if discovery.enabled {
        if let Ok(client) = ServiceRegistryClient::from_config(discovery) {
            if let Some(url) = client.resolve_artifact(ARTIFACT_ADS).await {
                return Some(normalize_ads_base(&url));
            }
        }
        if let Some(url) = discovery
            .fallback_urls
            .get(ARTIFACT_ADS)
            .cloned()
            .or_else(|| discovery.fallback_urls.get("ads").cloned())
        {
            return Some(normalize_ads_base(&url));
        }
    }

    if let Some(issuer) = auth.issuer.as_ref().filter(|u| !u.trim().is_empty()) {
        let broker = issuer.trim_end_matches('/');
        return Some(format!("{broker}/ads/v1"));
    }

    let _ = gateway_base;
    None
}

fn normalize_ads_base(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/ads/v1") {
        trimmed.to_string()
    } else if trimmed.ends_with("/ads") {
        format!("{trimmed}/v1")
    } else {
        format!("{trimmed}/ads/v1")
    }
}

/// Preferences for choosing one registry entry when several share the same artifact.
#[derive(Debug, Clone, Default)]
pub struct ServiceSelectionPrefs {
    pub preferred_environment: Option<String>,
    pub preferred_organization: Option<String>,
    pub preferred_service_id: Option<String>,
    pub local_base_url: Option<String>,
}

impl ServiceSelectionPrefs {
    pub fn from_discovery_config(config: &DiscoveryConfig) -> Self {
        Self {
            preferred_environment: config.preferred_environment.clone(),
            preferred_organization: config.preferred_organization.clone(),
            preferred_service_id: config.preferred_service_id.clone(),
            local_base_url: config
                .registration_base_url
                .clone()
                .or_else(|| std::env::var("FERRUM_PUBLIC_BASE_URL").ok()),
        }
    }
}

/// Score a registry entry for artifact disambiguation (higher = better match).
pub fn score_service_match(service: &RegisteredService, prefs: &ServiceSelectionPrefs) -> i32 {
    let mut score = 0;
    if let Some(ref want) = prefs.preferred_service_id {
        if service.info.id == *want {
            score += 1_000;
        }
    }
    if let Some(ref want) = prefs.preferred_environment {
        if service
            .info
            .environment
            .as_deref()
            .is_some_and(|env| env.eq_ignore_ascii_case(want))
        {
            score += 100;
        }
    }
    if let Some(ref want) = prefs.preferred_organization {
        if service.info.organization.name.eq_ignore_ascii_case(want) {
            score += 50;
        }
    }
    if let Some(ref base) = prefs.local_base_url {
        let base = base.trim_end_matches('/');
        if service.url.trim_end_matches('/').starts_with(base)
            || service
                .info
                .organization
                .url
                .trim_end_matches('/')
                .starts_with(base)
        {
            score += 25;
        }
    }
    score
}

/// Pick the best URL for an artifact from a registry listing (stable tie-break on service id).
pub fn select_service_url(
    services: &[RegisteredService],
    artifact: &str,
    prefs: &ServiceSelectionPrefs,
) -> Option<String> {
    let mut matches: Vec<&RegisteredService> = services
        .iter()
        .filter(|svc| svc.info.r#type.artifact.eq_ignore_ascii_case(artifact))
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by(|a, b| {
        let sa = score_service_match(a, prefs);
        let sb = score_service_match(b, prefs);
        sb.cmp(&sa).then_with(|| a.info.id.cmp(&b.info.id))
    });
    matches.first().map(|svc| svc.url.clone())
}

/// Resolve DRS URL for services under the same organization as a federated ADS origin.
pub fn drs_url_for_ads_origin(
    services: &[RegisteredService],
    ads_origin: &str,
    prefs: &ServiceSelectionPrefs,
) -> Option<String> {
    let ads = services.iter().find(|s| s.info.id == ads_origin)?;
    let org = &ads.info.organization.name;
    let org_drs: Vec<RegisteredService> = services
        .iter()
        .filter(|s| {
            s.info.r#type.artifact.eq_ignore_ascii_case(ARTIFACT_DRS)
                && s.info.organization.name == *org
        })
        .cloned()
        .collect();
    select_service_url(&org_drs, ARTIFACT_DRS, prefs)
}

/// Resolve WES URL for services under the same organization as a federated ADS origin.
pub fn wes_url_for_ads_origin(
    services: &[RegisteredService],
    ads_origin: &str,
    prefs: &ServiceSelectionPrefs,
) -> Option<String> {
    let ads = services.iter().find(|s| s.info.id == ads_origin)?;
    let org = &ads.info.organization.name;
    let org_wes: Vec<RegisteredService> = services
        .iter()
        .filter(|s| {
            s.info.r#type.artifact.eq_ignore_ascii_case(ARTIFACT_WES)
                && s.info.organization.name == *org
        })
        .cloned()
        .collect();
    select_service_url(&org_wes, ARTIFACT_WES, prefs)
}

const LIST_TTL: Duration = Duration::from_secs(60);

struct CachedListing {
    services: Vec<RegisteredService>,
    fetched_at: Instant,
}

fn listing_cache() -> &'static RwLock<HashMap<String, CachedListing>> {
    static CACHE: OnceLock<RwLock<HashMap<String, CachedListing>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

async fn listing_from_cache(registry_url: &str) -> Option<Vec<RegisteredService>> {
    let guard = listing_cache().read().await;
    guard
        .get(registry_url)
        .and_then(|c| (c.fetched_at.elapsed() < LIST_TTL).then(|| c.services.clone()))
}

async fn store_listing(registry_url: &str, services: Vec<RegisteredService>) {
    listing_cache().write().await.insert(
        registry_url.to_string(),
        CachedListing {
            services,
            fetched_at: Instant::now(),
        },
    );
}

async fn invalidate_listing(registry_url: &str) {
    listing_cache().write().await.remove(registry_url);
}

/// Client for GA4GH Service Registry read/write APIs.
#[derive(Clone)]
pub struct ServiceRegistryClient {
    http: Client,
    registry_url: String,
    registration_key: Option<String>,
    fallback: HashMap<String, String>,
    selection: ServiceSelectionPrefs,
}

impl ServiceRegistryClient {
    /// Build a client from Ferrum discovery configuration.
    pub fn from_config(config: &DiscoveryConfig) -> Result<Self, DiscoveryError> {
        let registry_url = config
            .service_registry_url
            .as_ref()
            .map(|url| url.trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .ok_or(DiscoveryError::MissingRegistryUrl)?;

        let registration_key = if config.auto_register {
            Some(
                config
                    .registration_api_key()
                    .map_err(|_| DiscoveryError::MissingApiKey)?,
            )
        } else {
            config.registration_api_key().ok()
        };

        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(DiscoveryError::Http)?,
            registry_url,
            registration_key,
            fallback: config.fallback_urls.clone(),
            selection: ServiceSelectionPrefs::from_discovery_config(config),
        })
    }

    /// Register or update a GA4GH service in the registry.
    pub async fn register(&self, service: &RegisteredService) -> Result<(), DiscoveryError> {
        let key = self
            .registration_key
            .as_deref()
            .ok_or(DiscoveryError::MissingApiKey)?;

        let response = self
            .http
            .post(format!("{}/services", self.registry_url))
            .header("X-API-Key", key)
            .json(service)
            .send()
            .await?;

        if response.status().is_success() {
            invalidate_listing(&self.registry_url).await;
            tracing::info!(
                service_id = %service.info.id,
                url = %service.url,
                "registered service in GA4GH service registry"
            );
            return Ok(());
        }

        Err(DiscoveryError::RegistryHttp {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        })
    }

    /// Resolve a service URL by GA4GH artifact name (e.g. `drs`, `beacon`).
    pub async fn resolve_artifact(&self, artifact: &str) -> Option<String> {
        match self.list().await {
            Ok(services) => select_service_url(&services, artifact, &self.selection)
                .or_else(|| self.fallback.get(artifact).cloned()),
            Err(err) => {
                tracing::warn!(
                    artifact,
                    error = %err,
                    "service registry lookup failed; using fallback URL if configured"
                );
                self.fallback.get(artifact).cloned()
            }
        }
    }

    /// All registry URLs for an artifact (for federation / catalog harvest).
    pub async fn list_artifact_urls(&self, artifact: &str) -> Vec<(String, String)> {
        match self.list().await {
            Ok(services) => services
                .into_iter()
                .filter(|svc| svc.info.r#type.artifact.eq_ignore_ascii_case(artifact))
                .map(|svc| (svc.info.id.clone(), svc.url.clone()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Preload the in-memory cache from the registry (best-effort).
    pub async fn warm_cache(&self) {
        let _ = self.list().await;
    }

    /// List all registered services (cached for `LIST_TTL`).
    pub async fn list(&self) -> Result<Vec<RegisteredService>, DiscoveryError> {
        if let Some(services) = listing_from_cache(&self.registry_url).await {
            return Ok(services);
        }
        let response = self
            .http
            .get(format!("{}/services", self.registry_url))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(DiscoveryError::RegistryHttp {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }

        let services = response
            .json::<Vec<RegisteredService>>()
            .await
            .map_err(|err| DiscoveryError::InvalidResponse(err.to_string()))?;
        store_listing(&self.registry_url, services.clone()).await;
        Ok(services)
    }

    /// Remove one service this process registered. A non-success status is an error.
    pub async fn deregister(&self, id: &str) -> Result<(), DiscoveryError> {
        let key = self
            .registration_key
            .as_deref()
            .ok_or(DiscoveryError::MissingApiKey)?;
        let response = self
            .http
            .delete(format!("{}/services/{id}", self.registry_url))
            .header("X-API-Key", key)
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        if response.status().is_success() {
            invalidate_listing(&self.registry_url).await;
            return Ok(());
        }
        Err(DiscoveryError::RegistryHttp {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        })
    }
}

/// Register all enabled Ferrum GA4GH services with the service registry.
pub async fn register_ferrum_services(
    client: &ServiceRegistryClient,
    gateway_base: &str,
    services: &ferrum_core::ServicesConfig,
    environment: &str,
) -> Result<(), DiscoveryError> {
    let base = gateway_base.trim_end_matches('/');

    let org = ServiceOrganization {
        name: "Ferrum".to_string(),
        url: base.to_string(),
        contact_url: None,
    };

    let mut registrations = Vec::new();

    if services.enable_drs {
        registrations.push(build_service(
            "org.synapticfour.ferrum.drs",
            "Ferrum DRS",
            "drsservice",
            "1.3.0",
            &org,
            format!("{base}/ga4gh/drs/v1"),
            environment,
        ));
    }
    if services.enable_beacon {
        registrations.push(build_service(
            "org.synapticfour.ferrum.beacon",
            "Ferrum Beacon",
            "beacon",
            "2.0",
            &org,
            format!("{base}/ga4gh/beacon/v2"),
            environment,
        ));
    }
    if services.enable_htsget {
        registrations.push(build_service(
            "org.synapticfour.ferrum.htsget",
            "Ferrum htsget",
            "htsget",
            "1.3",
            &org,
            format!("{base}/ga4gh/htsget/v1"),
            environment,
        ));
    }
    if services.enable_wes {
        registrations.push(build_service(
            "org.synapticfour.ferrum.wes",
            "Ferrum WES",
            "wes",
            "1.1.0",
            &org,
            format!("{base}/ga4gh/wes/v1"),
            environment,
        ));
    }
    if services.enable_tes {
        registrations.push(build_service(
            "org.synapticfour.ferrum.tes",
            "Ferrum TES",
            "tes",
            "1.1.0",
            &org,
            format!("{base}/ga4gh/tes/v1"),
            environment,
        ));
    }
    if services.enable_trs {
        registrations.push(build_service(
            "org.synapticfour.ferrum.trs",
            "Ferrum TRS",
            "tool-registry",
            "2.0.2",
            &org,
            format!("{base}/ga4gh/trs/v2"),
            environment,
        ));
    }

    for service in registrations {
        client.register(&service).await?;
    }

    Ok(())
}

/// Service ids `register_ferrum_services` would POST for this process.
///
/// A service that is off in this config is omitted, including a row left by an older config.
pub fn enabled_registration_ids(services: &ferrum_core::ServicesConfig) -> Vec<String> {
    let mut ids = Vec::new();
    if services.enable_drs {
        ids.push("org.synapticfour.ferrum.drs".to_string());
    }
    if services.enable_beacon {
        ids.push("org.synapticfour.ferrum.beacon".to_string());
    }
    if services.enable_htsget {
        ids.push("org.synapticfour.ferrum.htsget".to_string());
    }
    if services.enable_wes {
        ids.push("org.synapticfour.ferrum.wes".to_string());
    }
    if services.enable_tes {
        ids.push("org.synapticfour.ferrum.tes".to_string());
    }
    if services.enable_trs {
        ids.push("org.synapticfour.ferrum.trs".to_string());
    }
    ids
}

fn build_service(
    id: &str,
    name: &str,
    artifact: &str,
    version: &str,
    org: &ServiceOrganization,
    url: String,
    environment: &str,
) -> RegisteredService {
    RegisteredService {
        info: ServiceInfo {
            id: id.to_string(),
            name: name.to_string(),
            r#type: ServiceType {
                group: "org.ga4gh".to_string(),
                artifact: artifact.to_string(),
                version: version.to_string(),
            },
            organization: org.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: Some(format!("Ferrum {name}")),
            documentation_url: None,
            created_at: None,
            updated_at: None,
            environment: Some(environment.to_string()),
        },
        url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn registers_and_lists_services() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/services"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![RegisteredService {
                    info: ServiceInfo {
                        id: "org.example.drs".to_string(),
                        name: "Example DRS".to_string(),
                        r#type: ServiceType {
                            group: "org.ga4gh".to_string(),
                            artifact: "drsservice".to_string(),
                            version: "1.3.0".to_string(),
                        },
                        organization: ServiceOrganization {
                            name: "Example".to_string(),
                            url: "https://example.org".to_string(),
                            contact_url: None,
                        },
                        version: "0.1.0".to_string(),
                        description: None,
                        documentation_url: None,
                        created_at: None,
                        updated_at: None,
                        environment: Some("test".to_string()),
                    },
                    url: "https://example.org/ga4gh/drs/v1".to_string(),
                }]),
            )
            .mount(&server)
            .await;

        let config = DiscoveryConfig {
            enabled: true,
            service_registry_url: Some(server.uri()),
            registration_api_key_env: "TEST_REGISTRY_KEY".to_string(),
            auto_register: true,
            registration_base_url: None,
            fallback_urls: HashMap::new(),
            preferred_environment: None,
            preferred_organization: None,
            preferred_service_id: None,
            heartbeat_interval_secs: 300,
        };
        std::env::set_var("TEST_REGISTRY_KEY", "secret");
        let client = ServiceRegistryClient::from_config(&config).expect("client");

        let service = build_service(
            "org.test.drs",
            "Test DRS",
            "drsservice",
            "1.3.0",
            &ServiceOrganization {
                name: "Test".to_string(),
                url: "https://test".to_string(),
                contact_url: None,
            },
            "https://test/ga4gh/drs/v1".to_string(),
            "test",
        );
        client.register(&service).await.expect("register");

        let url = client.resolve_artifact("drsservice").await;
        assert_eq!(url.as_deref(), Some("https://example.org/ga4gh/drs/v1"));
        std::env::remove_var("TEST_REGISTRY_KEY");
    }

    #[tokio::test]
    async fn resolve_artifact_reuses_cached_listing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![RegisteredService {
                    info: ServiceInfo {
                        id: "org.example.drs".to_string(),
                        name: "Example DRS".to_string(),
                        r#type: ServiceType {
                            group: "org.ga4gh".to_string(),
                            artifact: "drsservice".to_string(),
                            version: "1.3.0".to_string(),
                        },
                        organization: ServiceOrganization {
                            name: "Example".to_string(),
                            url: "https://example.org".to_string(),
                            contact_url: None,
                        },
                        version: "0.1.0".to_string(),
                        description: None,
                        documentation_url: None,
                        created_at: None,
                        updated_at: None,
                        environment: Some("test".to_string()),
                    },
                    url: "https://example.org/ga4gh/drs/v1".to_string(),
                }]),
            )
            .expect(1)
            .mount(&server)
            .await;

        let config = DiscoveryConfig {
            enabled: true,
            service_registry_url: Some(server.uri()),
            registration_api_key_env: "UNUSED_REGISTRY_KEY".to_string(),
            auto_register: false,
            registration_base_url: None,
            fallback_urls: HashMap::new(),
            preferred_environment: None,
            preferred_organization: None,
            preferred_service_id: None,
            heartbeat_interval_secs: 300,
        };
        let client = ServiceRegistryClient::from_config(&config).expect("client");
        let url1 = client.resolve_artifact("drsservice").await;
        let client2 = ServiceRegistryClient::from_config(&config).expect("client");
        let url2 = client2.resolve_artifact("drsservice").await;
        assert_eq!(url1.as_deref(), Some("https://example.org/ga4gh/drs/v1"));
        assert_eq!(url2, url1);
    }

    #[test]
    fn select_service_prefers_configured_environment_and_id() {
        let org_a = ServiceOrganization {
            name: "Institute A".to_string(),
            url: "https://a.example.org".to_string(),
            contact_url: None,
        };
        let org_b = ServiceOrganization {
            name: "Institute B".to_string(),
            url: "https://b.example.org".to_string(),
            contact_url: None,
        };
        let services = vec![
            build_service(
                "org.b.tes",
                "B TES",
                "tes",
                "1.1",
                &org_b,
                "https://b.example.org/ga4gh/tes/v1".to_string(),
                "production",
            ),
            build_service(
                "org.a.tes",
                "A TES",
                "tes",
                "1.1",
                &org_a,
                "https://a.example.org/ga4gh/tes/v1".to_string(),
                "staging",
            ),
            build_service(
                "org.local.tes",
                "Local TES",
                "tes",
                "1.1",
                &org_a,
                "https://local.example.org/ga4gh/tes/v1".to_string(),
                "production",
            ),
        ];
        let prefs = ServiceSelectionPrefs {
            preferred_environment: Some("production".to_string()),
            preferred_service_id: Some("org.local.tes".to_string()),
            preferred_organization: None,
            local_base_url: Some("https://local.example.org".to_string()),
        };
        let url = select_service_url(&services, "tes", &prefs).unwrap();
        assert_eq!(url, "https://local.example.org/ga4gh/tes/v1");
    }

    #[test]
    fn drs_url_for_ads_origin_picks_same_org() {
        let org_a = ServiceOrganization {
            name: "Institute A".to_string(),
            url: "https://a.example.org".to_string(),
            contact_url: None,
        };
        let org_b = ServiceOrganization {
            name: "Institute B".to_string(),
            url: "https://b.example.org".to_string(),
            contact_url: None,
        };
        let services = vec![
            build_service(
                "org.a.ads",
                "A ADS",
                "access-decision-service",
                "1.0",
                &org_a,
                "https://a.example.org/ads/v1".to_string(),
                "production",
            ),
            build_service(
                "org.a.drs",
                "A DRS",
                "drsservice",
                "1.3",
                &org_a,
                "https://a.example.org/ga4gh/drs/v1".to_string(),
                "production",
            ),
            build_service(
                "org.b.drs",
                "B DRS",
                "drsservice",
                "1.3",
                &org_b,
                "https://b.example.org/ga4gh/drs/v1".to_string(),
                "production",
            ),
        ];
        let prefs = ServiceSelectionPrefs::default();
        let url = drs_url_for_ads_origin(&services, "org.a.ads", &prefs).unwrap();
        assert_eq!(url, "https://a.example.org/ga4gh/drs/v1");
    }

    #[test]
    fn disabled_service_is_omitted_from_deregister_ids() {
        let services = ferrum_core::ServicesConfig {
            enable_drs: true,
            enable_htsget: false,
            ..ferrum_core::ServicesConfig::default()
        };
        let ids = enabled_registration_ids(&services);
        assert!(ids.iter().any(|id| id == "org.synapticfour.ferrum.drs"));
        assert!(ids.iter().all(|id| !id.contains("htsget")));
        assert_eq!(
            ferrum_core::config::DiscoveryConfig::default().heartbeat_interval_secs,
            300
        );
    }

    #[tokio::test]
    async fn heartbeat_reposts_move_updated_at_on_a_stale_registry() {
        let rows = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
            String,
            i64,
        >::new()));
        let server = MockServer::start().await;
        Mock::given(path("/services"))
            .respond_with(LiveRegistry {
                stale_after: 10,
                rows: std::sync::Arc::clone(&rows),
            })
            .mount(&server)
            .await;

        std::env::set_var("FERRUM_HEARTBEAT_TEST_KEY", "secret");
        let config = ferrum_core::config::DiscoveryConfig {
            service_registry_url: Some(server.uri()),
            auto_register: true,
            registration_api_key_env: "FERRUM_HEARTBEAT_TEST_KEY".to_string(),
            ..ferrum_core::config::DiscoveryConfig::default()
        };
        let client = ServiceRegistryClient::from_config(&config).expect("client");
        let services = ferrum_core::ServicesConfig {
            enable_drs: true,
            enable_beacon: false,
            enable_htsget: false,
            enable_wes: false,
            enable_tes: false,
            enable_trs: false,
            ..ferrum_core::ServicesConfig::default()
        };
        register_ferrum_services(&client, "http://ferrum.example", &services, "development")
            .await
            .expect("first post");
        let first = *rows
            .lock()
            .expect("rows")
            .get("org.synapticfour.ferrum.drs")
            .expect("drs row");
        let start = unix_now();
        while unix_now() == start {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        register_ferrum_services(&client, "http://ferrum.example", &services, "development")
            .await
            .expect("heartbeat post");
        let second = *rows
            .lock()
            .expect("rows")
            .get("org.synapticfour.ferrum.drs")
            .expect("drs row");
        assert!(second > first, "re-POST moves updatedAt");

        let listed: Vec<serde_json::Value> = client
            .http
            .get(format!("{}/services", server.uri()))
            .send()
            .await
            .expect("get")
            .json()
            .await
            .expect("json");
        let row = listed
            .iter()
            .find(|row| row["id"] == "org.synapticfour.ferrum.drs")
            .expect("listed");
        assert_eq!(row["stale"], false);
        assert_ne!(row["updatedAt"], rfc3339(first));

        rows.lock()
            .expect("rows")
            .insert("org.synapticfour.ferrum.drs".to_string(), unix_now() - 30);
        let stale_listed: Vec<serde_json::Value> = client
            .http
            .get(format!("{}/services", server.uri()))
            .send()
            .await
            .expect("get")
            .json()
            .await
            .expect("json");
        let stale_row = stale_listed
            .iter()
            .find(|row| row["id"] == "org.synapticfour.ferrum.drs")
            .expect("listed");
        assert_eq!(stale_row["stale"], true);
    }

    #[tokio::test]
    async fn deregister_deletes_only_the_named_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/services/org.synapticfour.ferrum.drs"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        std::env::set_var("FERRUM_HEARTBEAT_TEST_KEY", "secret");
        let config = ferrum_core::config::DiscoveryConfig {
            service_registry_url: Some(server.uri()),
            auto_register: true,
            registration_api_key_env: "FERRUM_HEARTBEAT_TEST_KEY".to_string(),
            ..ferrum_core::config::DiscoveryConfig::default()
        };
        let client = ServiceRegistryClient::from_config(&config).expect("client");
        client
            .deregister("org.synapticfour.ferrum.drs")
            .await
            .expect("delete");
    }

    fn unix_now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0)
    }

    fn rfc3339(unix: i64) -> String {
        let days = unix.div_euclid(86_400);
        let tod = unix.rem_euclid(86_400) as u32;
        let hour = tod / 3600;
        let min = (tod % 3600) / 60;
        let sec = tod % 60;
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let mut y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        if m <= 2 {
            y += 1;
        }
        format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
    }

    struct LiveRegistry {
        stale_after: i64,
        rows: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, i64>>>,
    }

    impl wiremock::Respond for LiveRegistry {
        fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
            let now = unix_now();
            if request.method.as_str() == "GET" {
                let rows = self.rows.lock().expect("rows");
                let body: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|(id, updated)| {
                        serde_json::json!({
                            "id": id,
                            "updatedAt": rfc3339(*updated),
                            "stale": now.saturating_sub(*updated) > self.stale_after,
                        })
                    })
                    .collect();
                return wiremock::ResponseTemplate::new(200).set_body_json(body);
            }
            if request.method.as_str() == "POST" {
                let value: serde_json::Value =
                    serde_json::from_slice(&request.body).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(id) = value.get("id").and_then(|item| item.as_str()) {
                    self.rows.lock().expect("rows").insert(id.to_string(), now);
                }
                return wiremock::ResponseTemplate::new(204);
            }
            wiremock::ResponseTemplate::new(404)
        }
    }
}
