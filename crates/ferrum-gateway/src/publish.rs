// SPDX-License-Identifier: BUSL-1.1
//! Publish workspace-private DRS objects to the ADS catalog (institute/public).

use axum::http::StatusCode;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Json, Router,
};
use ferrum_beacon::repo::BeaconRepo;
use ferrum_core::{
    is_workspace_editor_or_owner, AuthClaims, BackgroundWorkGate, FerrumConfig, FerrumPool,
};
use ferrum_drs::repo::DrsRepo;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

#[derive(Clone)]
pub struct PublishState {
    pub pool: sqlx::PgPool,
    pub config: Arc<FerrumConfig>,
    pub background_gate: Option<Arc<BackgroundWorkGate>>,
    pub federation: Option<Arc<ferrum_federation::FederationClient>>,
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
    /// When true (default), link pathogen annotations to Beacon on publish when organism metadata exists.
    #[serde(default = "default_true")]
    pub index_beacon: bool,
    /// When true (default), index VCF variants into Beacon when the object looks like VCF.
    #[serde(default = "default_true")]
    pub index_variants: bool,
    /// When true, probe configured Beacon federation peers after local Beacon indexing.
    #[serde(default)]
    pub index_beacon_federate: bool,
}

fn default_true() -> bool {
    true
}

fn default_visibility() -> String {
    "institute".to_string()
}

#[derive(Debug, Serialize)]
pub struct PublishDatasetResponse {
    pub ads_dataset_id: String,
    pub object_id: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beacon_indexed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants_indexed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcf_index_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beacon_peers_probed: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PublishIndexStatusResponse {
    pub object_id: String,
    pub vcf_index_status: Option<String>,
    pub variants_indexed: Option<usize>,
}

async fn probe_beacon_federation_on_publish(
    federation: &ferrum_federation::FederationClient,
    ads_dataset_id: &str,
) -> usize {
    if !federation.is_enabled() {
        return 0;
    }
    let envelope = serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": { "datasets": [ads_dataset_id] }
    });
    federation.query_peers(&envelope).await.len()
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

async fn index_beacon_for_publish(
    pool: &sqlx::PgPool,
    object_id: &str,
    ads_dataset_id: &str,
    display_name: &str,
    description: Option<&str>,
) -> bool {
    let ferrum_pool = FerrumPool::Postgres(pool.clone());
    let drs = DrsRepo::new(ferrum_pool.clone(), "localhost".into());
    let beacon = BeaconRepo::new(ferrum_pool);

    let Some(organism) = drs.pathogen_organism(object_id).await.ok().flatten() else {
        return false;
    };

    if beacon
        .ensure_dataset(ads_dataset_id, display_name, description, "unknown")
        .await
        .is_err()
    {
        warn!(object_id, ads_dataset_id, "beacon dataset upsert failed");
        return false;
    }

    match drs
        .link_pathogen_to_dataset(object_id, ads_dataset_id)
        .await
    {
        Ok(rows) if rows > 0 => {
            tracing::info!(
                object_id,
                ads_dataset_id,
                organism = %organism,
                "linked pathogen annotation to published Beacon dataset"
            );
            true
        }
        Ok(_) => false,
        Err(err) => {
            warn!(object_id, error = %err, "pathogen beacon link failed");
            false
        }
    }
}

fn local_drs_base_url(config: &FerrumConfig) -> Option<String> {
    std::env::var("FERRUM_PUBLIC_BASE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .or_else(|| config.discovery.registration_base_url.clone())
        .map(|base| format!("{}/ga4gh/drs/v1", base.trim_end_matches('/')))
}

pub async fn index_variants_for_publish(
    pool: &sqlx::PgPool,
    object_id: &str,
    ads_dataset_id: &str,
) -> usize {
    let ferrum_pool = FerrumPool::Postgres(pool.clone());
    let drs = DrsRepo::new(ferrum_pool.clone(), "localhost".into());
    let Some((name, mime, backend, storage_key)) =
        drs.get_object_publish_info(object_id).await.ok().flatten()
    else {
        return 0;
    };
    if backend != "local"
        || !ferrum_beacon::vcf_index::is_vcf_object(name.as_deref(), mime.as_deref())
    {
        return 0;
    }
    let path = ferrum_beacon::vcf_index::local_object_path(&storage_key);
    if !path.is_file() {
        warn!(
            object_id,
            storage_key, "VCF indexing skipped: local file not found"
        );
        return 0;
    }
    let beacon = BeaconRepo::new(ferrum_pool.clone());
    if beacon
        .ensure_dataset(
            ads_dataset_id,
            name.as_deref().unwrap_or(object_id),
            None,
            "GRCh38",
        )
        .await
        .is_err()
    {
        return 0;
    }
    match ferrum_beacon::vcf_index::index_vcf_path(&ferrum_pool, ads_dataset_id, &path).await {
        Ok(n) => {
            if n > 0 {
                tracing::info!(
                    object_id,
                    ads_dataset_id,
                    variants = n,
                    "indexed VCF variants into Beacon"
                );
            }
            n
        }
        Err(err) => {
            warn!(object_id, error = %err, "VCF beacon indexing failed");
            0
        }
    }
}

fn spawn_vcf_index_job(
    pool: sqlx::PgPool,
    background_gate: Option<Arc<BackgroundWorkGate>>,
    object_id: String,
    ads_dataset_id: String,
) {
    if background_gate
        .as_ref()
        .is_some_and(|g| !g.allows_background_work())
    {
        tracing::info!(object_id = %object_id, "deferring VCF indexing while in low-power mode");
        let pool_bg = pool.clone();
        let object_id_bg = object_id.clone();
        tokio::spawn(async move {
            let ferrum_pool = FerrumPool::Postgres(pool_bg);
            let drs_bg = DrsRepo::new(ferrum_pool, "localhost".into());
            let _ = drs_bg
                .set_vcf_index_status(&object_id_bg, "deferred_low_power")
                .await;
        });
        return;
    }

    tokio::spawn(async move {
        let ferrum_pool = FerrumPool::Postgres(pool.clone());
        let drs = DrsRepo::new(ferrum_pool, "localhost".into());
        let _ = drs.set_vcf_index_status(&object_id, "running").await;
        let count = index_variants_for_publish(&pool, &object_id, &ads_dataset_id).await;
        let status = if count > 0 { "completed" } else { "skipped" };
        let _ = drs.set_vcf_index_status(&object_id, status).await;
        if count > 0 {
            let _ = drs.set_variants_indexed_count(&object_id, count).await;
        }
    });
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

    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT workspace_id, dataset_id FROM drs_objects WHERE id = $1")
            .bind(&body.object_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (workspace_id, existing_dataset) =
        row.ok_or((StatusCode::NOT_FOUND, "DRS object not found".into()))?;

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

    let ads_url = resolve_ads_datasets_url(&state.config)
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "ADS not configured".into()))?;

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
        "remote_drs_base_url": local_drs_base_url(&state.config),
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

    let beacon_indexed = if body.index_beacon {
        Some(
            index_beacon_for_publish(
                &state.pool,
                &body.object_id,
                &ads_id,
                &body.name,
                body.description.as_deref(),
            )
            .await,
        )
    } else {
        None
    };

    let beacon_peers_probed = if body.index_beacon_federate {
        match state.federation.as_ref() {
            Some(f) => Some(probe_beacon_federation_on_publish(f, &ads_id).await),
            None => None,
        }
    } else {
        None
    };

    let (variants_indexed, vcf_index_status) = if body.index_variants {
        let ferrum_pool = FerrumPool::Postgres(state.pool.clone());
        let drs = DrsRepo::new(ferrum_pool, "localhost".into());
        let looks_like_vcf = drs
            .get_object_publish_info(&body.object_id)
            .await
            .ok()
            .flatten()
            .is_some_and(|(name, mime, backend, _)| {
                backend == "local"
                    && ferrum_beacon::vcf_index::is_vcf_object(name.as_deref(), mime.as_deref())
            });

        if looks_like_vcf {
            let _ = drs.set_vcf_index_status(&body.object_id, "pending").await;
            spawn_vcf_index_job(
                state.pool.clone(),
                state.background_gate.clone(),
                body.object_id.clone(),
                ads_id.clone(),
            );
            (None, Some("pending".to_string()))
        } else {
            (None, Some("skipped".to_string()))
        }
    } else {
        (None, None)
    };

    Ok(Json(PublishDatasetResponse {
        ads_dataset_id: ads_id,
        object_id: body.object_id,
        visibility: body.visibility,
        beacon_indexed,
        variants_indexed,
        vcf_index_status,
        beacon_peers_probed,
    }))
}

async fn get_publish_index_status(
    State(state): State<Arc<PublishState>>,
    Path(object_id): Path<String>,
) -> Result<Json<PublishIndexStatusResponse>, (StatusCode, String)> {
    let ferrum_pool = FerrumPool::Postgres(state.pool.clone());
    let drs = DrsRepo::new(ferrum_pool, "localhost".into());
    let status = drs
        .get_vcf_index_status(&object_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let variants_indexed = drs
        .get_variants_indexed_count(&object_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(PublishIndexStatusResponse {
        object_id,
        vcf_index_status: status,
        variants_indexed,
    }))
}

pub fn publish_router(
    pool: sqlx::PgPool,
    config: &FerrumConfig,
    background_gate: Option<Arc<BackgroundWorkGate>>,
    federation: Option<Arc<ferrum_federation::FederationClient>>,
) -> Router {
    let state = Arc::new(PublishState {
        pool,
        config: Arc::new(config.clone()),
        background_gate,
        federation,
    });
    Router::new()
        .route("/datasets/publish", post(publish_dataset))
        .route(
            "/datasets/publish/:object_id/index-status",
            get(get_publish_index_status),
        )
        .with_state(state)
}
