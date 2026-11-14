// SPDX-License-Identifier: Apache-2.0
//! App state for Cohort service.

use crate::repo::CohortRepo;
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<CohortRepo>,
}
