//! App state for TES.

use crate::executor::ExecutorBackend;
use crate::repo::TesRepo;
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<TesRepo>,
    pub executor: ExecutorBackend,
}
