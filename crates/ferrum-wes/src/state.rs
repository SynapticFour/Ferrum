//! App state for WES.

use crate::log_stream::LogStreamRegistry;
use crate::metrics::MetricsCollector;
use crate::multiqc::MultiQCRunner;
use crate::repo::WesRepo;
use crate::run_manager::RunManager;
use ferrum_core::ProvenanceStore;
use std::sync::Arc;

pub struct AppState {
    pub repo: Arc<WesRepo>,
    pub run_manager: Arc<RunManager>,
    pub log_registry: Arc<LogStreamRegistry>,
    /// When set, POST workflow metadata to this TRS base URL (e.g. http://localhost:8080/ga4gh/trs/v2) on run submit.
    pub trs_register_url: Option<String>,
    /// When set, record WES input/output provenance and serve GET /runs/{id}/provenance and GET /provenance/graph.
    pub provenance_store: Option<Arc<ProvenanceStore>>,
    /// When set, collect run metrics and expose cost endpoints.
    pub metrics: Option<Arc<MetricsCollector>>,
    /// Set to true when the metrics sampling loop has been started (lazy init).
    pub metrics_sampler_started: Arc<std::sync::atomic::AtomicBool>,
    /// When set, run MultiQC after each completed run and ingest report into DRS.
    pub multiqc_runner: Option<Arc<MultiQCRunner>>,
}
