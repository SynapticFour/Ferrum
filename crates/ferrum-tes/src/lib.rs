//! GA4GH Task Execution Service (TES) 1.1.

pub mod error;
pub mod executor;
pub mod executors;
pub mod handlers;
pub mod repo;
pub mod state;
pub mod types;

use axum::routing::{get, post};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::executor::ExecutorBackend;
use crate::executors::{PodmanExecutor, SlurmExecutor};
use crate::handlers::{cancel_task, create_task, get_service_info, get_task, list_tasks};
use crate::repo::TesRepo;
use crate::state::AppState;
use crate::types::{CreateTaskRequest, CreateTaskResponse, Task, TaskListResponse, TaskSummary, TesServiceInfo};

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::create_task,
        handlers::list_tasks,
        handlers::get_task,
        handlers::cancel_task,
    ),
    components(schemas(
        TesServiceInfo,
        CreateTaskRequest,
        CreateTaskResponse,
        TaskListResponse,
        TaskSummary,
        Task,
        types::TesExecutor,
        types::TesInput,
        types::TesOutput,
        types::TaskLog,
        handlers::ListTasksQuery,
    ))
)]
pub struct TesApiDoc;

/// Backend selector: "podman" | "slurm" | "docker" (if feature enabled).
pub fn make_executor(backend: &str, work_dir: Option<PathBuf>) -> ExecutorBackend {
    let work_dir = work_dir.unwrap_or_else(|| std::env::temp_dir().join("tes-jobs"));
    match backend.to_lowercase().as_str() {
        "slurm" => Arc::new(SlurmExecutor::new(work_dir)),
        #[cfg(feature = "docker")]
        "docker" => Arc::new(
            crate::executors::DockerExecutor::connect_default().expect("Docker connection failed"),
        ),
        _ => Arc::new(PodmanExecutor::new()),
    }
}

/// Returns a router that responds 503 for all TES routes (used when gateway has TES enabled but no DB).
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async { (axum::http::StatusCode::SERVICE_UNAVAILABLE, "TES not configured") })
}

/// Returns the TES router. Requires a PostgreSQL pool and optional backend ("podman" | "slurm").
/// Mount at /ga4gh/tes/v1 in gateway.
pub fn router(pool: sqlx::PgPool, backend: Option<String>, work_dir: Option<PathBuf>) -> Router {
    let backend = backend.as_deref().unwrap_or("podman");
    let executor = make_executor(backend, work_dir);
    let repo = Arc::new(TesRepo::new(pool));
    let state = Arc::new(AppState { repo, executor });
    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}:cancel", post(cancel_task))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", TesApiDoc::openapi()))
        .with_state(state)
}
