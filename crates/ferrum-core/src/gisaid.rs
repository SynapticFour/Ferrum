// SPDX-License-Identifier: BUSL-1.1
//! GISAID metadata validation and required-field checks for outbreak packaging.

use crate::error::{FerrumError, Result};
use serde_json::Value;

pub const GISAID_REQUIRED_FIELDS: &[&str] = &[
    "collection_date",
    "location",
    "host",
    "submitting_lab",
    "submitting_lab_address",
    "originating_lab",
];

/// Validate GISAID metadata shape at ingest time.
pub fn validate_gisaid_metadata(value: &Value) -> Result<()> {
    let Some(obj) = value.as_object() else {
        return Err(FerrumError::ValidationError(
            "gisaid_metadata must be a JSON object".into(),
        ));
    };
    for field in GISAID_REQUIRED_FIELDS {
        let v = obj.get(*field).and_then(|x| x.as_str()).unwrap_or("");
        if v.trim().is_empty() {
            return Err(FerrumError::ValidationError(format!(
                "gisaid_metadata.{field} is required"
            )));
        }
    }
    Ok(())
}

/// Return missing required field names (empty = complete).
pub fn missing_gisaid_fields(value: Option<&Value>) -> Vec<&'static str> {
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return GISAID_REQUIRED_FIELDS.to_vec();
    };
    GISAID_REQUIRED_FIELDS
        .iter()
        .copied()
        .filter(|field| {
            obj.get(*field)
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.trim().is_empty())
        })
        .collect()
}
