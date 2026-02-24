//! GA4GH Task Execution Service (TES) as a WES execution backend.
//! Submits each WES run as a single TES task (container running the workflow engine).

use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::error::{Result, WesError};
use crate::types::RunState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

/// Minimal TES task for CreateTask (GA4GH TES 1.1).
#[derive(Debug, Serialize)]
struct TesTaskRequest {
    executors: Vec<TesExecutor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inputs: Option<Vec<TesInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<TesOutput>>,
}

#[derive(Debug, Serialize)]
struct TesExecutor {
    image: String,
    command: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TesInput {
    url: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct TesOutput {
    path: String,
}

#[derive(Debug, Deserialize)]
struct TesTaskResponse {
    id: String,
    state: Option<String>,
}

pub struct TesExecutorBackend {
    base_url: String,
    client: reqwest::Client,
    /// run_id -> TES task id
    run_to_task: RwLock<HashMap<String, String>>,
}

impl TesExecutorBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            run_to_task: RwLock::new(HashMap::new()),
        }
    }

    fn default_image_and_command(workflow_type: &str, workflow_url: &str) -> (String, Vec<String>) {
        match workflow_type.to_lowercase().as_str() {
            "nextflow" | "nxf" => (
                "nextflow/nextflow:latest".to_string(),
                vec!["nextflow".to_string(), "run".to_string(), workflow_url.to_string()],
            ),
            "cwl" => (
                "quay.io/commonwl/cwltool:latest".to_string(),
                vec!["cwltool".to_string(), workflow_url.to_string()],
            ),
            "wdl" => (
                "broadinstitute/cromwell:latest".to_string(),
                vec![
                    "java".to_string(),
                    "-jar".to_string(),
                    "/app/cromwell.jar".to_string(),
                    "run".to_string(),
                    workflow_url.to_string()],
            ),
            "snakemake" => (
                "snakemake/snakemake:latest".to_string(),
                vec!["snakemake".to_string(), "--snakefile".to_string(), workflow_url.to_string(), "--cores".to_string(), "1".to_string()],
            ),
            _ => (
                "alpine:latest".to_string(),
                vec!["echo".to_string(), format!("workflow {}", workflow_url)],
            ),
        }
    }
}

#[async_trait]
impl WorkflowExecutor for TesExecutorBackend {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![
            ("Nextflow".to_string(), vec!["22.10".to_string(), "23.04".to_string()]),
            ("CWL".to_string(), vec!["1.0".to_string(), "1.1".to_string(), "1.2".to_string()]),
            ("WDL".to_string(), vec!["1.0".to_string(), "1.1".to_string()]),
            ("Snakemake".to_string(), vec!["7".to_string()]),
        ]
    }

    async fn submit(
        &self,
        run: &WesRun,
        _work_dir: &Path,
        _log_sink: Option<Arc<crate::log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let (image, command) = Self::default_image_and_command(&run.workflow_type, &run.workflow_url);
        let body = TesTaskRequest {
            executors: vec![TesExecutor { image, command }],
            inputs: None,
            outputs: None,
        };
        let url = format!("{}/tasks", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WesError::Executor(format!("TES create task: {}", e)))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| WesError::Executor(e.to_string()))?;
        if !status.is_success() {
            return Err(WesError::Executor(format!("TES returned {}: {}", status, text)));
        }
        let task: TesTaskResponse = serde_json::from_str(&text)
            .map_err(|e| WesError::Executor(format!("TES response parse: {}", e)))?;
        let run_id = run.run_id.clone();
        self.run_to_task
            .write()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .insert(run_id.clone(), task.id.clone());
        Ok(ProcessHandle { run_id })
    }

    async fn cancel(&self, handle: &ProcessHandle) -> Result<()> {
        let task_id = self
            .run_to_task
            .read()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .get(&handle.run_id)
            .cloned();
        if let Some(id) = task_id {
            let url = format!("{}/tasks/{}:cancel", self.base_url, id);
            let _ = self.client.post(&url).send().await;
            self.run_to_task
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(&handle.run_id);
        }
        Ok(())
    }

    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)> {
        let task_id = self
            .run_to_task
            .read()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .get(&handle.run_id)
            .cloned();
        let Some(id) = task_id else {
            return Ok((RunState::Unknown, None));
        };
        let url = format!("{}/tasks/{}", self.base_url, id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| WesError::Executor(format!("TES get task: {}", e)))?;
        if !resp.status().is_success() {
            return Ok((RunState::Unknown, None));
        }
        let text = resp.text().await.map_err(|e| WesError::Executor(e.to_string()))?;
        let task: TesTaskResponse = serde_json::from_str(&text).unwrap_or(TesTaskResponse {
            id: id.clone(),
            state: Some("UNKNOWN".to_string()),
        });
        let state = match task.state.as_deref().unwrap_or("UNKNOWN") {
            "QUEUED" => RunState::Queued,
            "INITIALIZING" => RunState::Initializing,
            "RUNNING" => RunState::Running,
            "PAUSED" => RunState::Paused,
            "COMPLETE" => RunState::Complete,
            "EXECUTOR_ERROR" => RunState::ExecutorError,
            "SYSTEM_ERROR" => RunState::SystemError,
            "CANCELED" | "CANCELING" => RunState::Canceled,
            _ => RunState::Unknown,
        };
        if state != RunState::Running && state != RunState::Queued && state != RunState::Initializing && state != RunState::Paused && state != RunState::Unknown {
            self.run_to_task
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(&handle.run_id);
        }
        Ok((state, None))
    }
}
