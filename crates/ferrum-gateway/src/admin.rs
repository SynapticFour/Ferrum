//! A07/A09: Admin routes — token revocation, security events. Require admin auth.

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
        return (StatusCode::FORBIDDEN, Json(RevokeResponse { revoked: false }));
    }
    let jti = req.jti.trim();
    if jti.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(RevokeResponse { revoked: false }));
    }
    let r = sqlx::query("INSERT INTO revoked_tokens (jti, reason) VALUES ($1, $2) ON CONFLICT (jti) DO NOTHING")
        .bind(jti)
        .bind(None::<String>)
        .execute(&state.pool)
        .await;
    match r {
        Ok(rows) => (StatusCode::OK, Json(RevokeResponse { revoked: rows.rows_affected() > 0 })),
        Err(e) => {
            tracing::warn!(?e, "revoke_token db error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(RevokeResponse { revoked: false }))
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
        return (StatusCode::FORBIDDEN, Json(EventsResponse { events: vec![] }));
    }
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);
    let severity = q.severity.as_deref().filter(|s| !s.is_empty());
    type EventRow = (String, String, String, Option<String>, Option<String>, Option<String>, Option<serde_json::Value>, Option<chrono::DateTime<chrono::Utc>>);
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
                    .map(|(id, event_type, severity, sub, ip_address, resource_id, details, occurred_at)| SecurityEventRow {
                        id,
                        event_type,
                        severity,
                        sub,
                        ip_address,
                        resource_id,
                        details,
                        occurred_at: occurred_at.map(|t| t.to_rfc3339()),
                    })
                    .collect(),
            }),
        ),
        Err(e) => {
            tracing::warn!(?e, "list_security_events db error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(EventsResponse { events: vec![] }))
        }
    }
}

/// Admin router: requires admin auth. Mount at /admin.
pub fn admin_router(pool: sqlx::PgPool) -> Router {
    let state = Arc::new(AdminState { pool });
    Router::new()
        .route("/tokens/revoke", post(revoke_token))
        .route("/security/events", get(list_security_events))
        .with_state(state)
}
