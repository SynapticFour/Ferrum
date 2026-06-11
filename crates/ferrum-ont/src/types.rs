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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntQualityMetrics {
    pub mean_qscore: f32,
    pub read_count: u64,
    pub n50: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_length_histogram: Vec<(u32, u32)>,
}
