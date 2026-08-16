// SPDX-License-Identifier: BUSL-1.1
//! Pluggable reference genome registry for Ferrum.

pub mod handlers;
pub mod mismatch;
pub mod registry;
pub mod types;

pub use handlers::reference_api_v1_router;
pub use mismatch::check_reference_mismatch;
pub use registry::ReferenceRegistry;
pub use types::{
    LoadReferenceRequest, PopulationScope, ReferenceGenome, RegisterReferenceRequest,
    WesReferenceWarning,
};
