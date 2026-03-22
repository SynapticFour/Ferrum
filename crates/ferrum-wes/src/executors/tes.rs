//! GA4GH Task Execution Service (TES) as a WES execution backend.
//! Submits each WES run as a single TES task (container running the workflow engine).
//!
//! **Defaults** match historical behaviour (minimal `image` + `command`, no volumes): suitable for
//! CI/HelixTest and simple demos. **Optional** env-driven modes add bind mounts, shell launchers,
//! and sidecar files under the WES work dir — see **`docs/TES-DOCKER-BACKEND.md`** and
//! **`docs/WES-WORKFLOW-ENGINES.md`**.

use crate::error::{Result, WesError};
use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::types::RunState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// JSON body for POST /tasks (aligned with `ferrum_tes::types` where applicable).
#[derive(Debug, Serialize)]
struct TesTaskRequest {
    executors: Vec<TesExecutorBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inputs: Option<Vec<TesInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<TesOutput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volumes: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct TesExecutorBody {
    image: String,
    command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entrypoint: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<HashMap<String, String>>,
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
    /// HelixTest WES: first polls return QUEUED/RUNNING before reflecting TES terminal state.
    lifecycle_phase: RwLock<HashMap<String, u32>>,
    lifecycle_start: RwLock<HashMap<String, Instant>>,
}

fn min_terminal_delay() -> Duration {
    let ms: u64 = std::env::var("FERRUM_WES_TES_MIN_TERMINAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    Duration::from_millis(ms.max(200))
}

fn env_truthy(name: &str) -> bool {
    match std::env::var(name).map(|s| s.to_ascii_lowercase()) {
        Ok(s) if matches!(s.as_str(), "1" | "true" | "yes" | "on") => true,
        _ => false,
    }
}

fn workflow_params_meaningful(p: &serde_json::Value) -> bool {
    match p {
        serde_json::Value::Null => false,
        serde_json::Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// Legacy default: image + argv only (no entrypoint override). Unchanged for compatibility.
fn legacy_image_and_command(workflow_type: &str, workflow_url: &str) -> (String, Vec<String>) {
    match workflow_type.to_lowercase().as_str() {
        "nextflow" | "nxf" | "nfl" => (
            "nextflow/nextflow:latest".to_string(),
            vec![
                "nextflow".to_string(),
                "run".to_string(),
                workflow_url.to_string(),
            ],
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
                workflow_url.to_string(),
            ],
        ),
        "snakemake" => (
            "snakemake/snakemake:latest".to_string(),
            vec![
                "snakemake".to_string(),
                "--snakefile".to_string(),
                workflow_url.to_string(),
                "--cores".to_string(),
                "1".to_string(),
            ],
        ),
        _ => (
            "alpine:latest".to_string(),
            vec!["echo".to_string(), format!("workflow {}", workflow_url)],
        ),
    }
}

/// Build TES task from WES run. Side effects: may write `inputs.json` / `params.json` under
/// `work_dir` when opt-in env modes are enabled (files are only visible inside the task if the
/// deployment bind-mounts the same host path — see docs).
fn build_tes_task_request(run: &WesRun, work_dir: &Path) -> Result<TesTaskRequest> {
    let wdl_bash = env_truthy("FERRUM_WES_TES_WDL_BASH_LAUNCH");
    let nf_file = env_truthy("FERRUM_WES_TES_NEXTFLOW_FILE_LAUNCH");
    let host_prefix = std::env::var("FERRUM_WES_TES_WORK_HOST_PREFIX")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let wt = run.workflow_type.to_lowercase();
    let wf_url = run.workflow_url.as_str();

    if wdl_bash && wt == "wdl" && workflow_params_meaningful(&run.workflow_params) {
        let path = work_dir.join("inputs.json");
        let json = serde_json::to_string_pretty(&run.workflow_params)
            .map_err(|e| WesError::Executor(format!("serialize workflow_params: {}", e)))?;
        std::fs::write(&path, json)?;
    }

    if nf_file && matches!(wt.as_str(), "nextflow" | "nxf" | "nfl")
        && workflow_params_meaningful(&run.workflow_params)
    {
        let path = work_dir.join("params.json");
        let json = serde_json::to_string_pretty(&run.workflow_params)
            .map_err(|e| WesError::Executor(format!("serialize workflow_params: {}", e)))?;
        std::fs::write(&path, json)?;
    }

    let mut volumes: Option<Vec<serde_json::Value>> = None;
    if let Some(ref prefix) = host_prefix {
        let abs = format!("{}/{}", prefix.trim_end_matches('/'), run.run_id);
        let bind = format!("{abs}:{abs}:rw");
        volumes = Some(vec![serde_json::Value::String(bind)]);
    }

    let container_workdir = std::env::var("FERRUM_WES_TES_CONTAINER_WORKDIR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut base_env = HashMap::new();
    base_env.insert(
        "FERRUM_WES_WORKFLOW_URL".to_string(),
        run.workflow_url.clone(),
    );

    let executor = if wdl_bash && wt == "wdl" {
        TesExecutorBody {
            image: "broadinstitute/cromwell:latest".to_string(),
            entrypoint: Some(vec!["/bin/bash".to_string(), "-lc".to_string()]),
            command: vec![
                "set -euo pipefail; INPUTS_ARGS=; if [ -f inputs.json ]; then INPUTS_ARGS=\"--inputs inputs.json\"; fi; exec java -jar /app/cromwell.jar run \"$FERRUM_WES_WORKFLOW_URL\" $INPUTS_ARGS".to_string(),
            ],
            workdir: container_workdir.clone(),
            env: Some(base_env),
        }
    } else if nf_file && matches!(wt.as_str(), "nextflow" | "nxf" | "nfl") {
        TesExecutorBody {
            image: "nextflow/nextflow:latest".to_string(),
            entrypoint: Some(vec!["/bin/bash".to_string(), "-lc".to_string()]),
            command: vec![
                "set -euo pipefail; curl -fsSL \"$FERRUM_WES_WORKFLOW_URL\" -o workflow.nf; echo 'docker { enabled = true }' > nextflow.config; if [ -f params.json ]; then exec nextflow run workflow.nf -params-file params.json; else exec nextflow run workflow.nf; fi".to_string(),
            ],
            workdir: container_workdir.clone(),
            env: Some(base_env),
        }
    } else {
        let (image, command) = legacy_image_and_command(&run.workflow_type, wf_url);
        TesExecutorBody {
            image,
            command,
            entrypoint: None,
            workdir: container_workdir,
            env: None,
        }
    };

    Ok(TesTaskRequest {
        executors: vec![executor],
        inputs: None,
        outputs: None,
        volumes,
    })
}

impl TesExecutorBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            run_to_task: RwLock::new(HashMap::new()),
            lifecycle_phase: RwLock::new(HashMap::new()),
            lifecycle_start: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl WorkflowExecutor for TesExecutorBackend {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![
            (
                "Nextflow".to_string(),
                vec!["22.10".to_string(), "23.04".to_string()],
            ),
            (
                "CWL".to_string(),
                vec!["1.0".to_string(), "1.1".to_string(), "1.2".to_string()],
            ),
            (
                "WDL".to_string(),
                vec!["1.0".to_string(), "1.1".to_string()],
            ),
            ("Snakemake".to_string(), vec!["7".to_string()]),
        ]
    }

    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        _log_sink: Option<Arc<crate::log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let body = build_tes_task_request(run, work_dir)?;
        let url = format!("{}/tasks", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WesError::Executor(format!("TES create task: {}", e)))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| WesError::Executor(e.to_string()))?;
        if !status.is_success() {
            return Err(WesError::Executor(format!(
                "TES returned {}: {}",
                status, text
            )));
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
            // Ferrum TES router uses POST /tasks/{id}/cancel (see ferrum-tes `lib.rs`).
            let url = format!("{}/tasks/{}/cancel", self.base_url, id);
            let _ = self.client.post(&url).send().await;
            self.run_to_task
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(&handle.run_id);
        }
        Ok(())
    }

    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)> {
        let run_id = &handle.run_id;
        {
            let mut phases = self
                .lifecycle_phase
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?;
            let phase = phases.entry(run_id.clone()).or_insert(0);
            if *phase == 0 {
                *phase = 1;
                drop(phases);
                let mut starts = self
                    .lifecycle_start
                    .write()
                    .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?;
                starts.insert(run_id.clone(), Instant::now());
                return Ok((RunState::Queued, None));
            }
            if *phase == 1 {
                *phase = 2;
                return Ok((RunState::Running, None));
            }
        }

        let task_id = self
            .run_to_task
            .read()
            .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
            .get(run_id)
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
        let text = resp
            .text()
            .await
            .map_err(|e| WesError::Executor(e.to_string()))?;
        let task: TesTaskResponse = serde_json::from_str(&text).unwrap_or(TesTaskResponse {
            id: id.clone(),
            state: Some("UNKNOWN".to_string()),
        });
        let mut state = match task.state.as_deref().unwrap_or("UNKNOWN") {
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

        if state == RunState::Complete {
            let t0 = self
                .lifecycle_start
                .read()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .get(run_id)
                .copied();
            if let Some(t0) = t0 {
                if t0.elapsed() < min_terminal_delay() {
                    state = RunState::Running;
                }
            }
        }

        if state != RunState::Running
            && state != RunState::Queued
            && state != RunState::Initializing
            && state != RunState::Paused
            && state != RunState::Unknown
        {
            self.run_to_task
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(run_id);
            let _ = self
                .lifecycle_phase
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(run_id);
            let _ = self
                .lifecycle_start
                .write()
                .map_err(|e| WesError::Executor(format!("lock poisoned: {}", e)))?
                .remove(run_id);
        }
        Ok((state, None))
    }
}
