//! Task executor trait and backends.

use crate::error::Result;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Backend name (e.g. "podman", "docker", "slurm").
    fn name(&self) -> &'static str;

    /// Run a task. Returns external_id (e.g. job id) if backend provides one.
    async fn run(&self, task_id: &str, request: &CreateTaskRequest) -> Result<Option<String>>;

    /// Cancel a running task (best-effort).
    async fn cancel(&self, task_id: &str, external_id: Option<&str>) -> Result<()>;

    /// Poll current state (if backend supports it). Default returns Unknown.
    async fn poll_state(&self, _task_id: &str, _external_id: Option<&str>) -> Result<TaskState> {
        Ok(TaskState::Unknown)
    }
}

pub type ExecutorBackend = Arc<dyn TaskExecutor>;
