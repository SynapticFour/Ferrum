//! GA4GH Data Repository Service (DRS) 1.4.

pub mod error;
pub mod handlers;
pub mod ingest;
pub mod presign;
pub mod repo;
pub mod state;
pub mod types;
pub mod uri;

use axum::{
    routing::{get, post},
    Router,
};
use crate::error::{JsonResult, ViewResult};
pub use state::AppState;
use handlers::{
    delete_object, get_access, get_object, get_object_provenance, get_object_view, get_service_info, list_objects,
    options_object, post_object, put_object,
};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::get_object,
        handlers::get_object_provenance,
        handlers::options_object,
        handlers::get_access,
        handlers::post_object,
        handlers::put_object,
        handlers::delete_object,
        handlers::list_objects,
        ingest::ingest_file,
        ingest::ingest_url,
        ingest::ingest_batch,
    ),
    components(schemas(
        handlers::ExpandQuery,
        types::DrsObject,
        types::AccessUrl,
        types::ContentsObject,
        types::CreateObjectRequest,
        types::UpdateObjectRequest,
        types::ListObjectsQuery,
        types::IngestUrlRequest,
        types::IngestBatchRequest,
        types::IngestBatchItem,
        handlers::CreatedResponse,
        handlers::UpdatedResponse,
        handlers::DeletedResponse,
        ingest::IngestFileResponse,
        ingest::IngestUrlResponse,
        ingest::IngestBatchResponse,
        handlers::ProvenanceQuery,
        handlers::ProvenanceResponse,
        handlers::ProvenanceGraphResponse,
        ferrum_core::Checksum,
        ferrum_core::AccessMethod,
        ferrum_core::ServiceInfo,
    ))
)]
pub struct DrsApiDoc;

/// Build the DRS router. Mount at e.g. /ga4gh/drs/v1.
/// Requires AppState with repo (and optional storage for ingest).
pub fn router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/service-info", get(get_service_info))
        .route(
            "/objects",
            get(|s, q| async move { JsonResult(list_objects(s, q).await) })
                .post(|s, j| async move { JsonResult(post_object(s, j).await) }),
        )
        .route(
            "/objects/{object_id}",
            get(|s, p, q, h| async move { JsonResult(get_object(s, p, q, h).await) })
                .put(|s, p, j| async move { JsonResult(put_object(s, p, j).await) })
                .delete(|s, p| async move { JsonResult(delete_object(s, p).await) })
                .options(options_object),
        )
        .route(
            "/objects/{object_id}/provenance",
            get(|s, p, q| async move { JsonResult(get_object_provenance(s, p, q).await) }),
        )
        .route(
            "/objects/{object_id}/access/{access_id}",
            get(|s, p, h| async move { JsonResult(get_access(s, p, h).await) }),
        )
        .route(
            "/objects/{object_id}/view",
            get(|s, p| async move { ViewResult(get_object_view(s, p).await) }),
        )
        .route(
            "/ingest/file",
            post(|s, m| async move { JsonResult(ingest::ingest_file(s, m).await) }),
        )
        .route(
            "/ingest/url",
            post(|s, j| async move { JsonResult(ingest::ingest_url(s, j).await) }),
        )
        .route(
            "/ingest/batch",
            post(|s, j| async move { JsonResult(ingest::ingest_batch(s, j).await) }),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", DrsApiDoc::openapi()))
        .with_state(state)
}

/// Router that returns 503 for all routes when DRS is enabled but not configured (no DB/storage).
pub fn router_unconfigured() -> Router {
    use axum::response::IntoResponse;
    async fn unconfigured() -> impl IntoResponse {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "DRS not configured")
    }
    Router::new()
        .route("/service-info", get(unconfigured))
        .route("/objects", get(unconfigured).post(unconfigured))
        .route("/objects/{object_id}", get(unconfigured).put(unconfigured).delete(unconfigured))
        .route("/objects/{object_id}/access/{access_id}", get(unconfigured))
        .route("/objects/{object_id}/view", get(unconfigured))
        .route("/ingest/file", post(unconfigured))
        .route("/ingest/url", post(unconfigured))
        .route("/ingest/batch", post(unconfigured))
        .fallback(get(unconfigured))
}
