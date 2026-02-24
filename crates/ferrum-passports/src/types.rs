//! GA4GH Passport and Visa claim structures (passport_v1).

use serde::{Deserialize, Serialize};

/// Visa object inside a Visa JWT (ga4gh_visa_v1 claim).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisaObject {
    pub r#type: String, // AffiliationAndRole | AcceptedTermsAndPolicies | ResearcherStatus | ControlledAccessGrants | LinkedIdentities
    pub asserted: i64,
    pub value: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<serde_json::Value>,
}

/// Row from passport_visa_grants for building visas.
#[derive(Debug, Clone)]
pub struct VisaGrantRow {
    pub id: uuid::Uuid,
    pub user_sub: String,
    pub user_iss: String,
    pub dataset_id: String,
    pub visa_type: String,
    pub value: String,
    pub source: String,
    pub conditions: Option<serde_json::Value>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
