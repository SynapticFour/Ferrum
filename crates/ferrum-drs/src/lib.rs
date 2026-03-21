//! GA4GH Data Repository Service (DRS) 1.4.

pub mod api_v1;
pub mod error;
pub mod handlers;
pub mod ingest;
pub mod presign;
pub mod repo;
pub mod state;
pub mod types;
pub mod uri;

use crate::error::{JsonResult, StreamResult, ViewResult};
use crate::types::{CreateObjectRequest, ListObjectsQuery};
pub use api_v1::{ingest_api_v1_router, ingest_api_v1_router_unconfigured};
use axum::{
    extract::{Extension, Multipart, Query, State},
    routing::{get, post},
    Json, Router,
};
use handlers::{
    delete_object, get_access, get_object, get_object_provenance, get_object_stream,
    get_object_view, get_service_info, list_bundle_contents, list_objects, options_object,
    post_object, put_object,
};
pub use state::AppState;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_service_info,
        handlers::get_object,
        handlers::get_object_provenance,
        handlers::list_bundle_contents,
        handlers::options_object,
        handlers::get_access,
        handlers::get_object_stream,
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
        handlers::BundleContentsQuery,
        handlers::BundleContentsPage,
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

async fn list_objects_json(
    state: State<Arc<AppState>>,
    query: Query<ListObjectsQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> impl axum::response::IntoResponse {
    JsonResult(list_objects(state, query, auth).await)
}

async fn post_object_json(
    state: State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    req: Json<CreateObjectRequest>,
) -> impl axum::response::IntoResponse {
    JsonResult(post_object(state, req, auth).await)
}

async fn ingest_file_json(
    state: State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    m: Multipart,
) -> impl axum::response::IntoResponse {
    JsonResult(ingest::ingest_file(state, m, auth).await)
}

async fn ingest_url_json(
    state: State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    req: Json<crate::types::IngestUrlRequest>,
) -> impl axum::response::IntoResponse {
    JsonResult(ingest::ingest_url(state, req, auth).await)
}

async fn ingest_batch_json(
    state: State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    req: Json<crate::types::IngestBatchRequest>,
) -> impl axum::response::IntoResponse {
    JsonResult(ingest::ingest_batch(state, req, auth).await)
}

/// Build the DRS router. Mount at e.g. /ga4gh/drs/v1.
/// Requires AppState with repo (and optional storage for ingest).
pub fn router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/objects", get(list_objects_json).post(post_object_json))
        .route(
            "/objects/:object_id",
            get(get_object)
                .put(|s, p, j| async move { JsonResult(put_object(s, p, j).await) })
                .delete(|s, p| async move { JsonResult(delete_object(s, p).await) })
                .options(options_object),
        )
        .route(
            "/objects/:object_id/contents",
            get(|s, p, q, h, auth| async move {
                JsonResult(list_bundle_contents(s, p, q, h, auth).await)
            }),
        )
        .route(
            "/objects/:object_id/provenance",
            get(|s, p, q| async move { JsonResult(get_object_provenance(s, p, q).await) }),
        )
        .route(
            "/objects/:object_id/access/:access_id",
            get(|s, p, h, auth| async move { JsonResult(get_access(s, p, h, auth).await) }),
        )
        .route(
            "/objects/:object_id/view",
            get(|s, p, auth| async move { ViewResult(get_object_view(s, p, auth).await) }),
        )
        .route(
            "/objects/:object_id/stream",
            get(
                |s, p, h, auth| async move { StreamResult(get_object_stream(s, p, h, auth).await) },
            ),
        )
        .route("/ingest/file", post(ingest_file_json))
        .route("/ingest/url", post(ingest_url_json))
        .route("/ingest/batch", post(ingest_batch_json))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", DrsApiDoc::openapi()))
        .with_state(state)
}

/// Router that returns 503 for all routes when DRS is enabled but not configured (no DB/storage).
pub fn router_unconfigured() -> Router {
    use axum::response::IntoResponse;
    async fn unconfigured() -> impl IntoResponse {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "DRS not configured",
        )
    }
    Router::new()
        .route("/service-info", get(unconfigured))
        .route("/objects", get(unconfigured).post(unconfigured))
        .route(
            "/objects/:object_id",
            get(unconfigured).put(unconfigured).delete(unconfigured),
        )
        .route("/objects/:object_id/access/:access_id", get(unconfigured))
        .route("/objects/:object_id/view", get(unconfigured))
        .route("/objects/:object_id/stream", get(unconfigured))
        .route("/ingest/file", post(unconfigured))
        .route("/ingest/url", post(unconfigured))
        .route("/ingest/batch", post(unconfigured))
        .fallback(get(unconfigured))
}
