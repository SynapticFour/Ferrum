//! Tracks running workflows and dispatches to executors.

use crate::error::Result;
use crate::executor::{ProcessHandle, WesRun, WorkflowExecutor};
use crate::executors::{CromwellExecutor, CwltoolExecutor, NextflowExecutor, SnakemakeExecutor, TesExecutorBackend};
use crate::log_stream::LogStreamRegistry;
use crate::repo::WesRepo;
use crate::types::RunState;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RunManager {
    repo: Arc<WesRepo>,
    nextflow: NextflowExecutor,
    cwltool: CwltoolExecutor,
    cromwell: CromwellExecutor,
    snakemake: SnakemakeExecutor,
    tes: Option<Arc<TesExecutorBackend>>,
    log_registry: Arc<LogStreamRegistry>,
    run_to_executor: RwLock<HashMap<String, ExecutorKind>>,
    work_dir_base: PathBuf,
}

#[derive(Clone, Copy)]
enum ExecutorKind {
    Nextflow,
    Cwltool,
    Cromwell,
    Snakemake,
    Tes,
}

impl RunManager {
    pub fn new(repo: Arc<WesRepo>, work_dir_base: PathBuf, log_registry: Arc<LogStreamRegistry>) -> Self {
        Self {
            repo,
            nextflow: NextflowExecutor::new(),
            cwltool: CwltoolExecutor::new(),
            cromwell: CromwellExecutor::new(),
            snakemake: SnakemakeExecutor::new(),
            tes: None,
            log_registry,
            run_to_executor: RwLock::new(HashMap::new()),
            work_dir_base,
        }
    }

    /// Create RunManager with an optional TES backend. When `tes_url` is Some, runs are submitted to TES instead of local executors.
    pub fn with_tes(mut self, tes_url: Option<String>) -> Self {
        self.tes = tes_url.map(|u| Arc::new(TesExecutorBackend::new(u)));
        self
    }

    fn executor_for_type(&self, workflow_type: &str) -> Option<&dyn WorkflowExecutor> {
        if let Some(ref tes) = self.tes {
            return Some(tes.as_ref());
        }
        match workflow_type.to_lowercase().as_str() {
            "nextflow" | "nxf" => Some(&self.nextflow),
            "cwl" => Some(&self.cwltool),
            "wdl" => Some(&self.cromwell),
            "snakemake" => Some(&self.snakemake),
            _ => None,
        }
    }

    pub fn nextflow(&self) -> &NextflowExecutor {
        &self.nextflow
    }
    pub fn cwltool(&self) -> &CwltoolExecutor {
        &self.cwltool
    }
    pub fn cromwell(&self) -> &CromwellExecutor {
        &self.cromwell
    }
    pub fn snakemake(&self) -> &SnakemakeExecutor {
        &self.snakemake
    }

    pub fn all_executors(&self) -> Vec<&dyn WorkflowExecutor> {
        if let Some(ref tes) = self.tes {
            return vec![tes.as_ref() as &dyn WorkflowExecutor];
        }
        vec![
            &self.nextflow as &dyn WorkflowExecutor,
            &self.cwltool,
            &self.cromwell,
            &self.snakemake,
        ]
    }

    /// Submit a run: create work dir, create log stream, call executor, register handle, persist work_dir.
    pub async fn submit(&self, run: &WesRun) -> Result<ProcessHandle> {
        let work_dir = self.work_dir_base.join(&run.run_id);
        std::fs::create_dir_all(&work_dir).map_err(crate::error::WesError::Io)?;
        let use_tes = self.tes.is_some();
        let log_sink = if use_tes { None } else { Some(self.log_registry.create(&run.run_id, work_dir.clone()).await) };
        let executor = self
            .executor_for_type(&run.workflow_type)
            .ok_or_else(|| crate::error::WesError::Validation(format!("unsupported workflow type: {}", run.workflow_type)))?;
        let handle = executor.submit(run, &work_dir, log_sink).await?;
        if let Some(work_dir_str) = work_dir.to_str() {
            self.repo.set_work_dir(&handle.run_id, work_dir_str).await?;
        }
        let kind = if self.tes.is_some() {
            ExecutorKind::Tes
        } else {
            match run.workflow_type.to_lowercase().as_str() {
                "nextflow" | "nxf" => ExecutorKind::Nextflow,
                "cwl" => ExecutorKind::Cwltool,
                "wdl" => ExecutorKind::Cromwell,
                "snakemake" => ExecutorKind::Snakemake,
                _ => return Err(crate::error::WesError::Validation("unknown type".into())),
            }
        };
        self.run_to_executor.write().await.insert(handle.run_id.clone(), kind);
        self.repo.set_start_time(&handle.run_id).await?;
        Ok(handle)
    }

    pub async fn cancel(&self, run_id: &str) -> Result<()> {
        let handle = ProcessHandle { run_id: run_id.to_string() };
        let kind = self.run_to_executor.read().await.get(run_id).copied();
        let executor: Option<&dyn WorkflowExecutor> = kind.map(|k| match k {
            ExecutorKind::Nextflow => &self.nextflow as &dyn WorkflowExecutor,
            ExecutorKind::Cwltool => &self.cwltool as &dyn WorkflowExecutor,
            ExecutorKind::Cromwell => &self.cromwell as &dyn WorkflowExecutor,
            ExecutorKind::Snakemake => &self.snakemake as &dyn WorkflowExecutor,
            ExecutorKind::Tes => self.tes.as_deref().unwrap() as &dyn WorkflowExecutor,
        });
        if let Some(exec) = executor {
            exec.cancel(&handle).await?;
            self.run_to_executor.write().await.remove(run_id);
            self.log_registry.remove(run_id).await;
            self.repo.update_state(run_id, RunState::Canceled).await?;
        }
        Ok(())
    }

    pub async fn poll_status(&self, run_id: &str) -> Result<RunState> {
        let handle = ProcessHandle { run_id: run_id.to_string() };
        let kind = self.run_to_executor.read().await.get(run_id).copied();
        let executor: Option<&dyn WorkflowExecutor> = kind.map(|k| match k {
            ExecutorKind::Nextflow => &self.nextflow as &dyn WorkflowExecutor,
            ExecutorKind::Cwltool => &self.cwltool as &dyn WorkflowExecutor,
            ExecutorKind::Cromwell => &self.cromwell as &dyn WorkflowExecutor,
            ExecutorKind::Snakemake => &self.snakemake as &dyn WorkflowExecutor,
            ExecutorKind::Tes => self.tes.as_deref().unwrap() as &dyn WorkflowExecutor,
        });
        if let Some(exec) = executor {
            let (state, exit_code) = exec.poll_status(&handle).await?;
            if state != RunState::Running && state != RunState::Unknown {
                self.run_to_executor.write().await.remove(run_id);
                self.log_registry.remove(run_id).await;
                self.repo.update_state(run_id, state).await?;
                if let Some(row) = self.repo.get_run(run_id).await? {
                    let (_, _, _, _, _, _, _, _, start_time, end_time, _, _) = row;
                    let stdout_url = Some(format!("/runs/{}/logs/stdout", run_id));
                    let stderr_url = Some(format!("/runs/{}/logs/stderr", run_id));
                    let end = end_time.unwrap_or_else(Utc::now);
                    let _ = self.repo.upsert_run_log(
                        run_id,
                        "main",
                        &[],
                        start_time,
                        Some(end),
                        stdout_url.as_deref(),
                        stderr_url.as_deref(),
                        exit_code,
                    ).await;
                }
            }
            return Ok(state);
        }
        Ok(RunState::Unknown)
    }

    pub fn repo(&self) -> &WesRepo {
        &self.repo
    }

    pub fn log_registry(&self) -> &LogStreamRegistry {
        &self.log_registry
    }
}
