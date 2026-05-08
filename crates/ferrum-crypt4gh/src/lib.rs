//! Ferrum Crypt4GH: encryption, transparent DRS proxy, policy engine, key distribution.

pub mod encryption;
pub mod error;
pub mod policy;
pub mod proxy;

pub use encryption::{
    encrypt_bytes_for_pubkey, generate_keypair, load_recipient_keys, recipient_keys_from_pubkey,
    reencrypt_bytes, stream_decrypt, stream_encrypt, stream_reencrypt, C4ghKeys, DatabaseKeyStore,
    KeyStore, LocalKeyStore,
};
pub use error::{Crypt4GHError, Result};
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
#[openapi(paths(get_service_info), components(schemas(Crypt4GhServiceInfo)))]
pub struct Crypt4GhApiDoc;

/// Returns the Crypt4GH router (service-info + OpenAPI docs).
/// Mount at e.g. /ga4gh/crypt4gh/v1.
pub fn router() -> Router {
    Router::new()
        .route("/service-info", get(get_service_info))
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
