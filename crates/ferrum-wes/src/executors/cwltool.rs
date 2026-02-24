//! CWL executor via cwltool.

use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::error::Result;
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
pub struct CwltoolExecutor {
    processes: RwLock<HashMap<String, Tracker>>,
}

impl CwltoolExecutor {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkflowExecutor for CwltoolExecutor {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![("CWL".to_string(), vec!["1.0".to_string(), "1.1".to_string(), "1.2".to_string()])]
    }

    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        log_sink: Option<Arc<log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let run_id = run.run_id.clone();
        let workflow_url = run.workflow_url.clone();
        let params_file = work_dir.join("params.json");
        if let Some(_params) = run.workflow_params.as_object() {
            let _ = std::fs::write(&params_file, serde_json::to_string_pretty(&run.workflow_params).unwrap_or_default());
        }
        let mut cmd = tokio::process::Command::new("cwltool");
        cmd.args(["--outdir", work_dir.join("out").to_str().unwrap_or("out"), &workflow_url])
            .current_dir(work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if params_file.exists() {
            cmd.arg(params_file);
        }
        let mut child = cmd.spawn().map_err(|e| crate::error::WesError::Executor(e.to_string()))?;
        if let Some(ref sink) = log_sink {
            log_stream::pipe_child_logs(child.stdout.take(), child.stderr.take(), Arc::clone(sink));
        }
        self.processes.write().map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?.insert(run_id.clone(), Tracker { child });
        Ok(ProcessHandle { run_id })
    }

    async fn cancel(&self, handle: &ProcessHandle) -> Result<()> {
        if let Some(mut t) = self.processes.write().map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?.remove(&handle.run_id) {
            let _ = t.child.start_kill();
        }
        Ok(())
    }

    async fn poll_status(&self, handle: &ProcessHandle) -> Result<(RunState, Option<i32>)> {
        let mut guard = self.processes.write().map_err(|e| crate::error::WesError::Executor(format!("lock poisoned: {}", e)))?;
        if let Some(t) = guard.get_mut(&handle.run_id) {
            match t.child.try_wait().map_err(crate::error::WesError::Io)? {
                Some(s) => {
                    let code = s.code();
                    guard.remove(&handle.run_id);
                    let state = if s.success() { RunState::Complete } else { RunState::ExecutorError };
                    return Ok((state, code));
                }
                None => return Ok((RunState::Running, None)),
            }
        }
        Ok((RunState::Unknown, None))
    }
}
