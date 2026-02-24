//! GA4GH Workflow Execution Service (WES) 1.1.

pub mod config;
pub mod error;
pub mod executor;
pub mod executors;
pub mod handlers;
pub mod log_stream;
pub mod provenance_helpers;
pub mod repo;
pub mod run_manager;
pub mod state;
pub mod types;

use axum::{
    routing::{get, post},
    Router,
};
use axum::http::StatusCode;
use std::path::PathBuf;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handlers::{
    cancel_run, export_ro_crate, get_provenance_graph, get_run_log, get_run_provenance, get_run_status,
    get_service_info, get_stderr, get_stdout, list_runs, list_tasks, post_runs, stream_logs, ListRunsQuery,
};
use crate::repo::WesRepo;
use crate::run_manager::RunManager;
use crate::state::AppState;
use crate::types::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::list_runs,
        handlers::post_runs,
        handlers::get_run_status,
        handlers::get_run_log,
        handlers::cancel_run,
        handlers::list_tasks,
        handlers::get_run_provenance,
        handlers::get_provenance_graph,
    ),
    components(schemas(
        WesServiceInfo,
        RunListResponse,
        RunSummary,
        RunIdResponse,
        RunStatus,
        RunState,
        RunLog,
        Log,
        TaskLog,
        RunRequestRef,
        ListRunsQuery,
        handlers::TaskListResponse,
        ServiceType,
        Organization,
        WorkflowTypeVersion,
        WorkflowEngineVersion,
        DefaultWorkflowEngineParameter,
        handlers::RunProvenanceResponse,
        handlers::RunProvenanceGraphResponse,
        handlers::ProvenanceGraphQuery,
    ))
)]
pub struct WesApiDoc;

/// Returns a router that responds 503 Service Unavailable for all WES routes (used when gateway has WES enabled but no DB/config).
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async { (StatusCode::SERVICE_UNAVAILABLE, "WES not configured") })
}

/// Build the WES router. Mount at e.g. /ga4gh/wes/v1.
/// Requires a PostgreSQL pool and an optional work directory base for run workspaces.
/// If `tes_url` is Some, all runs are submitted to that GA4GH TES endpoint instead of local executors.
/// If `trs_register_url` is Some (e.g. http://host/ga4gh/trs/v2), workflow submissions are auto-registered to TRS.
/// If `provenance_store` is Some, WES records input/output provenance and exposes /runs/{id}/provenance and /provenance/graph.
pub fn router(
    pool: sqlx::PgPool,
    work_dir_base: Option<PathBuf>,
    tes_url: Option<String>,
    trs_register_url: Option<String>,
    provenance_store: Option<Arc<ferrum_core::ProvenanceStore>>,
) -> Router {
    let work_dir_base = work_dir_base.unwrap_or_else(|| std::env::temp_dir().join("wes-runs"));
    std::fs::create_dir_all(&work_dir_base).ok();
    let repo = Arc::new(WesRepo::new(pool));
    let log_registry = Arc::new(log_stream::LogStreamRegistry::new(256));
    let run_manager = Arc::new(
        RunManager::new(Arc::clone(&repo), work_dir_base, Arc::clone(&log_registry)).with_tes(tes_url),
    );
    let state = AppState {
        repo,
        run_manager,
        log_registry,
        trs_register_url,
        provenance_store,
    };
    let state = Arc::new(state);

    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/runs", get(list_runs).post(post_runs))
        .route("/runs/{run_id}", get(get_run_log))
        .route("/runs/{run_id}/status", get(get_run_status))
        .route("/runs/{run_id}/cancel", post(cancel_run))
        .route("/runs/{run_id}/tasks", get(list_tasks))
        .route("/runs/{run_id}/logs/stream", get(stream_logs))
        .route("/runs/{run_id}/logs/stdout", get(get_stdout))
        .route("/runs/{run_id}/logs/stderr", get(get_stderr))
        .route("/runs/{run_id}/provenance", get(get_run_provenance))
        .route("/runs/{run_id}/export/ro-crate", get(export_ro_crate))
        .route("/provenance/graph", get(get_provenance_graph))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", WesApiDoc::openapi()))
        .with_state(state)
}
