//! Podman executor: subprocess `podman run`.

use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

pub struct PodmanExecutor {
    running: Arc<RwLock<HashMap<String, tokio::process::Child>>>,
}

impl PodmanExecutor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for PodmanExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for PodmanExecutor {
    fn name(&self) -> &'static str {
        "podman"
    }

    async fn run(&self, task_id: &str, request: &CreateTaskRequest) -> Result<Option<String>> {
        if request.executors.is_empty() {
            return Err(TesError::Validation("executors required".into()));
        }
        let exec = request
            .executors
            .first()
            .cloned()
            .ok_or_else(|| TesError::Validation("executors required".into()))?;
        let mut cmd = Command::new("podman");
        cmd.arg("run")
            .arg("--rm")
            .arg("--name")
            .arg(format!("tes-{}", task_id));
        if let Some(ep) = &exec.entrypoint {
            if let Some(first) = ep.first() {
                cmd.arg("--entrypoint").arg(first);
            }
        }
        cmd.arg(&exec.image);
        if let Some(ep) = &exec.entrypoint {
            for arg in ep.iter().skip(1) {
                cmd.arg(arg);
            }
        }
        cmd.args(&exec.command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(env) = &exec.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        let child = cmd.spawn().map_err(|e| TesError::Executor(e.to_string()))?;
        self.running
            .write()
            .await
            .insert(task_id.to_string(), child);
        Ok(Some(format!("tes-{}", task_id)))
    }

    async fn cancel(&self, task_id: &str, _external_id: Option<&str>) -> Result<()> {
        let name = format!("tes-{}", task_id);
        let _ = Command::new("podman").args(["kill", &name]).output().await;
        if let Some(mut child) = self.running.write().await.remove(task_id) {
            let _ = child.start_kill();
        }
        Ok(())
    }

    async fn poll_state(&self, task_id: &str, _external_id: Option<&str>) -> Result<TaskState> {
        let mut guard = self.running.write().await;
        if let Some(child) = guard.get_mut(task_id) {
            match child.try_wait().map_err(TesError::Io)? {
                Some(s) => {
                    guard.remove(task_id);
                    return Ok(if s.success() {
                        TaskState::Complete
                    } else {
                        TaskState::ExecutorError
                    });
                }
                None => return Ok(TaskState::Running),
            }
        }
        Ok(TaskState::Unknown)
    }
}
