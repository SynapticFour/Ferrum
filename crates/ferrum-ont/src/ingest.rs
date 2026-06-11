use crate::error::{OntError, Result};
use crate::types::{OntFormat, OntIngestRequest, OntQualityMetrics};
use serde_json::Value;

/// MIME type for canonical DRS object storage.
pub fn mime_for_format(format: OntFormat) -> &'static str {
    match format {
        OntFormat::Fast5 => "application/x-fast5",
        OntFormat::Pod5 => "application/x-pod5",
        OntFormat::Blow5 => "application/x-blow5",
        OntFormat::Fastq => "application/x-fastq",
    }
}

/// Validate ONT ingest metadata before storage.
pub fn validate_ingest_request(req: &OntIngestRequest) -> Result<()> {
    if req.run_id.trim().is_empty() {
        return Err(OntError::Validation("run_id is required".into()));
    }
    if req.sample_id.trim().is_empty() {
        return Err(OntError::Validation("sample_id is required".into()));
    }
    if req.organism.trim().is_empty() {
        return Err(OntError::Validation("organism is required".into()));
    }
    if req.format == OntFormat::Fastq && !req.dorado_basecalled {
        return Err(OntError::Validation(
            "FASTQ ingest requires dorado_basecalled=true (basecalling is external)".into(),
        ));
    }
    Ok(())
}

/// Build DRS create-object fields from an ONT ingest request and uploaded bytes.
pub fn build_create_request(
    req: &OntIngestRequest,
    size: i64,
    storage_backend: &str,
    storage_key: &str,
) -> OntCreateFields {
    let name = format!(
        "{}_{}_{}.{}",
        req.sample_id,
        req.run_id,
        req.organism,
        extension_for_format(req.format)
    );
    let description = Some(format!(
        "ONT {} run={} sample={} organism={}",
        format_label(req.format),
        req.run_id,
        req.sample_id,
        req.organism
    ));
    OntCreateFields {
        name: Some(name),
        description,
        mime_type: Some(mime_for_format(req.format).to_string()),
        size,
        storage_backend: storage_backend.to_string(),
        storage_key: storage_key.to_string(),
        ont_metrics: ont_metrics_json(req),
        organism: req.organism.clone(),
    }
}

/// Fields needed by ferrum-drs to create an ONT DRS object.
#[derive(Debug, Clone)]
pub struct OntCreateFields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size: i64,
    pub storage_backend: String,
    pub storage_key: String,
    pub ont_metrics: Option<Value>,
    pub organism: String,
}

fn extension_for_format(format: OntFormat) -> &'static str {
    match format {
        OntFormat::Fast5 => "fast5",
        OntFormat::Pod5 => "pod5",
        OntFormat::Blow5 => "blow5",
        OntFormat::Fastq => "fastq",
    }
}

fn format_label(format: OntFormat) -> &'static str {
    match format {
        OntFormat::Fast5 => "FAST5",
        OntFormat::Pod5 => "POD5",
        OntFormat::Blow5 => "BLOW5",
        OntFormat::Fastq => "FASTQ",
    }
}

fn ont_metrics_json(req: &OntIngestRequest) -> Option<Value> {
    let mut obj = serde_json::Map::new();
    obj.insert("run_id".into(), Value::String(req.run_id.clone()));
    obj.insert("sample_id".into(), Value::String(req.sample_id.clone()));
    obj.insert("organism".into(), Value::String(req.organism.clone()));
    obj.insert(
        "format".into(),
        Value::String(format!("{:?}", req.format).to_lowercase()),
    );
    obj.insert(
        "dorado_basecalled".into(),
        Value::Bool(req.dorado_basecalled),
    );
    if let Some(ref qm) = req.quality_metrics {
        obj.insert("quality".into(), quality_metrics_value(qm));
    }
    Some(Value::Object(obj))
}

fn quality_metrics_value(qm: &OntQualityMetrics) -> Value {
    serde_json::json!({
        "mean_qscore": qm.mean_qscore,
        "read_count": qm.read_count,
        "n50": qm.n50,
        "read_length_histogram": qm.read_length_histogram,
    })
}

/// Synthetic POD5-like header for tests (magic bytes + minimal stub payload).
pub fn synthetic_pod5_bytes(sample_id: &str) -> Vec<u8> {
    let mut buf = b"POD5\0\x01\x00".to_vec();
    buf.extend_from_slice(sample_id.as_bytes());
    buf.extend_from_slice(b"\0STUB_ONT_DATA");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validate_rejects_empty_run_id() {
        let req = OntIngestRequest {
            format: OntFormat::Pod5,
            source_path: PathBuf::from("/tmp/x.pod5"),
            run_id: "  ".into(),
            sample_id: "s1".into(),
            organism: "Plasmodium_falciparum".into(),
            dorado_basecalled: false,
            quality_metrics: None,
        };
        assert!(validate_ingest_request(&req).is_err());
    }

    #[test]
    fn build_create_request_sets_mime() {
        let req = OntIngestRequest {
            format: OntFormat::Pod5,
            source_path: PathBuf::from("/tmp/x.pod5"),
            run_id: "run1".into(),
            sample_id: "s1".into(),
            organism: "Mycobacterium_tuberculosis".into(),
            dorado_basecalled: false,
            quality_metrics: Some(OntQualityMetrics {
                mean_qscore: 12.5,
                read_count: 1000,
                n50: 15000,
                read_length_histogram: vec![(1000, 50), (2000, 30)],
            }),
        };
        let fields = build_create_request(&req, 4096, "local", "drs/abc");
        assert_eq!(fields.mime_type.as_deref(), Some("application/x-pod5"));
        assert!(fields.ont_metrics.is_some());
    }
}
