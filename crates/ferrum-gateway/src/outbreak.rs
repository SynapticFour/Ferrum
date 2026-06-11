//! Outbreak Mode HTTP API (`/api/v1/outbreak/*`).

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use ferrum_core::{
    ActivateRequest, ApproveDownloadRequest, AuthClaims, DeactivateRequest, OutbreakService,
    ResidencyAuditLog,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
struct OutbreakRouterState {
    service: Arc<OutbreakService>,
    residency_audit: Option<Arc<ResidencyAuditLog>>,
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
}

pub fn outbreak_router(
    service: Arc<OutbreakService>,
    residency_audit: Option<Arc<ResidencyAuditLog>>,
) -> Router {
    Router::new()
        .route("/activate", post(post_activate))
        .route("/deactivate", post(post_deactivate))
        .route(
            "/approve-download/:drs_id",
            post(post_approve_download),
        )
        .with_state(OutbreakRouterState {
            service,
            residency_audit,
        })
}

async fn post_activate(
    State(state): State<OutbreakRouterState>,
    auth: Option<Extension<AuthClaims>>,
    Json(body): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_activator(&auth)?;
    if !state.service.is_enabled() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "outbreak_disabled",
            "outbreak mode is disabled in configuration".to_string(),
        ));
    }
    let record = state.service.activate(&body).await.map_err(|e| {
        api_error(StatusCode::BAD_REQUEST, "activation_failed", e.to_string())
    })?;
    if let Some(ref audit) = state.residency_audit {
        let requester = auth.as_ref().and_then(|a| a.0.sub());
        let _ = audit
            .append(
                "outbreak_activated",
                None,
                requester,
                None,
                false,
                None,
            )
            .await;
    }
    Ok(Json(serde_json::json!({
        "id": record.id,
        "policy": record.policy_name,
        "trigger_pathogen": record.trigger_pathogen,
        "active": record.active,
    })))
}

async fn post_deactivate(
    State(state): State<OutbreakRouterState>,
    auth: Option<Extension<AuthClaims>>,
    Json(body): Json<DeactivateRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let actor = activator_sub(auth.as_ref())?;
    state
        .service
        .deactivate(&body, &actor)
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, "deactivation_failed", e.to_string()))?;
    if let Some(ref audit) = state.residency_audit {
        let requester = auth.as_ref().and_then(|a| a.0.sub());
        let _ = audit
            .append(
                "outbreak_deactivated",
                None,
                requester,
                None,
                false,
                None,
            )
            .await;
    }
    Ok(Json(serde_json::json!({
        "policy": body.policy,
        "active": false,
        "reason": body.reason,
    })))
}

async fn post_approve_download(
    State(state): State<OutbreakRouterState>,
    auth: Option<Extension<AuthClaims>>,
    Path(drs_id): Path<String>,
    Json(body): Json<ApproveDownloadRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_activator(&auth)?;
    let active = state.service.active_policies().await.map_err(|e| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            e.to_string(),
        )
    })?;
    let policy = active.into_iter().next().ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "no_active_policy",
            "no active outbreak policy".to_string(),
        )
    })?;
    state
        .service
        .approve_download(&policy, &drs_id, &body)
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, "approval_failed", e.to_string()))?;
    Ok(Json(serde_json::json!({
        "drs_id": drs_id,
        "policy": policy,
        "recipient": body.recipient,
        "approved": true,
    })))
}

fn require_activator(auth: &Option<Extension<AuthClaims>>) -> Result<(), Response> {
    let claims = auth.as_ref().ok_or_else(|| {
        api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "authentication required".to_string(),
        )
    })?;
    if !claims.0.is_outbreak_activator() && !claims.0.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "outbreak_activator role required".to_string(),
        ));
    }
    Ok(())
}

fn activator_sub(auth: Option<&Extension<AuthClaims>>) -> Result<String, Response> {
    let claims = auth.ok_or_else(|| {
        api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "authentication required".to_string(),
        )
    })?;
    if !claims.0.is_outbreak_activator() && !claims.0.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "outbreak_activator role required".to_string(),
        ));
    }
    Ok(claims
        .0
        .sub()
        .unwrap_or("unknown")
        .to_string())
}

fn api_error(status: StatusCode, code: &'static str, message: String) -> Response {
    (
        status,
        Json(ApiError {
            code,
            message,
        }),
    )
        .into_response()
}
