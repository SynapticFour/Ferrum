// SPDX-License-Identifier: BUSL-1.1
//! Ferrum Crypt4GH: encryption, transparent DRS proxy, policy engine, key distribution.

pub mod encryption;
pub mod error;
pub mod policy;
pub mod proxy;

pub use encryption::{
    encrypt_bytes_for_pubkey, generate_keypair, load_recipient_keys, recipient_keys_from_pubkey,
    reencrypt_bytes, stream_decrypt, stream_encrypt, stream_reencrypt, C4ghKeys, KeyStore,
    LocalKeyStore,
};
pub use error::{Crypt4GHError, Result};
pub use policy::{DataAccessPolicy, PolicyEngine, VISA_TYPE_CONTROLLED_ACCESS_GRANTS};
pub use proxy::{Crypt4GHLayer, Crypt4GHProxyConfig, HEADER_CRYPT4GH_PUBLIC_KEY};

use axum::{routing::get, Json, Router};
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::{Config, SwaggerUi};

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
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/openapi.json", Crypt4GhApiDoc::openapi())
                .config(Config::new(["/ga4gh/crypt4gh/v1/openapi.json"])),
        )
}

#[utoipa::path(get, path = "/service-info", responses((status = 200, body = Crypt4GhServiceInfo)))]
async fn get_service_info() -> Json<Crypt4GhServiceInfo> {
    Json(Crypt4GhServiceInfo {
        id: "ferrum-crypt4gh".to_string(),
        name: "Ferrum Crypt4GH".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod swagger_nest_tests {
    use super::router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn nested_swagger_ui_points_at_nested_openapi() {
        let app = Router::new().nest("/ga4gh/crypt4gh/v1", router());

        let openapi = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/ga4gh/crypt4gh/v1/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(openapi.status(), StatusCode::OK);

        let init = app
            .oneshot(
                Request::builder()
                    .uri("/ga4gh/crypt4gh/v1/swagger-ui/swagger-initializer.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(init.status(), StatusCode::OK);
        let bytes = init.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("/ga4gh/crypt4gh/v1/openapi.json"),
            "swagger-initializer must fetch the nested openapi path, got: {text}"
        );
        assert!(
            !text.contains("\"url\": \"/openapi.json\""),
            "swagger-initializer must not point at host-root /openapi.json: {text}"
        );
    }
}
