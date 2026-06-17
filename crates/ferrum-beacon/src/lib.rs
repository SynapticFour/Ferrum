//! GA4GH Beacon v2 API.

pub mod error;
pub mod federation;
pub mod handlers;
pub mod pathogen;
pub mod query;
pub mod repo;
pub mod vcf_index;

use axum::routing::{get, post};
use axum::Router;
use ferrum_core::{OutbreakService, ResidencyAuditLog};
use ferrum_federation::FederationClient;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::repo::BeaconRepo;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::get_info,
        handlers::get_map,
        handlers::query_variants,
        handlers::query_individuals,
        handlers::query_biosamples,
    ),
    components(schemas(
        handlers::BeaconInfoResponse,
        handlers::VariantQueryRequest,
        handlers::VariantQueryResponse,
    ))
)]
pub struct BeaconApiDoc;

/// Returns a router that responds 503 when Beacon is not configured.
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Beacon not configured",
        )
    })
}

/// Returns the Beacon v2 router. Requires a database pool. Mount at /ga4gh/beacon/v2 in gateway.
pub fn router(pool: ferrum_core::FerrumPool) -> Router {
    router_with_services(pool, None, None, None, None)
}

/// Beacon router with optional Outbreak Mode integration for emergency access audit.
pub fn router_with_outbreak(
    pool: ferrum_core::FerrumPool,
    outbreak: Option<std::sync::Arc<OutbreakService>>,
) -> Router {
    router_with_services(pool, outbreak, None, None, None)
}

/// Beacon router with outbreak, federation, residency audit, and reference registry integration.
pub fn router_with_services(
    pool: ferrum_core::FerrumPool,
    outbreak: Option<std::sync::Arc<OutbreakService>>,
    federation: Option<std::sync::Arc<FederationClient>>,
    residency_audit: Option<std::sync::Arc<ResidencyAuditLog>>,
    reference_registry: Option<std::sync::Arc<ferrum_reference::ReferenceRegistry>>,
) -> Router {
    let state = Arc::new(handlers::AppState {
        repo: Arc::new(BeaconRepo::new(pool)),
        outbreak,
        federation,
        residency_audit,
        reference_registry,
    });
    Router::new()
        .route("/service-info", get(handlers::get_service_info))
        .route("/info", get(handlers::get_info))
        .route("/map", get(handlers::get_map))
        .route("/query", post(handlers::query_variants))
        .route("/g_variants", get(handlers::get_g_variants))
        .route("/g_variants/query", post(handlers::query_variants))
        .route("/individuals/query", post(handlers::query_individuals))
        .route("/biosamples/query", post(handlers::query_biosamples))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", BeaconApiDoc::openapi()))
        .with_state(state)
}
