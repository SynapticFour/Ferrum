//! GA4GH Tool Registry Service (TRS) 2.0.1.

pub mod error;
pub mod handlers;
pub mod repo;
pub mod types;

use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handlers::{
    get_descriptor, get_service_info, get_tool, get_tool_versions, list_tools, register_tool,
    ListToolsQuery,
};
use crate::repo::TrsRepo;
use crate::types::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::list_tools,
        handlers::get_tool,
        handlers::get_tool_versions,
        handlers::get_descriptor,
        handlers::register_tool,
    ),
    components(schemas(
        TrsServiceInfo,
        Tool,
        ToolClass,
        ToolVersion,
        ToolVersionsResponse,
        ToolListResponse,
        ListToolsQuery,
        RegisterToolRequest,
    ))
)]
pub struct TrsApiDoc;

/// Returns a router that responds 503 for all TRS routes when not configured.
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async { (axum::http::StatusCode::SERVICE_UNAVAILABLE, "TRS not configured") })
}

/// Returns the TRS router. Requires a PostgreSQL pool. Mount at /ga4gh/trs/v2 in gateway.
pub fn router(pool: sqlx::PgPool) -> Router {
    let state = Arc::new(handlers::AppState {
        repo: Arc::new(TrsRepo::new(pool)),
    });
    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/tools", get(list_tools))
        .route("/tools/{id}", get(get_tool))
        .route("/tools/{id}/versions", get(get_tool_versions))
        .route("/tools/{id}/versions/{version_id}/descriptor/{descriptor_type}", get(get_descriptor))
        .route("/internal/register", axum::routing::post(register_tool))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", TrsApiDoc::openapi()))
        .with_state(state)
}
