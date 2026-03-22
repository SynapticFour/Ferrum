//! GA4GH TES 1.1 types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Task state (TES 1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    Unknown,
    Queued,
    Initializing,
    Running,
    Paused,
    Complete,
    ExecutorError,
    SystemError,
    Canceled,
    Canceling,
}

impl TaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Unknown => "UNKNOWN",
            TaskState::Queued => "QUEUED",
            TaskState::Initializing => "INITIALIZING",
            TaskState::Running => "RUNNING",
            TaskState::Paused => "PAUSED",
            TaskState::Complete => "COMPLETE",
            TaskState::ExecutorError => "EXECUTOR_ERROR",
            TaskState::SystemError => "SYSTEM_ERROR",
            TaskState::Canceled => "CANCELED",
            TaskState::Canceling => "CANCELING",
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "QUEUED" => TaskState::Queued,
            "INITIALIZING" => TaskState::Initializing,
            "RUNNING" => TaskState::Running,
            "PAUSED" => TaskState::Paused,
            "COMPLETE" => TaskState::Complete,
            "EXECUTOR_ERROR" => TaskState::ExecutorError,
            "SYSTEM_ERROR" => TaskState::SystemError,
            "CANCELED" => TaskState::Canceled,
            "CANCELING" => TaskState::Canceling,
            _ => TaskState::Unknown,
        }
    }
}

/// Executor: image + command (TES 1.1).
///
/// **Docker / Podman:** If the image defines an `ENTRYPOINT`, your `command` is passed as
/// additional arguments to that entrypoint. To run a shell wrapper (e.g. `bash -lc '…'`) instead,
/// set [`Self::entrypoint`] explicitly (e.g. `["/bin/bash", "-lc"]`) and put the script line in
/// `command`, or clear the image entrypoint via `["/bin/sh", "-c"]` plus a single joined command
/// string — see **`docs/TES-DOCKER-BACKEND.md`** in the Ferrum repository.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TesExecutor {
    pub image: String,
    pub command: Vec<String>,
    /// Overrides the container image entrypoint (Docker `Entrypoint`). When set, `command` follows
    /// as argv after the entrypoint vector (Docker API semantics).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Input file (url -> path in container).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TesInput {
    pub url: String,
    pub path: String,
}

/// Output path to capture.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TesOutput {
    pub path: String,
}

/// Create task request (POST /tasks body).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<TesInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<TesOutput>>,
    pub executors: Vec<TesExecutor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<std::collections::HashMap<String, String>>,
}

/// Full task (response).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    pub id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<TesInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<TesOutput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executors: Option<Vec<TesExecutor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<TaskLog>>,
}

/// Task log (executor log).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskLog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// Create task response (task id).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateTaskResponse {
    pub id: String,
}

/// List tasks response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskSummary {
    pub id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TesServiceInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}
