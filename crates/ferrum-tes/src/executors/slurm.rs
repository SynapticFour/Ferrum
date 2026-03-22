//! Slurm executor: generate job script and submit via sbatch.

use super::slurm_compat;
use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::json;
use std::path::PathBuf;
use tokio::process::Command;

pub struct SlurmExecutor {
    work_dir: PathBuf,
}

impl SlurmExecutor {
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }
}

/// `podman run` invocation for Slurm `#SBATCH --wrap` (space-separated; avoid spaces in args when possible).
fn build_podman_cli_line(exec: &crate::types::TesExecutor) -> String {
    let mut parts: Vec<String> = vec!["podman".into(), "run".into(), "--rm".into()];
    if let Some(ep) = &exec.entrypoint {
        if let Some(first) = ep.first() {
            parts.push("--entrypoint".into());
            parts.push(first.clone());
        }
    }
    parts.push(exec.image.clone());
    if let Some(ep) = &exec.entrypoint {
        for x in ep.iter().skip(1) {
            parts.push(x.clone());
        }
    }
    parts.extend(exec.command.iter().cloned());
    parts.join(" ")
}

const DEFAULT_SUBMIT_TEMPLATE: &str = r#"#!/bin/bash
#SBATCH --job-name={{job_name}}
#SBATCH --output={{output}}
#SBATCH --error={{error}}
#SBATCH --wrap="{{executor_command}}"
"#;

fn render_submit_script(task_id: &str, request: &CreateTaskRequest) -> Result<String> {
    let exec = request
        .executors
        .first()
        .ok_or_else(|| TesError::Validation("executors required".into()))?;

    let job_name = format!("tes-{}", task_id);
    let output = format!("tes-{}-%j.out", task_id);
    let error = format!("tes-{}-%j.err", task_id);
    let executor_command = build_podman_cli_line(exec);

    let mut hb = Handlebars::new();
    hb.register_template_string("submit", DEFAULT_SUBMIT_TEMPLATE)
        .map_err(|e| TesError::Executor(format!("handlebars: {}", e)))?;

    let data = json!({
        "job_name": job_name,
        "output": output,
        "error": error,
        "executor_command": executor_command,
    });

    hb.render("submit", &data)
        .map_err(|e| TesError::Executor(format!("handlebars render: {}", e)))
}

/// Parse `sbatch` stdout like:
/// - `Submitted batch job 12345`
/// - `Submitted batch job 12345_1` (array job)
/// into the base job id `12345`.
fn parse_base_job_id(stdout: &str) -> Result<String> {
    let token = stdout
        .trim()
        .split_whitespace()
        .last()
        .ok_or_else(|| TesError::Executor("sbatch: missing stdout token".into()))?;

    // Array jobs can carry suffixes like `12345_1`.
    let base = token.split('_').next().unwrap_or(token).trim();
    if base.is_empty() {
        return Err(TesError::Executor(
            "sbatch: could not extract base job id".into(),
        ));
    }
    Ok(base.to_string())
}

fn squeue_reason_is_retriable(reason: &str) -> bool {
    const RETRIABLE_SLURM_REASONS: &[&str] = &[
        "No suitable partition available",
        "ReqNodeNotAvail",
        "Resources",
        "Priority",
        "QOSMaxJobsPerUserLimit",
    ];

    RETRIABLE_SLURM_REASONS.iter().any(|r| reason.contains(r))
}

fn map_squeue_state_to_task_state(state: &str, reason: &str) -> TaskState {
    // Learned from Funnel/SLURM reference behavior: `squeue` is transient,
    // and PENDING-with-reason is not necessarily a failure.
    match state {
        "PENDING" => {
            if squeue_reason_is_retriable(reason) {
                TaskState::Queued
            } else {
                TaskState::Initializing
            }
        }
        "RUNNING" | "COMPLETING" => TaskState::Running,
        _ => TaskState::Unknown,
    }
}

fn map_sacct_state_to_task_state(state: &str) -> TaskState {
    // Learned from Funnel/SLURM reference behavior: use `sacct` for terminal states.
    match state.trim() {
        "COMPLETED" => TaskState::Complete,
        s if s.starts_with("FAILED") => TaskState::ExecutorError,
        s if s.starts_with("CANCELLED") => TaskState::Canceled,
        "TIMEOUT" | "NODE_FAIL" => TaskState::SystemError,
        s if s.starts_with("OUT_OF_MEMORY") => TaskState::ExecutorError,
        _ => TaskState::Unknown,
    }
}

fn extract_first_sacct_state(stdout: &str) -> Option<&str> {
    // With `--parsable2 --noheader`, slurm often emits multiple lines (steps).
    // We take the first non-empty token.
    stdout
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .and_then(|l| l.split('|').next())
        .map(|s| s.trim())
}

#[async_trait]
impl TaskExecutor for SlurmExecutor {
    fn name(&self) -> &'static str {
        "slurm"
    }

    async fn run(&self, task_id: &str, request: &CreateTaskRequest) -> Result<Option<String>> {
        slurm_compat::warn_old_glibc_process_spawn_if_needed();
        if request.executors.is_empty() {
            return Err(TesError::Validation("executors required".into()));
        }
        let script = render_submit_script(task_id, request)?;
        let script_path = self.work_dir.join(format!("{}.sh", task_id));
        tokio::fs::write(&script_path, script)
            .await
            .map_err(TesError::Io)?;
        let out = Command::new("sbatch")
            .arg(&script_path)
            .output()
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(TesError::Executor(format!("sbatch failed: {}", stderr)));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let job_id = parse_base_job_id(&stdout)?;
        Ok(Some(job_id))
    }

    async fn cancel(&self, task_id: &str, external_id: Option<&str>) -> Result<()> {
        let job_id = external_id.unwrap_or(task_id);
        let _ = Command::new("scancel").arg(job_id).output().await;
        Ok(())
    }

    async fn poll_state(&self, task_id: &str, external_id: Option<&str>) -> Result<TaskState> {
        let job_id = external_id.unwrap_or(task_id);

        // 1) squeue for transient / non-terminal states.
        // Format: `<STATE>|<REASON>`
        let squeue_out = Command::new("squeue")
            .args(["-j", job_id, "-h", "-o", "%T|%R"])
            .output()
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        let squeue_stdout = String::from_utf8_lossy(&squeue_out.stdout);

        if !squeue_stdout.trim().is_empty() {
            // Example line: `PENDING|No suitable partition available`
            let mut parts = squeue_stdout.trim().splitn(2, '|');
            let state = parts.next().unwrap_or("").trim();
            let reason = parts.next().unwrap_or("").trim();
            return Ok(map_squeue_state_to_task_state(state, reason));
        }

        // 2) squeue has no record (job likely left the queue): fallback to sacct.
        // Example token: `COMPLETED`
        let sacct_out = Command::new("sacct")
            .args(["-j", job_id, "--format=State", "--noheader", "--parsable2"])
            .output()
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        let sacct_stdout = String::from_utf8_lossy(&sacct_out.stdout);

        if let Some(state) = extract_first_sacct_state(&sacct_stdout) {
            return Ok(map_sacct_state_to_task_state(state));
        }

        Ok(TaskState::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CreateTaskRequest;
    use crate::types::TesExecutor;

    #[test]
    fn test_parse_base_job_id_normal() {
        let stdout = "Submitted batch job 12345\n";
        assert_eq!(parse_base_job_id(stdout).unwrap(), "12345");
    }

    #[test]
    fn test_parse_base_job_id_array() {
        let stdout = "Submitted batch job 12345_1\n";
        assert_eq!(parse_base_job_id(stdout).unwrap(), "12345");
    }

    #[test]
    fn test_map_squeue_pending_retriable_reason_to_queued() {
        let state = map_squeue_state_to_task_state(
            "PENDING",
            "No suitable partition available for some reason",
        );
        assert_eq!(state, TaskState::Queued);
    }

    #[test]
    fn test_map_squeue_pending_non_retriable_to_initializing() {
        let state = map_squeue_state_to_task_state("PENDING", "Some other reason");
        assert_eq!(state, TaskState::Initializing);
    }

    #[test]
    fn test_map_sacct_completed() {
        assert_eq!(
            map_sacct_state_to_task_state("COMPLETED"),
            TaskState::Complete
        );
    }

    #[test]
    fn test_map_sacct_failed() {
        assert_eq!(
            map_sacct_state_to_task_state("FAILED"),
            TaskState::ExecutorError
        );
    }

    #[test]
    fn test_map_sacct_cancelled_substate() {
        assert_eq!(
            map_sacct_state_to_task_state("CANCELLED+COMPLETED"),
            TaskState::Canceled
        );
    }

    #[test]
    fn test_render_submit_script_includes_wrap_command() {
        let req = CreateTaskRequest {
            name: None,
            description: None,
            inputs: None,
            outputs: None,
            executors: vec![TesExecutor {
                image: "alpine:3.20".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                entrypoint: None,
                workdir: None,
                stdin: None,
                stdout: None,
                stderr: None,
                env: None,
            }],
            resources: None,
            volumes: None,
            tags: None,
        };

        let script = render_submit_script("task1", &req).unwrap();
        assert!(script.contains("#SBATCH --job-name=tes-task1"));
        assert!(script.contains(r#"--wrap="podman run --rm alpine:3.20 echo hello""#));
    }

    #[test]
    fn test_render_submit_script_entrypoint_before_image() {
        let req = CreateTaskRequest {
            name: None,
            description: None,
            inputs: None,
            outputs: None,
            executors: vec![TesExecutor {
                image: "img:latest".to_string(),
                command: vec!["-c".into(), "echo ok".into()],
                entrypoint: Some(vec!["/bin/sh".into()]),
                workdir: None,
                stdin: None,
                stdout: None,
                stderr: None,
                env: None,
            }],
            resources: None,
            volumes: None,
            tags: None,
        };
        let script = render_submit_script("t2", &req).unwrap();
        assert!(script.contains(
            r#"--wrap="podman run --rm --entrypoint /bin/sh img:latest -c echo ok""#
        ));
    }
}
