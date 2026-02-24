//! Slurm executor: submit via sbatch, poll via squeue, metrics via sacct after completion.
//! When a job completes, sacct is run and results are inserted into task_metrics.

use crate::error::{Result, WesError};
use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::metrics::MetricsCollector;
use crate::types::RunState;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::RwLock;

/// sacct output row: JobID, CPUTime, MaxRSS, ElapsedRaw, TotalCPU (--parsable2).
#[derive(Debug)]
pub struct SacctRow {
    job_id: String,
    #[allow(dead_code)]
    cpu_time: Option<String>,
    max_rss: Option<String>,
    elapsed_raw: Option<i64>,
    #[allow(dead_code)]
    total_cpu: Option<String>,
}

fn parse_sacct_line(line: &str) -> Option<SacctRow> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 5 {
        return None;
    }
    let elapsed_raw = parts.get(3).and_then(|s| s.trim().parse::<i64>().ok());
    Some(SacctRow {
        job_id: parts[0].trim().to_string(),
        cpu_time: parts.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        max_rss: parts.get(2).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        elapsed_raw,
        total_cpu: parts.get(4).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
    })
}

/// Parse MaxRSS from sacct: can be "1024K", "1024M", "1G", or plain bytes.
fn parse_max_rss(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() || s == "0" {
        return Some(0);
    }
    let (num, unit) = if s.ends_with('K') {
        (s.trim_end_matches('K').trim(), 1024_i64)
    } else if s.ends_with('M') {
        (s.trim_end_matches('M').trim(), 1024 * 1024)
    } else if s.ends_with('G') {
        (s.trim_end_matches('G').trim(), 1024 * 1024 * 1024)
    } else {
        (s, 1)
    };
    let n: i64 = num.parse().ok()?;
    Some(n * unit)
}

/// Run sacct for a job and return parsed rows.
pub fn sacct_job(job_id: &str) -> Result<Vec<SacctRow>> {
    let out = Command::new("sacct")
        .args([
            "-j",
            job_id,
            "--format=JobID,CPUTime,MaxRSS,ElapsedRaw,TotalCPU",
            "--noheader",
            "--parsable2",
        ])
        .output()
        .map_err(|e| WesError::Executor(format!("sacct failed: {}", e)))?;
    if !out.status.success() {
        return Err(WesError::Executor(format!(
            "sacct failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let rows: Vec<SacctRow> = stdout
        .lines()
        .filter_map(parse_sacct_line)
        .collect();
    Ok(rows)
}

struct Tracker {
    job_id: String,
}

#[derive(Default)]
pub struct SlurmExecutor {
    run_to_job: RwLock<HashMap<String, Tracker>>,
    metrics: Option<Arc<MetricsCollector>>,
}

impl SlurmExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_metrics(mut self, metrics: Option<Arc<MetricsCollector>>) -> Self {
        self.metrics = metrics;
        self
    }

    fn write_run_script(work_dir: &Path, run: &WesRun) -> Result<std::path::PathBuf> {
        let script_path = work_dir.join("slurm_run.sh");
        let cmd = match run.workflow_type.to_lowercase().as_str() {
            "nextflow" | "nxf" => {
                format!("nextflow run {} 2>&1", run.workflow_url)
            }
            "cwl" => {
                let outdir = work_dir.join("out");
                format!(
                    "cwltool --outdir {} {} 2>&1",
                    outdir.display(),
                    run.workflow_url
                )
            }
            "wdl" => {
                format!("java -jar cromwell.jar run {} 2>&1", run.workflow_url)
            }
            "snakemake" => {
                format!("snakemake --snakefile {} --cores 1 2>&1", run.workflow_url)
            }
            _ => return Err(WesError::Validation("unsupported workflow type for Slurm".into())),
        };
        let content = format!(
            "#!/bin/bash\n#SBATCH --job-name=wes-{}\ncd \"{}\"\n{}\n",
            run.run_id,
            work_dir.display(),
            cmd
        );
        std::fs::write(&script_path, content).map_err(WesError::Io)?;
        Ok(script_path)
    }
}

#[async_trait]
impl WorkflowExecutor for SlurmExecutor {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![
            ("Nextflow".to_string(), vec!["22.10".to_string()]),
            ("CWL".to_string(), vec!["1.0".to_string(), "1.1".to_string()]),
            ("WDL".to_string(), vec!["1.0".to_string()]),
            ("Snakemake".to_string(), vec!["7".to_string()]),
        ]
    }

    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        _log_sink: Option<Arc<crate::log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let run_id = run.run_id.clone();
        let script_path = Self::write_run_script(work_dir, run)?;
        let work_dir_buf = work_dir.to_path_buf();
        let out = tokio::task::spawn_blocking(move || {
            Command::new("sbatch")
                .arg(&script_path)
                .current_dir(&work_dir_buf)
                .output()
        })
        .await
        .map_err(|e| WesError::Executor(e.to_string()))?;
        let out = out.map_err(|e| WesError::Executor(e.to_string()))?;
        if !out.status.success() {
            return Err(WesError::Executor(format!(
                "sbatch failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let job_id = stdout
            .trim()
            .lines()
            .last()
            .and_then(|l| l.strip_prefix("Submitted batch job "))
            .map(|s| s.trim().to_string())
            .ok_or_else(|| WesError::Executor("sbatch did not return job id".into()))?;
        self.run_to_job
            .write()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .insert(run_id.clone(), Tracker { job_id: job_id.clone() });
        Ok(ProcessHandle { run_id })
    }

    async fn cancel(&self, handle: &ProcessHandle) -> Result<()> {
        if let Some(t) = self
            .run_to_job
            .write()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .remove(&handle.run_id)
        {
            let _ = Command::new("scancel").arg(&t.job_id).status();
        }
        Ok(())
    }

    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)> {
        let job_id = {
            let guard = self
                .run_to_job
                .read()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?;
            guard.get(&handle.run_id).map(|t| t.job_id.clone())
        };
        let Some(job_id) = job_id else {
            return Ok((RunState::Unknown, None));
        };
        let job_id_clone = job_id.clone();
        let still_running = tokio::task::spawn_blocking(move || {
            let out = Command::new("squeue").args(["-j", &job_id_clone, "-h"]).output();
            out.ok().map(|o| !o.stdout.is_empty()).unwrap_or(false)
        })
        .await
        .map_err(|e| WesError::Executor(e.to_string()))?;
        if still_running {
            return Ok((RunState::Running, None));
        }
        self.run_to_job
            .write()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .remove(&handle.run_id);
        let rows = sacct_job(&job_id)?;
        let exit_code = rows.first().and_then(|_| {
            let out = Command::new("scontrol")
                .args(["show", "job", &job_id])
                .output();
            let stdout = out.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string())?;
            stdout
                .split("ExitCode=")
                .nth(1)
                .and_then(|s| s.split(':').next())
                .and_then(|s| s.parse::<i32>().ok())
        });
        let state = if exit_code == Some(0) {
            RunState::Complete
        } else {
            RunState::ExecutorError
        };
        if let Some(ref metrics) = self.metrics {
            for row in &rows {
                let memory_peak_mb = row
                    .max_rss
                    .as_deref()
                    .and_then(parse_max_rss)
                    .map(|b| b / (1024 * 1024));
                let _ = metrics
                    .insert_task_metrics(
                        &handle.run_id,
                        &row.job_id,
                        &row.job_id,
                        None,
                        None,
                        row.elapsed_raw.map(|x| x as i32),
                        None,
                        None,
                        None,
                        memory_peak_mb,
                        None,
                        None,
                        exit_code,
                        "slurm",
                        None,
                    )
                    .await;
            }
        }
        Ok((state, exit_code))
    }
}
