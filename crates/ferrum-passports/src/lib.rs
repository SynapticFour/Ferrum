//! GA4GH Passports & Visas (AAI) API: Passport Broker, Visa Issuer, JWKS, admin for visa grants.

pub mod config;
pub mod error;
pub mod handlers;
pub mod keys;
pub mod repo;
pub mod types;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::PassportConfig;
use crate::handlers::{AppState, OidcConfiguration, TokenResponse};
use crate::handlers::{
    admin_create_visa_grant, admin_delete_visa_grant, admin_list_visa_grants,
    get_oidc_configuration, authorize, token, userinfo, jwks,
};
use crate::keys::SigningKeys;
use crate::repo::PassportRepo;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::get_oidc_configuration,
        handlers::admin_list_visa_grants,
        handlers::admin_create_visa_grant,
    ),
    components(schemas(OidcConfiguration, TokenResponse, handlers::CreateVisaGrantRequest))
)]
pub struct PassportsApiDoc;

/// Returns a router that responds 503 when Passports is not configured.
pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Passports not configured",
        )
    })
}

/// Returns the Passports router. Requires a PostgreSQL pool. Mount at /passports/v1 in gateway.
pub fn router(pool: sqlx::PgPool) -> Router {
    let config = PassportConfig::from_env();
    let keys = SigningKeys::from_config(&config).expect("signing keys");
    let state = Arc::new(AppState {
        config,
        keys,
        repo: Arc::new(PassportRepo::new(pool)),
    });
    Router::new()
        .route("/.well-known/openid-configuration", get(get_oidc_configuration))
        .route("/.well-known/jwks.json", get(jwks))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .route("/userinfo", get(userinfo))
        .route("/admin/visa_grants", get(admin_list_visa_grants).post(admin_create_visa_grant))
        .route("/admin/visa_grants/:id", axum::routing::delete(admin_delete_visa_grant))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", PassportsApiDoc::openapi()))
        .with_state(state)
}
