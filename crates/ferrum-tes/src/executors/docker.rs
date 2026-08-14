//! Docker executor via bollard.
//! GA4GH demo: volume binds from the task (WES symmetric workdir), docker.sock, static docker CLI,
//! compose network mode, and explicit executor entrypoint/cmd (Cromwell / Nextflow).

use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, StartContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{ContainerStateStatusEnum, HostConfig};
use bollard::Docker;
use futures_util::StreamExt;

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

    fn split_image_ref(image: &str) -> (String, String) {
        let image = image.trim();
        if let Some((name, tag)) = image.rsplit_once(':') {
            if !tag.contains('/') {
                return (name.to_string(), tag.to_string());
            }
        }
        (image.to_string(), "latest".to_string())
    }

    async fn ensure_image(&self, image: &str) -> Result<()> {
        let platform = std::env::var("FERRUM_TES_DOCKER_PLATFORM")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if platform.is_none() && self.docker.inspect_image(image).await.is_ok() {
            return Ok(());
        }
        let (from_image, tag) = Self::split_image_ref(image);
        tracing::info!(image = %image, ?platform, "TES Docker: pulling image");
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: from_image.as_str(),
                tag: tag.as_str(),
                platform: platform.as_deref().unwrap_or(""),
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(item) = stream.next().await {
            item.map_err(|e| TesError::Executor(format!("docker pull {image}: {e}")))?;
        }
        Ok(())
    }

    fn collect_binds(request: &CreateTaskRequest) -> Result<Vec<String>> {
        let mut binds = Vec::new();
        if std::env::var("FERRUM_TES_DOCKER_MOUNT_SOCKET")
            .map(|s| {
                matches!(
                    s.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
        {
            binds.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
        }
        if let (Ok(host), Ok(cont)) = (
            std::env::var("FERRUM_TES_DOCKER_CLI_HOST_PATH"),
            std::env::var("FERRUM_TES_DOCKER_CLI_CONTAINER_PATH"),
        ) {
            let host = host.trim();
            let cont = cont.trim();
            if !host.is_empty() && !cont.is_empty() {
                binds.push(format!("{}:{}:ro", host, cont));
            }
        }
        if let Ok(extra) = std::env::var("FERRUM_TES_EXTRA_BINDS") {
            for p in extra.split(',') {
                let p = p.trim();
                if !p.is_empty() {
                    binds.push(p.to_string());
                }
            }
        }
        if let Some(vols) = &request.volumes {
            binds.extend(crate::bind_policy::request_volume_binds(
                vols,
                &crate::bind_policy::allowed_bind_prefixes(),
            )?);
        }
        Ok(binds)
    }

    /// Prefer explicit TES executor entrypoint/cmd (WES bash/file modes). Fall back to the
    /// flattened `sh -lc script` shape for older callers.
    fn entrypoint_and_cmd(
        exec: &crate::types::TesExecutor,
    ) -> (Option<Vec<String>>, Option<Vec<String>>) {
        if let Some(ep) = &exec.entrypoint {
            let cmd = if exec.command.is_empty() {
                None
            } else {
                Some(exec.command.clone())
            };
            return (Some(ep.clone()), cmd);
        }
        let shell_bin = exec.command.first().map(|s| s.as_str());
        let is_shell = matches!(shell_bin, Some("sh" | "bash" | "/bin/sh" | "/bin/bash"));
        if exec.command.len() == 3
            && is_shell
            && (exec.command[1] == "-lc" || exec.command[1] == "-c")
        {
            return (
                Some(vec![exec.command[0].clone(), exec.command[1].clone()]),
                Some(vec![exec.command[2].clone()]),
            );
        }
        if exec.command.is_empty() {
            (None, None)
        } else {
            (None, Some(exec.command.clone()))
        }
    }

    async fn fetch_container_logs(&self, id: &str) -> Result<(String, String)> {
        let opts = Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: "all".to_string(),
            ..Default::default()
        });
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut stream = self.docker.logs(id, opts);
        while let Some(chunk) = stream.next().await {
            match chunk.map_err(|e| TesError::Executor(e.to_string()))? {
                LogOutput::StdOut { message } | LogOutput::Console { message } => {
                    stdout.push_str(&String::from_utf8_lossy(&message));
                }
                LogOutput::StdErr { message } => {
                    stderr.push_str(&String::from_utf8_lossy(&message));
                }
                LogOutput::StdIn { .. } => {}
            }
        }
        Ok((stdout, stderr))
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
        self.ensure_image(&exec.image).await?;
        let name = format!("tes-{}", task_id);
        let binds = Self::collect_binds(request)?;
        let (entrypoint, cmd) = Self::entrypoint_and_cmd(exec);
        let network_mode = std::env::var("FERRUM_TES_DOCKER_NETWORK_MODE")
            .or_else(|_| std::env::var("FERRUM_TES_DOCKER_NETWORK"))
            .ok()
            .filter(|s| !s.trim().is_empty());
        let extra_hosts = std::env::var("FERRUM_TES_DOCKER_EXTRA_HOSTS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());
        let host_config = HostConfig {
            binds: if binds.is_empty() { None } else { Some(binds) },
            network_mode,
            extra_hosts,
            ..Default::default()
        };
        let config = Config {
            image: Some(exec.image.clone()),
            entrypoint,
            cmd,
            working_dir: exec.workdir.clone(),
            env: exec.env.as_ref().map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
            }),
            host_config: Some(host_config),
            ..Default::default()
        };
        let platform = std::env::var("FERRUM_TES_DOCKER_PLATFORM")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let opts = CreateContainerOptions { name, platform };
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
        let status = inspect.state.as_ref().and_then(|s| s.status.clone());
        match status {
            Some(ContainerStateStatusEnum::RUNNING) => Ok(TaskState::Running),
            Some(ContainerStateStatusEnum::EXITED) => {
                let exit = inspect
                    .state
                    .as_ref()
                    .and_then(|s| s.exit_code)
                    .unwrap_or(1);
                Ok(if exit == 0 {
                    TaskState::Complete
                } else {
                    TaskState::ExecutorError
                })
            }
            _ => Ok(TaskState::Unknown),
        }
    }

    async fn fetch_logs(
        &self,
        _task_id: &str,
        external_id: Option<&str>,
    ) -> Result<Option<(String, String)>> {
        let Some(id) = external_id else {
            return Ok(None);
        };
        Ok(Some(self.fetch_container_logs(id).await?))
    }
}
