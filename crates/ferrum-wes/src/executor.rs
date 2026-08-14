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

/// Backend for execution: local subprocess or Slurm. LSF is not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutorBackend {
    #[default]
    Local,
    Slurm,
}

impl std::str::FromStr for ExecutorBackend {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "slurm" => Ok(ExecutorBackend::Slurm),
            "lsf" => Err("LSF is not implemented; use backend \"local\" or \"slurm\"".into()),
            "local" | "" => Ok(ExecutorBackend::Local),
            other => Err(format!(
                "unknown WES executor backend {other:?}; expected \"local\" or \"slurm\""
            )),
        }
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

    /// Optional: PID of the main process for this run (for metrics sampling). Default None.
    fn process_id_for_metrics(&self, _run_id: &str) -> Option<u32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutorBackend;
    use std::str::FromStr;

    #[test]
    fn lsf_is_not_silently_mapped_to_local() {
        let err = ExecutorBackend::from_str("lsf").unwrap_err();
        assert!(err.contains("not implemented"));
        assert_eq!(
            ExecutorBackend::from_str("slurm").unwrap(),
            ExecutorBackend::Slurm
        );
        assert_eq!(
            ExecutorBackend::from_str("local").unwrap(),
            ExecutorBackend::Local
        );
        assert!(ExecutorBackend::from_str("kubernetes").is_err());
    }
}
