// SPDX-License-Identifier: BUSL-1.1

//! GA4GH Service Registry client for Ferrum gateway integration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ferrum_core::DiscoveryConfig;
use ga4gh_types::{ServiceInfo, ServiceOrganization, ServiceType};
use reqwest::Client;
use serde::{Deserialize, Serialize};
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

/// Client for GA4GH Service Registry read/write APIs.
#[derive(Clone)]
pub struct ServiceRegistryClient {
    http: Client,
    registry_url: String,
    registration_key: Option<String>,
    cache: Arc<RwLock<HashMap<String, String>>>,
    fallback: HashMap<String, String>,
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
            cache: Arc::new(RwLock::new(HashMap::new())),
            fallback: config.fallback_urls.clone(),
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
            self.cache
                .write()
                .await
                .insert(service.info.id.clone(), service.url.clone());
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
            Ok(services) => {
                let mut cache = self.cache.write().await;
                for service in &services {
                    cache.insert(service.info.id.clone(), service.url.clone());
                }
                services
                    .into_iter()
                    .find(|svc| svc.info.r#type.artifact.eq_ignore_ascii_case(artifact))
                    .map(|svc| svc.url)
            }
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

    /// List all registered services.
    pub async fn list(&self) -> Result<Vec<RegisteredService>, DiscoveryError> {
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

        response
            .json::<Vec<RegisteredService>>()
            .await
            .map_err(|err| DiscoveryError::InvalidResponse(err.to_string()))
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
}
