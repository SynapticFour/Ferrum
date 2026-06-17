//! Forward WES run submissions to a remote node when federated compute-pool tags are present.

use crate::error::WesError;
use crate::state::AppState;
use axum::http::header;

async fn load_registry_services(
    config: &ferrum_core::FerrumConfig,
) -> Vec<ferrum_discovery::RegisteredService> {
    if !config.discovery.enabled {
        return Vec::new();
    }
    let Ok(registry) = ferrum_discovery::ServiceRegistryClient::from_config(&config.discovery)
    else {
        return Vec::new();
    };
    registry.list().await.unwrap_or_default()
}

/// When tags include `ads_compute_pool_id` and a remote target (`remote_wes_base_url` or
/// `federation_origin`), POST the run body to the peer WES and return its run id.
pub async fn try_forward_federated_run(
    app: &AppState,
    tags: &serde_json::Value,
    bytes: &[u8],
    auth_header: Option<&str>,
) -> Result<Option<String>, WesError> {
    let Some(cfg) = app.federation_config.as_ref() else {
        return Ok(None);
    };
    let map = match tags.as_object() {
        Some(m) => m,
        None => return Ok(None),
    };
    let pool_id = match map.get("ads_compute_pool_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id,
        _ => return Ok(None),
    };
    let remote_base = map
        .get("remote_wes_base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_end_matches('/').to_string());
    let origin = map
        .get("federation_origin")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "local");
    if remote_base.is_none() && origin.is_none() {
        return Ok(None);
    }

    let prefs = ferrum_discovery::ServiceSelectionPrefs::from_discovery_config(&cfg.discovery);
    let wes_base = if let Some(url) = remote_base {
        url
    } else {
        let registry = load_registry_services(cfg).await;
        ferrum_discovery::wes_url_for_ads_origin(&registry, origin.unwrap(), &prefs).ok_or_else(
            || WesError::Validation("could not resolve remote WES for federation origin".into()),
        )?
    };

    if !wes_base.starts_with("http://") && !wes_base.starts_with("https://") {
        return Err(WesError::Validation(
            "remote_wes_base_url must be http(s)".into(),
        ));
    }

    let ads_base = map.get("ads_base_url").and_then(|v| v.as_str());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| WesError::Other(e.into()))?;

    let mut req = client
        .post(format!("{wes_base}/runs"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(bytes.to_vec());
    if let Some(token) = auth_header {
        req = req.header(header::AUTHORIZATION, token);
    }
    if let Some(ads) = ads_base.filter(|s| !s.is_empty()) {
        req = req.header("X-ADS-Base-URL", ads);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| WesError::Other(anyhow::anyhow!("remote WES request failed: {e}")))?;
    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| WesError::Other(e.into()))?;
    if !status.is_success() {
        let detail = String::from_utf8_lossy(&body);
        return Err(WesError::Forbidden(format!(
            "remote WES returned {status} for compute pool {pool_id}: {detail}"
        )));
    }
    let parsed: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
        WesError::Other(anyhow::anyhow!(
            "invalid remote WES response for pool {pool_id}: {e}"
        ))
    })?;
    let run_id = parsed
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            WesError::Other(anyhow::anyhow!(
                "remote WES response missing run_id for pool {pool_id}"
            ))
        })?;
    Ok(Some(run_id.to_string()))
}
