//! API Gateway: merges all GA4GH service routers under standard paths.
//! A01: Auth middleware on every request. A05: Security headers, CORS from config.

#[cfg(feature = "full")]
mod admin;
pub mod audit;
#[cfg(feature = "discovery")]
mod access;
#[cfg(feature = "full")]
mod publish;
#[cfg(feature = "full")]
mod federation;
pub mod outbreak;
pub mod power;
pub mod shutdown;
use axum::http::header;
use axum::response::IntoResponse;
use axum::{routing::get, Router};
use ferrum_core::config::watch::ConfigWatcher;
use ferrum_core::config::FerrumConfig;
use ferrum_core::health::health_router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// WES router params: pool, work dir base, optional TES URL, optional TRS register URL, optional provenance store, optional pricing config, optional MultiQC config, optional DRS ingest base URL, allowed_workflow_sources. When None, WES routes return 503.
#[cfg(feature = "full")]
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
    Option<std::sync::Arc<ferrum_core::AdsIntrospectClient>>,
);
#[cfg(not(feature = "full"))]
pub type WesRouterParams = ();

/// TES router params: pool, backend name ("podman" | "slurm"), optional work dir. When None, TES routes return 503.
#[cfg(feature = "full")]
pub type TesRouterParams = (sqlx::PgPool, Option<String>, Option<std::path::PathBuf>);
#[cfg(not(feature = "full"))]
pub type TesRouterParams = ();

/// TRS router params: pool. When None, TRS routes return 503.
#[cfg(feature = "full")]
pub type TrsRouterParams = sqlx::PgPool;
#[cfg(not(feature = "full"))]
pub type TrsRouterParams = ();

/// Beacon router params: pool. When None, Beacon routes return 503.
pub type BeaconRouterParams = Option<ferrum_core::FerrumPool>;

/// Passports router params: pool. When None, Passports routes return 503.
#[cfg(feature = "full")]
pub type PassportRouterParams = Option<sqlx::PgPool>;
#[cfg(not(feature = "full"))]
pub type PassportRouterParams = Option<()>;

/// Cohorts router params: pool. When None, Cohorts routes return 503.
#[cfg(feature = "full")]
pub type CohortRouterParams = Option<sqlx::PgPool>;
#[cfg(not(feature = "full"))]
pub type CohortRouterParams = Option<()>;

/// Workspaces router params: pool. When None, Workspaces routes return 503.
#[cfg(feature = "full")]
pub type WorkspacesRouterParams = Option<sqlx::PgPool>;
#[cfg(not(feature = "full"))]
pub type WorkspacesRouterParams = Option<()>;

/// Build the unified gateway app with all GA4GH routes.
/// Config can be used to enable/disable services via `config.services`.
/// When DRS is enabled, pass Some(drs_state) with DB/storage; None returns 503 for DRS routes.
/// When htsget is enabled, pass Some(htsget_state) (same DB as DRS + public base URL for stream links); None returns 503 for htsget.
/// When WES is enabled, pass Some(wes_params); None and enable_wes yields 503 for WES routes.
/// When admin_pool is Some, mounts /admin (token revoke, security events); requires admin auth.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(feature = "full"), allow(unused_variables))]
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
    #[cfg(feature = "full")] admin_pool: Option<sqlx::PgPool>,
    shutdown_coordinator: Arc<shutdown::ShutdownCoordinator>,
    config_watch_rx: Option<watch::Receiver<Arc<FerrumConfig>>>,
) -> Router {
    let cfg = config;
    let hot_reload = config_watch_rx.is_some();

    // Resolve auth config deterministically:
    // 1) config file values when present
    // 2) strict env config when available
    // 3) demo fallback
    // Optional env override: FERRUM_AUTH__REQUIRE_AUTH=true|false (explicit only).
    let mut resolved_auth = cfg
        .map(|c| ferrum_core::AuthMiddlewareConfig::from_crate_config(&c.auth))
        .or_else(ferrum_core::AuthMiddlewareConfig::from_env_strict)
        .unwrap_or_else(ferrum_core::AuthMiddlewareConfig::demo);

    if let Ok(override_value) = std::env::var("FERRUM_AUTH__REQUIRE_AUTH") {
        let parsed = override_value.trim().to_ascii_lowercase();
        match parsed.as_str() {
            "true" => resolved_auth.require_auth = true,
            "false" => resolved_auth.require_auth = false,
            _ => tracing::warn!(
                value = %override_value,
                "invalid FERRUM_AUTH__REQUIRE_AUTH value; expected true/false"
            ),
        }
    }

    if !resolved_auth.require_auth {
        tracing::warn!(
            "authentication is running in demo mode (require_auth=false); this is intended for local/demo deployments only"
        );
    }
    let auth_config = Arc::new(resolved_auth);
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
    if hot_reload || cfg.map(|c| c.services.enable_drs).unwrap_or(true) {
        match &drs_state {
            Some(state) => {
                app = app.nest("/ga4gh/drs/v1", ferrum_drs::router(state.clone()));
                app = app.nest(
                    "/api/v1/ingest",
                    ferrum_drs::ingest_api_v1_router(std::sync::Arc::new(state.clone())),
                );
            }
            None => {
                app = app.nest("/ga4gh/drs/v1", ferrum_drs::router_unconfigured());
                app = app.nest(
                    "/api/v1/ingest",
                    ferrum_drs::ingest_api_v1_router_unconfigured(),
                );
            }
        }
    }
    #[cfg(feature = "full")]
    if hot_reload || cfg.map(|c| c.services.enable_trs).unwrap_or(true) {
        let trs_router = match trs_params {
            Some(pool) => ferrum_trs::router(pool),
            None => ferrum_trs::router_unconfigured(),
        };
        app = app.nest("/ga4gh/trs/v2", trs_router);
    }
    #[cfg(feature = "full")]
    if hot_reload || cfg.map(|c| c.services.enable_wes).unwrap_or(true) {
        let wes_router = match &wes_params {
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
                ads_introspect,
            )) => ferrum_wes::router(
                pool.clone(),
                work_dir.clone(),
                tes_url.clone(),
                trs_register_url.clone(),
                provenance_store.clone(),
                pricing.clone(),
                multiqc_config.clone(),
                drs_ingest_base_url.clone(),
                allowed_workflow_sources.clone(),
                ads_introspect.clone(),
            ),
            None => ferrum_wes::router_unconfigured(),
        };
        app = app.nest("/ga4gh/wes/v1", wes_router);
    }
    #[cfg(feature = "full")]
    if hot_reload || cfg.map(|c| c.services.enable_tes).unwrap_or(true) {
        let tes_router = match tes_params {
            Some((pool, backend, work_dir)) => ferrum_tes::router(pool, backend, work_dir),
            None => ferrum_tes::router_unconfigured(),
        };
        app = app.nest("/ga4gh/tes/v1", tes_router);
    }
    if hot_reload || cfg.map(|c| c.services.enable_beacon).unwrap_or(true) {
        let outbreak_service = cfg
            .filter(|c| c.outbreak.enabled)
            .zip(beacon_params.clone())
            .map(|(c, pool)| Arc::new(ferrum_core::OutbreakService::new(pool, c.outbreak.clone())));
        let residency_audit = beacon_params
            .clone()
            .map(|pool| Arc::new(ferrum_core::ResidencyAuditLog::new(pool)));
        let federation = cfg.zip(beacon_params.clone()).and_then(|(c, _)| {
            if !c.federation.enabled {
                return None;
            }
            ferrum_federation::FederationRuntime::from_config(&c.federation)
                .ok()
                .map(ferrum_federation::FederationClient::new)
                .map(Arc::new)
        });
        let reference_registry = drs_state
            .as_ref()
            .map(|ds| {
                Arc::new(ferrum_reference::ReferenceRegistry::new(
                    ds.repo.pool().clone(),
                ))
            })
            .or_else(|| {
                beacon_params
                    .clone()
                    .map(|pool| Arc::new(ferrum_reference::ReferenceRegistry::new(pool)))
            });
        let beacon_router = match beacon_params {
            Some(pool) => ferrum_beacon::router_with_services(
                pool,
                outbreak_service.clone(),
                federation,
                residency_audit.clone(),
                reference_registry.clone(),
            ),
            None => ferrum_beacon::router_unconfigured(),
        };
        app = app.nest("/ga4gh/beacon/v2", beacon_router);
        if let Some(svc) = outbreak_service {
            app = app.nest(
                "/api/v1/outbreak",
                outbreak::outbreak_router(svc, residency_audit.clone()),
            );
        }
        if let Some(audit) = residency_audit {
            app = app.nest("/api/v1/audit", audit::audit_router(audit));
        }
        if let Some(registry) = reference_registry {
            app = app.nest(
                "/api/v1/references",
                ferrum_reference::reference_api_v1_router(registry),
            );
        }
    }
    #[cfg(feature = "full")]
    if hot_reload
        || cfg
            .map(|c| c.services.enable_passports && !c.auth.is_external())
            .unwrap_or(true)
    {
        let passport_router = match passport_params {
            Some(pool) => ferrum_passports::router(pool),
            None => ferrum_passports::router_unconfigured(),
        };
        app = app.nest("/passports/v1", passport_router);
    }
    if hot_reload || cfg.map(|c| c.services.enable_crypt4gh).unwrap_or(true) {
        app = app.nest("/ga4gh/crypt4gh/v1", ferrum_crypt4gh::router());
    }
    if hot_reload || cfg.map(|c| c.services.enable_htsget).unwrap_or(true) {
        let hts_router = match htsget_state {
            Some(state) => ferrum_htsget::router(state),
            None => ferrum_htsget::router_unconfigured(),
        };
        app = app.nest("/ga4gh/htsget/v1", hts_router);
    }
    #[cfg(feature = "full")]
    if let Some(pool) = cohort_params {
        app = app.nest("/cohorts/v1", ferrum_cohorts::router(pool));
    }
    #[cfg(feature = "full")]
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
    #[cfg(feature = "full")]
    {
        app = app
            .nest("/admin/federation", federation::federation_router(cfg))
            .nest("/admin", admin::admin_router(admin_pool.as_ref(), cfg));
    }
    #[cfg(feature = "discovery")]
    if let Some(cfg) = config {
        if cfg.auth.is_external() || cfg.auth.ads_url.is_some() || cfg.discovery.enabled {
            app = app.nest("/access/v1", access::access_router(cfg));
        }
    }
    #[cfg(feature = "full")]
    if let (Some(pool), Some(cfg)) = (
        admin_pool.clone().or_else(|| {
            wes_params
                .as_ref()
                .map(|(pool, _, _, _, _, _, _, _, _, _)| pool.clone())
        }),
        config,
    ) {
        app = app.nest("/api/v1", publish::publish_router(pool, cfg));
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

    // Lesson 9: graceful shutdown for long-running transfers.
    // We reject new DRS stream requests with 503 and track in-flight streams until body drain ends.
    let shutdown_for_mw = Arc::clone(&shutdown_coordinator);
    app = app.layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let shutdown = Arc::clone(&shutdown_for_mw);
            async move {
                let path = req.uri().path().to_string();
                let is_drs_stream =
                    req.method() == axum::http::Method::GET && is_drs_stream_path(&path);
                if !is_drs_stream {
                    return next.run(req).await;
                }

                if shutdown.is_shutting_down() {
                    let mut res = (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "Service shutting down",
                    )
                        .into_response();
                    res.headers_mut().insert(
                        axum::http::header::RETRY_AFTER,
                        axum::http::HeaderValue::from_static("60"),
                    );
                    return res;
                }

                // Keep the guard alive until the response (including its streaming body) is dropped.
                // http::Extensions are dropped when the Response is dropped by the server runtime.
                let guard = shutdown.register_transfer();
                let mut response = next.run(req).await;
                response.extensions_mut().insert(guard);
                response
            }
        },
    ));

    // A01: Auth middleware wraps the complete router (all nests). Apply last so every request to /workspaces, /cohorts, etc. goes through it.
    let auth_cfg = auth_config.clone();
    app = app.layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let config = std::sync::Arc::clone(&auth_cfg);
            async move { ferrum_core::auth_middleware_with_config(Some(config), req, next).await }
        },
    ));

    // Config hot-reload gating:
    // Learned from production hot-reload patterns: when config changes, return `503 Service Unavailable`
    // for disabled services without restarting the HTTP server or rebuilding routers.
    if let Some(config_rx) = config_watch_rx {
        app = app.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let rx = config_rx.clone();
                async move {
                    // `tokio::sync::watch::Ref` is not `Send`, so keep it in a tight scope and
                    // compute a plain `bool` before awaiting `next.run(req)`.
                    let enabled = {
                        let cfg = rx.borrow();
                        let path = req.uri().path();

                        if path.starts_with("/ga4gh/drs/v1")
                            || path.starts_with("/api/v1/ingest")
                            || path.starts_with("/objects")
                        {
                            cfg.services.enable_drs
                        } else if path.starts_with("/ga4gh/wes/v1") {
                            cfg!(feature = "full") && cfg.services.enable_wes
                        } else if path.starts_with("/ga4gh/tes/v1") {
                            cfg!(feature = "full") && cfg.services.enable_tes
                        } else if path.starts_with("/ga4gh/trs/v2") {
                            cfg!(feature = "full") && cfg.services.enable_trs
                        } else if path.starts_with("/ga4gh/beacon/v2") {
                            cfg.services.enable_beacon
                        } else if path.starts_with("/passports/v1") {
                            cfg!(feature = "full") && cfg.services.enable_passports
                        } else if path.starts_with("/ga4gh/crypt4gh/v1") {
                            cfg.services.enable_crypt4gh
                        } else if path.starts_with("/ga4gh/htsget/v1") {
                            cfg.services.enable_htsget
                        } else {
                            true
                        }
                    };

                    if enabled {
                        next.run(req).await
                    } else {
                        (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            "service disabled via hot-reload config",
                        )
                            .into_response()
                    }
                }
            },
        ));
    }

    app
}

/// Minimal gateway app for laptop/embed integration tests (hides full vs laptop `app` arg differences).
pub fn app_laptop_embed(
    config: Option<&ferrum_core::AppConfig>,
    drs_state: Option<ferrum_drs::AppState>,
    beacon_params: BeaconRouterParams,
    shutdown_coordinator: Arc<shutdown::ShutdownCoordinator>,
) -> Router {
    app(
        config,
        drs_state,
        None,
        None,
        None,
        None,
        beacon_params,
        None,
        None,
        None,
        #[cfg(feature = "full")]
        None,
        shutdown_coordinator,
        None,
    )
}

async fn ui_placeholder() -> &'static str {
    "UI not built. Add frontend to services/ui and rebuild."
}

fn is_drs_stream_path(path: &str) -> bool {
    (path.starts_with("/ga4gh/drs/v1/objects/") && path.ends_with("/stream"))
        || (path.starts_with("/objects/") && path.ends_with("/stream"))
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
    #[cfg(feature = "full")] admin_pool: Option<sqlx::PgPool>,
) -> Result<(), std::io::Error> {
    let shutdown_coordinator = Arc::new(shutdown::ShutdownCoordinator::new());
    let drain_timeout_secs: u64 = std::env::var("FERRUM_DRAIN_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let drain_timeout = Duration::from_secs(drain_timeout_secs);

    // Config hot-reload wiring: spawn ConfigWatcher when a concrete config path is
    // provided via `FERRUM_CONFIG`, and pass the `watch::Receiver` into app-level
    // middleware so it can gate GA4GH routes without a restart.
    let mut config_watch_rx: Option<watch::Receiver<Arc<FerrumConfig>>> = None;
    if let Ok(p) = std::env::var("FERRUM_CONFIG") {
        let path = PathBuf::from(p);
        if path.exists() {
            let (rx, _handle) = ConfigWatcher::spawn(path);
            let mut log_rx = rx.clone();
            config_watch_rx = Some(rx);
            tokio::spawn(async move {
                loop {
                    if log_rx.changed().await.is_err() {
                        break;
                    }
                    let cfg = log_rx.borrow();
                    tracing::info!(
                        bind = %cfg.bind,
                        enable_beacon = cfg.services.enable_beacon,
                        enable_drs = cfg.services.enable_drs,
                        enable_tes = cfg.services.enable_tes,
                        enable_wes = cfg.services.enable_wes,
                        "config reloaded (hot reload listener)"
                    );
                }
            });
        } else {
            tracing::warn!(path = ?path, "FERRUM_CONFIG configured but file does not exist");
        }
    }

    let power_cfg = config.as_ref().map(|c| c.power.clone()).unwrap_or_default();
    let power_state = Arc::new(tokio::sync::Mutex::new(power::PowerState::new(power_cfg)));
    let background_gate = drs_state.as_ref().and_then(|s| s.background_gate.clone());
    power::spawn_power_watcher(
        Arc::clone(&power_state),
        beacon_params.clone(),
        background_gate.clone(),
    );

    if let Some(ref ds) = drs_state {
        if let (Some(bw), Some(tq), Some(gate)) = (
            ds.bandwidth.clone(),
            ds.transfer_queue.clone(),
            ds.background_gate.clone(),
        ) {
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if !gate.allows_background_work() {
                        continue;
                    }
                    let drained = tq.drain_if_ready(bw.as_ref());
                    if !drained.is_empty() {
                        tracing::info!(
                            count = drained.len(),
                            "transfer queue drained (client should retry deferred transfers)"
                        );
                    }
                }
            });
        }
    }

    if let Some(ref cfg) = config {
        #[cfg(feature = "discovery")]
        if cfg.discovery.enabled {
            if let Ok(client) = ferrum_discovery::ServiceRegistryClient::from_config(&cfg.discovery)
            {
                client.warm_cache().await;
            }
        }
        #[cfg(feature = "external-auth")]
        if cfg.discovery.enabled && cfg.discovery.auto_register {
            if let Ok(client) = ferrum_discovery::ServiceRegistryClient::from_config(&cfg.discovery)
            {
                let gateway_base = cfg
                    .discovery
                    .registration_base_url
                    .clone()
                    .or_else(|| std::env::var("FERRUM_PUBLIC_BASE_URL").ok())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| format!("http://{}", bind));
                let services = cfg.services.clone();
                let environment = cfg
                    .africa
                    .as_ref()
                    .map(|_| "africa".to_string())
                    .unwrap_or_else(|| "development".to_string());
                if let Err(err) = ferrum_discovery::register_ferrum_services(
                    &client,
                    &gateway_base,
                    &services,
                    &environment,
                )
                .await
                {
                    tracing::warn!(error = %err, "service registry auto-registration failed");
                }
            } else {
                tracing::warn!("discovery.auto_register enabled but service registry client could not be built");
            }
        }
        if cfg.auth.is_external() {
            tracing::info!(
                "external auth mode: ferrum-passports disabled; validating Passports via ga4gh-infra broker"
            );
        }
    }

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
        #[cfg(feature = "full")]
        admin_pool,
        Arc::clone(&shutdown_coordinator),
        config_watch_rx,
    );

    let app = app.layer(axum::middleware::from_fn(move |req, next| {
        let ps = Arc::clone(&power_state);
        async move { power::power_limit_middleware(ps, req, next).await }
    }));

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Gateway listening on {}", bind);

    let shutdown_for_server = Arc::clone(&shutdown_coordinator);
    let server_shutdown = async move {
        // Prefer SIGTERM in production, but fall back to Ctrl+C.
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            if let Ok(mut term) = signal(SignalKind::terminate()) {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            } else {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        shutdown_for_server.shutdown(drain_timeout).await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(server_shutdown)
        .await
}
