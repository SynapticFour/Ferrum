//! A07/A09: Admin routes — token revocation, security events, config. Config is public (sanitized).

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub struct AdminState {
    pub pool: Option<sqlx::PgPool>,
    /// Sanitized config for GET /admin/config (no secrets).
    pub config: Option<SanitizedConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedAuth {
    pub mode: String,
    pub require_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_public_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_login_url: Option<String>,
    /// Whether ADS-backed dataset access requests are available (external auth / ga4gh-infra).
    #[serde(default)]
    pub access_requests_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedDiscovery {
    pub enabled: bool,
    pub auto_register: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_registry_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedCompute {
    pub tes_backend: String,
    pub wes_trs_auto_register: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedIngest {
    /// Max single upload size in bytes (ingest API + gateway body limit).
    pub max_upload_bytes: u64,
    /// Max bytes per `POST /api/v1/ingest/upload/chunk` request body.
    pub max_chunk_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub bind: String,
    pub database: SanitizedDatabase,
    pub storage: SanitizedStorage,
    pub services: SanitizedServices,
    pub compute: SanitizedCompute,
    pub ingest: SanitizedIngest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery: Option<SanitizedDiscovery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<SanitizedAuth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedDatabase {
    pub driver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_set: Option<bool>,
    pub run_migrations: bool,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedStorage {
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_bucket: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedServices {
    pub enable_drs: bool,
    pub enable_wes: bool,
    pub enable_tes: bool,
    pub enable_trs: bool,
    pub enable_beacon: bool,
    pub enable_passports: bool,
    pub enable_crypt4gh: bool,
    /// True when a Crypt4GH key directory is configured so ingest can encrypt uploads.
    pub crypt4gh_ingest_ready: bool,
}

fn crypt4gh_ingest_ready(c: &ferrum_core::FerrumConfig) -> bool {
    let dir = std::env::var("FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            c.encryption
                .crypt4gh_key_dir
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .cloned()
        });
    let Some(dir) = dir else {
        return false;
    };
    let key_id = std::env::var("FERRUM_ENCRYPTION__CRYPT4GH_MASTER_KEY_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| c.encryption.crypt4gh_master_key_id.clone());
    let sec = std::path::Path::new(&dir).join(format!("{key_id}.sec"));
    let pub_key = std::path::Path::new(&dir).join(format!("{key_id}.pub"));
    sec.is_file() && pub_key.is_file()
}

/// POST /admin/tokens/revoke — revoke a token by jti (A07).
#[derive(Deserialize)]
pub struct RevokeRequest {
    pub jti: String,
}

#[derive(Serialize)]
pub struct RevokeResponse {
    pub revoked: bool,
}

async fn revoke_token(
    State(state): State<Arc<AdminState>>,
    Extension(auth): Extension<ferrum_core::AuthClaims>,
    Json(req): Json<RevokeRequest>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return (
            StatusCode::FORBIDDEN,
            Json(RevokeResponse { revoked: false }),
        );
    }
    let Some(pool) = state.pool.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(RevokeResponse { revoked: false }),
        );
    };
    let jti = req.jti.trim();
    if jti.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(RevokeResponse { revoked: false }),
        );
    }
    let r = sqlx::query(
        "INSERT INTO revoked_tokens (jti, reason) VALUES ($1, $2) ON CONFLICT (jti) DO NOTHING",
    )
    .bind(jti)
    .bind(None::<String>)
    .execute(pool)
    .await;
    match r {
        Ok(rows) => (
            StatusCode::OK,
            Json(RevokeResponse {
                revoked: rows.rows_affected() > 0,
            }),
        ),
        Err(e) => {
            tracing::warn!(?e, "revoke_token db error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RevokeResponse { revoked: false }),
            )
        }
    }
}

/// GET /admin/security/events — paginated security events (A09).
#[derive(Deserialize)]
pub struct EventsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub severity: Option<String>,
}

#[derive(Serialize)]
pub struct SecurityEventRow {
    pub id: String,
    pub event_type: String,
    pub severity: String,
    pub sub: Option<String>,
    pub ip_address: Option<String>,
    pub resource_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub occurred_at: Option<String>,
}

#[derive(Serialize)]
pub struct EventsResponse {
    pub events: Vec<SecurityEventRow>,
}

async fn list_security_events(
    State(state): State<Arc<AdminState>>,
    Extension(auth): Extension<ferrum_core::AuthClaims>,
    axum::extract::Query(q): axum::extract::Query<EventsQuery>,
) -> impl IntoResponse {
    if !auth.is_admin() {
        return (
            StatusCode::FORBIDDEN,
            Json(EventsResponse { events: vec![] }),
        );
    }
    let Some(pool) = state.pool.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(EventsResponse { events: vec![] }),
        );
    };
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);
    let severity = q.severity.as_deref().filter(|s| !s.is_empty());
    type EventRow = (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<serde_json::Value>,
        Option<chrono::DateTime<chrono::Utc>>,
    );
    let rows: Result<Vec<EventRow>, _> = if let Some(sev) = severity {
        sqlx::query_as(
            "SELECT id, event_type, severity, sub, ip_address, resource_id, details, occurred_at FROM security_events WHERE severity = $1 ORDER BY occurred_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(sev)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT id, event_type, severity, sub, ip_address, resource_id, details, occurred_at FROM security_events ORDER BY occurred_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
    };
    match rows {
        Ok(list) => (
            StatusCode::OK,
            Json(EventsResponse {
                events: list
                    .into_iter()
                    .map(
                        |(
                            id,
                            event_type,
                            severity,
                            sub,
                            ip_address,
                            resource_id,
                            details,
                            occurred_at,
                        )| SecurityEventRow {
                            id,
                            event_type,
                            severity,
                            sub,
                            ip_address,
                            resource_id,
                            details,
                            occurred_at: occurred_at.map(|t| t.to_rfc3339()),
                        },
                    )
                    .collect(),
            }),
        ),
        Err(e) => {
            tracing::warn!(?e, "list_security_events db error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(EventsResponse { events: vec![] }),
            )
        }
    }
}

async fn get_config(State(state): State<Arc<AdminState>>) -> impl IntoResponse {
    match &state.config {
        Some(c) => (
            StatusCode::OK,
            Json(serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
        ),
        None => (
            StatusCode::OK,
            Json(
                serde_json::json!({ "message": "Configuration not loaded (no config file or env)." }),
            ),
        ),
    }
}

/// Admin router: mount at /admin. GET /config is public (sanitized); revoke and security/events require admin auth.
pub fn admin_router(
    pool: Option<&sqlx::PgPool>,
    config: Option<&ferrum_core::FerrumConfig>,
) -> Router {
    let deployment_mode = config.map(|c| {
        if c.is_offline_first() {
            if c.services.enable_wes || c.services.enable_trs {
                "connected".to_string()
            } else {
                "offline".to_string()
            }
        } else {
            "full".to_string()
        }
    });
    let sanitized = config.map(|c| {
        let broker_public = std::env::var("FERRUM_PUBLIC_BROKER_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| c.auth.issuer.clone());
        let broker_login_url = broker_public.as_ref().map(|b| {
            let idp =
                std::env::var("FERRUM_BROKER_LOGIN_IDP").unwrap_or_else(|_| "keycloak".to_string());
            format!(
                "{}/login/{}",
                b.trim_end_matches('/'),
                idp.trim_matches('/')
            )
        });
        let auth_mode = match c.auth.mode {
            ferrum_core::config::AuthMode::Builtin => "builtin",
            ferrum_core::config::AuthMode::External => "external",
        };
        let access_requests_enabled = c.auth.is_external()
            || c.auth
                .ads_url
                .as_ref()
                .is_some_and(|u| !u.trim().is_empty())
            || c.discovery.enabled;
        let tes_backend =
            std::env::var("FERRUM_TES_BACKEND").unwrap_or_else(|_| "noop".to_string());
        let wes_trs_auto_register = !matches!(
            std::env::var("FERRUM_WES_TRS_AUTO_REGISTER")
                .unwrap_or_else(|_| "true".to_string())
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false" | "no" | "off"
        );
        SanitizedConfig {
            bind: c.bind.clone(),
            database: SanitizedDatabase {
                driver: c.database.driver.clone(),
                url_set: c.database.url.as_ref().map(|_| true),
                run_migrations: c.database.run_migrations,
                max_connections: c.database.max_connections,
                min_connections: c.database.min_connections,
                acquire_timeout_secs: c.database.acquire_timeout_secs,
                idle_timeout_secs: c.database.idle_timeout_secs,
                max_lifetime_secs: c.database.max_lifetime_secs,
            },
            storage: SanitizedStorage {
                backend: c.storage.backend.clone(),
                s3_endpoint: c.storage.s3_endpoint.clone(),
                s3_bucket: c.storage.s3_bucket.clone(),
            },
            services: SanitizedServices {
                enable_drs: c.services.enable_drs,
                enable_wes: c.services.enable_wes,
                enable_tes: c.services.enable_tes,
                enable_trs: c.services.enable_trs,
                enable_beacon: c.services.enable_beacon,
                enable_passports: c.services.enable_passports,
                enable_crypt4gh: c.services.enable_crypt4gh,
                crypt4gh_ingest_ready: crypt4gh_ingest_ready(c),
            },
            compute: SanitizedCompute {
                tes_backend,
                wes_trs_auto_register,
            },
            ingest: SanitizedIngest {
                max_upload_bytes: c.ingest.effective_max_upload_bytes(),
                max_chunk_bytes: ferrum_drs::ingest_chunk::INGEST_CHUNK_CEILING_BYTES,
            },
            discovery: Some(SanitizedDiscovery {
                enabled: c.discovery.enabled,
                auto_register: c.discovery.auto_register,
                service_registry_url: c.discovery.service_registry_url.clone(),
                registration_base_url: c.discovery.registration_base_url.clone(),
            }),
            auth: Some(SanitizedAuth {
                mode: auth_mode.to_string(),
                require_auth: c.auth.require_auth,
                broker_public_url: broker_public,
                broker_login_url,
                access_requests_enabled,
            }),
            deployment_mode,
        }
    });
    let state = Arc::new(AdminState {
        pool: pool.cloned(),
        config: sanitized,
    });
    Router::new()
        .route("/config", get(get_config))
        .route("/tokens/revoke", post(revoke_token))
        .route("/security/events", get(list_security_events))
        .with_state(state)
}
