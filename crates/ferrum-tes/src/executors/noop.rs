//! No-op executor for demo/CI: marks tasks as complete immediately.

use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;

pub struct NoopExecutor;

impl NoopExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TaskExecutor for NoopExecutor {
    fn name(&self) -> &'static str {
        "noop"
    }

    async fn run(&self, _task_id: &str, _request: &CreateTaskRequest) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cancel(&self, _task_id: &str, _external_id: Option<&str>) -> Result<()> {
        Ok(())
    }

    async fn poll_state(&self, _task_id: &str, _external_id: Option<&str>) -> Result<TaskState> {
        Ok(TaskState::Complete)
    }
}
