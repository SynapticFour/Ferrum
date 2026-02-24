//! App state for Cohort service.

use crate::repo::CohortRepo;
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<CohortRepo>,
}
