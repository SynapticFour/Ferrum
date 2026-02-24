//! App state for WES.

use crate::log_stream::LogStreamRegistry;
use crate::repo::WesRepo;
use crate::run_manager::RunManager;
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<WesRepo>,
    pub run_manager: Arc<RunManager>,
    pub log_registry: Arc<LogStreamRegistry>,
    /// When set, POST workflow metadata to this TRS base URL (e.g. http://localhost:8080/ga4gh/trs/v2) on run submit.
    pub trs_register_url: Option<String>,
}
