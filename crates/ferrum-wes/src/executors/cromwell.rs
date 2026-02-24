//! WDL executor via Cromwell (Java JAR).

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
pub struct CromwellExecutor {
    processes: RwLock<HashMap<String, Tracker>>,
}

impl CromwellExecutor {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkflowExecutor for CromwellExecutor {
    fn supported_languages(&self) -> Vec<(String, Vec<String>)> {
        vec![("WDL".to_string(), vec!["1.0".to_string(), "1.1".to_string(), "draft-2".to_string()])]
    }

    async fn submit(
        &self,
        run: &WesRun,
        work_dir: &Path,
        log_sink: Option<Arc<log_stream::LogSink>>,
    ) -> Result<ProcessHandle> {
        let run_id = run.run_id.clone();
        let workflow_url = run.workflow_url.clone();
        let params_path = work_dir.join("inputs.json");
        let _ = std::fs::write(&params_path, serde_json::to_string_pretty(&run.workflow_params).unwrap_or_else(|_| "{}".to_string()));
        let mut cmd = tokio::process::Command::new("java");
        cmd.args(["-jar", "cromwell.jar", "run", &workflow_url, "--inputs", params_path.to_str().unwrap_or("inputs.json")])
            .current_dir(work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
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
