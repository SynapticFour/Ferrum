//! Cohort Browser API: named versioned sample collections with phenotype, DRS, Beacon, WES integration.

pub mod error;
pub mod handlers;
pub mod query;
pub mod repo;
pub mod state;
pub mod types;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::error::CohortJsonResult;
use crate::handlers::{
    add_samples, clone_cohort, create_cohort, delete_cohort, export_cohort, freeze_cohort,
    get_cohort, get_sample, get_schema, list_cohorts, list_samples, list_versions, query_cohort,
    remove_sample, update_cohort, update_sample, cohort_stats,
};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    components(schemas(
        types::CohortSummary,
        types::CohortDetail,
        types::CreateCohortRequest,
        types::UpdateCohortRequest,
        types::CohortSample,
        types::AddSampleRequest,
        types::AddSamplesBatchRequest,
        types::PhenotypeSchemaField,
        types::QueryResult,
        types::QueryFacets,
        types::CohortStats,
        types::CohortVersionInfo,
        query::CohortQuery,
        query::Filter,
        query::QueryLogic,
        handlers::ListCohortsResponse,
        handlers::ListSamplesResponse,
    ))
)]
pub struct CohortApiDoc;

/// Returns a router that responds 503 when Cohorts is not configured.
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Cohorts service not configured",
        )
    })
}

/// Build the Cohort Browser router. Mount at /cohorts/v1.
pub fn router(pool: sqlx::PgPool) -> Router {
    let state = Arc::new(AppState {
        repo: Arc::new(repo::CohortRepo::new(pool)),
    });
    Router::new()
        .route(
            "/cohorts",
            get(|s, auth, q| async move { CohortJsonResult(list_cohorts(s, auth, q).await) })
                .post(|s, auth, h, j| async move { CohortJsonResult(create_cohort(s, auth, h, j).await) }),
        )
        .route(
            "/cohorts/:id",
            get(|s, p, auth| async move { CohortJsonResult(get_cohort(s, p, auth).await) })
                .put(|s, p, auth, j| async move { CohortJsonResult(update_cohort(s, p, auth, j).await) })
                .delete(|s, p, auth| async move { CohortJsonResult(delete_cohort(s, p, auth).await) }),
        )
        .route(
            "/cohorts/:id/freeze",
            post(|s, p, auth| async move { CohortJsonResult(freeze_cohort(s, p, auth).await) }),
        )
        .route(
            "/cohorts/:id/clone",
            post(|s, p, auth, h, j| async move { CohortJsonResult(clone_cohort(s, p, auth, h, j).await) }),
        )
        .route(
            "/cohorts/:id/export",
            get(|s, p, auth, q| async move { CohortJsonResult(export_cohort(s, p, auth, q).await) }),
        )
        .route(
            "/cohorts/:id/samples",
            get(|s, p, auth, q| async move { CohortJsonResult(list_samples(s, p, auth, q).await) })
                .post(|s, p, auth, h, j| async move { CohortJsonResult(add_samples(s, p, auth, h, j).await) }),
        )
        .route(
            "/cohorts/:id/samples/:sid",
            get(|s, p, auth| async move { CohortJsonResult(get_sample(s, p, auth).await) })
                .put(|s, p, auth, j| async move { CohortJsonResult(update_sample(s, p, auth, j).await) })
                .delete(|s, p, auth| async move { CohortJsonResult(remove_sample(s, p, auth).await) }),
        )
        .route(
            "/cohorts/:id/query",
            post(|s, p, auth, j| async move { CohortJsonResult(query_cohort(s, p, auth, j).await) }),
        )
        .route(
            "/phenotype-schema",
            get(|s| async move { CohortJsonResult(get_schema(s).await) }),
        )
        .route(
            "/cohorts/:id/stats",
            get(|s, p, auth| async move { CohortJsonResult(cohort_stats(s, p, auth).await) }),
        )
        .route(
            "/cohorts/:id/versions",
            get(|s, p, auth| async move { CohortJsonResult(list_versions(s, p, auth).await) }),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", CohortApiDoc::openapi()))
        .with_state(state)
}
