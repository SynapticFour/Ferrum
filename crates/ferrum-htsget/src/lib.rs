// SPDX-License-Identifier: Apache-2.0
//! GA4GH htsget 1.3.0-style tickets backed by DRS.
//!
//! Ticket endpoints resolve a DRS object (same DB as DRS), enforce dataset auth like DRS, and
//! return a JSON ticket whose `urls` point at DRS `/objects/{id}/stream` on this deployment.
//! Genomic range requests (`referenceName` / `start` / `end` / POST `regions`) and `class=header`
//! are answered from the object's index with [htsget-rs](https://github.com/umccr/htsget-rs)
//! (see [`region`]); plain BAM/VCF.gz/BCF bodies are cut at record boundaries (`bgzf_edges`).
//! Requests without regions keep the whole-object ticket. `fields`, `tags` and `notags` are
//! accepted but not applied (service-info says so).

mod bgzf_edges;
mod error;
mod handlers;
pub mod region;
mod ticket;

use axum::http::StatusCode;
use axum::routing::{get, post, Router};
use ferrum_drs::repo::DrsRepo;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config, SwaggerUi};

/// State shared with the gateway: DRS repository handle + public URL prefix for stream links.
#[derive(Clone)]
pub struct HtsgetState {
    pub repo: Arc<DrsRepo>,
    pub public_base_url: String,
    /// Object storage (same as DRS). Needed for region tickets: htsget-rs reads the index and
    /// the file header. Without it only whole-object tickets work.
    pub storage: Option<Arc<dyn ferrum_storage::ObjectStorage>>,
    /// Directory with `{crypt4gh_master_key_id}.sec` (node key) for Crypt4GH region tickets.
    pub crypt4gh_key_dir: Option<std::path::PathBuf>,
    pub crypt4gh_master_key_id: String,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Ferrum htsget",
        description = "GA4GH htsget tickets over DRS streams (whole object, regions and class=header via the object index)",
        version = "1.3.0"
    ),
    paths(
        handlers::reads_service_info,
        handlers::variants_service_info,
        handlers::get_reads_ticket,
        handlers::get_variants_ticket,
        handlers::post_reads_ticket,
        handlers::post_variants_ticket,
    )
)]
pub struct HtsgetApiDoc;

/// htsget router. Mount at `/ga4gh/htsget/v1` in the gateway.
pub fn router(state: Arc<HtsgetState>) -> Router {
    // Same order as ferrum-drs: merge SwaggerUi before `with_state` so nested ticket routes keep state.
    Router::new()
        .route("/reads/service-info", get(handlers::reads_service_info))
        .route(
            "/variants/service-info",
            get(handlers::variants_service_info),
        )
        // `:id` = single segment (same style as DRS `/objects/:object_id`). Slashes in ids → `%2F`.
        .route("/reads/:id", get(handlers::get_reads_ticket))
        .route("/reads/:id", post(handlers::post_reads_ticket))
        .route("/variants/:id", get(handlers::get_variants_ticket))
        .route("/variants/:id", post(handlers::post_variants_ticket))
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/openapi.json", HtsgetApiDoc::openapi())
                .config(Config::new(["/ga4gh/htsget/v1/openapi.json"])),
        )
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
