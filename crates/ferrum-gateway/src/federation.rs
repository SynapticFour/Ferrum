//! Admin federation helpers: GA4GH service-registry list + register-this-node.
//! Used by the Ferrum UI Settings → Federation panel (Phase 1).

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use ferrum_core::FerrumConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct FederationState {
    pub config: Option<Arc<FerrumConfig>>,
}

#[derive(Debug, Serialize)]
pub struct FederationStatusResponse {
    pub discovery_enabled: bool,
    pub auto_register: bool,
    pub service_registry_url: Option<String>,
    pub registration_base_url: Option<String>,
    pub public_base_url: Option<String>,
    pub services: FederationServices,
}

#[derive(Debug, Serialize)]
pub struct FederationServices {
    pub drs: bool,
    pub beacon: bool,
    pub htsget: bool,
    pub wes: bool,
    pub tes: bool,
    pub trs: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegistryConnectRequest {
    pub registry_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterNodeRequest {
    pub registry_url: String,
    pub api_key: String,
    pub public_base_url: String,
    #[serde(default)]
    pub node_id_prefix: Option<String>,
    #[serde(default)]
    pub organization_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisteredService {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(rename = "type")]
    pub service_type: ServiceType,
    pub version: String,
    #[serde(default)]
    pub environment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceType {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct RegistryListResponse {
    pub services: Vec<RegisteredService>,
}

#[derive(Debug, Serialize)]
pub struct RegisterNodeResponse {
    pub registered: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RemoteTrsToolsResponse {
    pub trs_base_url: String,
    pub tools: serde_json::Value,
}

fn public_base_url(cfg: &FerrumConfig) -> Option<String> {
    cfg.discovery
        .registration_base_url
        .clone()
        .or_else(|| std::env::var("FERRUM_PUBLIC_BASE_URL").ok())
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

async fn registry_get(
    registry_url: &str,
    api_key: Option<&str>,
    path: &str,
) -> Result<reqwest::Response, (StatusCode, String)> {
    let url = format!(
        "{}/{}",
        registry_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut req = client.get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        req = req.header("X-API-Key", key);
    }
    req.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("registry request failed: {e}"),
        )
    })
}

async fn registry_post(
    registry_url: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, (StatusCode, String)> {
    let url = format!("{}/services", registry_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    client
        .post(url)
        .header("X-API-Key", api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("registry register failed: {e}"),
            )
        })
}

async fn get_status(State(state): State<Arc<FederationState>>) -> impl IntoResponse {
    let Some(cfg) = state.config.as_ref() else {
        return (
            StatusCode::OK,
            Json(FederationStatusResponse {
                discovery_enabled: false,
                auto_register: false,
                service_registry_url: None,
                registration_base_url: None,
                public_base_url: std::env::var("FERRUM_PUBLIC_BASE_URL").ok(),
                services: FederationServices {
                    drs: true,
                    beacon: true,
                    htsget: true,
                    wes: false,
                    tes: false,
                    trs: false,
                },
            }),
        );
    };
    (
        StatusCode::OK,
        Json(FederationStatusResponse {
            discovery_enabled: cfg.discovery.enabled,
            auto_register: cfg.discovery.auto_register,
            service_registry_url: cfg.discovery.service_registry_url.clone(),
            registration_base_url: cfg.discovery.registration_base_url.clone(),
            public_base_url: public_base_url(cfg),
            services: FederationServices {
                drs: cfg.services.enable_drs,
                beacon: cfg.services.enable_beacon,
                htsget: cfg.services.enable_htsget,
                wes: cfg.services.enable_wes,
                tes: cfg.services.enable_tes,
                trs: cfg.services.enable_trs,
            },
        }),
    )
}

async fn list_registry_services(
    Json(body): Json<RegistryConnectRequest>,
) -> Result<Json<RegistryListResponse>, (StatusCode, String)> {
    let registry_url = body.registry_url.trim();
    if registry_url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "registry_url required".into()));
    }
    let resp = registry_get(registry_url, body.api_key.as_deref(), "services").await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("registry returned HTTP {status}: {text}"),
        ));
    }
    let services: Vec<RegisteredService> = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("invalid registry JSON: {e}"),
        )
    })?;
    Ok(Json(RegistryListResponse { services }))
}

fn service_payload(
    id: &str,
    name: &str,
    artifact: &str,
    spec_version: &str,
    url: &str,
    org_name: &str,
    org_url: &str,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "type": {
            "group": "org.ga4gh",
            "artifact": artifact,
            "version": spec_version
        },
        "organization": {
            "name": org_name,
            "url": org_url
        },
        "version": "0.1.0",
        "url": url,
        "environment": "development"
    })
}

async fn register_this_node(
    State(state): State<Arc<FederationState>>,
    Json(body): Json<RegisterNodeRequest>,
) -> Result<Json<RegisterNodeResponse>, (StatusCode, String)> {
    let registry_url = body.registry_url.trim();
    let public_base = body.public_base_url.trim().trim_end_matches('/');
    if registry_url.is_empty() || public_base.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "registry_url and public_base_url required".into(),
        ));
    }
    if body.api_key.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "api_key required".into()));
    }

    let cfg = state.config.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "gateway config not loaded".into(),
    ))?;

    let prefix = body
        .node_id_prefix
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("org.ferrum.laptop");
    let org_name = body
        .organization_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("Ferrum Node");
    let org_url = public_base;

    let mut registrations: Vec<(&str, &str, &str, &str, String)> = Vec::new();
    if cfg.services.enable_drs {
        registrations.push((
            "drs",
            "Ferrum DRS",
            "drsservice",
            "1.3.0",
            format!("{public_base}/ga4gh/drs/v1"),
        ));
    }
    if cfg.services.enable_beacon {
        registrations.push((
            "beacon",
            "Ferrum Beacon",
            "beacon",
            "2.0",
            format!("{public_base}/ga4gh/beacon/v2"),
        ));
    }
    if cfg.services.enable_htsget {
        registrations.push((
            "htsget",
            "Ferrum htsget",
            "htsget",
            "1.3",
            format!("{public_base}/ga4gh/htsget/v1"),
        ));
    }
    if cfg.services.enable_wes {
        registrations.push((
            "wes",
            "Ferrum WES",
            "wes",
            "1.1.0",
            format!("{public_base}/ga4gh/wes/v1"),
        ));
    }
    if cfg.services.enable_tes {
        registrations.push((
            "tes",
            "Ferrum TES",
            "tes",
            "1.1.0",
            format!("{public_base}/ga4gh/tes/v1"),
        ));
    }
    if cfg.services.enable_trs {
        registrations.push((
            "trs",
            "Ferrum TRS",
            "tool-registry",
            "2.0.2",
            format!("{public_base}/ga4gh/trs/v2"),
        ));
    }

    let mut registered_ids = Vec::new();
    for (suffix, name, artifact, spec, url) in registrations {
        let id = format!("{prefix}.{suffix}");
        let payload = service_payload(&id, name, artifact, spec, &url, org_name, org_url);
        let resp = registry_post(registry_url, body.api_key.trim(), &payload).await?;
        if resp.status().is_success() || resp.status().as_u16() == 409 {
            registered_ids.push(id);
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("register {suffix} failed HTTP {status}: {text}"),
            ));
        }
    }

    Ok(Json(RegisterNodeResponse {
        registered: registered_ids,
    }))
}

async fn remote_trs_tools(
    axum::extract::Query(q): axum::extract::Query<RemoteTrsQuery>,
) -> Result<Json<RemoteTrsToolsResponse>, (StatusCode, String)> {
    let trs_base = q.trs_base_url.trim().trim_end_matches('/');
    if trs_base.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "trs_base_url required".into()));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let resp = client
        .get(format!("{trs_base}/tools"))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("TRS returned HTTP {status}: {text}"),
        ));
    }
    let tools: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(RemoteTrsToolsResponse {
        trs_base_url: trs_base.to_string(),
        tools,
    }))
}

#[derive(Debug, Deserialize)]
pub struct RemoteTrsQuery {
    pub trs_base_url: String,
}

pub fn federation_router(config: Option<&FerrumConfig>) -> Router {
    let state = Arc::new(FederationState {
        config: config.map(|c| Arc::new(c.clone())),
    });
    Router::new()
        .route("/status", get(get_status))
        .route("/registry/services", post(list_registry_services))
        .route("/registry/register-node", post(register_this_node))
        .route("/proxy/trs/tools", get(remote_trs_tools))
        .with_state(state)
}
