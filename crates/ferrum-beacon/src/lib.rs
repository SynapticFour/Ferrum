//! GA4GH Beacon v2 API.

pub mod error;
pub mod handlers;
pub mod repo;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handlers::{
    get_info, get_map, get_service_info, query_biosamples, query_individuals, query_variants,
};
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

/// Returns the Beacon v2 router. Requires a PostgreSQL pool. Mount at /ga4gh/beacon/v2 in gateway.
pub fn router(pool: sqlx::PgPool) -> Router {
    let state = Arc::new(handlers::AppState {
        repo: Arc::new(BeaconRepo::new(pool)),
    });
    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/info", get(get_info))
        .route("/map", get(get_map))
        // Compatibility alias: some clients (incl. HelixTest) use POST /query for variants.
        .route("/query", post(query_variants))
        .route("/g_variants/query", post(query_variants))
        .route("/individuals/query", post(query_individuals))
        .route("/biosamples/query", post(query_biosamples))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", BeaconApiDoc::openapi()))
        .with_state(state)
}
