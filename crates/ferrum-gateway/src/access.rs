//! ADS proxy for researcher dataset access requests (ga4gh-infra integration).
//! Resolves the ADS base URL via service discovery at request time.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use ferrum_core::FerrumConfig;
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
    let ads = state.ads_base_url().await?;
    let url = format!("{ads}/{}", ads_path.trim_start_matches('/'));

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

    let mut ads_bases: Vec<(String, String)> = Vec::new();
    if let Ok(local) = state.ads_base_url().await {
        ads_bases.push(("local".to_string(), local));
    }

    #[cfg(feature = "discovery")]
    if state.config.discovery.enabled {
        if let Ok(registry) =
            ferrum_discovery::ServiceRegistryClient::from_config(&state.config.discovery)
        {
            if let Ok(services) = registry.list().await {
                for svc in services {
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
        }
    }

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
                                obj.insert(
                                    "federation_origin".to_string(),
                                    serde_json::Value::String(origin.clone()),
                                );
                                obj.insert(
                                    "ads_base_url".to_string(),
                                    serde_json::Value::String(base.clone()),
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

    Ok(axum::Json(serde_json::json!({
        "datasets": merged,
        "sources": ads_bases.iter().map(|(o, u)| serde_json::json!({"origin": o, "ads_base_url": u})).collect::<Vec<_>>(),
        "errors": errors,
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

async fn get_me_grants(
    State(state): State<Arc<AccessProxyState>>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    forward(state, "GET", "me/grants", req).await
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
        .route("/datasets/:id", get(get_dataset))
        .route("/me/projects", get(get_me_projects))
        .route("/me/access-requests", get(get_me_access_requests))
        .route("/me/grants", get(get_me_grants))
        .route("/projects", post(post_projects))
        .route("/access-requests", post(post_access_requests))
        .route("/access-requests/:id", get(get_access_request))
        .with_state(state)
}
