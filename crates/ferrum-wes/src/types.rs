//! WES 1.1 API types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Run state (GA4GH WES 1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunState {
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
    Preempted,
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::Unknown => "UNKNOWN",
            RunState::Queued => "QUEUED",
            RunState::Initializing => "INITIALIZING",
            RunState::Running => "RUNNING",
            RunState::Paused => "PAUSED",
            RunState::Complete => "COMPLETE",
            RunState::ExecutorError => "EXECUTOR_ERROR",
            RunState::SystemError => "SYSTEM_ERROR",
            RunState::Canceled => "CANCELED",
            RunState::Canceling => "CANCELING",
            RunState::Preempted => "PREEMPTED",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "QUEUED" => RunState::Queued,
            "INITIALIZING" => RunState::Initializing,
            "RUNNING" => RunState::Running,
            "PAUSED" => RunState::Paused,
            "COMPLETE" => RunState::Complete,
            "EXECUTOR_ERROR" => RunState::ExecutorError,
            "SYSTEM_ERROR" => RunState::SystemError,
            "CANCELED" => RunState::Canceled,
            "CANCELING" => RunState::Canceling,
            "PREEMPTED" => RunState::Preempted,
            _ => RunState::Unknown,
        }
    }
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// GET /runs/{run_id}/status response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunStatus {
    pub run_id: String,
    pub state: RunState,
}

/// Run summary in list (includes timing and tags).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunSummary {
    pub run_id: String,
    pub state: RunState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    pub tags: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumed_from_run_id: Option<String>,
}

/// GET /runs response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunListResponse {
    pub runs: Vec<RunSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Run request (from multipart form).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RunRequest {
    pub workflow_params: Option<serde_json::Value>,
    pub workflow_type: String,
    pub workflow_type_version: String,
    pub workflow_url: String,
    pub tags: Option<serde_json::Value>,
    pub workflow_engine_parameters: Option<serde_json::Value>,
    pub workflow_engine: Option<String>,
    pub workflow_engine_version: Option<String>,
}

/// Log entry (main run or task).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Log {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
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

/// Task log (with id).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskLog {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
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

/// GET /runs/{run_id} full log response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunLog {
    pub run_id: String,
    pub request: RunRequestRef,
    pub state: RunState,
    pub run_log: Log,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_logs: Option<Vec<TaskLog>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_logs_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Value>,
    /// GA4GH extension: implementation-specific fields (e.g. ferrum:multiqc_status, ferrum:multiqc_report_drs_id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// Minimal request reference in RunLog.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunRequestRef {
    pub workflow_type: String,
    pub workflow_type_version: String,
    pub workflow_url: String,
}

/// POST /runs response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunIdResponse {
    pub run_id: String,
}

/// Workflow type version (supported versions for a type).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkflowTypeVersion {
    pub workflow_type_version: Vec<String>,
}

/// Workflow engine version.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkflowEngineVersion {
    pub workflow_engine_version: Vec<String>,
}

/// ServiceInfo (GA4GH) with workflow types and engines.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WesServiceInfo {
    pub id: String,
    pub name: String,
    pub r#type: ServiceType,
    pub description: Option<String>,
    pub organization: Organization,
    pub version: String,
    pub workflow_type_versions: std::collections::HashMap<String, WorkflowTypeVersion>,
    pub supported_wes_versions: Vec<String>,
    pub supported_filesystem_protocols: Vec<String>,
    pub workflow_engine_versions: std::collections::HashMap<String, WorkflowEngineVersion>,
    pub default_workflow_engine_parameters: Vec<DefaultWorkflowEngineParameter>,
    pub system_state_counts: std::collections::HashMap<String, i64>,
    pub auth_instructions_url: String,
    pub tags: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ServiceType {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Organization {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DefaultWorkflowEngineParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: Option<String>,
    pub default_value: Option<String>,
}
