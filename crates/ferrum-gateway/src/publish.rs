//! Publish workspace-private DRS objects to the ADS catalog (institute/public).

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Extension, Json, Router,
};
use ferrum_core::{is_workspace_editor_or_owner, AuthClaims, FerrumConfig, FerrumPool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct PublishState {
    pub pool: sqlx::PgPool,
    pub config: Arc<FerrumConfig>,
}

#[derive(Debug, Deserialize)]
pub struct PublishDatasetRequest {
    pub object_id: String,
    pub name: String,
    pub description: Option<String>,
    pub duo_codes: Vec<String>,
    /// `draft`, `institute`, or `public` (default `institute`).
    #[serde(default = "default_visibility")]
    pub visibility: String,
    pub dac_group: Option<String>,
    #[serde(default)]
    pub auto_approve_enabled: bool,
}

fn default_visibility() -> String {
    "institute".to_string()
}

#[derive(Debug, Serialize)]
pub struct PublishDatasetResponse {
    pub ads_dataset_id: String,
    pub object_id: String,
    pub visibility: String,
}

fn resolve_ads_datasets_url(config: &FerrumConfig) -> Option<String> {
    let ads_base = config
        .auth
        .ads_url
        .clone()
        .filter(|u| !u.trim().is_empty())
        .or_else(|| {
            config
                .auth
                .issuer
                .as_ref()
                .map(|issuer| format!("{}/ads/v1", issuer.trim_end_matches('/')))
        })?;
    let base = ads_base.trim_end_matches('/');
    Some(if base.ends_with("/ads/v1") {
        format!("{base}/datasets")
    } else if base.ends_with("/ads") {
        format!("{base}/v1/datasets")
    } else {
        format!("{base}/ads/v1/datasets")
    })
}

async fn publish_dataset(
    State(state): State<Arc<PublishState>>,
    Extension(auth): Extension<AuthClaims>,
    Json(body): Json<PublishDatasetRequest>,
) -> Result<Json<PublishDatasetResponse>, (StatusCode, String)> {
    if body.duo_codes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "duo_codes required".into()));
    }
    let sub = auth
        .sub()
        .ok_or((StatusCode::FORBIDDEN, "missing subject".into()))?;

    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT workspace_id, dataset_id FROM drs_objects WHERE id = $1",
    )
    .bind(&body.object_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (workspace_id, existing_dataset) = row.ok_or((
        StatusCode::NOT_FOUND,
        "DRS object not found".into(),
    ))?;

    if existing_dataset.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "object already published to ADS".into(),
        ));
    }

    let ws_id = workspace_id.ok_or((
        StatusCode::BAD_REQUEST,
        "object must belong to a workspace before publishing".into(),
    ))?;

    let pool = FerrumPool::Postgres(state.pool.clone());
    if !auth.is_admin()
        && !is_workspace_editor_or_owner(&pool, &ws_id, sub)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        return Err((
            StatusCode::FORBIDDEN,
            "must be workspace editor or owner to publish".into(),
        ));
    }

    let ads_url = resolve_ads_datasets_url(&state.config).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "ADS not configured".into(),
    ))?;

    let api_key = std::env::var(&state.config.auth.ads_api_key_env).map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "ADS DAC API key not configured".into(),
        )
    })?;

    let payload = serde_json::json!({
        "name": body.name,
        "description": body.description,
        "duo_codes": body.duo_codes,
        "external_id": format!("drs:{}", body.object_id),
        "auto_approve_enabled": body.auto_approve_enabled,
        "dac_group": body.dac_group,
        "visibility": body.visibility,
        "resource_type": "dataset",
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&ads_url)
        .header("X-API-Key", api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("ADS register failed HTTP {status}: {text}"),
        ));
    }

    let created: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let ads_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::BAD_GATEWAY,
            "ADS response missing dataset id".into(),
        ))?
        .to_string();

    sqlx::query("UPDATE drs_objects SET dataset_id = $1, updated_time = NOW() WHERE id = $2")
        .bind(&ads_id)
        .bind(&body.object_id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PublishDatasetResponse {
        ads_dataset_id: ads_id,
        object_id: body.object_id,
        visibility: body.visibility,
    }))
}

pub fn publish_router(pool: sqlx::PgPool, config: &FerrumConfig) -> Router {
    let state = Arc::new(PublishState {
        pool,
        config: Arc::new(config.clone()),
    });
    Router::new()
        .route("/datasets/publish", post(publish_dataset))
        .with_state(state)
}
