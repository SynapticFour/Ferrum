//! Watch a MinKNOW output directory and register new reads with a running Ferrum gateway.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn is_ont_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.ends_with(".pod5")
        || name.ends_with(".fast5")
        || name.ends_with(".blow5")
        || name.ends_with(".fastq")
        || name.ends_with(".fq")
        || name.ends_with(".fastq.gz")
        || name.ends_with(".fq.gz")
}

fn infer_ont_format(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".pod5") {
        "pod5"
    } else if name.ends_with(".fast5") {
        "fast5"
    } else if name.ends_with(".blow5") {
        "blow5"
    } else {
        "fastq"
    }
}

fn sample_id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sample")
        .to_string()
}

pub async fn watch_and_ingest(
    watch_dir: PathBuf,
    gateway: &str,
    poll_secs: u64,
    dry_run: bool,
    meta_bundle: Option<PathBuf>,
    collector: Option<String>,
) -> Result<(), String> {
    if !watch_dir.is_dir() {
        return Err(format!(
            "watch path is not a directory: {}",
            watch_dir.display()
        ));
    }
    let gateway = gateway.trim_end_matches('/').to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let ferrum_meta = meta_bundle
        .as_ref()
        .map(std::fs::read_to_string)
        .transpose()
        .map_err(|e| e.to_string())?;

    let mut seen = HashSet::new();
    for entry in std::fs::read_dir(&watch_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() && is_ont_candidate(&path) {
            seen.insert(path.clone());
            ingest_file(
                &client,
                &gateway,
                &path,
                dry_run,
                ferrum_meta.as_deref(),
                collector.as_deref(),
            )
            .await?;
        }
    }

    println!(
        "[ferrum] Watching {} for ONT files (poll every {}s). Ctrl+C to stop.",
        watch_dir.display(),
        poll_secs
    );

    loop {
        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
        for entry in std::fs::read_dir(&watch_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() || !is_ont_candidate(&path) {
                continue;
            }
            if seen.insert(path.clone()) {
                ingest_file(
                    &client,
                    &gateway,
                    &path,
                    dry_run,
                    ferrum_meta.as_deref(),
                    collector.as_deref(),
                )
                .await?;
            }
        }
    }
}

async fn ingest_file(
    client: &reqwest::Client,
    gateway: &str,
    path: &Path,
    dry_run: bool,
    ferrum_meta: Option<&str>,
    collector: Option<&str>,
) -> Result<(), String> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("read");
    println!("[ferrum] New file: {file_name}");
    if dry_run {
        println!("[ferrum] dry-run: would POST to {gateway}/api/v1/ingest/ont");
        return Ok(());
    }

    let format = infer_ont_format(path);
    let sample_id = sample_id_from_path(path);
    let mut meta = serde_json::json!({
        "format": format,
        "source_path": path.display().to_string(),
        "run_id": format!("watch-{}", chrono::Utc::now().format("%Y%m%dT%H%M%S")),
        "sample_id": sample_id,
        "organism": "unknown",
        "dorado_basecalled": format == "fastq",
    });
    if let Some(c) = collector {
        meta["collector"] = serde_json::Value::String(c.to_string());
        meta["collected_at"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
    }
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name.to_string())
        .mime_str("application/octet-stream")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::multipart::Form::new()
        .text("ont_metadata", meta.to_string())
        .part("file", part);
    if let Some(bundle) = ferrum_meta {
        form = form.text("ferrum_meta", bundle.to_string());
    }

    let url = format!("{gateway}/api/v1/ingest/ont");
    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("ingest failed ({status}): {body}"));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    println!(
        "[ferrum] Ingested {} → object_id={}",
        file_name,
        body.get("object_id")
            .or_else(|| body.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("?")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ont_extensions() {
        assert!(is_ont_candidate(Path::new("/data/run/sample.pod5")));
        assert!(is_ont_candidate(Path::new("/data/run/sample.fastq.gz")));
        assert!(!is_ont_candidate(Path::new("/data/run/readme.txt")));
    }
}
