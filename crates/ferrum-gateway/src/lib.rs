//! API Gateway: merges all GA4GH service routers under standard paths.

use axum::{routing::get, Router};
use ferrum_core::health::health_router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// WES router params: pool, work dir base, optional TES URL, optional TRS register URL. When None, WES routes return 503.
pub type WesRouterParams = (sqlx::PgPool, Option<std::path::PathBuf>, Option<String>, Option<String>);

/// TES router params: pool, backend name ("podman" | "slurm"), optional work dir. When None, TES routes return 503.
pub type TesRouterParams = (sqlx::PgPool, Option<String>, Option<std::path::PathBuf>);

/// TRS router params: pool. When None, TRS routes return 503.
pub type TrsRouterParams = sqlx::PgPool;

/// Beacon router params: pool. When None, Beacon routes return 503.
pub type BeaconRouterParams = Option<sqlx::PgPool>;

/// Passports router params: pool. When None, Passports routes return 503.
pub type PassportRouterParams = Option<sqlx::PgPool>;

/// Build the unified gateway app with all GA4GH routes.
/// Config can be used to enable/disable services via `config.services`.
/// When DRS is enabled, pass Some(drs_state) with DB/storage; None returns 503 for DRS routes.
/// When WES is enabled, pass Some(wes_params); None and enable_wes yields 503 for WES routes.
pub fn app(
    config: Option<&ferrum_core::AppConfig>,
    drs_state: Option<ferrum_drs::AppState>,
    wes_params: Option<WesRouterParams>,
    tes_params: Option<TesRouterParams>,
    trs_params: Option<TrsRouterParams>,
    beacon_params: BeaconRouterParams,
    passport_params: PassportRouterParams,
) -> Router {
    let cfg = config;

    let mut app = Router::new()
        .merge(health_router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    // GA4GH standard paths
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
            Some((pool, work_dir, tes_url, trs_register_url)) => {
                ferrum_wes::router(pool, work_dir, tes_url, trs_register_url, None)
            }
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

    // UI: static files from services/ui (when built/present)
    let ui_path = std::path::Path::new("services/ui");
    if ui_path.exists() {
        app = app.nest_service(
            "/ui",
            tower_http::services::ServeDir::new(ui_path),
        );
    } else {
        app = app
            .route("/ui", get(ui_placeholder))
            .route("/ui/*path", get(ui_placeholder));
    }

    app
}

async fn ui_placeholder() -> &'static str {
    "UI not built. Add frontend to services/ui and rebuild."
}

/// Run the gateway server on the given address.
/// Pass Some(drs_state) when DRS is enabled; Some(wes_params) when WES is enabled; Some(tes_params) when TES is enabled.
pub async fn run(
    bind: SocketAddr,
    config: Option<ferrum_core::AppConfig>,
    drs_state: Option<ferrum_drs::AppState>,
    wes_params: Option<WesRouterParams>,
    tes_params: Option<TesRouterParams>,
    trs_params: Option<TrsRouterParams>,
    beacon_params: BeaconRouterParams,
    passport_params: PassportRouterParams,
) -> Result<(), std::io::Error> {
    let app = app(config.as_ref(), drs_state, wes_params, tes_params, trs_params, beacon_params, passport_params);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Gateway listening on {}", bind);
    axum::serve(listener, app).await
}
