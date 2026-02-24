//! Slurm executor: generate job script and submit via sbatch.

use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
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

#[async_trait]
impl TaskExecutor for SlurmExecutor {
    fn name(&self) -> &'static str {
        "slurm"
    }

    async fn run(&self, task_id: &str, request: &CreateTaskRequest) -> Result<Option<String>> {
        if request.executors.is_empty() {
            return Err(TesError::Validation("executors required".into()));
        }
        let exec = &request.executors[0];
        let job_name = format!("tes-{}", task_id);
        let script = format!(
            "#!/bin/bash\n#SBATCH --job-name={}\n#SBATCH --output=tes-{}-%j.out\n#SBATCH --error=tes-{}-%j.err\n#SBATCH --wrap=\"podman run --rm {} {}\"\n",
            job_name,
            task_id,
            task_id,
            exec.image,
            exec.command.join(" ")
        );
        let script_path = self.work_dir.join(format!("{}.sh", task_id));
        tokio::fs::write(&script_path, script).await.map_err(TesError::Io)?;
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
        let job_id = stdout
            .trim()
            .strip_prefix("Submitted batch job ")
            .unwrap_or(stdout.trim())
            .trim()
            .to_string();
        Ok(Some(job_id))
    }

    async fn cancel(&self, task_id: &str, external_id: Option<&str>) -> Result<()> {
        let job_id = external_id.unwrap_or(task_id);
        let _ = Command::new("scancel").arg(job_id).output().await;
        Ok(())
    }

    async fn poll_state(&self, task_id: &str, external_id: Option<&str>) -> Result<TaskState> {
        let job_id = external_id.unwrap_or(task_id);
        let out = Command::new("scontrol")
            .args(["show", "job", job_id])
            .output()
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("JobState=PENDING") || stdout.contains("JobState=CONFIGURING") {
            return Ok(TaskState::Queued);
        }
        if stdout.contains("JobState=RUNNING") {
            return Ok(TaskState::Running);
        }
        if stdout.contains("JobState=COMPLETED") {
            return Ok(TaskState::Complete);
        }
        if stdout.contains("JobState=CANCELLED") || stdout.contains("JobState=FAILED") {
            return Ok(TaskState::ExecutorError);
        }
        Ok(TaskState::Unknown)
    }
}
