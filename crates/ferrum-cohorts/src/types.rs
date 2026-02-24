//! Request/response types for Cohort API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_sub: String,
    pub workspace_id: Option<String>,
    pub version: i32,
    pub is_frozen: bool,
    pub sample_count: i32,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_sub: String,
    pub workspace_id: Option<String>,
    pub version: i32,
    pub is_frozen: bool,
    pub sample_count: i32,
    pub tags: Vec<String>,
    pub filter_criteria: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCohortRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub filter_criteria: serde_json::Value,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateCohortRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub filter_criteria: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortSample {
    pub id: String,
    pub cohort_id: String,
    pub sample_id: String,
    pub drs_object_ids: Vec<String>,
    pub phenotype: serde_json::Value,
    pub added_at: DateTime<Utc>,
    pub added_by: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AddSampleRequest {
    pub sample_id: String,
    #[serde(default)]
    pub drs_object_ids: Vec<String>,
    #[serde(default)]
    pub phenotype: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AddSamplesBatchRequest {
    pub samples: Vec<AddSampleRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PhenotypeSchemaField {
    pub id: String,
    pub field_name: String,
    pub display_name: String,
    pub field_type: String,
    pub ontology: Option<String>,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryResult {
    pub matched_sample_ids: Vec<String>,
    pub total_count: usize,
    pub facets: QueryFacets,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct QueryFacets {
    #[serde(default)]
    pub by_sex: std::collections::HashMap<String, usize>,
    #[serde(default)]
    pub by_diagnosis: std::collections::HashMap<String, usize>,
    #[serde(default)]
    pub by_sequencing_type: std::collections::HashMap<String, usize>,
    #[serde(default)]
    pub by_data_type: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortStats {
    pub cohort_id: String,
    pub sample_count: usize,
    pub total_data_size_bytes: i64,
    pub data_type_breakdown: std::collections::HashMap<String, DataTypeStat>,
    pub phenotype_completeness: std::collections::HashMap<String, f64>,
    pub sex_distribution: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DataTypeStat {
    pub count: usize,
    pub total_size: i64,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CohortVersionInfo {
    pub id: String,
    pub cohort_id: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub note: Option<String>,
}
