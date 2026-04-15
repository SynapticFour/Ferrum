//! Ferrum MII Connect: validates FHIR resources against vendored MII profile metadata.

pub mod sync;

pub use sync::{
    build_manifest_from_sync_inputs, download_package_bytes, fhir_package_download_url,
    load_sync_spec, profile_package_from_tgz, resource_rules_from_fhir_tgz, SyncError, SyncSpec,
    SyncSpecEntry,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiiModule {
    Person,
    Encounter,
    Consent,
    Diagnosis,
    Procedure,
    Laboratory,
    Medication,
    Oncology,
    PathologyReport,
    MolecularGeneticReport,
    MolecularTumorBoard,
    Microbiology,
    Imaging,
    IntensiveCare,
    Genomics,
    Document,
    ResearchStudy,
    RareDiseases,
    ProsProms,
    Biobank,
}

impl MiiModule {
    pub fn all_default17() -> Vec<Self> {
        vec![
            Self::Person,
            Self::Encounter,
            Self::Consent,
            Self::Diagnosis,
            Self::Procedure,
            Self::Laboratory,
            Self::Medication,
            Self::Oncology,
            Self::PathologyReport,
            Self::MolecularGeneticReport,
            Self::MolecularTumorBoard,
            Self::Microbiology,
            Self::Imaging,
            Self::IntensiveCare,
            Self::Biobank,
            Self::Document,
            Self::ResearchStudy,
        ]
    }

    pub fn all_core5() -> Vec<Self> {
        vec![
            Self::Diagnosis,
            Self::Procedure,
            Self::Laboratory,
            Self::Genomics,
            Self::Biobank,
        ]
    }

    pub fn parse_list(parts: &[String]) -> Result<Vec<Self>, ValidateError> {
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            let m = match p.trim().to_ascii_lowercase().as_str() {
                "person" => Self::Person,
                "encounter" | "fall" | "case" | "behandlungsfall" => Self::Encounter,
                "consent" => Self::Consent,
                "diagnosis" | "diagnose" => Self::Diagnosis,
                "procedure" | "prozedur" => Self::Procedure,
                "laboratory" | "labor" => Self::Laboratory,
                "medication" | "medikation" => Self::Medication,
                "oncology" | "onkologie" => Self::Oncology,
                "pathology_report" | "pathology" | "pathologie" => Self::PathologyReport,
                "molecular_genetic_report" | "molecular_genetics" | "molekulargenetik" => {
                    Self::MolecularGeneticReport
                }
                "molecular_tumor_board" | "mtb" => Self::MolecularTumorBoard,
                "microbiology" | "mikrobiologie" => Self::Microbiology,
                "imaging" | "bildgebung" => Self::Imaging,
                "intensive_care" | "icu" | "intensivmedizin" => Self::IntensiveCare,
                "genomics" => Self::Genomics,
                "document" | "dokument" => Self::Document,
                "research_study" | "forschungsvorhaben" => Self::ResearchStudy,
                "rare_diseases" | "seltene_erkrankungen" => Self::RareDiseases,
                "pros_proms" | "proms" => Self::ProsProms,
                "biobank" | "bioproben" => Self::Biobank,
                other => {
                    return Err(ValidateError::Config(format!(
                        "unknown module '{other}', expected one of person, encounter, consent, diagnosis, procedure, laboratory, medication, oncology, pathology_report, molecular_genetic_report, molecular_tumor_board, microbiology, imaging, intensive_care, biobank, document, research_study, genomics, rare_diseases, pros_proms"
                    )));
                }
            };
            if !out.contains(&m) {
                out.push(m);
            }
        }
        Ok(out)
    }
}

impl Display for MiiModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Person => "person",
            Self::Encounter => "encounter",
            Self::Consent => "consent",
            Self::Diagnosis => "diagnosis",
            Self::Procedure => "procedure",
            Self::Laboratory => "laboratory",
            Self::Medication => "medication",
            Self::Oncology => "oncology",
            Self::PathologyReport => "pathology_report",
            Self::MolecularGeneticReport => "molecular_genetic_report",
            Self::MolecularTumorBoard => "molecular_tumor_board",
            Self::Microbiology => "microbiology",
            Self::Imaging => "imaging",
            Self::IntensiveCare => "intensive_care",
            Self::Genomics => "genomics",
            Self::Document => "document",
            Self::ResearchStudy => "research_study",
            Self::RareDiseases => "rare_diseases",
            Self::ProsProms => "pros_proms",
            Self::Biobank => "biobank",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiiValidationConfig {
    pub enabled: bool,
    pub modules: Vec<MiiModule>,
    pub profile_set_version: String,
    pub strict_mode: bool,
    pub max_errors: Option<usize>,
    pub offline_only: bool,
}

impl Default for MiiValidationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            modules: MiiModule::all_default17(),
            profile_set_version: "mii-kds-default17-v1".to_string(),
            strict_mode: false,
            max_errors: None,
            offline_only: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileManifest {
    pub profile_set_version: String,
    pub generated_at: DateTime<Utc>,
    pub packages: Vec<ProfilePackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePackage {
    pub package_name: String,
    pub package_version: String,
    pub module: MiiModule,
    pub resources: Vec<ResourceRule>,
    /// SHA-256 of the downloaded FHIR NPM package `.tgz` (audit trail). Set when manifest is produced by `mii sync-manifest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRule {
    pub resource_type: String,
    pub accepted_profiles: Vec<String>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub code: String,
    pub message: String,
    pub gap_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConformance {
    pub resource_id: String,
    pub resource_type: String,
    pub module: Option<MiiModule>,
    pub profile: Option<String>,
    pub status: ResourceStatus,
    pub issues: Vec<ValidationIssue>,
    pub gap_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceSummary {
    pub total_resources: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub generated_at: DateTime<Utc>,
    pub ferrum_component: String,
    pub profile_set_version: String,
    pub manifest_sha256: String,
    pub summary: ConformanceSummary,
    pub gap_list: Vec<String>,
    pub resources: Vec<ResourceConformance>,
}

#[derive(Debug, Error)]
pub enum ValidateError {
    #[error("io error: {0}")]
    Io(String),
    #[error("json parse error: {0}")]
    Json(String),
    #[error("config error: {0}")]
    Config(String),
}

fn normalize_meta_profiles(resource: &Value) -> Vec<String> {
    resource
        .get("meta")
        .and_then(|m| m.get("profile"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn extract_resources(input: &str) -> Result<Vec<Value>, ValidateError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.lines().count() > 1 && !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        let mut out = Vec::new();
        for line in trimmed.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parsed: Value =
                serde_json::from_str(line).map_err(|e| ValidateError::Json(e.to_string()))?;
            out.push(parsed);
        }
        return Ok(out);
    }

    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|e| ValidateError::Json(e.to_string()))?;
    if let Some(arr) = parsed.as_array() {
        return Ok(arr.clone());
    }

    if parsed.get("resourceType").and_then(Value::as_str) == Some("Bundle") {
        let mut out = Vec::new();
        if let Some(entries) = parsed.get("entry").and_then(Value::as_array) {
            for e in entries {
                if let Some(res) = e.get("resource") {
                    out.push(res.clone());
                }
            }
        }
        return Ok(out);
    }

    Ok(vec![parsed])
}

pub fn load_manifest(path: &Path) -> Result<(ProfileManifest, String), ValidateError> {
    let data = fs::read(path).map_err(|e| ValidateError::Io(e.to_string()))?;
    let sha = format!("{:x}", Sha256::digest(&data));
    let parsed: ProfileManifest =
        serde_json::from_slice(&data).map_err(|e| ValidateError::Json(e.to_string()))?;
    Ok((parsed, sha))
}

pub fn validate_payload(
    payload: &str,
    cfg: &MiiValidationConfig,
    manifest: &ProfileManifest,
    manifest_sha256: &str,
) -> Result<ConformanceReport, ValidateError> {
    if cfg.modules.is_empty() {
        return Err(ValidateError::Config(
            "module set is empty; configure at least one active module".to_string(),
        ));
    }
    let resources = extract_resources(payload)?;
    let mut results = Vec::with_capacity(resources.len());
    let mut errors = 0usize;
    let mut gap_tags = BTreeSet::new();

    let mut rules: HashMap<(MiiModule, String), &ResourceRule> = HashMap::new();
    for pkg in &manifest.packages {
        for r in &pkg.resources {
            rules.insert((pkg.module, r.resource_type.clone()), r);
        }
    }

    for (idx, resource) in resources.iter().enumerate() {
        let resource_type = resource
            .get("resourceType")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string();
        let resource_id = resource
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("idx:{idx}"));
        let meta_profiles = normalize_meta_profiles(resource);
        let candidates: Vec<(MiiModule, &ResourceRule)> = cfg
            .modules
            .iter()
            .filter_map(|m| rules.get(&(*m, resource_type.clone())).map(|r| (*m, *r)))
            .collect();

        let module = if candidates.is_empty() {
            None
        } else if !meta_profiles.is_empty() {
            candidates
                .iter()
                .find(|(_, rule)| {
                    meta_profiles
                        .iter()
                        .any(|p| rule.accepted_profiles.iter().any(|a| a == p))
                })
                .map(|(m, _)| *m)
                .or_else(|| Some(candidates[0].0))
        } else {
            Some(candidates[0].0)
        };

        let mut issues = Vec::new();
        let mut profile: Option<String> = None;

        if resource_type == "Unknown" {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                code: "missing_resource_type".to_string(),
                message: "resource has no resourceType".to_string(),
                gap_tag: Some("structure_missing_resource_type".to_string()),
            });
            gap_tags.insert("structure_missing_resource_type".to_string());
        }

        if let Some(module_value) = module {
            if !cfg.modules.contains(&module_value) {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Info,
                    code: "module_not_enabled".to_string(),
                    message: format!(
                        "module '{module_value}' is not enabled in current profile set"
                    ),
                    gap_tag: None,
                });
            } else if let Some(rule) = rules.get(&(module_value, resource_type.clone())) {
                if !meta_profiles.is_empty() {
                    if let Some(found) = meta_profiles
                        .iter()
                        .find(|p| rule.accepted_profiles.iter().any(|accepted| accepted == *p))
                    {
                        profile = Some(found.to_string());
                    } else {
                        issues.push(ValidationIssue {
                            severity: IssueSeverity::Error,
                            code: "profile_mismatch".to_string(),
                            message: format!(
                                "resource meta.profile does not match accepted MII profiles for {}",
                                resource_type
                            ),
                            gap_tag: Some("profile_mismatch".to_string()),
                        });
                        gap_tags.insert("profile_mismatch".to_string());
                    }
                } else {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::Error,
                        code: "missing_meta_profile".to_string(),
                        message:
                            "resource has no meta.profile; cannot verify MII profile conformance"
                                .to_string(),
                        gap_tag: Some("missing_meta_profile".to_string()),
                    });
                    gap_tags.insert("missing_meta_profile".to_string());
                }
            } else {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    code: "missing_profile_rule".to_string(),
                    message: format!(
                        "no vendored rule found for module '{}' and resource '{}'",
                        module_value, resource_type
                    ),
                    gap_tag: Some("missing_profile_rule".to_string()),
                });
                gap_tags.insert("missing_profile_rule".to_string());
            }
        } else {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Info,
                code: "outside_manifest_scope".to_string(),
                message: format!(
                    "resource type '{}' is outside current configured MII manifest scope",
                    resource_type
                ),
                gap_tag: Some("outside_manifest_scope".to_string()),
            });
            gap_tags.insert("outside_manifest_scope".to_string());
        }

        let status = if issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Error))
        {
            errors += 1;
            ResourceStatus::Fail
        } else if issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Warning))
        {
            ResourceStatus::Skipped
        } else {
            ResourceStatus::Pass
        };

        let item_gap_tags = issues
            .iter()
            .filter_map(|i| i.gap_tag.clone())
            .collect::<Vec<_>>();
        results.push(ResourceConformance {
            resource_id,
            resource_type,
            module,
            profile,
            status,
            issues,
            gap_tags: item_gap_tags,
        });

        if let Some(max) = cfg.max_errors {
            if errors >= max {
                break;
            }
        }
    }

    let mut summary = ConformanceSummary {
        total_resources: results.len(),
        passed: results
            .iter()
            .filter(|r| matches!(r.status, ResourceStatus::Pass))
            .count(),
        failed: results
            .iter()
            .filter(|r| matches!(r.status, ResourceStatus::Fail))
            .count(),
        skipped: results
            .iter()
            .filter(|r| matches!(r.status, ResourceStatus::Skipped))
            .count(),
    };

    if cfg.profile_set_version != manifest.profile_set_version {
        gap_tags.insert("profile_set_version_mismatch".to_string());
        summary.skipped += 1;
    }

    Ok(ConformanceReport {
        generated_at: Utc::now(),
        ferrum_component: "ferrum-mii-connect".to_string(),
        profile_set_version: manifest.profile_set_version.clone(),
        manifest_sha256: manifest_sha256.to_string(),
        summary,
        gap_list: gap_tags.into_iter().collect(),
        resources: results,
    })
}

pub fn read_payload(path: &Path) -> Result<String, ValidateError> {
    fs::read_to_string(path).map_err(|e| ValidateError::Io(e.to_string()))
}

pub fn read_payload_from_input(path: &Path) -> Result<String, ValidateError> {
    if path.is_dir() {
        let mut files = fs::read_dir(path)
            .map_err(|e| ValidateError::Io(e.to_string()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect::<Vec<_>>();
        files.sort();

        let mut all = Vec::<Value>::new();
        for file in files {
            let raw = fs::read_to_string(&file).map_err(|e| ValidateError::Io(e.to_string()))?;
            let mut part = extract_resources(&raw)?;
            all.append(&mut part);
        }
        return serde_json::to_string(&all).map_err(|e| ValidateError::Json(e.to_string()));
    }
    read_payload(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest() -> ProfileManifest {
        ProfileManifest {
            profile_set_version: "test-v1".to_string(),
            generated_at: Utc::now(),
            packages: vec![ProfilePackage {
                package_name: "de.medizininformatikinitiative.kerndatensatz.diagnose".to_string(),
                package_version: "1.0.0".to_string(),
                module: MiiModule::Diagnosis,
                resources: vec![ResourceRule {
                    resource_type: "Condition".to_string(),
                    accepted_profiles: vec![
                        "https://www.medizininformatik-initiative.de/fhir/core/modul-diagnose/StructureDefinition/Diagnose"
                            .to_string(),
                    ],
                    checksum: None,
                }],
                package_sha256: None,
            }],
        }
    }

    #[test]
    fn default_module_set_contains_17_modules() {
        assert_eq!(MiiModule::all_default17().len(), 17);
        assert!(MiiModule::all_default17().contains(&MiiModule::Person));
        assert!(MiiModule::all_default17().contains(&MiiModule::ResearchStudy));
    }

    #[test]
    fn validates_bundle_and_finds_profile_mismatch() {
        let payload = r#"{
          "resourceType":"Bundle",
          "entry":[{"resource":{
            "resourceType":"Condition",
            "id":"cond-1",
            "meta":{"profile":["https://example.org/unknown-profile"]}
          }}]
        }"#;
        let cfg = MiiValidationConfig {
            enabled: true,
            ..Default::default()
        };
        let report =
            validate_payload(payload, &cfg, &manifest(), "abc").expect("validation report");
        assert_eq!(report.summary.total_resources, 1);
        assert_eq!(report.summary.failed, 1);
        assert!(report.gap_list.iter().any(|g| g == "profile_mismatch"));
    }

    #[test]
    fn unknown_module_string_is_rejected() {
        let modules = vec!["diagnosis".to_string(), "unknown".to_string()];
        let err = MiiModule::parse_list(&modules).expect_err("must fail");
        assert!(format!("{err}").contains("unknown module"));
    }

    #[test]
    fn max_errors_stops_validation_early() {
        let payload = r#"{
          "resourceType":"Bundle",
          "entry":[
            {"resource":{"resourceType":"Condition","id":"c1","meta":{"profile":["x"]}}},
            {"resource":{"resourceType":"Condition","id":"c2","meta":{"profile":["x"]}}}
          ]
        }"#;
        let cfg = MiiValidationConfig {
            enabled: true,
            max_errors: Some(1),
            ..Default::default()
        };
        let report = validate_payload(payload, &cfg, &manifest(), "abc").expect("report");
        assert_eq!(report.summary.total_resources, 1);
        assert_eq!(report.summary.failed, 1);
    }

    #[test]
    fn profile_set_mismatch_is_reported_as_gap() {
        let payload = r#"{"resourceType":"Condition","id":"ok","meta":{"profile":["https://www.medizininformatik-initiative.de/fhir/core/modul-diagnose/StructureDefinition/Diagnose"]}}"#;
        let cfg = MiiValidationConfig {
            enabled: true,
            profile_set_version: "different-version".to_string(),
            ..Default::default()
        };
        let report = validate_payload(payload, &cfg, &manifest(), "abc").expect("report");
        assert!(report
            .gap_list
            .iter()
            .any(|g| g == "profile_set_version_mismatch"));
    }

    #[test]
    fn read_payload_from_directory_merges_files() {
        let base = std::env::temp_dir().join(format!(
            "ferrum_mii_connect_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&base).expect("mkdir");

        let f1 = base.join("a.json");
        let f2 = base.join("b.json");
        let mut w1 = fs::File::create(&f1).expect("f1");
        let mut w2 = fs::File::create(&f2).expect("f2");
        writeln!(
            w1,
            "{{\"resourceType\":\"Condition\",\"id\":\"r1\",\"meta\":{{\"profile\":[\"https://www.medizininformatik-initiative.de/fhir/core/modul-diagnose/StructureDefinition/Diagnose\"]}}}}"
        )
        .expect("write1");
        writeln!(
            w2,
            "{{\"resourceType\":\"Procedure\",\"id\":\"r2\",\"meta\":{{\"profile\":[\"https://www.medizininformatik-initiative.de/fhir/core/modul-prozedur/StructureDefinition/Prozedur\"]}}}}"
        )
        .expect("write2");

        let merged = read_payload_from_input(&base).expect("merge");
        let parsed: Value = serde_json::from_str(&merged).expect("json");
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), 2);

        let _ = fs::remove_file(f1);
        let _ = fs::remove_file(f2);
        let _ = fs::remove_dir(base);
    }

    #[test]
    fn empty_module_configuration_is_rejected() {
        let cfg = MiiValidationConfig {
            enabled: true,
            modules: Vec::new(),
            ..Default::default()
        };
        let payload = r#"{"resourceType":"Condition","id":"cond-1"}"#;
        let err = validate_payload(payload, &cfg, &manifest(), "abc").expect_err("must reject");
        assert!(format!("{err}").contains("module set is empty"));
    }
}
