//! Automatic MultiQC report generation for completed WES runs: scan QC outputs, run MultiQC, ingest report into DRS.

use crate::error::{Result, WesError};
use crate::repo::WesRepo;
use ferrum_core::MultiQCConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

const MULTIQC_TIMEOUT_SECS: u64 = 600; // 10 min
const MIN_QC_FILES: usize = 2;

/// Runs MultiQC for completed runs: scan work dir, run container, ingest report + data into DRS, update run outputs.
pub struct MultiQCRunner {
    pub config: MultiQCConfig,
    /// Base URL for DRS API (e.g. http://gateway:8080/ga4gh/drs/v1). Ingest at {base}/ingest/file.
    pub drs_ingest_base_url: String,
    pub repo: Arc<WesRepo>,
    pub provenance_store: Option<Arc<ferrum_core::ProvenanceStore>>,
    client: reqwest::Client,
}

impl MultiQCRunner {
    pub fn new(
        config: MultiQCConfig,
        drs_ingest_base_url: String,
        repo: Arc<WesRepo>,
        provenance_store: Option<Arc<ferrum_core::ProvenanceStore>>,
    ) -> Self {
        Self {
            config,
            drs_ingest_base_url: drs_ingest_base_url.trim_end_matches('/').to_string(),
            repo,
            provenance_store,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap(),
        }
    }

    /// Run MultiQC for a completed run: scan, run container, ingest, provenance, merge outputs. Called once when run reaches COMPLETE.
    pub async fn run_for_completed_run(&self, run_id: &str) -> Result<Option<String>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let row = self
            .repo
            .get_run(run_id)
            .await?
            .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
        let (
            _run_id,
            workflow_url,
            workflow_type,
            _wtv,
            _params,
            _ep,
            _tags,
            _state,
            _st,
            _et,
            outputs,
            work_dir_opt,
            _owner,
            _resumed,
            _checkpoint,
        ) = row;
        let work_dir = match work_dir_opt {
            Some(d) => PathBuf::from(d),
            None => return Ok(None),
        };
        if !work_dir.is_dir() {
            return Ok(None);
        }
        let run_for = &self.config.run_for;
        let applicable = run_for.is_empty()
            || run_for.iter().any(|s| s == "*")
            || run_for
                .iter()
                .any(|s| s.eq_ignore_ascii_case(workflow_type.trim()));
        if !applicable {
            return Ok(None);
        }
        // Skip if we already ran MultiQC for this run
        if let Some(obj) = outputs.get("ferrum:multiqc_status") {
            if obj
                .as_str()
                .map(|s| s == "complete" || s == "skipped")
                .unwrap_or(false)
            {
                return Ok(outputs
                    .get("ferrum:multiqc_report_drs_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string));
            }
        }
        let qc_files = self.scan_for_qc_files(&work_dir).await?;
        if qc_files.len() < MIN_QC_FILES {
            self.repo
                .merge_run_outputs(
                    run_id,
                    &serde_json::json!({
                        "ferrum:multiqc_status": "skipped",
                        "ferrum:multiqc_skip_reason": "insufficient QC files"
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                )
                .await?;
            return Ok(None);
        }
        let out_dir = std::env::temp_dir().join(format!("multiqc-{}", run_id));
        if out_dir.exists() {
            let _ = tokio::fs::remove_dir_all(&out_dir).await;
        }
        tokio::fs::create_dir_all(&out_dir)
            .await
            .map_err(WesError::Io)?;
        let run_multiqc = self
            .run_multiqc_container(&work_dir, &out_dir, run_id)
            .await;
        if let Err(e) = run_multiqc {
            tracing::warn!(run_id = %run_id, "multiqc run failed: {}", e);
            self.repo
                .merge_run_outputs(
                    run_id,
                    &serde_json::json!({
                        "ferrum:multiqc_status": "failed",
                        "ferrum:multiqc_error": e.to_string()
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                )
                .await?;
            return Ok(None);
        }
        let report_path = out_dir.join("report.html");
        let data_path = out_dir.join("report_data.zip");
        if !report_path.is_file() {
            self.repo
                .merge_run_outputs(
                    run_id,
                    &serde_json::json!({
                        "ferrum:multiqc_status": "failed",
                        "ferrum:multiqc_error": "report.html not produced"
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                )
                .await?;
            return Ok(None);
        }
        let report_name = self
            .config
            .report_name_template
            .replace("{workflow_type}", &workflow_type)
            .replace("{run_id}", run_id);
        let ingest_url = format!("{}/ingest/file", self.drs_ingest_base_url);
        let report_bytes = tokio::fs::read(&report_path).await.map_err(WesError::Io)?;
        let report_id = self
            .ingest_file_multipart(
                &ingest_url,
                &report_name,
                "report.html",
                "text/html",
                &report_bytes,
                true,
                run_id,
                &workflow_type,
                &workflow_url,
            )
            .await?;
        let mut updates = serde_json::Map::new();
        updates.insert(
            "multiqc_report".to_string(),
            serde_json::Value::String(format!("drs://ferrum/{}", report_id)),
        );
        updates.insert(
            "ferrum:multiqc_status".to_string(),
            serde_json::Value::String("complete".to_string()),
        );
        updates.insert(
            "ferrum:multiqc_report_drs_id".to_string(),
            serde_json::Value::String(report_id.clone()),
        );
        let data_id = if data_path.is_file() {
            let data_bytes = tokio::fs::read(&data_path).await.map_err(WesError::Io)?;
            match self
                .ingest_file_multipart(
                    &ingest_url,
                    &format!(
                        "MultiQC Data — {} {}",
                        workflow_type,
                        &run_id[..run_id.len().min(8)]
                    ),
                    "report_data.zip",
                    "application/zip",
                    &data_bytes,
                    true,
                    run_id,
                    &workflow_type,
                    &workflow_url,
                )
                .await
            {
                Ok(id) => {
                    updates.insert(
                        "multiqc_data".to_string(),
                        serde_json::Value::String(format!("drs://ferrum/{}", id)),
                    );
                    if let Some(ref store) = self.provenance_store {
                        let _ = store.record_wes_output(run_id, &id).await;
                    }
                    Some(id)
                }
                Err(e) => {
                    tracing::warn!(run_id = %run_id, "multiqc data ingest failed: {}", e);
                    None
                }
            }
        } else {
            None
        };
        if let Some(ref store) = self.provenance_store {
            let _ = store.record_wes_output(run_id, &report_id).await;
        }
        self.repo.merge_run_outputs(run_id, &updates).await?;
        let _ = tokio::fs::remove_dir_all(&out_dir).await;
        let _ = data_id;
        Ok(Some(report_id))
    }

    /// Scan work_dir for QC files matching config.scan_patterns. Returns paths under work_dir.
    pub async fn scan_for_qc_files(&self, work_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        let mut stack = vec![work_dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(e) => e,
                Err(_) => continue,
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if is_dir {
                    if name.starts_with('.') && name != "." && name != ".." {
                        continue;
                    }
                    if name == "report_data" || name == "multiqc_data" {
                        continue;
                    }
                    for pat in &self.config.scan_patterns {
                        let trimmed = pat.trim();
                        if trimmed.ends_with('/') {
                            let pattern = trimmed.trim_end_matches('/');
                            if glob_match(pattern, name) {
                                out.push(path.clone());
                                break;
                            }
                        }
                    }
                    stack.push(path);
                } else {
                    for pat in &self.config.scan_patterns {
                        let trimmed = pat.trim();
                        if !trimmed.ends_with('/') && glob_match(trimmed, name) {
                            out.push(path.clone());
                            break;
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    async fn run_multiqc_container(
        &self,
        work_dir: &Path,
        out_dir: &Path,
        _run_id: &str,
    ) -> Result<()> {
        let work_str = work_dir
            .canonicalize()
            .map_err(WesError::Io)?
            .display()
            .to_string();
        let out_str = out_dir.display().to_string();
        let runner = which_runner();
        let args: Vec<String> = match runner.as_deref() {
            Some(_) => vec![
                "run".into(),
                "--rm".into(),
                "-v".into(),
                format!("{}:/work:ro", work_str),
                "-v".into(),
                format!("{}:/output", out_str),
                self.config.image.clone(),
                "multiqc".into(),
                "/work".into(),
                "--outdir".into(),
                "/output".into(),
                "--filename".into(),
                "report".into(),
                "--force".into(),
                "--no-megaqc-upload".into(),
                "--export".into(),
            ],
            None => {
                return Err(WesError::Other(anyhow::anyhow!(
                    "neither podman nor docker found for MultiQC"
                )))
            }
        };
        let mut cmd = Command::new(runner.as_deref().unwrap());
        cmd.args(&args);
        cmd.current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let child = cmd.spawn().map_err(WesError::Io)?;
        let output = tokio::time::timeout(
            Duration::from_secs(MULTIQC_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| WesError::Other(anyhow::anyhow!("multiqc run timed out")))?
        .map_err(WesError::Io)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WesError::Other(anyhow::anyhow!(
                "multiqc failed: {}",
                stderr
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn ingest_file_multipart(
        &self,
        url: &str,
        name: &str,
        file_name: &str,
        mime_type: &str,
        data: &[u8],
        encrypt: bool,
        _run_id: &str,
        _workflow_type: &str,
        _workflow_url: &str,
    ) -> Result<String> {
        let part = reqwest::multipart::Part::bytes(data.to_vec())
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .map_err(|e| WesError::Other(anyhow::anyhow!("{}", e)))?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("name", name.to_string())
            .text("encrypt", if encrypt { "true" } else { "false" });
        let res = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| WesError::Other(anyhow::anyhow!("{}", e)))?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(WesError::Other(anyhow::anyhow!(
                "DRS ingest failed: {} {}",
                status,
                body
            )));
        }
        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| WesError::Other(anyhow::anyhow!("{}", e)))?;
        let id = json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| WesError::Other(anyhow::anyhow!("ingest response missing id")))?
            .to_string();
        Ok(id)
    }
}

fn which_runner() -> Option<String> {
    if std::process::Command::new("podman")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("podman".to_string());
    }
    if std::process::Command::new("docker")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("docker".to_string());
    }
    None
}

/// Simple glob: * matches any chars (no path sep). For single filename match.
fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern.contains('/') {
        return false;
    }
    let mut pi = 0;
    let mut ni = 0;
    let pb = pattern.as_bytes();
    let nb = name.as_bytes();
    while pi < pb.len() {
        match pb[pi] {
            b'*' => {
                pi += 1;
                while pi < pb.len() && pb[pi] == b'*' {
                    pi += 1;
                }
                if pi >= pb.len() {
                    return true;
                }
                let next = pb[pi];
                while ni < nb.len() && nb[ni] != next {
                    ni += 1;
                }
                if ni >= nb.len() {
                    return false;
                }
            }
            b'?' => {
                if ni >= nb.len() {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
            c => {
                if ni >= nb.len() || nb[ni] != c {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
        }
    }
    ni >= nb.len()
}
