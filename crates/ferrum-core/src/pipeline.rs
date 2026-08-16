// SPDX-License-Identifier: BUSL-1.1
//! Field analysis pipeline configuration and helpers (Phase 5 / T5).

use serde::Deserialize;

/// Post-ingest pipeline behaviour for Edge and hub deployments.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PipelineConfig {
    /// After ingest, mark BAM/CRAM/VCF/BCF objects as htsget-ready in object metadata.
    pub auto_htsget_index: bool,
    /// After ingest, index VCF objects into Beacon (SNV rows, capped).
    pub auto_index_beacon: bool,
    /// Default Beacon dataset id for auto VCF indexing on Edge.
    pub default_beacon_dataset: String,
    /// External NanoStat binary path (default: search `nanostat` on PATH).
    pub nanostat_bin: Option<String>,
    /// When true, `ferrum pipeline qc` may use file-size heuristics if NanoStat is missing (CI/demo).
    pub allow_qc_stub: bool,
    /// Default WES base URL for variant-calling forward (`ferrum pipeline forward-wes`).
    pub default_wes_url: Option<String>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            auto_htsget_index: true,
            auto_index_beacon: true,
            default_beacon_dataset: "field-edge".into(),
            nanostat_bin: None,
            allow_qc_stub: false,
            default_wes_url: None,
        }
    }
}

/// Alignment / variant file kinds relevant to htsget tickets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtsgetFileKind {
    ReadsBam,
    ReadsCram,
    VariantsVcf,
    VariantsBcf,
    Other,
}

/// Classify a DRS object for htsget endpoint routing (mirrors ferrum-htsget ticket logic).
pub fn classify_htsget_file(mime_type: Option<&str>, name: Option<&str>) -> HtsgetFileKind {
    let mime = mime_type
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let name_l = name.map(|s| s.to_ascii_lowercase()).unwrap_or_default();

    if mime.contains("cram") || name_l.ends_with(".cram") {
        return HtsgetFileKind::ReadsCram;
    }
    if mime.contains("bam") || mime.contains("vnd.ga4gh.bam") || name_l.ends_with(".bam") {
        return HtsgetFileKind::ReadsBam;
    }
    if mime.contains("bcf") || name_l.ends_with(".bcf") {
        return HtsgetFileKind::VariantsBcf;
    }
    if mime.contains("vcf") || name_l.ends_with(".vcf") || name_l.ends_with(".vcf.gz") {
        return HtsgetFileKind::VariantsVcf;
    }
    HtsgetFileKind::Other
}

pub fn is_htsget_supported(kind: HtsgetFileKind) -> bool {
    !matches!(kind, HtsgetFileKind::Other)
}

pub fn is_vcf_like(name: Option<&str>, mime: Option<&str>) -> bool {
    matches!(
        classify_htsget_file(mime, name),
        HtsgetFileKind::VariantsVcf | HtsgetFileKind::VariantsBcf
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_bam_and_vcf() {
        assert_eq!(
            classify_htsget_file(None, Some("sample.bam")),
            HtsgetFileKind::ReadsBam
        );
        assert_eq!(
            classify_htsget_file(None, Some("variants.vcf.gz")),
            HtsgetFileKind::VariantsVcf
        );
        assert_eq!(
            classify_htsget_file(None, Some("notes.txt")),
            HtsgetFileKind::Other
        );
    }
}
