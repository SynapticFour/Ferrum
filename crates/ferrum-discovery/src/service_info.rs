// SPDX-License-Identifier: Apache-2.0
// Vendored from ga4gh-infra `ga4gh-types` (ServiceInfo / ServiceOrganization / ServiceType only)
// so Ferrum builds without a sibling checkout. Original: Apache-2.0.

//! GA4GH Service Info types (v1.0.0).

use serde::{Deserialize, Serialize};

/// GA4GH service type descriptor (`group`, `artifact`, `version`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceType {
    /// Namespace in reverse domain name format (e.g. `org.ga4gh`).
    pub group: String,
    /// Name of the API or GA4GH specification implemented.
    pub artifact: String,
    /// Version of the API or specification.
    pub version: String,
}

/// Organization providing a GA4GH service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceOrganization {
    /// Name of the organization responsible for the service.
    pub name: String,
    /// URL of the organization's website.
    pub url: String,
    /// Contact URL or mailto link for the service provider.
    #[serde(rename = "contactUrl", skip_serializing_if = "Option::is_none")]
    pub contact_url: Option<String>,
}

/// GA4GH `/service-info` response object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Unique service identifier (reverse domain name notation recommended).
    pub id: String,
    /// Human-readable service name.
    pub name: String,
    /// Service type descriptor.
    pub r#type: ServiceType,
    /// Organization providing the service.
    pub organization: ServiceOrganization,
    /// Service version string.
    pub version: String,
    /// Human-readable service description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// URL of service documentation.
    #[serde(rename = "documentationUrl", skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    /// Timestamp when the service was first deployed (RFC 3339).
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Timestamp when the service was last updated (RFC 3339).
    #[serde(rename = "updatedAt", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Deployment environment (e.g. `prod`, `test`, `dev`, `staging`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}
