//! Docker executor via bollard.
//!
//! Default behaviour is unchanged: one container from `executors[0]` with image, optional
//! entrypoint, command, env, workdir. **Opt-in** host settings (binds, network, platform) come
//! from `CreateTaskRequest.volumes` and/or **`FERRUM_TES_DOCKER_*`** environment variables on the
//! TES process — see **`docs/TES-DOCKER-BACKEND.md`**.

use crate::error::{Result, TesError};
use crate::executor::TaskExecutor;
use crate::types::{CreateTaskRequest, TaskState};
use async_trait::async_trait;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::models::{ContainerStateStatusEnum, HostConfig};
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

fn env_truthy(name: &str) -> bool {
    match std::env::var(name).map(|s| s.to_ascii_lowercase()) {
        Ok(s) if matches!(s.as_str(), "1" | "true" | "yes" | "on") => true,
        _ => false,
    }
}

/// Parse one TES `volumes[]` entry into a Docker bind string (`host:container[:opts]`).
fn parse_volume_entry(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        let t = s.trim();
        return (!t.is_empty()).then(|| t.to_string());
    }
    let obj = v.as_object()?;
    if let Some(bind) = obj.get("bind").and_then(|x| x.as_str()) {
        let t = bind.trim();
        return (!t.is_empty()).then(|| t.to_string());
    }
    let host = obj
        .get("host")
        .or_else(|| obj.get("Host"))
        .or_else(|| obj.get("source"))
        .and_then(|x| x.as_str())?;
    let dest = obj
        .get("container")
        .or_else(|| obj.get("Container"))
        .or_else(|| obj.get("target"))
        .and_then(|x| x.as_str())?;
    let mode = obj.get("mode").and_then(|x| x.as_str()).unwrap_or("rw");
    Some(format!("{}:{}:{}", host.trim(), dest.trim(), mode.trim()))
}

fn collect_binds(request: &CreateTaskRequest) -> Vec<String> {
    let mut binds: Vec<String> = request
        .volumes
        .as_ref()
        .map(|vols| {
            vols.iter()
                .filter_map(parse_volume_entry)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if env_truthy("FERRUM_TES_DOCKER_MOUNT_SOCKET") {
        binds.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
    }

    if let Ok(host_path) = std::env::var("FERRUM_TES_DOCKER_CLI_HOST_PATH") {
        let host_path = host_path.trim();
        if !host_path.is_empty() {
            let dest = std::env::var("FERRUM_TES_DOCKER_CLI_CONTAINER_PATH")
                .unwrap_or_else(|_| "/usr/local/bin/docker-host".to_string());
            let dest = dest.trim();
            binds.push(format!("{host_path}:{dest}:ro"));
        }
    }

    binds
}

fn build_host_config(binds: Vec<String>) -> Option<HostConfig> {
    let mut hc = HostConfig {
        binds: if binds.is_empty() { None } else { Some(binds) },
        ..Default::default()
    };

    if let Ok(nm) = std::env::var("FERRUM_TES_DOCKER_NETWORK_MODE") {
        let nm = nm.trim().to_string();
        if !nm.is_empty() {
            hc.network_mode = Some(nm);
        }
    }

    if let Ok(eh) = std::env::var("FERRUM_TES_DOCKER_EXTRA_HOSTS") {
        let parts: Vec<String> = eh
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            hc.extra_hosts = Some(parts);
        }
    }

    let empty = hc.binds.is_none() && hc.network_mode.is_none() && hc.extra_hosts.is_none();
    if empty {
        None
    } else {
        Some(hc)
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
        let binds = collect_binds(request);
        let host_config = build_host_config(binds);

        let config = Config {
            image: Some(exec.image.clone()),
            entrypoint: exec.entrypoint.clone(),
            cmd: Some(exec.command.clone()),
            env: exec.env.as_ref().map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
            }),
            working_dir: exec.workdir.clone(),
            host_config,
            ..Default::default()
        };

        let platform = std::env::var("FERRUM_TES_DOCKER_PLATFORM")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

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
        let state = inspect.state.as_ref().and_then(|s| s.status);
        match state {
            Some(ContainerStateStatusEnum::RUNNING) => Ok(TaskState::Running),
            Some(ContainerStateStatusEnum::EXITED) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_volume_string() {
        let v = serde_json::json!("/a:/b:rw");
        assert_eq!(parse_volume_entry(&v), Some("/a:/b:rw".to_string()));
    }

    #[test]
    fn parse_volume_object() {
        let v = serde_json::json!({"host": "/x", "container": "/y", "mode": "ro"});
        assert_eq!(parse_volume_entry(&v), Some("/x:/y:ro".to_string()));
    }
}
