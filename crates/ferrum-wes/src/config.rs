// SPDX-License-Identifier: BUSL-1.1
//! WES configuration.

use crate::executor::ExecutorBackend;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WesConfig {
    /// Work directory base for run work dirs (e.g. /tmp/wes-runs).
    pub work_dir_base: Option<PathBuf>,
    /// Executor backend: "local" | "slurm". LSF is not implemented.
    #[serde(default)]
    pub executor: ExecutorBackendConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorBackendConfig {
    pub backend: Option<String>,
}

impl Default for ExecutorBackendConfig {
    fn default() -> Self {
        Self {
            backend: Some("local".to_string()),
        }
    }
}

impl WesConfig {
    pub fn executor_backend(&self) -> std::result::Result<ExecutorBackend, String> {
        self.executor.backend.as_deref().unwrap_or("local").parse()
    }
}
