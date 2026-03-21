//! GA4GH Workflow Execution Service (WES) 1.1.

pub mod checkpoint;
pub mod config;
pub mod error;
pub mod executor;
pub mod executors;
pub mod handlers;
pub mod helixtest_ferrum;
pub mod log_stream;
pub mod metrics;
pub mod multiqc;
pub mod output_sampling;
pub mod process_sampler;
pub mod provenance_helpers;
pub mod repo;
pub mod run_manager;
pub mod state;
pub mod types;

use axum::http::StatusCode;
use axum::{
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handlers::{
    cancel_run, export_ro_crate, get_cache_stats, get_cost_summary, get_provenance_graph,
    get_run_log, get_run_metrics, get_run_metrics_report, get_run_provenance, get_run_status,
    get_service_info, get_stderr, get_stdout, list_runs, list_tasks, post_cost_estimate, post_runs,
    resume_run, stream_logs, ListRunsQuery,
};
use crate::multiqc::MultiQCRunner;
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
/// If `pricing` is Some, metrics and cost endpoints are enabled.
/// If `multiqc_config` and `drs_ingest_base_url` are both Some (and multiqc enabled), MultiQC runs after each completed run and ingests report into DRS.
#[allow(clippy::too_many_arguments)]
pub fn router(
    pool: sqlx::PgPool,
    work_dir_base: Option<PathBuf>,
    tes_url: Option<String>,
    trs_register_url: Option<String>,
    provenance_store: Option<Arc<ferrum_core::ProvenanceStore>>,
    pricing: Option<ferrum_core::PricingConfig>,
    multiqc_config: Option<ferrum_core::MultiQCConfig>,
    drs_ingest_base_url: Option<String>,
    allowed_workflow_sources: Vec<String>,
) -> Router {
    let checkpoint_store = Some(Arc::new(crate::checkpoint::CheckpointStore::new(
        pool.clone(),
        drs_ingest_base_url.clone(),
    )));
    let work_dir_base = work_dir_base.unwrap_or_else(|| std::env::temp_dir().join("wes-runs"));
    std::fs::create_dir_all(&work_dir_base).ok();
    let work_dir_base_for_restore = work_dir_base.clone();
    let repo = Arc::new(WesRepo::new(pool.clone()));
    let repo_restore = Arc::clone(&repo);
    let log_registry = Arc::new(log_stream::LogStreamRegistry::new(256));
    let metrics = pricing.map(|p| Arc::new(crate::metrics::MetricsCollector::new(pool.clone(), p)));
    let multiqc_runner = multiqc_config
        .filter(|c| c.enabled)
        .zip(drs_ingest_base_url)
        .map(|(config, base_url)| {
            Arc::new(MultiQCRunner::new(
                config,
                base_url,
                Arc::clone(&repo),
                provenance_store.clone(),
            ))
        });
    let run_manager = Arc::new(
        RunManager::new(Arc::clone(&repo), work_dir_base, Arc::clone(&log_registry))
            .with_tes(tes_url)
            .with_metrics(metrics.clone())
            .with_multiqc(multiqc_runner.clone()),
    );
    let state = AppState {
        repo,
        run_manager,
        log_registry,
        trs_register_url,
        provenance_store,
        metrics,
        metrics_sampler_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        multiqc_runner,
        allowed_workflow_sources,
        checkpoint_store,
    };
    let state = Arc::new(state);

    // Learned from Sapporo: on restart, recover runs that were persisted as RUNNING
    // but no longer have a live process. This is best-effort and must not block startup.
    tokio::spawn(async move {
        let Ok(entries) = std::fs::read_dir(&work_dir_base_for_restore) else {
            return;
        };
        let mut sys = sysinfo::System::new_all();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let run_id = match path.file_name().and_then(|n| n.to_str()) {
                Some(r) => r.to_string(),
                None => continue,
            };
            let state_path = path.join("state.json");
            let Ok(bytes) = std::fs::read(&state_path) else {
                continue;
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if v.get("state").and_then(|s| s.as_str()) != Some("RUNNING") {
                continue;
            }
            let Some(pid) = v
                .get("engine_pid")
                .and_then(|p| p.as_u64())
                .and_then(|p| u32::try_from(p).ok())
            else {
                // No PID stored -> skip (e.g. TES-backed runs).
                continue;
            };

            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
            let exists = sys.process(sysinfo::Pid::from_u32(pid)).is_some();

            if !exists {
                let _ = repo_restore
                    .update_state(&run_id, crate::types::RunState::SystemError)
                    .await;
            }
        }
    });

    let mut r = Router::new()
        .route("/service-info", get(get_service_info))
        .route("/runs", get(list_runs).post(post_runs))
        // Axum path params use `:param` (not `{param}`).
        .route("/runs/:run_id", get(get_run_log))
        .route("/runs/:run_id/status", get(get_run_status))
        .route("/runs/:run_id/cancel", post(cancel_run))
        .route("/runs/:run_id/resume", post(resume_run))
        .route("/runs/:run_id/tasks", get(list_tasks))
        .route("/runs/:run_id/logs/stream", get(stream_logs))
        .route("/runs/:run_id/logs/stdout", get(get_stdout))
        .route("/runs/:run_id/logs/stderr", get(get_stderr))
        .route("/runs/:run_id/provenance", get(get_run_provenance))
        .route("/runs/:run_id/export/ro-crate", get(export_ro_crate))
        .route("/provenance/graph", get(get_provenance_graph));
    r = r
        .route("/runs/:run_id/metrics", get(get_run_metrics))
        .route("/runs/:run_id/metrics/report", get(get_run_metrics_report))
        .route("/cost/estimate", axum::routing::post(post_cost_estimate))
        .route("/cost/summary", get(get_cost_summary))
        .route("/cache/stats", get(get_cache_stats))
        .route("/cache", axum::routing::delete(handlers::evict_cache));
    r.merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", WesApiDoc::openapi()))
        .with_state(state)
}

/// Background loop: every 30s sample active runs and record task metrics. Call once when metrics are enabled.
pub fn spawn_metrics_sampler(
    run_manager: Arc<RunManager>,
    metrics: Arc<crate::metrics::MetricsCollector>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let run_ids = run_manager.active_run_ids().await;
            for run_id in run_ids {
                if let Some(pid) = run_manager.process_id_for_run(&run_id).await {
                    if let Some(sample) =
                        crate::process_sampler::sample_process_as_task(pid, &run_id, "main")
                    {
                        let _ = metrics
                            .record_task_sample(&run_id, sample, 1.0, 0, "local", None)
                            .await;
                    }
                }
            }
        }
    });
}
