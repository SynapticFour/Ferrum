//! GA4GH common types: Checksum, AccessMethod, AccessType.

use serde::{Deserialize, Serialize};

/// GA4GH Checksum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Checksum {
    pub checksum: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

/// Access type for DRS AccessMethod (GA4GH standard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum AccessType {
    S3,
    Gs,
    Ftp,
    Gsiftp,
    Globus,
    Htsget,
    Https,
    File,
}

impl std::fmt::Display for AccessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessType::S3 => write!(f, "s3"),
            AccessType::Gs => write!(f, "gs"),
            AccessType::Ftp => write!(f, "ftp"),
            AccessType::Gsiftp => write!(f, "gsiftp"),
            AccessType::Globus => write!(f, "globus"),
            AccessType::Htsget => write!(f, "htsget"),
            AccessType::Https => write!(f, "https"),
            AccessType::File => write!(f, "file"),
        }
    }
}

/// GA4GH AccessMethod.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AccessMethod {
    #[serde(rename = "type")]
    pub access_type: AccessType,
    pub access_url: Option<AccessUrl>,
    pub access_id: Option<String>,
    pub region: Option<String>,
}

/// DRS Access URL (object or string).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum AccessUrl {
    String(String),
    Object(serde_json::Map<String, serde_json::Value>),
}

/// Minimal DRS Object (for compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DrsObject {
    pub id: String,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub created_time: Option<String>,
    pub updated_time: Option<String>,
    pub checksums: Option<Vec<Checksum>>,
    pub access_methods: Option<Vec<AccessMethod>>,
    pub description: Option<String>,
}

/// GA4GH Service info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub service_type: ServiceType,
    pub description: Option<String>,
    pub organization: Option<Organization>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceType {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Organization {
    pub name: String,
    pub url: Option<String>,
}
