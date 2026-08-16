// SPDX-License-Identifier: BUSL-1.1
//! Profile-specific validation beyond ferrum-core structural checks.

use crate::{issue, IssueSeverity, MetaValidationReport, ValidationIssue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaProfile {
    Core,
    Pathogen,
    H3Africa,
}

impl MetaProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            MetaProfile::Core => "core",
            MetaProfile::Pathogen => "pathogen",
            MetaProfile::H3Africa => "h3africa",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "core" | "ferrum-core" => Some(MetaProfile::Core),
            "pathogen" | "pathogen_surveillance" => Some(MetaProfile::Pathogen),
            "h3africa" | "h3_africa" => Some(MetaProfile::H3Africa),
            _ => None,
        }
    }
}

/// Infer profile from study `type` when not explicitly specified.
pub fn detect_profile(root: &serde_json::Value) -> MetaProfile {
    let study_type = root
        .get("studies")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    match study_type.as_str() {
        "PATHOGEN_SURVEILLANCE" | "OUTBREAK_RESPONSE" | "PATHOGEN_GENOMICS" => {
            MetaProfile::Pathogen
        }
        "H3AFRICA" => MetaProfile::H3Africa,
        _ => MetaProfile::Core,
    }
}

/// Primary alias used as `metadata_ref` on DRS objects (dataset preferred).
pub fn submission_alias(root: &serde_json::Value) -> Option<String> {
    root.get("datasets")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|d| d.get("alias"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            root.get("studies")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|s| s.get("alias"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
}

fn require_str(
    obj: &serde_json::Value,
    path: &str,
    key: &str,
    issues: &mut Vec<ValidationIssue>,
) -> bool {
    match obj.get(key).and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => true,
        _ => {
            issues.push(issue(
                IssueSeverity::Error,
                format!("{path}.{key}"),
                format!("required non-empty string field `{key}`"),
            ));
            false
        }
    }
}

fn validate_duo_codes(study: &serde_json::Value, path: &str, issues: &mut Vec<ValidationIssue>) {
    match study.get("data_use_conditions").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => {
            for (i, code) in arr.iter().enumerate() {
                let Some(s) = code.as_str() else {
                    issues.push(issue(
                        IssueSeverity::Error,
                        format!("{path}.data_use_conditions[{i}]"),
                        "DUO code must be a string",
                    ));
                    continue;
                };
                if !s.starts_with("DUO:") {
                    issues.push(issue(
                        IssueSeverity::Error,
                        format!("{path}.data_use_conditions[{i}]"),
                        format!("expected DUO code prefix, got `{s}`"),
                    ));
                }
            }
        }
        _ => {
            issues.push(issue(
                IssueSeverity::Error,
                format!("{path}.data_use_conditions"),
                "at least one DUO data_use_conditions entry is required",
            ));
        }
    }
}

pub fn apply_profile_rules(
    root: &serde_json::Value,
    profile: MetaProfile,
    issues: &mut Vec<ValidationIssue>,
) {
    match profile {
        MetaProfile::Core => {}
        MetaProfile::Pathogen => validate_pathogen_rules(root, issues),
        MetaProfile::H3Africa => validate_h3africa_rules(root, issues),
    }
}

fn validate_pathogen_rules(root: &serde_json::Value, issues: &mut Vec<ValidationIssue>) {
    if let Some(studies) = root.get("studies").and_then(|v| v.as_array()) {
        for (i, study) in studies.iter().enumerate() {
            let path = format!("studies[{i}]");
            let study_type = study
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            if study_type != "PATHOGEN_SURVEILLANCE"
                && study_type != "OUTBREAK_RESPONSE"
                && study_type != "PATHOGEN_GENOMICS"
            {
                issues.push(issue(
                    IssueSeverity::Error,
                    format!("{path}.type"),
                    "pathogen profile requires type PATHOGEN_SURVEILLANCE, OUTBREAK_RESPONSE, or PATHOGEN_GENOMICS",
                ));
            }
            validate_duo_codes(study, &path, issues);
        }
    }

    if let Some(samples) = root.get("samples").and_then(|v| v.as_array()) {
        for (i, sample) in samples.iter().enumerate() {
            let path = format!("samples[{i}]");
            require_str(sample, &path, "collection_date", issues);
            // LinkML PathogenSample requires collection_country (ISO-3166 alpha-2).
            // Historical Ferrum fixtures used collection_site; accept either.
            let has_country = sample
                .get("collection_country")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty());
            let has_site = sample
                .get("collection_site")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty());
            if !has_country && !has_site {
                issues.push(issue(
                    IssueSeverity::Error,
                    format!("{path}.collection_country"),
                    "pathogen profile requires collection_country (or legacy collection_site)",
                ));
            }
        }
    }

    if let Some(experiments) = root.get("experiments").and_then(|v| v.as_array()) {
        for (i, exp) in experiments.iter().enumerate() {
            let path = format!("experiments[{i}]");
            if exp
                .get("pathogen_organism")
                .and_then(|v| v.as_str())
                .is_none()
            {
                issues.push(issue(
                    IssueSeverity::Warning,
                    format!("{path}.pathogen_organism"),
                    "pathogen profile recommends pathogen_organism on experiments",
                ));
            }
        }
    }
}

fn validate_h3africa_rules(root: &serde_json::Value, issues: &mut Vec<ValidationIssue>) {
    if let Some(studies) = root.get("studies").and_then(|v| v.as_array()) {
        for (i, study) in studies.iter().enumerate() {
            let path = format!("studies[{i}]");
            let study_type = study
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            if study_type != "H3AFRICA" {
                issues.push(issue(
                    IssueSeverity::Error,
                    format!("{path}.type"),
                    "H3Africa profile requires type H3AFRICA",
                ));
            }
            validate_duo_codes(study, &path, issues);
        }
    }

    if let Some(individuals) = root.get("individuals").and_then(|v| v.as_array()) {
        for (i, ind) in individuals.iter().enumerate() {
            require_str(ind, &format!("individuals[{i}]"), "consent_type", issues);
        }
    }

    if let Some(samples) = root.get("samples").and_then(|v| v.as_array()) {
        for (i, sample) in samples.iter().enumerate() {
            let path = format!("samples[{i}]");
            require_str(sample, &path, "country", issues);
            require_str(sample, &path, "collection_date", issues);
        }
    }
}

/// Validate with explicit or auto-detected profile.
pub fn validate_submission(
    root: &serde_json::Value,
    profile: Option<MetaProfile>,
) -> MetaValidationReport {
    let profile = profile.unwrap_or_else(|| detect_profile(root));
    let mut report = crate::validate_core_submission(root);
    apply_profile_rules(root, profile, &mut report.issues);
    report.valid = !report
        .issues
        .iter()
        .any(|i| i.severity == IssueSeverity::Error);
    report
}

/// Parse YAML/JSON string to Value.
pub fn parse_submission_document(raw: &str, is_yaml: bool) -> Result<serde_json::Value, String> {
    if is_yaml {
        serde_yaml::from_str(raw).map_err(|e| e.to_string())
    } else {
        serde_json::from_str(raw).map_err(|e| e.to_string())
    }
}

pub fn submission_to_yaml(root: &serde_json::Value) -> Result<String, String> {
    serde_yaml::to_string(root).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FERRUM_META_VERSION;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../profiles/meta/fixtures")
            .join(name)
    }

    #[test]
    fn pathogen_fixture_passes() {
        let report =
            crate::validate_submission_file(&fixture("ferrum-pathogen-minimal-submission.yaml"))
                .expect("read");
        assert!(report.valid, "{report}");
    }

    #[test]
    fn h3africa_fixture_passes() {
        let report =
            crate::validate_submission_file(&fixture("ferrum-h3africa-minimal-submission.yaml"))
                .expect("read");
        assert!(report.valid, "{report}");
    }

    #[test]
    fn pathogen_missing_duo_fails() {
        let root = serde_json::json!({
            "ferrum_meta_version": FERRUM_META_VERSION,
            "studies": [{"alias": "s1", "title": "t", "description": "d", "type": "PATHOGEN_SURVEILLANCE"}],
            "individuals": [{"alias": "i1"}],
            "samples": [{"alias": "sa1", "individual_alias": "i1", "collection_date": "2026-01-01", "collection_site": "site"}],
            "experiments": [{"alias": "e1", "sample_alias": "sa1"}],
            "files": [{"alias": "f1"}],
            "datasets": [{"alias": "d1", "title": "t", "file_aliases": ["f1"]}]
        });
        let report = validate_submission(&root, Some(MetaProfile::Pathogen));
        assert!(!report.valid);
    }

    #[test]
    fn submission_alias_prefers_dataset() {
        let root = serde_json::json!({
            "studies": [{"alias": "study1"}],
            "datasets": [{"alias": "dataset1"}]
        });
        assert_eq!(submission_alias(&root).as_deref(), Some("dataset1"));
    }

    #[test]
    fn detect_profile_from_study_type() {
        let root = serde_json::json!({"studies": [{"type": "H3AFRICA"}]});
        assert_eq!(detect_profile(&root), MetaProfile::H3Africa);
        let pathogen = serde_json::json!({"studies": [{"type": "PATHOGEN_GENOMICS"}]});
        assert_eq!(detect_profile(&pathogen), MetaProfile::Pathogen);
    }

    #[test]
    fn pathogen_accepts_linkml_collection_country() {
        let root = serde_json::json!({
            "ferrum_meta_version": FERRUM_META_VERSION,
            "studies": [{"alias": "s1", "title": "t", "description": "d", "type": "PATHOGEN_GENOMICS", "data_use_conditions": ["DUO:0000007"]}],
            "individuals": [{"alias": "i1"}],
            "samples": [{"alias": "sa1", "individual_alias": "i1", "collection_date": "2026-01-01", "collection_country": "LR"}],
            "experiments": [{"alias": "e1", "sample_alias": "sa1"}],
            "files": [{"alias": "f1"}],
            "datasets": [{"alias": "d1", "title": "t", "file_aliases": ["f1"]}]
        });
        let report = validate_submission(&root, Some(MetaProfile::Pathogen));
        assert!(report.valid, "{report}");
    }

    #[test]
    fn duo_codes_require_prefix() {
        let root = serde_json::json!({
            "ferrum_meta_version": "0.1.0",
            "studies": [{"alias": "s1", "title": "t", "description": "d", "type": "H3AFRICA", "data_use_conditions": ["INVALID"]}],
            "individuals": [{"alias": "i1", "consent_type": "X"}],
            "samples": [{"alias": "sa1", "individual_alias": "i1", "country": "Kenya", "collection_date": "2026-01-01"}],
            "experiments": [{"alias": "e1", "sample_alias": "sa1"}],
            "files": [{"alias": "f1"}],
            "datasets": [{"alias": "d1", "title": "t", "file_aliases": ["f1"]}]
        });
        let report = validate_submission(&root, Some(MetaProfile::H3Africa));
        assert!(!report.valid);
    }
}
