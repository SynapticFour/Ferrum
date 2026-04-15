//! Deterministic manifest generation from FHIR NPM package tarballs (`.tgz`).

use crate::{MiiModule, ProfileManifest, ProfilePackage, ResourceRule};
use chrono::Utc;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

/// Sync specification: pinned packages + target profile set version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSpec {
    #[serde(default = "default_registry_base")]
    pub registry_base: String,
    pub profile_set_version: String,
    pub packages: Vec<SyncSpecEntry>,
}

fn default_registry_base() -> String {
    "https://packages.fhir.org".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSpecEntry {
    pub module: String,
    pub package_name: String,
    pub package_version: String,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("sync io: {0}")]
    Io(String),
    #[error("sync json: {0}")]
    Json(String),
    #[error("sync config: {0}")]
    Config(String),
    #[error("tarball parse: {0}")]
    Tarball(String),
}

pub fn load_sync_spec(path: &Path) -> Result<SyncSpec, SyncError> {
    let raw = std::fs::read_to_string(path).map_err(|e| SyncError::Io(e.to_string()))?;
    serde_json::from_str(&raw).map_err(|e| SyncError::Json(e.to_string()))
}

/// Public URL used to download a FHIR NPM package tarball (deterministic given name + version).
pub fn fhir_package_download_url(registry_base: &str, package_name: &str, version: &str) -> String {
    let base = registry_base.trim_end_matches('/');
    format!("{base}/{package_name}/{version}")
}

/// Download bytes from `url` (follow redirects). Intended for CLI / CI; offline workflows pass local `.tgz` paths instead.
pub fn download_package_bytes(url: &str) -> Result<Vec<u8>, SyncError> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| SyncError::Io(e.to_string()))?;
    let resp = client
        .get(url)
        .header("Accept", "application/gzip, application/octet-stream, */*")
        .send()
        .map_err(|e| SyncError::Io(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(SyncError::Io(format!("HTTP {} for {}", resp.status(), url)));
    }
    let bytes = resp.bytes().map_err(|e| SyncError::Io(e.to_string()))?;
    Ok(bytes.to_vec())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Extract StructureDefinition canonical URLs and resource types from a FHIR `.tgz` (npm package layout).
pub fn resource_rules_from_fhir_tgz(tgz_bytes: &[u8]) -> Result<Vec<ResourceRule>, SyncError> {
    let decoder = GzDecoder::new(tgz_bytes);
    let mut archive = tar::Archive::new(decoder);
    let mut by_type: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();

    for entry in archive
        .entries()
        .map_err(|e| SyncError::Tarball(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| SyncError::Tarball(e.to_string()))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| SyncError::Tarball(e.to_string()))?;
        let path_str = path.to_string_lossy();
        if !path_str.ends_with(".json") {
            continue;
        }
        if path_str.ends_with("package.json") && path.components().count() <= 2 {
            continue;
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| SyncError::Tarball(e.to_string()))?;
        let v: Value = serde_json::from_slice(&buf).map_err(|e| SyncError::Json(e.to_string()))?;
        if v.get("resourceType").and_then(Value::as_str) != Some("StructureDefinition") {
            continue;
        }
        let url = v
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| SyncError::Tarball("StructureDefinition missing url".to_string()))?;
        let resource_type = v
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| SyncError::Tarball("StructureDefinition missing type".to_string()))?
            .to_string();
        by_type
            .entry(resource_type)
            .or_default()
            .insert(url.to_string(), buf);
    }

    let mut rules = Vec::new();
    for (resource_type, urls) in by_type {
        let mut accepted = Vec::new();
        let mut file_hashes = Vec::new();
        for (url, raw) in urls {
            accepted.push(url);
            file_hashes.push(sha256_hex(&raw));
        }
        accepted.sort();
        file_hashes.sort();
        let checksum = if file_hashes.len() == 1 {
            Some(format!("sha256:{}", file_hashes[0]))
        } else {
            let joined = file_hashes.join("|");
            Some(format!("sha256:{}", sha256_hex(joined.as_bytes())))
        };
        rules.push(ResourceRule {
            resource_type,
            accepted_profiles: accepted,
            checksum,
        });
    }
    rules.sort_by(|a, b| a.resource_type.cmp(&b.resource_type));
    Ok(rules)
}

/// Build one [`ProfilePackage`] from a downloaded tarball + module.
pub fn profile_package_from_tgz(
    module: MiiModule,
    package_name: String,
    package_version: String,
    tgz_bytes: &[u8],
) -> Result<ProfilePackage, SyncError> {
    let package_sha256 = sha256_hex(tgz_bytes);
    let resources = resource_rules_from_fhir_tgz(tgz_bytes)?;
    if resources.is_empty() {
        return Err(SyncError::Tarball(format!(
            "no StructureDefinition resources found in package {package_name}@{package_version}"
        )));
    }
    Ok(ProfilePackage {
        package_name,
        package_version,
        module,
        resources,
        package_sha256: Some(package_sha256),
    })
}

/// Build a full [`ProfileManifest`] from spec + ordered tarball bytes (same order as `spec.packages`).
pub fn build_manifest_from_sync_inputs(
    spec: &SyncSpec,
    tgz_per_entry: &[Vec<u8>],
) -> Result<ProfileManifest, SyncError> {
    if spec.packages.len() != tgz_per_entry.len() {
        return Err(SyncError::Config(format!(
            "sync spec has {} packages but {} tarballs provided",
            spec.packages.len(),
            tgz_per_entry.len()
        )));
    }
    let mut packages = Vec::with_capacity(spec.packages.len());
    for (entry, bytes) in spec.packages.iter().zip(tgz_per_entry.iter()) {
        let module = MiiModule::parse_list(std::slice::from_ref(&entry.module))
            .map_err(|e| SyncError::Config(e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| SyncError::Config("empty module".to_string()))?;
        let pkg = profile_package_from_tgz(
            module,
            entry.package_name.clone(),
            entry.package_version.clone(),
            bytes,
        )?;
        packages.push(pkg);
    }
    Ok(ProfileManifest {
        profile_set_version: spec.profile_set_version.clone(),
        generated_at: Utc::now(),
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    use tar::Builder;

    fn minimal_tgz_with_sd(url: &str, typ: &str) -> Vec<u8> {
        let sd = r#"{"resourceType":"StructureDefinition","url":"URL_PLACEHOLDER","type":"TYPE_PLACEHOLDER","name":"Test"}"#
            .replace("URL_PLACEHOLDER", url)
            .replace("TYPE_PLACEHOLDER", typ);
        let mut tar_bytes = Vec::new();
        {
            let mut b = Builder::new(&mut tar_bytes);
            let mut header = tar::Header::new_gnu();
            header.set_size(sd.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            b.append_data(
                &mut header,
                "package/StructureDefinition-test.json",
                sd.as_bytes(),
            )
            .unwrap();
            b.finish().unwrap();
        }
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&tar_bytes).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn resource_rules_from_minimal_tgz() {
        let tgz = minimal_tgz_with_sd(
            "https://example.org/fhir/StructureDefinition/PatientX",
            "Patient",
        );
        let rules = resource_rules_from_fhir_tgz(&tgz).expect("rules");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].resource_type, "Patient");
        assert!(rules[0].accepted_profiles[0].contains("PatientX"));
        assert!(rules[0].checksum.as_ref().unwrap().starts_with("sha256:"));
    }
}
