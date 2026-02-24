//! DRS 1.4 API types (GA4GH schema).

use ferrum_core::{AccessMethod, Checksum};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// DRS Object (GA4GH DRS 1.4). Required: id, self_uri, size, created_time, checksums.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DrsObject {
    pub id: String,
    pub self_uri: String,
    pub size: i64,
    pub created_time: String,
    pub checksums: Vec<Checksum>,
    pub name: Option<String>,
    pub updated_time: Option<String>,
    pub version: Option<String>,
    pub mime_type: Option<String>,
    pub access_methods: Option<Vec<AccessMethod>>,
    pub contents: Option<Vec<ContentsObject>>,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

/// Contents of a bundle (nested or top-level).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentsObject {
    pub name: String,
    pub id: Option<String>,
    pub drs_uri: Option<Vec<String>>,
    pub contents: Option<Vec<ContentsObject>>,
}

/// Access URL response (GET /objects/{id}/access/{access_id}).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccessUrl {
    pub url: String,
    pub headers: Option<Vec<String>>,
}

/// List objects query (admin).
#[derive(Debug, Clone, Default, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListObjectsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub mime_type: Option<String>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
}

/// Create object request (admin POST /objects).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateObjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size: i64,
    pub checksums: Vec<ChecksumInput>,
    pub aliases: Option<Vec<String>>,
    pub storage_backend: String,
    pub storage_key: String,
    pub is_encrypted: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ChecksumInput {
    pub r#type: String,
    pub checksum: String,
}

/// Update object request (admin PUT /objects/{id}).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateObjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    pub checksums: Option<Vec<ChecksumInput>>,
    pub aliases: Option<Vec<String>>,
}

/// Ingest URL request (POST /ingest/url).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct IngestUrlRequest {
    pub url: String,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub aliases: Option<Vec<String>>,
}

/// Ingest batch request (POST /ingest/batch).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct IngestBatchRequest {
    pub items: Vec<IngestBatchItem>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IngestBatchItem {
    Url { url: String, name: Option<String>, mime_type: Option<String> },
    Path { path: String, name: Option<String> },
}
