//! Ferrum Crypt4GH: encryption, transparent DRS proxy, policy engine, key distribution.

pub mod encryption;
pub mod error;
pub mod keys;
pub mod policy;
pub mod proxy;

pub use encryption::{
    encrypt_bytes_for_pubkey, generate_keypair, load_recipient_keys, recipient_keys_from_pubkey,
    reencrypt_bytes, stream_decrypt, stream_encrypt, stream_reencrypt, C4ghKeys, DatabaseKeyStore,
    KeyStore, LocalKeyStore,
};
pub use error::{Crypt4GHError, Result};
pub use keys::{
    keys_router, KeyExchangeRequest, KeyExchangeResponse, KeyExchangeState, NodeKeypair,
    ObjectFetcher,
};
pub use policy::{DataAccessPolicy, PolicyEngine, VISA_TYPE_CONTROLLED_ACCESS_GRANTS};
pub use proxy::{Crypt4GHLayer, Crypt4GHProxyConfig, HEADER_CRYPT4GH_PUBLIC_KEY};

use axum::{routing::get, Json, Router};
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Serialize, ToSchema)]
pub struct Crypt4GhServiceInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(get_service_info, get_encrypted_object),
    components(schemas(
        Crypt4GhServiceInfo,
        keys::KeyExchangeRequest,
        keys::KeyExchangeResponse
    ))
)]
pub struct Crypt4GhApiDoc;

/// Returns the Crypt4GH router: service-info, objects, and keys exchange.
/// Mount at e.g. /ga4gh/crypt4gh/v1. Optionally nest keys_router at /keys.
pub fn router() -> Router {
    Router::new()
        .route("/service-info", get(get_service_info))
        .route("/objects/{object_id}", get(get_encrypted_object))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", Crypt4GhApiDoc::openapi()))
}

#[utoipa::path(get, path = "/service-info", responses((status = 200, body = Crypt4GhServiceInfo)))]
async fn get_service_info() -> Json<Crypt4GhServiceInfo> {
    Json(Crypt4GhServiceInfo {
        id: "ferrum-crypt4gh".to_string(),
        name: "Ferrum Crypt4GH".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/objects/{object_id}",
    responses((status = 200, description = "Crypt4GH-encrypted object stream"))
)]
async fn get_encrypted_object(
    axum::extract::Path(_object_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Use DRS endpoint with X-Crypt4GH-Public-Key header for re-encrypted stream, or POST /keys/exchange for header delivery"
    }))
}
