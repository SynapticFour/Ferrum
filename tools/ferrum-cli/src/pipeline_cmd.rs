//! Field analysis pipeline CLI (Phase 5 / T5).

use ferrum_beacon::repo::BeaconRepo;
use ferrum_core::{
    classify_htsget_file, is_vcf_like, DatabasePool, FerrumConfig, FerrumPool, PipelineConfig,
};
use ferrum_drs::repo::DrsRepo;
use ferrum_ont::OntQualityMetrics;
use std::path::{Path, PathBuf};
use std::process::Stdio;

async fn edge_pool(config: Option<&PathBuf>) -> Result<(FerrumConfig, FerrumPool), String> {
    let mut cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found".to_string())?;
    cfg.database.run_migrations = false;
    let db = DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    let pool = match db {
        DatabasePool::Postgres(p) => FerrumPool::Postgres(p),
        DatabasePool::Sqlite(p) => FerrumPool::Sqlite(p),
    };
    Ok((cfg, pool))
}

pub async fn pipeline_qc(
    object_id: &str,
    fastq: &Path,
    gateway: &str,
    allow_stub: bool,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, _pool) = edge_pool(config).await?;
    let metrics = run_nanostat_or_stub(fastq, &cfg.pipeline, allow_stub)?;
    post_ont_metrics(gateway, object_id, &metrics).await?;
    println!(
        "Posted QC metrics for {object_id}: mean_qscore={} read_count={} n50={}",
        metrics.mean_qscore, metrics.read_count, metrics.n50
    );
    Ok(())
}

fn run_nanostat_or_stub(
    fastq: &Path,
    pipeline: &PipelineConfig,
    allow_stub: bool,
) -> Result<OntQualityMetrics, String> {
    let bin = pipeline
        .nanostat_bin
        .clone()
        .unwrap_or_else(|| "nanostat".into());
    if let Ok(output) = std::process::Command::new(&bin)
        .arg("--fastq")
        .arg(fastq)
        .arg("-n")
        .arg("/dev/stdout")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        if output.status.success() {
            if let Some(m) = parse_nanostat_stdout(&String::from_utf8_lossy(&output.stdout)) {
                return Ok(m);
            }
        }
    }
    if allow_stub || pipeline.allow_qc_stub {
        let size = std::fs::metadata(fastq).map(|m| m.len()).unwrap_or(1024);
        return Ok(OntQualityMetrics {
            mean_qscore: 12.0,
            read_count: (size / 300).max(1),
            n50: (size / 2).max(500),
            read_length_histogram: vec![],
        });
    }
    Err(format!(
        "NanoStat (`{bin}`) not available; install nanostat or pass --allow-stub / set [pipeline] allow_qc_stub=true"
    ))
}

fn parse_nanostat_stdout(text: &str) -> Option<OntQualityMetrics> {
    let mut mean_qscore = None;
    let mut read_count = None;
    let mut n50 = None;
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("mean qscore") || lower.contains("mean read quality") {
            mean_qscore = extract_last_number(line);
        } else if lower.contains("number of reads") {
            read_count = extract_last_number(line).map(|n| n as u64);
        } else if lower.contains("n50") {
            n50 = extract_last_number(line).map(|n| n as u64);
        }
    }
    Some(OntQualityMetrics {
        mean_qscore: mean_qscore.unwrap_or(10.0),
        read_count: read_count.unwrap_or(1),
        n50: n50.unwrap_or(1000),
        read_length_histogram: vec![],
    })
}

fn extract_last_number(line: &str) -> Option<f32> {
    line.split_whitespace()
        .filter_map(|t| t.trim_matches(':').parse::<f32>().ok())
        .next_back()
}

async fn post_ont_metrics(
    gateway: &str,
    object_id: &str,
    metrics: &OntQualityMetrics,
) -> Result<(), String> {
    let url = format!(
        "{}/api/v1/ingest/ont-metrics",
        gateway.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "drs_object_id": object_id,
        "quality_metrics": metrics,
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "ont-metrics HTTP {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}

pub async fn pipeline_index_beacon(
    object_id: &str,
    dataset_id: Option<&str>,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, pool) = edge_pool(config).await?;
    let dataset = dataset_id
        .map(str::to_string)
        .unwrap_or_else(|| cfg.pipeline.default_beacon_dataset.clone());

    let repo = DrsRepo::new(pool.clone(), "localhost".into());
    let obj = repo
        .get_object(object_id, false)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("object {object_id} not found"))?;
    let name = obj.name.clone();
    let mime = obj.mime_type.clone();
    if !is_vcf_like(name.as_deref(), mime.as_deref()) {
        return Err("object does not look like VCF/BCF".into());
    }
    let Some((backend, storage_key, _enc)) = repo
        .get_storage_ref(object_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Err(format!("object {object_id} has no storage"));
    };
    if backend != "local" {
        return Err("index-beacon only supports local storage on Edge".into());
    }

    let objects_root = ferrum_core::resolve_objects_root(&cfg);
    let path = objects_root.join(&storage_key);
    if !path.is_file() {
        return Err(format!("local VCF not found at {}", path.display()));
    }
    let beacon = BeaconRepo::new(pool.clone());
    beacon
        .ensure_dataset(&dataset, object_id, name.as_deref(), "GRCh38")
        .await
        .map_err(|e| e.to_string())?;
    let n = ferrum_beacon::vcf_index::index_vcf_path(&pool, &dataset, &path)
        .await
        .map_err(|e| e.to_string())?;
    println!("Indexed {n} variant(s) from {object_id} into Beacon dataset `{dataset}`");
    Ok(())
}

pub async fn pipeline_forward_wes(
    workflow_url: &str,
    object_id: &str,
    wes_url: Option<&str>,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, _pool) = edge_pool(config).await?;
    let base = wes_url
        .map(str::to_string)
        .or_else(|| cfg.pipeline.default_wes_url.clone())
        .ok_or_else(|| "WES URL required (--wes-url or [pipeline] default_wes_url)".to_string())?;

    let body = serde_json::json!({
        "workflow_params": {
            "drs_object_id": object_id,
        },
        "workflow_url": workflow_url,
        "tags": {
            "ads_compute_pool_id": "field-edge",
            "federation_origin": "local"
        }
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/runs", base.trim_end_matches('/')))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("WES forward failed HTTP {status}: {text}"));
    }
    println!("Submitted WES run for {object_id} to {base}: {text}");
    Ok(())
}

pub async fn pipeline_htsget_status(
    object_id: &str,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (_cfg, pool) = edge_pool(config).await?;
    let repo = DrsRepo::new(pool, "localhost".into());
    let obj = repo
        .get_object(object_id, false)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "object not found".to_string())?;
    let meta = repo
        .get_metadata(object_id)
        .await
        .map_err(|e| e.to_string())?;
    let status = meta
        .into_iter()
        .find(|(k, _)| k == "htsget_index_status")
        .map(|(_, v)| v);
    let kind = classify_htsget_file(obj.mime_type.as_deref(), obj.name.as_deref());
    println!(
        "{object_id}: htsget_index_status={} file_kind={kind:?}",
        status.unwrap_or_else(|| "none".into())
    );
    Ok(())
}
