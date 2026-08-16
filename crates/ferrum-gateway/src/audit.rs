// SPDX-License-Identifier: BUSL-1.1
//! Data residency audit HTTP API (`/api/v1/audit/residency`).

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use ferrum_core::{residency_delete_blocked, AuthClaims, ResidencyAuditLog};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ResidencyQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

pub fn audit_router(audit: Arc<ResidencyAuditLog>) -> Router {
    Router::new()
        .route(
            "/residency",
            get(get_residency)
                .delete(method_not_allowed)
                .put(method_not_allowed)
                .post(method_not_allowed),
        )
        .route("/residency/verify", get(verify_residency))
        .with_state(audit)
}

async fn get_residency(
    State(audit): State<Arc<ResidencyAuditLog>>,
    auth: Option<Extension<AuthClaims>>,
    Query(q): Query<ResidencyQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    let from = parse_ts(q.from.as_deref())?;
    let to = parse_ts(q.to.as_deref())?;
    let is_admin = auth.as_ref().is_some_and(|c| c.0.is_admin());
    let requester = auth.as_ref().and_then(|c| c.0.sub());
    let result = audit
        .query_range_for_requester(from, to, requester, is_admin)
        .await
        .map_err(internal)?;
    Ok(Json(serde_json::json!({
        "entries": result.entries,
        "chain_valid": result.chain_valid,
    })))
}

async fn verify_residency(
    State(audit): State<Arc<ResidencyAuditLog>>,
) -> Result<Json<serde_json::Value>, Response> {
    let result = audit.verify().await.map_err(internal)?;
    Ok(Json(serde_json::json!({
        "chain_valid": result.chain_valid,
        "entry_count": result.entry_count,
        "first_timestamp": result.first_timestamp,
        "last_timestamp": result.last_timestamp,
        "last_hash": result.last_hash,
    })))
}

async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({
            "code": "method_not_allowed",
            "message": residency_delete_blocked().to_string(),
        })),
    )
        .into_response()
}

#[allow(clippy::result_large_err)]
fn parse_ts(s: Option<&str>) -> Result<Option<DateTime<Utc>>, Response> {
    match s {
        None => Ok(None),
        Some(v) => DateTime::parse_from_rfc3339(v)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(|e| bad_request(e.to_string())),
    }
}

fn bad_request(msg: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "code": "validation_error", "message": msg })),
    )
        .into_response()
}

fn internal(e: ferrum_core::FerrumError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "code": "internal_error", "message": e.to_string() })),
    )
        .into_response()
}
