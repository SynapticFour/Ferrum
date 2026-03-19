//! GA4GH htsget 1.3.0-style tickets backed by DRS.
//!
//! Ticket endpoints resolve a DRS object (same DB as DRS), enforce dataset auth like DRS, and
//! return a JSON ticket whose `urls` point at [`GET /ga4gh/drs/v1/objects/{id}/stream`](crate) on
//! this deployment. Genomic range / field / tag filters are validated where required by the spec;
//! the byte stream is always the **full** object (clients may filter locally; spec allows superset).

mod error;
mod handlers;
mod ticket;

use axum::http::StatusCode;
use axum::routing::{get, post, Router};
use ferrum_drs::repo::DrsRepo;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// State shared with the gateway: DRS repository handle + public URL prefix for stream links.
#[derive(Clone)]
pub struct HtsgetState {
    pub repo: Arc<DrsRepo>,
    pub public_base_url: String,
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Ferrum htsget",
    description = "GA4GH htsget 1.3.0 tickets; data via DRS stream",
    version = "1.3.0"
))]
pub struct HtsgetApiDoc;

/// htsget router. Mount at `/ga4gh/htsget/v1` in the gateway.
pub fn router(state: Arc<HtsgetState>) -> Router {
    // Same order as ferrum-drs: merge SwaggerUi before `with_state` so nested ticket routes keep state.
    Router::new()
        .route("/reads/service-info", get(handlers::reads_service_info))
        .route("/variants/service-info", get(handlers::variants_service_info))
        // `:id` = single segment (same style as DRS `/objects/:object_id`). Slashes in ids → `%2F`.
        .route("/reads/:id", get(handlers::get_reads_ticket))
        .route("/reads/:id", post(handlers::post_reads_ticket))
        .route("/variants/:id", get(handlers::get_variants_ticket))
        .route("/variants/:id", post(handlers::post_variants_ticket))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", HtsgetApiDoc::openapi()))
        .with_state(state)
}

/// Unconfigured router: all htsget routes return 503.
pub fn router_unconfigured() -> Router {
    async fn svc_unavailable() -> StatusCode {
        StatusCode::SERVICE_UNAVAILABLE
    }
    Router::new()
        .route("/reads/service-info", get(svc_unavailable))
        .route("/variants/service-info", get(svc_unavailable))
        .route("/reads/:id", get(svc_unavailable))
        .route("/reads/:id", post(svc_unavailable))
        .route("/variants/:id", get(svc_unavailable))
        .route("/variants/:id", post(svc_unavailable))
}
