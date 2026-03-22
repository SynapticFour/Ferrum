//! Docker executor via bollard.

use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::Docker;

pub struct DockerExecutor {
    docker: Docker,
}

impl DockerExecutor {
    pub fn new(docker: Docker) -> Self {
        Self { docker }
    }

    pub fn connect_default() -> std::result::Result<Self, bollard::errors::Error> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self::new(docker))
    }
}

#[async_trait]
impl TaskExecutor for DockerExecutor {
    fn name(&self) -> &'static str {
        "docker"
    }

    async fn run(&self, task_id: &str, request: &CreateTaskRequest) -> Result<Option<String>> {
        if request.executors.is_empty() {
            return Err(TesError::Validation("executors required".into()));
        }
        let exec = &request.executors[0];
        let name = format!("tes-{}", task_id);
        let config = Config {
            image: Some(exec.image.clone()),
            entrypoint: exec.entrypoint.clone(),
            cmd: Some(exec.command.clone()),
            env: exec.env.as_ref().map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
            }),
            ..Default::default()
        };
        let opts = CreateContainerOptions { name };
        let create = self
            .docker
            .create_container(Some(opts), config)
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        let id = create.id.clone();
        self.docker
            .start_container(&id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        Ok(Some(id))
    }

    async fn cancel(&self, _task_id: &str, external_id: Option<&str>) -> Result<()> {
        if let Some(id) = external_id {
            let _ = self.docker.stop_container(id, None).await;
        }
        Ok(())
    }

    async fn poll_state(&self, _task_id: &str, external_id: Option<&str>) -> Result<TaskState> {
        let Some(id) = external_id else {
            return Ok(TaskState::Unknown);
        };
        let inspect = self
            .docker
            .inspect_container(id, None)
            .await
            .map_err(|e| TesError::Executor(e.to_string()))?;
        let state = inspect.state.and_then(|s| s.status);
        match state.as_deref() {
            Some("running") => Ok(TaskState::Running),
            Some("exited") => {
                let exit = inspect.state.and_then(|s| s.exit_code).unwrap_or(1);
                Ok(if exit == 0 {
                    TaskState::Complete
                } else {
                    TaskState::ExecutorError
                })
            }
            _ => Ok(TaskState::Unknown),
        }
    }
}
