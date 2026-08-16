// SPDX-License-Identifier: BUSL-1.1
//! Non-fatal startup network probes for offline-first mode.

use ferrum_core::FerrumConfig;
use std::time::Duration;

pub const STARTUP_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Probe auth-related endpoints at startup; failures are warnings when offline-first.
pub async fn probe_auth_endpoints(cfg: &FerrumConfig, offline_first: bool) {
    if offline_first {
        tracing::info!("offline-first mode: skipping mandatory auth endpoint probes");
    }

    if let Some(ref url) = cfg.auth.jwks_url {
        probe_with_timeout("JWKS", url, offline_first).await;
    }
    for endpoint in &cfg.auth.passport_endpoints {
        probe_with_timeout("Passport", endpoint, offline_first).await;
    }
}

pub async fn probe_with_timeout(label: &str, url: &str, offline_first: bool) {
    let client = match reqwest::Client::builder()
        .timeout(STARTUP_PROBE_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target = label, error = %e, "could not build HTTP client for startup probe");
            return;
        }
    };

    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target = label, url = %url, "startup probe succeeded");
        }
        Ok(resp) => {
            let msg = format!("startup probe returned HTTP {}", resp.status());
            tracing::warn!(target = label, url = %url, %msg);
        }
        Err(e) => {
            if offline_first {
                tracing::warn!(
                    target = label,
                    url = %url,
                    error = %e,
                    "startup probe failed (non-fatal in offline-first mode)"
                );
            } else {
                tracing::warn!(target = label, url = %url, error = %e, "startup probe failed");
            }
        }
    }
}
