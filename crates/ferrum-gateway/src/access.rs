//! ADS proxy for researcher dataset access requests (ga4gh-infra integration).
//! Resolves the ADS base URL via service discovery at request time.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Router,
};
use ferrum_core::FerrumConfig;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AccessProxyState {
    pub config: Arc<FerrumConfig>,
    ads_base: Arc<RwLock<Option<String>>>,
}

impl AccessProxyState {
    pub fn new(config: FerrumConfig) -> Self {
        Self {
            config: Arc::new(config),
            ads_base: Arc::new(RwLock::new(None)),
        }
    }

    async fn ads_base_url(&self) -> Result<String, (StatusCode, String)> {
        {
            let guard = self.ads_base.read().await;
            if let Some(url) = guard.as_ref() {
                return Ok(url.clone());
            }
        }
        let gateway_base = self
            .config
            .discovery
            .registration_base_url
            .clone()
            .or_else(|| std::env::var("FERRUM_PUBLIC_BASE_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
        let resolved = ferrum_discovery::resolve_ads_url(
            &self.config.auth,
            &self.config.discovery,
            &gateway_base,
        )
        .await;
        let Some(url) = resolved else {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Access Decision Service (ADS) is not configured. Deploy with ga4gh-infra or set FERRUM_AUTH__ADS_URL.".into(),
            ));
        };
        *self.ads_base.write().await = Some(url.clone());
        Ok(url)
    }
}

async fn forward(
    state: Arc<AccessProxyState>,
    method: &str,
    ads_path: &str,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let ads = if let Some(header_url) = req
        .headers()
        .get("X-ADS-Base-URL")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let normalized = normalize_ads_from_service_url(header_url);
        let policy = ferrum_core::SsrfPolicy {
            allow_private_networks: false,
            allowed_schemes: vec!["https".into(), "http".into()],
            ..Default::default()
        };
        ferrum_core::validate_url_ssrf(&normalized, &policy).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "X-ADS-Base-URL failed SSRF checks".into(),
            )
        })?;
        normalized
    } else {
        state.ads_base_url().await?
    };
    let mut url = format!("{ads}/{}", ads_path.trim_start_matches('/'));
    if let Some(query) = req.uri().query() {
        url.push('?');
        url.push_str(query);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let body_bytes = if method == "GET" {
        None
    } else {
        Some(
            axum::body::to_bytes(req.into_body(), 2 * 1024 * 1024)
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
        )
    };

    let mut builder = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        _ => return Err((StatusCode::METHOD_NOT_ALLOWED, "unsupported method".into())),
    };
    if let Some(token) = auth {
        builder = builder.header(header::AUTHORIZATION, token);
    }
    if let Some(body) = body_bytes {
        builder = builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("ADS request failed: {e}")))?;

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let headers = resp.headers().clone();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, ct.clone());
    }
    Ok(response)
}

async fn get_status(State(state): State<Arc<AccessProxyState>>) -> impl IntoResponse {
    match state.ads_base_url().await {
        Ok(url) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "ads_available": true,
                "ads_base_url": url,
            })),
        ),
        Err((status, msg)) => (
            status,
            axum::Json(serde_json::json!({
                "ads_available": false,
                "message": msg,
            })),
        ),
    }
}

async fn get_catalog_datasets(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", "catalog/datasets", req).await
}

#[cfg(feature = "discovery")]
async fn load_registry_services(config: &FerrumConfig) -> Vec<ferrum_discovery::RegisteredService> {
    if !config.discovery.enabled {
        return Vec::new();
    }
    let Ok(registry) = ferrum_discovery::ServiceRegistryClient::from_config(&config.discovery)
    else {
        return Vec::new();
    };
    registry.list().await.unwrap_or_default()
}

#[cfg(feature = "discovery")]
async fn collect_ads_bases(
    state: &AccessProxyState,
    registry_services: &[ferrum_discovery::RegisteredService],
) -> Result<Vec<(String, String)>, (StatusCode, String)> {
    let mut ads_bases: Vec<(String, String)> = Vec::new();
    if let Ok(local) = state.ads_base_url().await {
        ads_bases.push(("local".to_string(), local));
    }
    if state.config.discovery.enabled {
        for svc in registry_services {
            if svc
                .info
                .r#type
                .artifact
                .eq_ignore_ascii_case(ferrum_discovery::ARTIFACT_ADS)
            {
                let base = normalize_ads_from_service_url(&svc.url);
                let origin = svc.info.id.clone();
                if !ads_bases.iter().any(|(_, u)| u == &base) {
                    ads_bases.push((origin, base));
                }
            }
        }
    }
    Ok(ads_bases)
}

#[cfg(feature = "discovery")]
fn enrich_catalog_entry(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    origin: &str,
    base: &str,
    registry_services: &[ferrum_discovery::RegisteredService],
    prefs: &ferrum_discovery::ServiceSelectionPrefs,
) {
    obj.insert(
        "federation_origin".to_string(),
        serde_json::Value::String(origin.to_string()),
    );
    obj.insert(
        "ads_base_url".to_string(),
        serde_json::Value::String(base.to_string()),
    );
    let remote_drs = obj
        .get("remote_drs_base_url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| ferrum_discovery::drs_url_for_ads_origin(registry_services, origin, prefs));
    if let Some(drs) = remote_drs {
        obj.insert(
            "remote_drs_base_url".to_string(),
            serde_json::Value::String(drs),
        );
    }
    let resource_type = obj
        .get("resource_type")
        .and_then(|v| v.as_str())
        .unwrap_or("dataset");
    if resource_type == "compute_pool" {
        if let Some(wes) =
            ferrum_discovery::wes_url_for_ads_origin(registry_services, origin, prefs)
        {
            obj.insert(
                "remote_wes_base_url".to_string(),
                serde_json::Value::String(wes),
            );
        }
    }
}

#[cfg(feature = "discovery")]
fn catalog_dedup_key(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    if let Some(ext) = obj.get("external_id").and_then(|v| v.as_str()) {
        let normalized = ext.strip_prefix("drs:").unwrap_or(ext);
        return Some(format!("ext:{normalized}"));
    }
    let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() {
        return None;
    }
    let dac = obj.get("dac_group").and_then(|v| v.as_str()).unwrap_or("");
    Some(format!("name:{name}:{dac}"))
}

#[cfg(feature = "discovery")]
fn origin_preference_score(
    origin: &str,
    registry_services: &[ferrum_discovery::RegisteredService],
    prefs: &ferrum_discovery::ServiceSelectionPrefs,
) -> i32 {
    if origin == "local" {
        return 10_000;
    }
    registry_services
        .iter()
        .find(|s| s.info.id == origin)
        .map(|s| ferrum_discovery::score_service_match(s, prefs))
        .unwrap_or(0)
}

#[cfg(feature = "discovery")]
fn dedup_federated_catalog(
    merged: Vec<serde_json::Value>,
    registry_services: &[ferrum_discovery::RegisteredService],
    prefs: &ferrum_discovery::ServiceSelectionPrefs,
) -> (Vec<serde_json::Value>, usize) {
    let mut best: std::collections::HashMap<String, (i32, serde_json::Value)> =
        std::collections::HashMap::new();
    let mut passthrough = Vec::new();
    let mut dropped = 0usize;

    for row in merged {
        let Some(obj) = row.as_object() else {
            passthrough.push(row);
            continue;
        };
        let Some(key) = catalog_dedup_key(obj) else {
            passthrough.push(row);
            continue;
        };
        let origin = obj
            .get("federation_origin")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let score = origin_preference_score(origin, registry_services, prefs);
        match best.get(&key) {
            Some((best_score, _)) if *best_score >= score => {
                dropped += 1;
            }
            _ => {
                best.insert(key, (score, row));
            }
        }
    }

    let mut out: Vec<serde_json::Value> = best.into_values().map(|(_, v)| v).collect();
    out.extend(passthrough);
    (out, dropped)
}

#[cfg(feature = "discovery")]
fn grant_dedup_key(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    if let Some(ext) = obj.get("external_id").and_then(|v| v.as_str()) {
        let normalized = ext.strip_prefix("drs:").unwrap_or(ext);
        if !normalized.is_empty() {
            return Some(format!("ext:{normalized}"));
        }
    }
    let dataset_id = obj.get("dataset_id").and_then(|v| v.as_str()).unwrap_or("");
    if dataset_id.is_empty() {
        return None;
    }
    Some(format!("ds:{dataset_id}"))
}

#[cfg(feature = "discovery")]
fn dedup_federated_grants(
    merged: Vec<serde_json::Value>,
    registry_services: &[ferrum_discovery::RegisteredService],
    prefs: &ferrum_discovery::ServiceSelectionPrefs,
) -> (Vec<serde_json::Value>, usize) {
    let mut best: std::collections::HashMap<String, (i32, serde_json::Value)> =
        std::collections::HashMap::new();
    let mut passthrough = Vec::new();
    let mut dropped = 0usize;

    for row in merged {
        let Some(obj) = row.as_object() else {
            passthrough.push(row);
            continue;
        };
        let Some(key) = grant_dedup_key(obj) else {
            passthrough.push(row);
            continue;
        };
        let origin = obj
            .get("federation_origin")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let score = origin_preference_score(origin, registry_services, prefs);
        match best.get(&key) {
            Some((best_score, _)) if *best_score >= score => {
                dropped += 1;
            }
            _ => {
                best.insert(key, (score, row));
            }
        }
    }

    let mut out: Vec<serde_json::Value> = best.into_values().map(|(_, v)| v).collect();
    out.extend(passthrough);
    (out, dropped)
}

fn build_ads_introspect(config: &FerrumConfig) -> Option<ferrum_core::AdsIntrospectClient> {
    let ads_url = config
        .auth
        .ads_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            config
                .auth
                .issuer
                .as_ref()
                .map(|issuer| format!("{}/ads/v1", issuer.trim_end_matches('/')))
        })?;
    ferrum_core::AdsIntrospectClient::from_env(&ads_url, &config.auth.ads_api_key_env).ok()
}

async fn ads_base_for_origin(
    state: &AccessProxyState,
    registry_services: &[ferrum_discovery::RegisteredService],
    origin: &str,
    explicit: Option<&str>,
) -> Result<String, (StatusCode, String)> {
    if let Some(url) = explicit.filter(|u| !u.trim().is_empty()) {
        return Ok(normalize_ads_from_service_url(url));
    }
    if origin == "local" {
        return state.ads_base_url().await;
    }
    let bases = collect_ads_bases(state, registry_services).await?;
    bases
        .into_iter()
        .find(|(o, _)| o == origin)
        .map(|(_, b)| b)
        .ok_or((
            StatusCode::BAD_REQUEST,
            format!("unknown federation origin: {origin}"),
        ))
}

async fn enforce_federated_introspect(
    config: &FerrumConfig,
    ads_base: &str,
    auth_header: Option<&str>,
    resource: &str,
    dataset_id: &str,
) -> Result<(), (StatusCode, String)> {
    let Some(client) = build_ads_introspect(config) else {
        return Ok(());
    };
    let Some(token) = auth_header.and_then(|h| h.strip_prefix("Bearer ").map(str::trim)) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Authorization required for federated resource access".into(),
        ));
    };
    let active = client
        .introspect_at_base(ads_base, token, resource, dataset_id)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, format!("ADS introspect failed: {e}")))?;
    if !active {
        return Err((
            StatusCode::FORBIDDEN,
            "ADS grant not active for federated resource".into(),
        ));
    }
    Ok(())
}

fn object_id_from_drs_path(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let mut parts = trimmed.split('/');
    if parts.next()? != "objects" {
        return None;
    }
    parts.next().map(str::to_string)
}

#[derive(Debug, Deserialize)]
pub struct FederatedDrsQuery {
    pub base_url: Option<String>,
    pub origin: Option<String>,
    pub ads_base_url: Option<String>,
    pub dataset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FederatedWesQuery {
    pub base_url: Option<String>,
    pub origin: Option<String>,
    pub ads_base_url: Option<String>,
    pub dataset_id: Option<String>,
}

async fn federated_drs_proxy(
    State(state): State<Arc<AccessProxyState>>,
    Query(q): Query<FederatedDrsQuery>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let registry_services = load_registry_services(&state.config).await;
    let prefs =
        ferrum_discovery::ServiceSelectionPrefs::from_discovery_config(&state.config.discovery);

    let base = if let Some(url) = q.base_url.as_ref().filter(|u| !u.trim().is_empty()) {
        url.trim_end_matches('/').to_string()
    } else if let Some(origin) = q.origin.as_deref() {
        ferrum_discovery::drs_url_for_ads_origin(&registry_services, origin, &prefs).ok_or((
            StatusCode::BAD_REQUEST,
            "could not resolve DRS URL for federation origin".into(),
        ))?
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "base_url or origin query parameter required".into(),
        ));
    };

    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err((StatusCode::BAD_REQUEST, "base_url must be http(s)".into()));
    }

    let ads_base = if let Some(origin) = q.origin.as_deref() {
        Some(
            ads_base_for_origin(
                &state,
                &registry_services,
                origin,
                q.ads_base_url.as_deref(),
            )
            .await?,
        )
    } else {
        q.ads_base_url
            .as_deref()
            .map(normalize_ads_from_service_url)
    };

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    if let (Some(ads_base), Some(dataset_id)) = (ads_base.as_deref(), q.dataset_id.as_deref()) {
        let resource = object_id_from_drs_path(&path)
            .map(|id| format!("drs:{id}"))
            .unwrap_or_else(|| format!("drs:{}", path.trim_start_matches('/')));
        enforce_federated_introspect(&state.config, ads_base, auth, &resource, dataset_id).await?;
    }

    let url = format!("{base}/{}", path.trim_start_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut builder = client.get(&url);
    if let Some(token) = auth {
        builder = builder.header(header::AUTHORIZATION, token);
    }
    if let Some(ads_base) = &ads_base {
        builder = builder.header("X-ADS-Base-URL", ads_base.as_str());
    }
    let resp = builder.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("remote DRS request failed: {e}"),
        )
    })?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let resp_headers = resp.headers().clone();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(ct) = resp_headers.get(header::CONTENT_TYPE) {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, ct.clone());
    }
    Ok(response)
}

async fn federated_wes_proxy(
    State(state): State<Arc<AccessProxyState>>,
    Query(q): Query<FederatedWesQuery>,
    Path(path): Path<String>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let registry_services = load_registry_services(&state.config).await;
    let prefs =
        ferrum_discovery::ServiceSelectionPrefs::from_discovery_config(&state.config.discovery);

    let base = if let Some(url) = q.base_url.as_ref().filter(|u| !u.trim().is_empty()) {
        url.trim_end_matches('/').to_string()
    } else if let Some(origin) = q.origin.as_deref() {
        ferrum_discovery::wes_url_for_ads_origin(&registry_services, origin, &prefs).ok_or((
            StatusCode::BAD_REQUEST,
            "could not resolve WES URL for federation origin".into(),
        ))?
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "base_url or origin query parameter required".into(),
        ));
    };

    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err((StatusCode::BAD_REQUEST, "base_url must be http(s)".into()));
    }

    let ads_base = if let Some(origin) = q.origin.as_deref() {
        Some(
            ads_base_for_origin(
                &state,
                &registry_services,
                origin,
                q.ads_base_url.as_deref(),
            )
            .await?,
        )
    } else {
        q.ads_base_url
            .as_deref()
            .map(normalize_ads_from_service_url)
    };

    let method = req.method().clone();
    let headers = req.headers().clone();
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let body_bytes = if method == Method::GET || method == Method::HEAD {
        None
    } else {
        Some(
            axum::body::to_bytes(req.into_body(), 8 * 1024 * 1024)
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
        )
    };

    if method == Method::POST && path.trim_matches('/') == "runs" {
        if let (Some(ads_base), Some(bytes)) = (ads_base.as_deref(), body_bytes.as_ref()) {
            let dataset_id = q.dataset_id.clone().or_else(|| {
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .ok()
                    .and_then(|v| {
                        v.get("tags")
                            .and_then(|t| t.get("ads_compute_pool_id"))
                            .and_then(|id| id.as_str())
                            .map(str::to_string)
                    })
            });
            if let Some(dataset_id) = dataset_id {
                enforce_federated_introspect(
                    &state.config,
                    ads_base,
                    auth,
                    &format!("wes:run:{dataset_id}"),
                    &dataset_id,
                )
                .await?;
            }
        }
    }

    let url = format!("{base}/{}", path.trim_start_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut builder = match method {
        Method::GET => client.get(&url),
        Method::POST => client.post(&url),
        Method::PUT => client.put(&url),
        Method::DELETE => client.delete(&url),
        _ => return Err((StatusCode::METHOD_NOT_ALLOWED, "unsupported method".into())),
    };
    if let Some(token) = auth {
        builder = builder.header(header::AUTHORIZATION, token);
    }
    if let Some(ads_base) = &ads_base {
        builder = builder.header("X-ADS-Base-URL", ads_base.as_str());
    }
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, ct.clone());
    }
    if let Some(body) = body_bytes {
        builder = builder.body(body);
    }

    let resp = builder.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("remote WES request failed: {e}"),
        )
    })?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let resp_headers = resp.headers().clone();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(ct) = resp_headers.get(header::CONTENT_TYPE) {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, ct.clone());
    }
    Ok(response)
}

async fn get_federated_catalog(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let registry_services = load_registry_services(&state.config).await;
    let selection_prefs =
        ferrum_discovery::ServiceSelectionPrefs::from_discovery_config(&state.config.discovery);
    let ads_bases = collect_ads_bases(&state, &registry_services).await?;

    let mut merged: Vec<serde_json::Value> = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (origin, base) in &ads_bases {
        let url = format!("{base}/catalog/datasets");
        let mut builder = client.get(&url);
        if let Some(token) = &auth {
            builder = builder.header(header::AUTHORIZATION, token);
        }
        match builder.send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = body.get("datasets").and_then(|d| d.as_array()) {
                        for entry in arr {
                            let mut row = entry.clone();
                            if let Some(obj) = row.as_object_mut() {
                                enrich_catalog_entry(
                                    obj,
                                    origin,
                                    base,
                                    &registry_services,
                                    &selection_prefs,
                                );
                            }
                            merged.push(row);
                        }
                    }
                }
            }
            Ok(resp) => {
                errors.push(serde_json::json!({
                    "origin": origin,
                    "ads_base_url": base,
                    "status": resp.status().as_u16(),
                }));
            }
            Err(err) => {
                errors.push(serde_json::json!({
                    "origin": origin,
                    "ads_base_url": base,
                    "error": err.to_string(),
                }));
            }
        }
    }

    if merged.is_empty() && !errors.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("federated catalog harvest failed: {errors:?}"),
        ));
    }

    let (datasets, duplicates_dropped) =
        dedup_federated_catalog(merged, &registry_services, &selection_prefs);

    Ok(axum::Json(serde_json::json!({
        "datasets": datasets,
        "sources": ads_bases.iter().map(|(o, u)| serde_json::json!({"origin": o, "ads_base_url": u})).collect::<Vec<_>>(),
        "errors": errors,
        "duplicates_dropped": duplicates_dropped,
    }))
    .into_response())
}

fn normalize_ads_from_service_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/ads/v1") {
        trimmed.to_string()
    } else if trimmed.ends_with("/ads") {
        format!("{trimmed}/v1")
    } else {
        format!("{trimmed}/ads/v1")
    }
}

async fn get_dataset(
    State(state): State<Arc<AccessProxyState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", &format!("datasets/{id}"), req).await
}

async fn get_me_projects(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", "me/projects", req).await
}

async fn get_me_access_requests(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", "me/access-requests", req).await
}

async fn ads_get_json(
    client: &reqwest::Client,
    ads_base: &str,
    path: &str,
    auth: Option<&str>,
) -> Result<serde_json::Value, String> {
    let url = format!("{ads_base}/{}", path.trim_start_matches('/'));
    let mut builder = client.get(&url);
    if let Some(token) = auth {
        builder = builder.header(header::AUTHORIZATION, token);
    }
    let resp = builder.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("ADS HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

fn enrich_grant_from_dataset(
    grant: &mut serde_json::Map<String, serde_json::Value>,
    dataset: &serde_json::Value,
    origin: &str,
    ads_base: &str,
    registry_services: &[ferrum_discovery::RegisteredService],
    prefs: &ferrum_discovery::ServiceSelectionPrefs,
) {
    grant.insert(
        "federation_origin".to_string(),
        serde_json::Value::String(origin.to_string()),
    );
    grant.insert(
        "ads_base_url".to_string(),
        serde_json::Value::String(ads_base.to_string()),
    );
    for key in [
        "name",
        "description",
        "external_id",
        "resource_type",
        "remote_drs_base_url",
    ] {
        if let Some(v) = dataset.get(key) {
            if key == "name" {
                grant.insert("dataset_name".to_string(), v.clone());
            } else {
                grant.insert(key.to_string(), v.clone());
            }
        }
    }
    let remote_drs = grant
        .get("remote_drs_base_url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| ferrum_discovery::drs_url_for_ads_origin(registry_services, origin, prefs));
    if let Some(drs) = remote_drs {
        grant.insert(
            "remote_drs_base_url".to_string(),
            serde_json::Value::String(drs),
        );
    }
    if grant.get("resource_type").and_then(|v| v.as_str()) == Some("compute_pool") {
        if let Some(wes) =
            ferrum_discovery::wes_url_for_ads_origin(registry_services, origin, prefs)
        {
            grant.insert(
                "remote_wes_base_url".to_string(),
                serde_json::Value::String(wes),
            );
        }
    }
}

async fn get_me_grants(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let registry_services = load_registry_services(&state.config).await;
    let selection_prefs =
        ferrum_discovery::ServiceSelectionPrefs::from_discovery_config(&state.config.discovery);
    let ads_bases = collect_ads_bases(&state, &registry_services).await?;

    let mut enriched: Vec<serde_json::Value> = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (origin, base) in &ads_bases {
        match ads_get_json(&client, base, "me/grants", auth.as_deref()).await {
            Ok(body) => {
                if let Some(grants) = body.get("grants").and_then(|g| g.as_array()) {
                    for grant in grants {
                        let mut row = grant.clone();
                        if let Some(obj) = row.as_object_mut() {
                            let dataset_id = obj
                                .get("dataset_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            if !dataset_id.is_empty() {
                                if let Ok(dataset) = ads_get_json(
                                    &client,
                                    base,
                                    &format!("datasets/{dataset_id}"),
                                    auth.as_deref(),
                                )
                                .await
                                {
                                    enrich_grant_from_dataset(
                                        obj,
                                        &dataset,
                                        origin,
                                        base,
                                        &registry_services,
                                        &selection_prefs,
                                    );
                                } else {
                                    obj.insert(
                                        "federation_origin".to_string(),
                                        serde_json::Value::String(origin.clone()),
                                    );
                                    obj.insert(
                                        "ads_base_url".to_string(),
                                        serde_json::Value::String(base.clone()),
                                    );
                                }
                            }
                        }
                        enriched.push(row);
                    }
                }
            }
            Err(err) => {
                errors.push(serde_json::json!({
                    "origin": origin,
                    "ads_base_url": base,
                    "error": err,
                }));
            }
        }
    }

    if enriched.is_empty() && errors.len() == ads_bases.len() && !ads_bases.is_empty() {
        return forward(state, "GET", "me/grants", req).await;
    }

    #[cfg(feature = "discovery")]
    let (enriched, duplicates_dropped) =
        dedup_federated_grants(enriched, &registry_services, &selection_prefs);
    #[cfg(not(feature = "discovery"))]
    let duplicates_dropped = 0usize;

    Ok(axum::Json(serde_json::json!({
        "grants": enriched,
        "errors": errors,
        "duplicates_dropped": duplicates_dropped,
    }))
    .into_response())
}

async fn post_projects(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "POST", "projects", req).await
}

async fn post_access_requests(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "POST", "access-requests", req).await
}

async fn get_access_request(
    State(state): State<Arc<AccessProxyState>>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", &format!("access-requests/{id}"), req).await
}

pub fn access_router(config: &FerrumConfig) -> Router {
    let state = Arc::new(AccessProxyState::new(config.clone()));
    Router::new()
        .route("/status", get(get_status))
        .route("/catalog/datasets", get(get_catalog_datasets))
        .route("/catalog/federated", get(get_federated_catalog))
        .route("/federated/drs/*path", get(federated_drs_proxy))
        .route("/federated/wes/*path", any(federated_wes_proxy))
        .route("/datasets/:id", get(get_dataset))
        .route("/me/projects", get(get_me_projects))
        .route("/me/access-requests", get(get_me_access_requests))
        .route("/me/grants", get(get_me_grants))
        .route("/projects", post(post_projects))
        .route("/access-requests", post(post_access_requests))
        .route("/access-requests/:id", get(get_access_request))
        .with_state(state)
}
