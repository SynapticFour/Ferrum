//! Nextflow executor (nextflow run). A03: workflow_params via --params-file only (no env/CLI injection).

use crate::error::Result;
use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::log_stream;
use crate::types::RunState;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

struct Tracker {
    child: tokio::process::Child,
}

#[derive(Default)]
pub struct NextflowExecutor {
    processes: RwLock<HashMap<String, Tracker>>,
}

impl NextflowExecutor {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkflowExecutor for NextflowExecutor {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![(
            "Nextflow".to_string(),
            vec!["22.10".to_string(), "23.04".to_string()],
        )]
    }

    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        log_sink: Option<Arc<log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let run_id = run.run_id.clone();
        let workflow_url = run.workflow_url.clone();
        let mut cmd = tokio::process::Command::new("nextflow");
        cmd.current_dir(work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        // A03: Pass params via file only to avoid injection; no user-controlled env/CLI args.
        if run.workflow_params.is_object()
            && run
                .workflow_params
                .as_object()
                .map(|o| !o.is_empty())
                .unwrap_or(false)
        {
            let params_path = work_dir.join("nextflow_params.json");
            tokio::fs::write(
                &params_path,
                serde_json::to_string(&run.workflow_params).unwrap_or_else(|_| "{}".to_string()),
            )
            .await
            .map_err(|e| crate::error::WesError::Executor(format!("write params file: {}", e)))?;
            cmd.args([
                "run",
                &workflow_url,
                "--params-file",
                params_path
                    .as_os_str()
                    .to_str()
                    .unwrap_or("nextflow_params.json"),
            ]);
        } else {
            cmd.args(["run", &workflow_url]);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| crate::error::WesError::Executor(e.to_string()))?;
        if let Some(ref sink) = log_sink {
            log_stream::pipe_child_logs(child.stdout.take(), child.stderr.take(), Arc::clone(sink));
        }
        self.processes
            .write()
            .map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?
            .insert(run_id.clone(), Tracker { child });
        Ok(ProcessHandle { run_id })
    }

    async fn cancel(&self, handle: &ProcessHandle) -> Result<()> {
        if let Some(mut t) = self
            .processes
            .write()
            .map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?
            .remove(&handle.run_id)
        {
            let _ = t.child.start_kill();
        }
        Ok(())
    }

    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)> {
        let mut guard = self
            .processes
            .write()
            .map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?;
        if let Some(t) = guard.get_mut(&handle.run_id) {
            match t.child.try_wait().map_err(crate::error::WesError::Io)? {
                Some(s) => {
                    let code = s.code();
                    guard.remove(&handle.run_id);
                    let state = if s.success() {
                        RunState::Complete
                    } else {
                        RunState::ExecutorError
                    };
                    return Ok((state, code));
                }
                None => return Ok((RunState::Running, None)),
            }
        }
        Ok((RunState::Unknown, None))
    }

    fn process_id_for_metrics(&self, run_id: &str) -> Option<u32> {
        self.processes
            .read()
            .ok()?
            .get(run_id)
            .and_then(|t| t.child.id())
    }
}
