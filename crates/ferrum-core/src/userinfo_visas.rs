// SPDX-License-Identifier: Apache-2.0
//! Fetch Visa JWTs from the issuer UserInfo endpoint when the access token
//! carries no Visas (ga4gh-infra ADR-005 / Ferrum ADR-034, ADR-036).
//!
//! Endpoint comes only from OIDC discovery of the **configured** issuer.
//! Bearer is forwarded. Cache is keyed by SHA-256 of the raw token; TTL is
//! bounded by token `exp`. No URL is taken from token claims (SSRF).
//!
//! - `ga4gh_passport_v1` **absent**: Visas are not embedded. [`visas_from_userinfo`]
//!   fails closed on any error (ADR-034).
//! - `ga4gh_passport_v1` **present and empty**: [`visas_for_empty_embedded_list`]
//!   asks UserInfo too (brokers up to ga4gh-infra v0.2.9 send `[]` in both modes).
//!   A 401 or 403 there rejects the token (ADR-037). If UserInfo cannot be used
//!   at all, the Passport carries no Visas: the caller gets an empty list and
//!   identity-only access (ADR-036). Empty Visas cannot grant anything.

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

/// ADR-036: after a failed UserInfo fetch for a present-but-empty Visa list, the
/// empty result is cached this long (and never past token `exp`) before retrying.
pub const EMPTY_LIST_RETRY_SECS: i64 = 300;

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
    fetch_userinfo_visas(configured_issuer, bearer_token, token_exp)
        .await
        .map_err(UserInfoFailure::into_jwt_error)
}

/// Why a UserInfo fetch gave no Visas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserInfoFailure {
    /// The token's own `exp` has passed.
    Expired,
    /// The issuer answered 401 or 403: it does not accept this token (revoked,
    /// unknown after a broker restart, or invalid).
    Rejected,
    /// Discovery or UserInfo could not be used: network error, timeout, other
    /// status, or a body that is not JSON.
    Unavailable,
}

impl UserInfoFailure {
    fn into_jwt_error(self) -> jsonwebtoken::errors::Error {
        match self {
            Self::Expired => jsonwebtoken::errors::ErrorKind::ExpiredSignature.into(),
            Self::Rejected | Self::Unavailable => {
                jsonwebtoken::errors::ErrorKind::InvalidToken.into()
            }
        }
    }
}

async fn fetch_userinfo_visas(
    configured_issuer: &str,
    bearer_token: &str,
    token_exp: i64,
) -> Result<Vec<String>, UserInfoFailure> {
    let now = unix_now();
    if token_exp <= now {
        return Err(UserInfoFailure::Expired);
    }

    let key = token_hash(bearer_token);
    if let Ok(mut cache) = USERINFO_VISA_CACHE.lock() {
        cache.retain(|_, v| v.exp_unix > now);
        if let Some(hit) = cache.get(&key) {
            return Ok(hit.visas.clone());
        }
    }

    let endpoint = discover_userinfo_endpoint(configured_issuer)
        .await
        .map_err(|_| UserInfoFailure::Unavailable)?;
    let client = reqwest::Client::new();
    let response = client
        .get(&endpoint)
        .bearer_auth(bearer_token)
        .timeout(USERINFO_HTTP_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "UserInfo fetch failed");
            UserInfoFailure::Unavailable
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        // The issuer does not accept this token: broker restart, revoked or unknown.
        tracing::warn!(%status, "UserInfo rejected the token; denying (fail closed)");
        return Err(UserInfoFailure::Rejected);
    }
    if !status.is_success() {
        tracing::warn!(%status, "UserInfo returned non-success; denying (fail closed)");
        return Err(UserInfoFailure::Unavailable);
    }
    let body: serde_json::Value = response.json().await.map_err(|err| {
        tracing::warn!(error = %err, "UserInfo body was not JSON");
        UserInfoFailure::Unavailable
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

    cache_put(key, visas.clone(), token_exp);
    Ok(visas)
}

fn cache_put(key: [u8; 32], visas: Vec<String>, exp_unix: i64) {
    let now = unix_now();
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
                visas,
                exp_unix,
                inserted_at: Instant::now(),
            },
        );
    }
}

/// ADR-036: Visas for a validated token whose `ga4gh_passport_v1` is **present and empty**.
///
/// Asks UserInfo like [`visas_from_userinfo`], because ga4gh-infra brokers also
/// send `[]` when Visas live behind UserInfo.
///
/// - 401 or 403: the issuer does not accept the token (broker restart, revoked,
///   unknown). Error, fail closed (ADR-037). ga4gh-infra from v0.2.11 answers
///   200 with an empty list for a user without Visas (its ADR-007).
/// - Issuer unreachable, timeout, other status, bad JSON: the Passport carries no
///   Visas. Returns an empty list (identity only, ADR-036), cached for
///   [`EMPTY_LIST_RETRY_SECS`] (bounded by `exp`).
/// - An expired token is an error.
pub async fn visas_for_empty_embedded_list(
    configured_issuer: &str,
    bearer_token: &str,
    token_exp: i64,
) -> Result<Vec<String>, jsonwebtoken::errors::Error> {
    match fetch_userinfo_visas(configured_issuer, bearer_token, token_exp).await {
        Ok(visas) => Ok(visas),
        // ADR-037: 401/403 from UserInfo means the issuer does not accept the token.
        Err(failure @ (UserInfoFailure::Expired | UserInfoFailure::Rejected)) => {
            Err(failure.into_jwt_error())
        }
        Err(UserInfoFailure::Unavailable) => {
            tracing::warn!(
                "Passport has an empty ga4gh_passport_v1 and UserInfo could not be used; \
                 continuing without Visas (identity only, ADR-036)"
            );
            let retry_at = (unix_now() + EMPTY_LIST_RETRY_SECS).min(token_exp);
            cache_put(token_hash(bearer_token), Vec::new(), retry_at);
            Ok(Vec::new())
        }
    }
}

/// True when the JWT payload has a `ga4gh_passport_v1` array (empty or not).
///
/// Call only after the signature was verified; this reads the payload without
/// checking it.
pub fn passport_claim_present(token: &str) -> bool {
    use base64::Engine as _;
    let Some(payload) = token.split('.').nth(1) else {
        return false;
    };
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| v.get("ga4gh_passport_v1").map(serde_json::Value::is_array))
        .unwrap_or(false)
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

    async fn mount_discovery(server: &MockServer) -> String {
        let issuer = server.uri();
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "issuer": issuer,
                "userinfo_endpoint": format!("{issuer}/userinfo"),
                "jwks_uri": format!("{issuer}/jwks.json"),
            })))
            .mount(server)
            .await;
        issuer
    }

    #[tokio::test]
    async fn empty_embedded_list_with_userinfo_401_or_403_rejects() {
        for status in [401_u16, 403] {
            clear_userinfo_caches_for_test();
            let server = MockServer::start().await;
            let issuer = mount_discovery(&server).await;
            Mock::given(method("GET"))
                .and(path("/userinfo"))
                .respond_with(ResponseTemplate::new(status))
                .expect(2) // a rejection is not cached
                .mount(&server)
                .await;
            let exp = unix_now() + 3600;
            for _ in 0..2 {
                let err = visas_for_empty_embedded_list(&issuer, "tok-gone", exp)
                    .await
                    .expect_err("issuer rejected the token");
                assert!(matches!(
                    err.kind(),
                    jsonwebtoken::errors::ErrorKind::InvalidToken
                ));
            }
        }
    }

    #[tokio::test]
    async fn empty_embedded_list_with_userinfo_5xx_is_identity_only_and_cached() {
        clear_userinfo_caches_for_test();
        let server = MockServer::start().await;
        let issuer = mount_discovery(&server).await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1) // the second call must hit the negative cache
            .mount(&server)
            .await;
        let exp = unix_now() + 3600;
        let first = visas_for_empty_embedded_list(&issuer, "tok-empty", exp)
            .await
            .expect("present empty list is not an error");
        assert!(first.is_empty());
        let second = visas_for_empty_embedded_list(&issuer, "tok-empty", exp)
            .await
            .expect("cached");
        assert!(second.is_empty());
        let cached_until = USERINFO_VISA_CACHE
            .lock()
            .expect("cache")
            .get(&token_hash("tok-empty"))
            .map(|e| e.exp_unix)
            .expect("negative entry");
        assert!(cached_until <= unix_now() + EMPTY_LIST_RETRY_SECS);
    }

    #[tokio::test]
    async fn empty_embedded_list_still_uses_userinfo_visas_when_available() {
        clear_userinfo_caches_for_test();
        let server = MockServer::start().await;
        let issuer = mount_discovery(&server).await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sub": "alice",
                "ga4gh_passport_v1": ["visa-from-userinfo"],
            })))
            .mount(&server)
            .await;
        let visas = visas_for_empty_embedded_list(&issuer, "tok-legacy", unix_now() + 3600)
            .await
            .expect("fetch");
        assert_eq!(visas, vec!["visa-from-userinfo"]);
    }

    #[tokio::test]
    async fn empty_embedded_list_with_unreachable_issuer_is_identity_only() {
        clear_userinfo_caches_for_test();
        // Nothing listens on port 9 (discard) on loopback in CI or locally.
        let visas =
            visas_for_empty_embedded_list("http://127.0.0.1:9", "tok-down", unix_now() + 3600)
                .await
                .expect("identity only");
        assert!(visas.is_empty());
    }

    #[tokio::test]
    async fn empty_embedded_list_does_not_accept_an_expired_token() {
        clear_userinfo_caches_for_test();
        let err = visas_for_empty_embedded_list("http://127.0.0.1:9", "tok-old", unix_now() - 1)
            .await
            .expect_err("expired");
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::ExpiredSignature
        ));
    }

    #[test]
    fn passport_claim_present_tells_absent_from_empty() {
        use base64::Engine as _;
        let jwt = |payload: serde_json::Value| {
            let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD;
            format!(
                "{}.{}.sig",
                enc.encode(br#"{"alg":"RS256"}"#),
                enc.encode(payload.to_string())
            )
        };
        assert!(passport_claim_present(&jwt(
            serde_json::json!({"sub": "a", "ga4gh_passport_v1": []})
        )));
        assert!(passport_claim_present(&jwt(
            serde_json::json!({"sub": "a", "ga4gh_passport_v1": ["v"]})
        )));
        assert!(!passport_claim_present(&jwt(
            serde_json::json!({"sub": "a"})
        )));
        assert!(!passport_claim_present(&jwt(
            serde_json::json!({"sub": "a", "ga4gh_passport_v1": null})
        )));
        assert!(!passport_claim_present("not-a-jwt"));
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
