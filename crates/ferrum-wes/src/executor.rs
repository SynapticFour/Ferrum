//! Workflow executor trait and process handle.

use crate::error::Result;
use crate::log_stream::LogSink;
use crate::types::RunState;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

/// Opaque handle returned by submit(); used for cancel and poll_status.
/// Implementations map this to a process (local Child) or job id (slurm/lsf).
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub run_id: String,
}

/// In-memory run record used when submitting (work_dir is set by RunManager).
pub struct WesRun {
    pub run_id: String,
    pub workflow_url: String,
    pub workflow_type: String,
    pub workflow_type_version: String,
    pub workflow_params: serde_json::Value,
    pub workflow_engine_params: serde_json::Value,
    pub work_dir: Option<std::path::PathBuf>,
}

/// Backend for execution: local subprocess, slurm, or lsf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutorBackend {
    #[default]
    Local,
    Slurm,
    Lsf,
}

impl std::str::FromStr for ExecutorBackend {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "slurm" => ExecutorBackend::Slurm,
            "lsf" => ExecutorBackend::Lsf,
            _ => ExecutorBackend::Local,
        })
    }
}

/// Workflow executor: submit, cancel, poll status.
#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    /// Supported (workflow_type, versions) e.g. ("CWL", ["1.0", "1.1"]), ("Nextflow", ["22.10"]).
    fn supported_languages(&self) -> Vec<(String, Vec<String>)>;

    /// Submit a run. work_dir is the directory to run in; log_sink optionally receives stdout/stderr for streaming and file write.
    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        log_sink: Option<Arc<LogSink>>,
    ) -> Result<ProcessHandle>;

    /// Cancel a run.
    async fn cancel(&self, handle: &ProcessHandle) -> Result<()>;

    /// Current state and, when terminal, the process exit code.
    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)>;
}
