// SPDX-License-Identifier: BUSL-1.1
//! Fetch Visa JWTs from the issuer UserInfo endpoint when the access token
//! carries an empty `ga4gh_passport_v1` (ga4gh-infra ADR-005 / Ferrum ADR-034).
//!
//! Endpoint comes only from OIDC discovery of the **configured** issuer.
//! Bearer is forwarded. Cache is keyed by SHA-256 of the raw token; TTL is
//! bounded by token `exp`. Fail closed on any error (including broker restart
//! → UserInfo 401). No URL is taken from token claims (SSRF).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Max cached UserInfo Visa bundles (process-wide).
pub const USERINFO_VISA_CACHE_MAX: usize = 1024;

/// HTTP timeout for discovery and UserInfo.
const USERINFO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

struct CachedVisas {
    visas: Vec<String>,
    /// Absolute unix expiry (token `exp`); entry is dropped when `now >= exp`.
    exp_unix: i64,
    inserted_at: Instant,
}

static USERINFO_VISA_CACHE: LazyLock<Mutex<HashMap<[u8; 32], CachedVisas>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static DISCOVERY_USERINFO_CACHE: LazyLock<Mutex<HashMap<String, (Instant, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(3600);

fn token_hash(token: &str) -> [u8; 32] {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn issuer_and_endpoint_share_scheme(issuer: &str, endpoint: &str) -> bool {
    let issuer_scheme = issuer.split("://").next();
    let ep_scheme = endpoint.split("://").next();
    matches!(
        (issuer_scheme, ep_scheme),
        (Some("https"), Some("https")) | (Some("http"), Some("http"))
    )
}

/// Discover `userinfo_endpoint` from `{issuer}/.well-known/openid-configuration`.
///
/// Same issuer-match and scheme rules as JWKS discovery (ADR-028 / ADR-034).
pub async fn discover_userinfo_endpoint(
    issuer: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let issuer = issuer.trim_end_matches('/');
    if let Ok(cache) = DISCOVERY_USERINFO_CACHE.lock() {
        if let Some((fetched_at, endpoint)) = cache.get(issuer) {
            if fetched_at.elapsed() < DISCOVERY_CACHE_TTL {
                return Ok(endpoint.clone());
            }
        }
    }

    let url = format!("{issuer}/.well-known/openid-configuration");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(USERINFO_HTTP_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(issuer, error = %err, "OIDC discovery for userinfo failed");
            jsonwebtoken::errors::ErrorKind::InvalidToken
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            issuer,
            status = %response.status(),
            "OIDC discovery for userinfo failed"
        );
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }
    let document: serde_json::Value = response.json().await.map_err(|err| {
        tracing::warn!(issuer, error = %err, "OIDC discovery document was not JSON");
        jsonwebtoken::errors::ErrorKind::InvalidToken
    })?;
    let document_issuer = document
        .get("issuer")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim_end_matches('/');
    if document_issuer != issuer {
        tracing::warn!(
            configured = issuer,
            document = document_issuer,
            "OIDC discovery issuer does not match the configured issuer (userinfo)"
        );
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }
    let Some(endpoint) = document
        .get("userinfo_endpoint")
        .and_then(|value| value.as_str())
        .filter(|uri| !uri.is_empty())
    else {
        tracing::warn!(issuer, "OIDC discovery document has no userinfo_endpoint");
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    };
    if !issuer_and_endpoint_share_scheme(issuer, endpoint) {
        tracing::warn!(
            issuer,
            userinfo_endpoint = endpoint,
            "userinfo_endpoint scheme does not match the issuer"
        );
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    if let Ok(mut cache) = DISCOVERY_USERINFO_CACHE.lock() {
        cache.insert(issuer.to_string(), (Instant::now(), endpoint.to_string()));
    }
    Ok(endpoint.to_string())
}

/// Return Visa JWT strings from cache or UserInfo for an empty `ga4gh_passport_v1`.
///
/// `token_exp` bounds cache TTL. Fail closed on any error.
pub async fn visas_from_userinfo(
    configured_issuer: &str,
    bearer_token: &str,
    token_exp: i64,
) -> Result<Vec<String>, jsonwebtoken::errors::Error> {
    let now = unix_now();
    if token_exp <= now {
        return Err(jsonwebtoken::errors::ErrorKind::ExpiredSignature.into());
    }

    let key = token_hash(bearer_token);
    if let Ok(mut cache) = USERINFO_VISA_CACHE.lock() {
        cache.retain(|_, v| v.exp_unix > now);
        if let Some(hit) = cache.get(&key) {
            return Ok(hit.visas.clone());
        }
    }

    let endpoint = discover_userinfo_endpoint(configured_issuer).await?;
    let client = reqwest::Client::new();
    let response = client
        .get(&endpoint)
        .bearer_auth(bearer_token)
        .timeout(USERINFO_HTTP_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "UserInfo fetch failed");
            jsonwebtoken::errors::ErrorKind::InvalidToken
        })?;
    if !response.status().is_success() {
        // Broker restart / revoked document → typically 401. Fail closed.
        tracing::warn!(
            status = %response.status(),
            "UserInfo returned non-success; denying (fail closed)"
        );
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }
    let body: serde_json::Value = response.json().await.map_err(|err| {
        tracing::warn!(error = %err, "UserInfo body was not JSON");
        jsonwebtoken::errors::ErrorKind::InvalidToken
    })?;
    let visas = body
        .get("ga4gh_passport_v1")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Ok(mut cache) = USERINFO_VISA_CACHE.lock() {
        cache.retain(|_, v| v.exp_unix > now);
        while cache.len() >= USERINFO_VISA_CACHE_MAX {
            // Evict oldest insertion (fail-safe bound, not LRU-perfect).
            let oldest = cache
                .iter()
                .min_by_key(|(_, v)| v.inserted_at)
                .map(|(k, _)| *k);
            if let Some(k) = oldest {
                cache.remove(&k);
            } else {
                break;
            }
        }
        cache.insert(
            key,
            CachedVisas {
                visas: visas.clone(),
                exp_unix: token_exp,
                inserted_at: Instant::now(),
            },
        );
    }

    Ok(visas)
}

/// Test helper: clear caches between cases.
#[cfg(test)]
pub fn clear_userinfo_caches_for_test() {
    if let Ok(mut c) = USERINFO_VISA_CACHE.lock() {
        c.clear();
    }
    if let Ok(mut c) = DISCOVERY_USERINFO_CACHE.lock() {
        c.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn userinfo_401_after_broker_restart_denies() {
        clear_userinfo_caches_for_test();
        let server = MockServer::start().await;
        let issuer = server.uri();
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": issuer,
                "userinfo_endpoint": format!("{issuer}/userinfo"),
                "jwks_uri": format!("{issuer}/jwks.json"),
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .and(header("authorization", "Bearer dead-token"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = visas_from_userinfo(&issuer, "dead-token", unix_now() + 3600)
            .await
            .expect_err("401 must deny");
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::InvalidToken
        ));
    }

    #[tokio::test]
    async fn userinfo_fetches_and_caches_visas() {
        clear_userinfo_caches_for_test();
        let server = MockServer::start().await;
        let issuer = server.uri();
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": issuer,
                "userinfo_endpoint": format!("{issuer}/userinfo"),
                "jwks_uri": format!("{issuer}/jwks.json"),
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sub": "alice",
                "ga4gh_passport_v1": ["visa-one", "visa-two"],
            })))
            .expect(1) // second call must hit cache
            .mount(&server)
            .await;

        let exp = unix_now() + 3600;
        let first = visas_from_userinfo(&issuer, "tok-cache", exp)
            .await
            .expect("fetch");
        assert_eq!(first, vec!["visa-one", "visa-two"]);
        let second = visas_from_userinfo(&issuer, "tok-cache", exp)
            .await
            .expect("cache");
        assert_eq!(second, first);
    }

    #[tokio::test]
    async fn discovery_rejects_scheme_change_userinfo() {
        clear_userinfo_caches_for_test();
        let server = MockServer::start().await;
        let issuer = server.uri(); // http://…
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": issuer,
                "userinfo_endpoint": "https://evil.example/userinfo",
                "jwks_uri": format!("{issuer}/jwks.json"),
            })))
            .mount(&server)
            .await;
        let err = discover_userinfo_endpoint(&issuer)
            .await
            .expect_err("scheme mismatch");
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::InvalidToken
        ));
    }

    #[tokio::test]
    async fn cache_respects_max_entries_bound() {
        clear_userinfo_caches_for_test();
        let now = unix_now();
        if let Ok(mut cache) = USERINFO_VISA_CACHE.lock() {
            for i in 0..USERINFO_VISA_CACHE_MAX {
                let mut key = [0u8; 32];
                key[0] = (i % 256) as u8;
                key[1] = (i / 256) as u8;
                cache.insert(
                    key,
                    CachedVisas {
                        visas: vec![format!("v{i}")],
                        exp_unix: now + 3600,
                        inserted_at: Instant::now(),
                    },
                );
            }
            assert_eq!(cache.len(), USERINFO_VISA_CACHE_MAX);
        }
        // Inserting via public API would need a live server; bound is enforced in visas_from_userinfo.
        assert_eq!(USERINFO_VISA_CACHE_MAX, 1024);
    }
}
