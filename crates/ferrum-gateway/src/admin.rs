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
    pub pool: sqlx::PgPool,
    /// Sanitized config for GET /admin/config (no secrets).
    pub config: Option<SanitizedConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedConfig {
    pub bind: String,
    pub database: SanitizedDatabase,
    pub storage: SanitizedStorage,
    pub services: SanitizedServices,
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
    .execute(&state.pool)
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
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT id, event_type, severity, sub, ip_address, resource_id, details, occurred_at FROM security_events ORDER BY occurred_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
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
pub fn admin_router(pool: sqlx::PgPool, config: Option<&ferrum_core::FerrumConfig>) -> Router {
    let sanitized = config.map(|c| SanitizedConfig {
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
        },
    });
    let state = Arc::new(AdminState {
        pool,
        config: sanitized,
    });
    Router::new()
        .route("/config", get(get_config))
        .route("/tokens/revoke", post(revoke_token))
        .route("/security/events", get(list_security_events))
        .with_state(state)
}
