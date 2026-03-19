//! API Gateway: merges all GA4GH service routers under standard paths.
//! A01: Auth middleware on every request. A05: Security headers, CORS from config.

mod admin;

use axum::http::header;
use axum::{routing::get, Router};
use ferrum_core::health::health_router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// WES router params: pool, work dir base, optional TES URL, optional TRS register URL, optional provenance store, optional pricing config, optional MultiQC config, optional DRS ingest base URL, allowed_workflow_sources. When None, WES routes return 503.
pub type WesRouterParams = (
    sqlx::PgPool,
    Option<std::path::PathBuf>,
    Option<String>,
    Option<String>,
    Option<std::sync::Arc<ferrum_core::ProvenanceStore>>,
    Option<ferrum_core::PricingConfig>,
    Option<ferrum_core::MultiQCConfig>,
    Option<String>,
    Vec<String>,
);

/// TES router params: pool, backend name ("podman" | "slurm"), optional work dir. When None, TES routes return 503.
pub type TesRouterParams = (sqlx::PgPool, Option<String>, Option<std::path::PathBuf>);

/// TRS router params: pool. When None, TRS routes return 503.
pub type TrsRouterParams = sqlx::PgPool;

/// Beacon router params: pool. When None, Beacon routes return 503.
pub type BeaconRouterParams = Option<sqlx::PgPool>;

/// Passports router params: pool. When None, Passports routes return 503.
pub type PassportRouterParams = Option<sqlx::PgPool>;

/// Cohorts router params: pool. When None, Cohorts routes return 503.
pub type CohortRouterParams = Option<sqlx::PgPool>;

/// Workspaces router params: pool. When None, Workspaces routes return 503.
pub type WorkspacesRouterParams = Option<sqlx::PgPool>;

/// Build the unified gateway app with all GA4GH routes.
/// Config can be used to enable/disable services via `config.services`.
/// When DRS is enabled, pass Some(drs_state) with DB/storage; None returns 503 for DRS routes.
/// When htsget is enabled, pass Some(htsget_state) (same DB as DRS + public base URL for stream links); None returns 503 for htsget.
/// When WES is enabled, pass Some(wes_params); None and enable_wes yields 503 for WES routes.
/// When admin_pool is Some, mounts /admin (token revoke, security events); requires admin auth.
#[allow(clippy::too_many_arguments)]
pub fn app(
    config: Option<&ferrum_core::AppConfig>,
    drs_state: Option<ferrum_drs::AppState>,
    htsget_state: Option<std::sync::Arc<ferrum_htsget::HtsgetState>>,
    wes_params: Option<WesRouterParams>,
    tes_params: Option<TesRouterParams>,
    trs_params: Option<TrsRouterParams>,
    beacon_params: BeaconRouterParams,
    passport_params: PassportRouterParams,
    cohort_params: CohortRouterParams,
    workspaces_pool: WorkspacesRouterParams,
    admin_pool: Option<sqlx::PgPool>,
) -> Router {
    let cfg = config;

    // Auth middleware config: use env FERRUM_AUTH__REQUIRE_AUTH so demo mode is reliable (config crate env parsing can vary).
    // Only when explicitly "true" do we use loaded config's auth; otherwise middleware gets demo() so unauthenticated requests get demo-user.
    let auth_config = match std::env::var("FERRUM_AUTH__REQUIRE_AUTH").as_deref() {
        Ok("true") => cfg
            .map(|c| {
                Arc::new(ferrum_core::AuthMiddlewareConfig::from_crate_config(
                    &c.auth,
                ))
            })
            .or_else(|| ferrum_core::AuthMiddlewareConfig::from_env_strict().map(Arc::new))
            .unwrap_or_else(|| {
                tracing::warn!(
                    "FERRUM_AUTH__REQUIRE_AUTH=true but no auth config file and FERRUM_AUTH__JWT_SECRET missing; using demo auth (HelixTest auth tests will fail)"
                );
                Arc::new(ferrum_core::AuthMiddlewareConfig::demo())
            }),
        _ => Arc::new(ferrum_core::AuthMiddlewareConfig::demo()),
    };
    let cors = cfg
        .and_then(|c| c.security.as_ref())
        .and_then(|s| {
            let origins: Vec<axum::http::HeaderValue> = s
                .allowed_origins
                .as_ref()?
                .iter()
                .filter_map(|o| axum::http::HeaderValue::try_from(o.as_str()).ok())
                .collect();
            if origins.is_empty() {
                return Some(CorsLayer::permissive());
            }
            Some(
                CorsLayer::new()
                    .allow_origin(origins)
                    .allow_credentials(s.allow_credentials.unwrap_or(false)),
            )
        })
        .unwrap_or_else(CorsLayer::permissive);

    let mut app = Router::new()
        .merge(health_router())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; object-src 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("geolocation=(), camera=(), microphone=()"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-powered-by"),
            axum::http::HeaderValue::from_static("Ferrum"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::SERVER,
            axum::http::HeaderValue::from_static("Ferrum"),
        ));

    // GA4GH standard paths (add all nests first)
    if cfg.map(|c| c.services.enable_drs).unwrap_or(true) {
        let drs_router = match drs_state {
            Some(state) => ferrum_drs::router(state),
            None => ferrum_drs::router_unconfigured(),
        };
        app = app.nest("/ga4gh/drs/v1", drs_router);
    }
    if cfg.map(|c| c.services.enable_trs).unwrap_or(true) {
        let trs_router = match trs_params {
            Some(pool) => ferrum_trs::router(pool),
            None => ferrum_trs::router_unconfigured(),
        };
        app = app.nest("/ga4gh/trs/v2", trs_router);
    }
    if cfg.map(|c| c.services.enable_wes).unwrap_or(true) {
        let wes_router = match wes_params {
            Some((
                pool,
                work_dir,
                tes_url,
                trs_register_url,
                provenance_store,
                pricing,
                multiqc_config,
                drs_ingest_base_url,
                allowed_workflow_sources,
            )) => ferrum_wes::router(
                pool,
                work_dir,
                tes_url,
                trs_register_url,
                provenance_store,
                pricing,
                multiqc_config,
                drs_ingest_base_url,
                allowed_workflow_sources,
            ),
            None => ferrum_wes::router_unconfigured(),
        };
        app = app.nest("/ga4gh/wes/v1", wes_router);
    }
    if cfg.map(|c| c.services.enable_tes).unwrap_or(true) {
        let tes_router = match tes_params {
            Some((pool, backend, work_dir)) => ferrum_tes::router(pool, backend, work_dir),
            None => ferrum_tes::router_unconfigured(),
        };
        app = app.nest("/ga4gh/tes/v1", tes_router);
    }
    if cfg.map(|c| c.services.enable_beacon).unwrap_or(true) {
        let beacon_router = match beacon_params {
            Some(pool) => ferrum_beacon::router(pool),
            None => ferrum_beacon::router_unconfigured(),
        };
        app = app.nest("/ga4gh/beacon/v2", beacon_router);
    }
    if cfg.map(|c| c.services.enable_passports).unwrap_or(true) {
        let passport_router = match passport_params {
            Some(pool) => ferrum_passports::router(pool),
            None => ferrum_passports::router_unconfigured(),
        };
        app = app.nest("/passports/v1", passport_router);
    }
    if cfg.map(|c| c.services.enable_crypt4gh).unwrap_or(true) {
        app = app.nest("/ga4gh/crypt4gh/v1", ferrum_crypt4gh::router());
    }
    if cfg.map(|c| c.services.enable_htsget).unwrap_or(true) {
        let hts_router = match htsget_state {
            Some(state) => ferrum_htsget::router(state),
            None => ferrum_htsget::router_unconfigured(),
        };
        app = app.nest("/ga4gh/htsget/v1", hts_router);
    }
    if let Some(pool) = cohort_params {
        app = app.nest("/cohorts/v1", ferrum_cohorts::router(pool));
    }
    if let Some(pool) = workspaces_pool {
        let (email_sender, invite_base_url) = match cfg.and_then(|c| c.email.as_ref()) {
            Some(email_cfg) => {
                let url = email_cfg.base_url.clone();
                #[cfg(feature = "workspaces_email")]
                let sender = ferrum_workspaces::SmtpEmailSender::new(email_cfg)
                    .ok()
                    .map(|s| Arc::new(s) as Arc<dyn ferrum_workspaces::email::EmailSender>);
                #[cfg(not(feature = "workspaces_email"))]
                let sender = None;
                (sender, url)
            }
            None => (None, None),
        };
        app = app.nest(
            "/workspaces/v1",
            ferrum_workspaces::router(pool, email_sender, invite_base_url),
        );
    }
    if let Some(pool) = admin_pool {
        app = app.nest("/admin", admin::admin_router(pool, cfg));
    }

    // UI: static files from services/ui (when built/present)
    let ui_path = std::path::Path::new("services/ui");
    if ui_path.exists() {
        app = app.nest_service("/ui", tower_http::services::ServeDir::new(ui_path));
    } else {
        app = app
            .route("/ui", get(ui_placeholder))
            .route("/ui/*path", get(ui_placeholder));
    }

    // A01: Auth middleware wraps the complete router (all nests). Apply last so every request to /workspaces, /cohorts, etc. goes through it.
    let auth_cfg = auth_config.clone();
    app = app.layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let config = std::sync::Arc::clone(&auth_cfg);
            async move { ferrum_core::auth_middleware_with_config(Some(config), req, next).await }
        },
    ));

    app
}

async fn ui_placeholder() -> &'static str {
    "UI not built. Add frontend to services/ui and rebuild."
}

/// Run the gateway server on the given address.
/// Pass Some(drs_state) when DRS is enabled; Some(wes_params) when WES is enabled; Some(tes_params) when TES is enabled.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    bind: SocketAddr,
    config: Option<ferrum_core::AppConfig>,
    drs_state: Option<ferrum_drs::AppState>,
    htsget_state: Option<std::sync::Arc<ferrum_htsget::HtsgetState>>,
    wes_params: Option<WesRouterParams>,
    tes_params: Option<TesRouterParams>,
    trs_params: Option<TrsRouterParams>,
    beacon_params: BeaconRouterParams,
    passport_params: PassportRouterParams,
    cohort_params: CohortRouterParams,
    workspaces_pool: WorkspacesRouterParams,
    admin_pool: Option<sqlx::PgPool>,
) -> Result<(), std::io::Error> {
    let app = app(
        config.as_ref(),
        drs_state,
        htsget_state,
        wes_params,
        tes_params,
        trs_params,
        beacon_params,
        passport_params,
        cohort_params,
        workspaces_pool,
        admin_pool,
    );
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Gateway listening on {}", bind);
    axum::serve(listener, app).await
}
