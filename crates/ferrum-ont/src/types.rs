// SPDX-License-Identifier: BUSL-1.1
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntFormat {
    Fast5,
    Pod5,
    Blow5,
    Fastq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntIngestRequest {
    pub format: OntFormat,
    pub source_path: PathBuf,
    pub run_id: String,
    pub sample_id: String,
    pub organism: String,
    pub dorado_basecalled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_metrics: Option<OntQualityMetrics>,
    /// Field collector (operator name or Passport sub).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<String>,
    /// ISO 8601 collection timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collected_at: Option<String>,
    /// Human-readable collection location label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_label: Option<String>,
    /// WGS84 latitude (decimal degrees).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    /// WGS84 longitude (decimal degrees).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntQualityMetrics {
    pub mean_qscore: f32,
    pub read_count: u64,
    pub n50: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_length_histogram: Vec<(u32, u32)>,
}
