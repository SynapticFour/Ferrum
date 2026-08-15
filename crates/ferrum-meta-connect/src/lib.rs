//! Offline validation for [ferrum-meta](https://github.com/SynapticFour/ferrum-meta) submissions.
//!
//! Structural subset of vendored LinkML YAML under `profiles/meta/schema/`
//! (`include_str!` in `schema_contract.rs`). Not a LinkML runtime — full
//! parity stays in the ferrum-meta Python toolchain. Refresh with
//! `scripts/sync-ferrum-meta-schemas.sh`.

mod import;
mod init;
pub mod profiles;
mod schema_contract;

pub use import::import_csv_to_submission;
pub use init::{build_init_template, InitParams};
pub use profiles::{
    detect_profile, parse_submission_document, submission_alias, submission_to_yaml,
    validate_submission, MetaProfile,
};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::Path;
use thiserror::Error;

pub const FERRUM_META_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IssueSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaValidationReport {
    pub valid: bool,
    pub ferrum_meta_version: Option<String>,
    pub issues: Vec<ValidationIssue>,
}

impl MetaValidationReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count()
    }
}

#[derive(Debug, Error)]
pub enum ValidateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

fn issue(
    severity: IssueSeverity,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationIssue {
    ValidationIssue {
        severity,
        path: path.into(),
        message: message.into(),
    }
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

fn validate_entity_aliases(
    items: &[serde_json::Value],
    section: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let path = format!("{section}[{i}]");
        if require_str(item, &path, "alias", issues) {
            if let Some(alias) = item.get("alias").and_then(|v| v.as_str()) {
                if !seen.insert(alias.to_string()) {
                    issues.push(issue(
                        IssueSeverity::Error,
                        format!("{path}.alias"),
                        format!("duplicate alias `{alias}` in {section}"),
                    ));
                }
            }
        }
    }
}

/// Validate a parsed ferrum-core submission document.
pub fn validate_core_submission(root: &serde_json::Value) -> MetaValidationReport {
    let mut issues = Vec::new();

    let version = root
        .get("ferrum_meta_version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    if version.as_deref() != Some(FERRUM_META_VERSION) {
        issues.push(issue(
            IssueSeverity::Warning,
            "ferrum_meta_version",
            format!(
                "expected {FERRUM_META_VERSION}, got {}",
                version.as_deref().unwrap_or("(missing)")
            ),
        ));
    }

    for (section, min) in [
        ("studies", 1usize),
        ("individuals", 1),
        ("samples", 1),
        ("experiments", 1),
        ("files", 1),
        ("datasets", 1),
    ] {
        match root.get(section).and_then(|v| v.as_array()) {
            Some(arr) if arr.len() >= min => validate_entity_aliases(arr, section, &mut issues),
            Some(arr) if arr.is_empty() => {
                issues.push(issue(
                    IssueSeverity::Error,
                    section,
                    format!("at least one {section} entry is required"),
                ));
            }
            Some(arr) => validate_entity_aliases(arr, section, &mut issues),
            None => {
                issues.push(issue(
                    IssueSeverity::Error,
                    section,
                    format!("required array `{section}` is missing"),
                ));
            }
        }
    }

    if let Some(studies) = root.get("studies").and_then(|v| v.as_array()) {
        for (i, study) in studies.iter().enumerate() {
            let path = format!("studies[{i}]");
            require_str(study, &path, "title", &mut issues);
            require_str(study, &path, "description", &mut issues);
            require_str(study, &path, "type", &mut issues);
        }
    }

    if let Some(samples) = root.get("samples").and_then(|v| v.as_array()) {
        for (i, sample) in samples.iter().enumerate() {
            require_str(
                sample,
                &format!("samples[{i}]"),
                "individual_alias",
                &mut issues,
            );
        }
    }

    if let Some(experiments) = root.get("experiments").and_then(|v| v.as_array()) {
        for (i, exp) in experiments.iter().enumerate() {
            require_str(
                exp,
                &format!("experiments[{i}]"),
                "sample_alias",
                &mut issues,
            );
        }
    }

    if let Some(datasets) = root.get("datasets").and_then(|v| v.as_array()) {
        for (i, ds) in datasets.iter().enumerate() {
            let path = format!("datasets[{i}]");
            require_str(ds, &path, "title", &mut issues);
            if ds.get("file_aliases").and_then(|v| v.as_array()).is_none() {
                issues.push(issue(
                    IssueSeverity::Error,
                    format!("{path}.file_aliases"),
                    "required array `file_aliases`",
                ));
            }
        }
    }

    let valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);
    debug_assert!(
        schema_contract::CORE_SCHEMA_YAML.contains(&format!("version: {FERRUM_META_VERSION}")),
        "vendored ferrum-core.yaml version must match FERRUM_META_VERSION"
    );
    debug_assert!(schema_contract::PATHOGEN_SCHEMA_YAML.contains("name: ferrum-pathogen-profile"));
    debug_assert!(schema_contract::H3AFRICA_SCHEMA_YAML.contains("name: ferrum-h3africa-profile"));
    MetaValidationReport {
        valid,
        ferrum_meta_version: version,
        issues,
    }
}

/// Read YAML or JSON from path and validate (profile auto-detected from study type).
pub fn validate_submission_file(path: &Path) -> Result<MetaValidationReport, ValidateError> {
    validate_submission_file_with_profile(path, None)
}

/// Read YAML or JSON from path and validate with an explicit profile.
pub fn validate_submission_file_with_profile(
    path: &Path,
    profile: Option<MetaProfile>,
) -> Result<MetaValidationReport, ValidateError> {
    let raw = std::fs::read_to_string(path)?;
    let is_yaml = path.extension().is_some_and(|e| e == "yaml" || e == "yml");
    let root = parse_submission_document(&raw, is_yaml).map_err(ValidateError::Parse)?;
    Ok(validate_submission(&root, profile))
}

impl Display for MetaValidationReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "ferrum-meta validation: {} (errors: {})",
            if self.valid { "PASS" } else { "FAIL" },
            self.error_count()
        )?;
        for issue in &self.issues {
            writeln!(
                f,
                "  [{:?}] {} — {}",
                issue.severity, issue.path, issue.message
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../profiles/meta/fixtures")
            .join(name)
    }

    #[test]
    fn valid_minimal_fixture_passes() {
        let report = validate_submission_file(&fixture("ferrum-core-minimal-submission.yaml"))
            .expect("fixture");
        assert!(report.valid, "{report}");
    }

    #[test]
    fn missing_studies_fails() {
        let root = serde_json::json!({
            "ferrum_meta_version": "0.1.0",
            "individuals": [{"alias": "i1"}],
            "samples": [{"alias": "s1", "individual_alias": "i1"}],
            "experiments": [{"alias": "e1", "sample_alias": "s1"}],
            "files": [{"alias": "f1"}],
            "datasets": [{"alias": "d1", "title": "t", "file_aliases": ["f1"]}]
        });
        let report = validate_core_submission(&root);
        assert!(!report.valid);
    }
}
