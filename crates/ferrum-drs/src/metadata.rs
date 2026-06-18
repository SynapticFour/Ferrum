//! ferrum-meta bundle validation and persistence at ingest.

use crate::error::DrsError;
use crate::repo::DrsRepo;
use ferrum_meta_connect::{detect_profile, submission_alias, validate_submission, MetaProfile};
use serde_json::Value;

pub struct StoredMetadata {
    pub metadata_ref: String,
    pub profile: MetaProfile,
}

pub fn validate_ferrum_meta_bundle(
    bundle: &Value,
    profile: Option<MetaProfile>,
) -> Result<(MetaProfile, ferrum_meta_connect::MetaValidationReport), String> {
    let profile = profile.unwrap_or_else(|| detect_profile(bundle));
    let report = validate_submission(bundle, Some(profile));
    if !report.valid {
        return Err(format!(
            "ferrum-meta validation failed with {} error(s)",
            report.error_count()
        ));
    }
    Ok((profile, report))
}

pub async fn store_ferrum_meta_bundle(
    repo: &DrsRepo,
    bundle: &Value,
    profile: Option<MetaProfile>,
) -> Result<StoredMetadata, DrsError> {
    let (profile, _report) =
        validate_ferrum_meta_bundle(bundle, profile).map_err(DrsError::Validation)?;
    let alias = submission_alias(bundle)
        .ok_or_else(|| DrsError::Validation("ferrum-meta submission has no alias".into()))?;
    let document = serde_json::to_string(bundle)
        .map_err(|e| DrsError::Validation(format!("serialize ferrum-meta: {e}")))?;
    repo.upsert_metadata_submission(&alias, profile.as_str(), &document)
        .await?;
    Ok(StoredMetadata {
        metadata_ref: alias,
        profile,
    })
}

pub async fn link_object_metadata_ref(
    repo: &DrsRepo,
    object_id: &str,
    metadata_ref: &str,
) -> Result<(), DrsError> {
    repo.set_object_metadata_ref(object_id, metadata_ref).await
}

pub fn provenance_destination(
    metadata_ref: Option<&str>,
    collector: Option<&str>,
    collected_at: Option<&str>,
    location_label: Option<&str>,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> String {
    serde_json::json!({
        "metadata_ref": metadata_ref,
        "collector": collector,
        "collected_at": collected_at,
        "location_label": location_label,
        "latitude": latitude,
        "longitude": longitude,
    })
    .to_string()
}
