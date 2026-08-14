//! Auth middleware: JWT validation (jsonwebtoken), GA4GH Passport extraction, Bearer + cookie. A07: revocation check.

use async_trait::async_trait;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

pub const VISA_ADMIN: &str = "ferrum:admin";
pub const VISA_OUTBREAK: &str = "ferrum:outbreak_activator";
pub const VISA_COLLECTOR: &str = "ferrum:collector";
pub const VISA_ANALYST: &str = "ferrum:analyst";
pub const VISA_SYNC_OPERATOR: &str = "ferrum:sync_operator";

/// Hub default: 1 hour. Field/offline nodes should set `jwks_cache_ttl_secs` (e.g. 7 days).
const DEFAULT_JWKS_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Fail-closed unless config/env explicitly disables auth. Set at gateway startup.
static REQUIRE_AUTH_RUNTIME: AtomicBool = AtomicBool::new(true);

/// GA4GH Visa object (ga4gh_visa_v1 claim value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisaObject {
    pub r#type: String,
    pub asserted: i64,
    pub value: String,
    pub source: String,
    #[serde(default)]
    pub conditions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub by: Option<String>,
}

/// Decoded GA4GH Passport JWT claims (includes ga4gh_passport_v1 array of Visa JWTs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportClaims {
    /// Standard: subject
    #[serde(default)]
    pub sub: Option<String>,
    /// Standard: issuer
    #[serde(default)]
    pub iss: Option<String>,
    /// Standard: expiration (seconds)
    #[serde(default)]
    pub exp: Option<i64>,
    /// Standard: issued at (seconds)
    #[serde(default)]
    pub iat: Option<i64>,
    /// Standard: JWT ID
    #[serde(default)]
    pub jti: Option<String>,
    /// GA4GH: array of Visa JWTs (compact serialization strings)
    #[serde(rename = "ga4gh_passport_v1", default)]
    pub ga4gh_passport_v1: Option<Vec<String>>,
    /// OAuth-style scope (HelixTest auth suite uses e.g. `drs.read`).
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub aud: Option<String>,
}

/// Claims stored in request extensions (set by auth middleware).
#[derive(Debug, Clone)]
pub enum AuthClaims {
    /// Standard JWT claims (e.g. from access token).
    Jwt {
        sub: String,
        iss: Option<String>,
        exp: i64,
        jti: Option<String>,
        scope: Option<String>,
        raw_token: Option<String>,
    },
    /// GA4GH Passport with decoded passport claims and optional decoded visas.
    Passport {
        claims: PassportClaims,
        visas: Vec<VisaObject>,
        raw_token: Option<String>,
    },
}

impl AuthClaims {
    /// Subject (user) identifier for access control (e.g. WES owner_sub, cohort membership).
    pub fn sub(&self) -> Option<&str> {
        match self {
            AuthClaims::Jwt { sub, .. } => Some(sub.as_str()),
            AuthClaims::Passport { claims, .. } => claims.sub.as_deref(),
        }
    }

    /// True if the token has the ferrum:admin role (Passport visas; JWT has no roles in core).
    pub fn is_admin(&self) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| v.value == VISA_ADMIN),
        }
    }

    /// True if the token has the ferrum:outbreak_activator role (Passport visa).
    pub fn is_outbreak_activator(&self) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| v.value == VISA_OUTBREAK),
        }
    }

    /// True if token may ingest field data (collector visa or admin).
    pub fn can_ingest(&self) -> bool {
        self.is_admin() || self.has_field_visa(VISA_COLLECTOR) || self.has_scope(VISA_COLLECTOR)
    }

    /// True if token may run sync operations (sync operator or admin).
    pub fn can_sync(&self) -> bool {
        self.is_admin()
            || self.has_field_visa(VISA_SYNC_OPERATOR)
            || self.has_scope(VISA_SYNC_OPERATOR)
    }

    /// True if token may query/analyze data (analyst, collector, or admin).
    pub fn can_analyze(&self) -> bool {
        self.is_admin()
            || self.has_field_visa(VISA_ANALYST)
            || self.has_field_visa(VISA_COLLECTOR)
            || self.has_scope(VISA_ANALYST)
            || self.has_scope(VISA_COLLECTOR)
    }

    fn has_field_visa(&self, visa_value: &str) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| v.value == visa_value),
        }
    }

    /// Passport issuer (iss claim) when available.
    pub fn issuer(&self) -> Option<&str> {
        match self {
            AuthClaims::Jwt { iss, .. } => iss.as_deref(),
            AuthClaims::Passport { claims, .. } => claims.iss.as_deref(),
        }
    }

    /// Subject or issuer identifier for outbreak emergency recipient matching.
    pub fn recipient_identity(&self) -> Option<&str> {
        self.issuer().or_else(|| self.sub())
    }

    /// True if the token has ControlledAccessGrants visa for the given dataset (DRS access control).
    pub fn has_dataset_grant(&self, dataset_id: &str) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas
                .iter()
                .any(|v| v.r#type == "ControlledAccessGrants" && v.value == dataset_id),
        }
    }

    /// Grant check for published DRS objects (ADS UUID and/or `drs:{object_id}` visa scope).
    pub fn has_published_dataset_access(&self, ads_dataset_id: &str, object_id: &str) -> bool {
        if self.is_admin() {
            return true;
        }
        let drs_scope = format!("drs:{object_id}");
        self.has_dataset_grant(ads_dataset_id) || self.has_dataset_grant(&drs_scope)
    }

    /// True if JWT `scope` claim contains `required` (space or comma separated).
    pub fn has_scope(&self, required: &str) -> bool {
        match self {
            AuthClaims::Jwt { scope, .. } => scope.as_deref().is_some_and(|s| {
                s.split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|t| !t.is_empty())
                    .any(|t| t == required)
            }),
            AuthClaims::Passport { .. } => false,
        }
    }

    /// JWT ID for revocation (A07). None if token has no jti.
    pub fn jti(&self) -> Option<&str> {
        match self {
            AuthClaims::Jwt { jti, .. } => jti.as_deref(),
            AuthClaims::Passport { claims, .. } => claims.jti.as_deref(),
        }
    }

    /// Original Bearer token (for ADS introspection).
    pub fn raw_token(&self) -> Option<&str> {
        match self {
            AuthClaims::Jwt { raw_token, .. } => raw_token.as_deref(),
            AuthClaims::Passport { raw_token, .. } => raw_token.as_deref(),
        }
    }

    #[cfg(feature = "clearinghouse")]
    fn iat(&self) -> Option<i64> {
        match self {
            AuthClaims::Jwt { .. } => None,
            AuthClaims::Passport { claims, .. } => claims.iat,
        }
    }
}

/// A07: Token revocation check (e.g. against revoked_tokens table).
#[async_trait]
pub trait RevocationCheck: Send + Sync {
    async fn is_revoked(&self, jti: &str) -> bool;
}

/// Revocation check using revoked_tokens table (Postgres).
pub struct RevokedTokensChecker {
    pool: sqlx::PgPool,
}

impl RevokedTokensChecker {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RevocationCheck for RevokedTokensChecker {
    async fn is_revoked(&self, jti: &str) -> bool {
        match sqlx::query_as::<_, (bool,)>("SELECT true FROM revoked_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_optional(&self.pool)
            .await
        {
            Ok(row) => row.is_some(),
            Err(e) => {
                tracing::error!(error = %e, "revocation lookup failed; treating token as revoked");
                true
            }
        }
    }
}

/// Record the resolved `require_auth` flag from config (gateway startup).
pub fn set_require_auth_runtime(enabled: bool) {
    REQUIRE_AUTH_RUNTIME.store(enabled, Ordering::Relaxed);
}

fn env_require_auth() -> Option<bool> {
    let v = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok()?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    Some(
        v.eq_ignore_ascii_case("true")
            || v == "1"
            || v.eq_ignore_ascii_case("yes")
            || v.eq_ignore_ascii_case("on"),
    )
}

/// True when auth is required. Env `FERRUM_AUTH__REQUIRE_AUTH` wins when set;
/// otherwise the runtime flag (default **true** / fail-closed).
///
/// Used by WES/DRS/TES handlers to reject anonymous callers.
pub fn require_auth_enabled() -> bool {
    env_require_auth().unwrap_or_else(|| REQUIRE_AUTH_RUNTIME.load(Ordering::Relaxed))
}

/// Auth config used by the middleware (from FerrumConfig).
#[derive(Clone)]
pub struct AuthMiddlewareConfig {
    pub jwt_secret: Option<Vec<u8>>,
    pub issuer: Option<String>,
    pub jwks_url: Option<String>,
    /// Local JWKS JSON file (offline field rotation).
    pub jwks_file: Option<String>,
    pub jwks_cache_ttl: Duration,
    pub passport_endpoints: Vec<String>,
    /// When false, requests without a token get synthetic "demo-user" claims (for demo mode).
    pub require_auth: bool,
    /// A07: Max token age in hours (reject if iat too old). 0 = disable.
    pub max_token_age_hours: u32,
    /// A07: If set, token with matching jti is rejected (revoked).
    pub revocation_check: Option<Arc<dyn RevocationCheck + Send + Sync>>,
    /// When true, Passport visas are verified via ga4gh-clearinghouse (requires `clearinghouse` feature).
    pub use_clearinghouse: bool,
    /// Expected JWT `aud` (optional). When set, HS256 and Passport JWTs must match.
    pub audience: Option<String>,
}

impl AuthMiddlewareConfig {
    pub fn from_crate_config(cfg: &crate::config::AuthConfig) -> Self {
        let jwks_url = cfg
            .jwks_url
            .clone()
            .or_else(|| std::env::var("FERRUM_AUTH__JWKS_URL").ok())
            .filter(|s| !s.is_empty());
        let issuer = cfg
            .issuer
            .clone()
            .or_else(|| std::env::var("FERRUM_AUTH__ISSUER").ok())
            .filter(|s| !s.is_empty());
        let jwks_file = cfg
            .jwks_file
            .clone()
            .or_else(|| std::env::var("FERRUM_AUTH__JWKS_FILE").ok())
            .filter(|s| !s.is_empty());
        let ttl_secs = std::env::var("FERRUM_AUTH__JWKS_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(cfg.jwks_cache_ttl_secs);
        Self {
            jwt_secret: cfg.jwt_secret.as_deref().map(|s| s.as_bytes().to_vec()),
            issuer,
            jwks_url,
            jwks_file,
            jwks_cache_ttl: Duration::from_secs(ttl_secs.max(60)),
            passport_endpoints: cfg.passport_endpoints.clone(),
            require_auth: cfg.require_auth,
            max_token_age_hours: cfg.max_token_age_hours,
            revocation_check: None,
            use_clearinghouse: cfg.clearinghouse || cfg.is_external(),
            audience: cfg
                .audience
                .clone()
                .or_else(|| std::env::var("FERRUM_AUTH__AUDIENCE").ok())
                .filter(|s| !s.is_empty()),
        }
    }

    /// Config for demo/unauthenticated mode: no JWT required, inject "demo-user" when no token present.
    pub fn demo() -> Self {
        Self {
            jwt_secret: None,
            issuer: None,
            jwks_url: None,
            jwks_file: None,
            jwks_cache_ttl: DEFAULT_JWKS_CACHE_TTL,
            passport_endpoints: Vec::new(),
            require_auth: false,
            max_token_age_hours: 24,
            revocation_check: None,
            use_clearinghouse: false,
            audience: None,
        }
    }

    /// Fail-closed: require a valid token; no demo-user injection.
    pub fn fail_closed() -> Self {
        Self {
            jwt_secret: None,
            issuer: None,
            jwks_url: None,
            jwks_file: None,
            jwks_cache_ttl: DEFAULT_JWKS_CACHE_TTL,
            passport_endpoints: Vec::new(),
            require_auth: true,
            max_token_age_hours: 24,
            revocation_check: None,
            use_clearinghouse: false,
            audience: None,
        }
    }

    /// Strict JWT validation from env (HelixTest `HELIXTEST_SHARED_SECRET` / `FERRUM_AUTH__JWT_SECRET`).
    pub fn from_env_strict() -> Option<Self> {
        let secret = std::env::var("FERRUM_AUTH__JWT_SECRET").ok()?;
        if secret.is_empty() {
            return None;
        }
        Some(Self {
            jwt_secret: Some(secret.into_bytes()),
            issuer: std::env::var("FERRUM_AUTH__ISSUER").ok(),
            jwks_url: std::env::var("FERRUM_AUTH__JWKS_URL").ok(),
            jwks_file: std::env::var("FERRUM_AUTH__JWKS_FILE").ok(),
            jwks_cache_ttl: DEFAULT_JWKS_CACHE_TTL,
            passport_endpoints: Vec::new(),
            require_auth: true,
            max_token_age_hours: 0,
            revocation_check: None,
            use_clearinghouse: false,
            audience: std::env::var("FERRUM_AUTH__AUDIENCE")
                .ok()
                .filter(|s| !s.is_empty()),
        })
    }
}

/// Extract Bearer token from Authorization header or from cookie (e.g. `ferrum_token`).
fn extract_token(request: &Request) -> Option<String> {
    let auth = request.headers().get("Authorization")?;
    let s = auth.to_str().ok()?;
    let prefix = "Bearer ";
    if let Some(stripped) = s.strip_prefix(prefix) {
        return Some(stripped.trim().to_string());
    }
    None
}

fn extract_token_from_cookie(request: &Request, cookie_name: &str) -> Option<String> {
    let cookie_header = request.headers().get("Cookie")?;
    let s = cookie_header.to_str().ok()?;
    for part in s.split(';') {
        let part = part.trim();
        let (name, value) = part.split_once('=')?;
        if name == cookie_name {
            return Some(value.to_string());
        }
    }
    None
}

/// Validate JWT and optionally GA4GH Passport; put [AuthClaims] in extensions.
/// Prefer [auth_middleware_with_config] when the caller can pass config directly (avoids relying on request extensions).
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let config = request
        .extensions()
        .get::<Arc<AuthMiddlewareConfig>>()
        .cloned();
    auth_middleware_with_config(config, request, next).await
}

fn unauthorized_json(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": "unauthorized",
            "message": message,
        })),
    )
        .into_response()
}

/// Same as [auth_middleware] but takes config explicitly. Use this when the gateway passes config in so it is always available.
pub async fn auth_middleware_with_config(
    config: Option<Arc<AuthMiddlewareConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let token =
        extract_token(&request).or_else(|| extract_token_from_cookie(&request, "ferrum_token"));

    let mut request = request;

    if let Some(token) = token {
        if let Some(ref cfg) = config {
            match decode_jwt_or_passport(&token, cfg).await {
                Ok(claims) => {
                    let revoked = if let (Some(jti), Some(check)) =
                        (claims.jti(), cfg.revocation_check.as_ref())
                    {
                        check.is_revoked(jti).await
                    } else {
                        false
                    };
                    if revoked {
                        return unauthorized_json("token revoked");
                    }
                    request.extensions_mut().insert(claims);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        jwks_url = ?cfg.jwks_url,
                        "JWT/Passport validation failed"
                    );
                    return unauthorized_json("invalid or expired token");
                }
            }
        } else {
            return unauthorized_json("authentication is not configured");
        }
    }

    // Demo mode: inject demo-user only when no token was sent and auth is not required.
    if request.extensions().get::<AuthClaims>().is_none() {
        let inject = config.as_ref().is_none_or(|cfg| !cfg.require_auth);
        if inject {
            let exp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64 + 86400 * 365)
                .unwrap_or(0);
            request.extensions_mut().insert(AuthClaims::Jwt {
                sub: "demo-user".to_string(),
                iss: Some("ferrum-demo".to_string()),
                exp,
                jti: None,
                scope: None,
                raw_token: None,
            });
        }
    }

    next.run(request).await
}

/// A07: Reject token if issued more than max_hours ago. 0 = skip check.
fn reject_token_if_too_old(
    iat: Option<i64>,
    max_hours: u32,
) -> Result<(), jsonwebtoken::errors::Error> {
    if max_hours == 0 {
        return Ok(());
    }
    let iat = iat.ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let max_age_secs = u64::from(max_hours) * 3600;
    if now.as_secs().saturating_sub(iat as u64) > max_age_secs {
        return Err(jsonwebtoken::errors::ErrorKind::ExpiredSignature.into());
    }
    Ok(())
}

/// Decode as standard JWT (HS256) or as GA4GH Passport.
async fn decode_jwt_or_passport(
    token: &str,
    cfg: &AuthMiddlewareConfig,
) -> Result<AuthClaims, jsonwebtoken::errors::Error> {
    #[cfg(feature = "clearinghouse")]
    if cfg.use_clearinghouse {
        if let Ok(claims) = decode_passport_via_clearinghouse(token, cfg).await {
            reject_token_if_too_old(claims.iat(), cfg.max_token_age_hours)?;
            return Ok(claims);
        }
    }

    // Try as GA4GH Passport (has ga4gh_passport_v1 claim)
    if let Ok(claims) = decode_passport_jwt(token, cfg).await {
        reject_token_if_too_old(claims.iat, cfg.max_token_age_hours)?;
        let visa_jwts = claims.ga4gh_passport_v1.as_deref().unwrap_or(&[]);
        let visas = if cfg.use_clearinghouse {
            decode_passport_visas_clearinghouse(token, visa_jwts).await
        } else {
            decode_passport_visas_verified(visa_jwts, cfg).await
        };
        return Ok(AuthClaims::Passport {
            claims: claims.clone(),
            visas,
            raw_token: Some(token.to_string()),
        });
    }

    // Try as standard JWT — OWASP A02: algorithm pinning, never accept none or HS256 when RS256 expected
    if let Some(ref secret) = cfg.jwt_secret {
        let key = DecodingKey::from_secret(secret);
        let mut validation = Validation::new(Algorithm::HS256);
        validation.algorithms = vec![Algorithm::HS256];
        validation.validate_exp = true;
        if let Some(ref iss) = cfg.issuer {
            validation.iss = Some(HashSet::from([iss.clone()]));
        }
        apply_audience(&mut validation, cfg);
        let data = decode::<PassportClaims>(token, &key, &validation)?;
        reject_token_if_too_old(data.claims.iat, cfg.max_token_age_hours)?;
        if let Some(ref scope) = data.claims.scope {
            if scope
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|s| {
                    matches!(
                        s,
                        VISA_COLLECTOR | VISA_ANALYST | VISA_SYNC_OPERATOR | VISA_ADMIN
                    )
                })
            {
                return Ok(scope_to_passport_claims(&data.claims, token));
            }
        }
        return Ok(AuthClaims::Jwt {
            sub: data.claims.sub.unwrap_or_default(),
            iss: data.claims.iss,
            exp: data.claims.exp.unwrap_or(0),
            jti: data.claims.jti,
            scope: data.claims.scope,
            raw_token: Some(token.to_string()),
        });
    }

    Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into())
}

struct CachedJwks {
    fetched_at: Instant,
    set: jsonwebtoken::jwk::JwkSet,
}

static JWKS_CACHE: LazyLock<Mutex<HashMap<String, CachedJwks>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn resolve_jwks_path(jwks_file: &str) -> std::path::PathBuf {
    jwks_file
        .strip_prefix("file://")
        .unwrap_or(jwks_file)
        .into()
}

fn load_jwks_from_file(
    path: &std::path::Path,
) -> Result<jsonwebtoken::jwk::JwkSet, jsonwebtoken::errors::Error> {
    let raw =
        std::fs::read_to_string(path).map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    serde_json::from_value(value).map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken.into())
}

async fn fetch_jwks_cached(
    cfg: &AuthMiddlewareConfig,
    force_refresh: bool,
) -> Result<jsonwebtoken::jwk::JwkSet, jsonwebtoken::errors::Error> {
    let cache_key = cfg
        .jwks_file
        .clone()
        .or_else(|| cfg.jwks_url.clone())
        .unwrap_or_default();

    if force_refresh {
        if let Ok(mut cache) = JWKS_CACHE.lock() {
            cache.remove(&cache_key);
        }
    } else if let Ok(cache) = JWKS_CACHE.lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.fetched_at.elapsed() < cfg.jwks_cache_ttl {
                return Ok(entry.set.clone());
            }
        }
    }

    let set = if let Some(ref file) = cfg.jwks_file {
        load_jwks_from_file(&resolve_jwks_path(file))?
    } else {
        let jwks_url = cfg
            .jwks_url
            .as_deref()
            .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
        let client = reqwest::Client::new();
        let mut last_err = None;
        let mut jwks_value = None;
        for attempt in 0..3 {
            match client
                .get(jwks_url)
                .timeout(Duration::from_secs(15))
                .send()
                .await
            {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(value) => {
                        jwks_value = Some(value);
                        break;
                    }
                    Err(e) => last_err = Some(format!("JWKS JSON decode: {e}")),
                },
                Err(e) => {
                    last_err = Some(e.to_string());
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
                    }
                }
            }
        }
        let jwks_value = jwks_value.ok_or_else(|| {
            tracing::warn!(
                jwks_url = %jwks_url,
                error = last_err.as_deref().unwrap_or("unknown"),
                "JWKS fetch failed after retries"
            );
            jsonwebtoken::errors::ErrorKind::InvalidToken
        })?;
        serde_json::from_value(jwks_value)
            .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?
    };

    if let Ok(mut cache) = JWKS_CACHE.lock() {
        cache.insert(
            cache_key,
            CachedJwks {
                fetched_at: Instant::now(),
                set: set.clone(),
            },
        );
    }

    Ok(set)
}

/// Pre-fetch JWKS at gateway startup so the first authenticated request avoids cold-start latency.
pub async fn warm_jwks_cache(cfg: &AuthMiddlewareConfig) {
    if cfg.jwks_url.is_none() && cfg.jwks_file.is_none() {
        return;
    }
    match fetch_jwks_cached(cfg, false).await {
        Ok(set) => tracing::info!(
            keys = set.keys.len(),
            jwks_url = ?cfg.jwks_url,
            "JWKS cache warmed"
        ),
        Err(e) => tracing::warn!(
            error = %e,
            jwks_url = ?cfg.jwks_url,
            "JWKS cache warm failed"
        ),
    }
}

fn peek_jwt_claim_string(token: &str, claim: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;
    value
        .get(claim)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn apply_audience(validation: &mut Validation, cfg: &AuthMiddlewareConfig) {
    if let Some(ref aud) = cfg.audience {
        validation.set_audience(&[aud.as_str()]);
        validation.validate_aud = true;
    } else {
        validation.validate_aud = false;
    }
}

fn passport_decode_validation(alg: Algorithm, cfg: &AuthMiddlewareConfig) -> Validation {
    let mut validation = Validation::new(alg);
    validation.validate_exp = true;
    validation.algorithms = vec![Algorithm::RS256, Algorithm::ES256];
    if let Some(iss) = cfg.issuer.as_deref() {
        validation.set_issuer(&[iss.trim_end_matches('/')]);
    }
    apply_audience(&mut validation, cfg);
    validation
}

fn map_passport_decode_error(
    err: jsonwebtoken::errors::Error,
    token: &str,
    cfg: &AuthMiddlewareConfig,
    kid: &str,
    stage: &str,
) -> jsonwebtoken::errors::Error {
    tracing::warn!(
        stage = stage,
        kind = ?err.kind(),
        error = %err,
        kid = %kid,
        token_iss = peek_jwt_claim_string(token, "iss").as_deref().unwrap_or(""),
        configured_iss = cfg.issuer.as_deref().unwrap_or(""),
        jwks_url = ?cfg.jwks_url,
        "Passport JWT decode failed"
    );
    err
}

#[cfg(feature = "clearinghouse")]
static CLEARINGHOUSE_CACHE: LazyLock<
    Mutex<HashMap<String, Arc<ga4gh_clearinghouse::Clearinghouse>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(feature = "clearinghouse")]
async fn clearinghouse_for_cfg(
    cfg: &AuthMiddlewareConfig,
) -> Result<Arc<ga4gh_clearinghouse::Clearinghouse>, jsonwebtoken::errors::Error> {
    use ga4gh_clearinghouse::{Clearinghouse, ClearinghouseConfig, TrustedBroker};

    let issuer = cfg
        .issuer
        .clone()
        .or_else(|| std::env::var("FERRUM_AUTH__ISSUER").ok())
        .filter(|s| !s.is_empty())
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let jwks_url = cfg
        .jwks_url
        .clone()
        .or_else(|| std::env::var("FERRUM_AUTH__JWKS_URL").ok())
        .filter(|s| !s.is_empty())
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    if !jwks_url.starts_with("http://") && !jwks_url.starts_with("https://") {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    let cache_key = format!("{}|{}", issuer.trim_end_matches('/'), jwks_url);
    if let Ok(cache) = CLEARINGHOUSE_CACHE.lock() {
        if let Some(existing) = cache.get(&cache_key) {
            return Ok(Arc::clone(existing));
        }
    }

    let clearinghouse = Clearinghouse::new(ClearinghouseConfig::new(
        vec![TrustedBroker::new(
            issuer.trim_end_matches('/').to_string(),
            jwks_url,
        )],
        cfg.jwks_cache_ttl,
    ))
    .await
    .map_err(|err| {
        tracing::warn!(error = %err, "failed to initialize ga4gh-clearinghouse");
        jsonwebtoken::errors::ErrorKind::InvalidToken
    })?;
    let clearinghouse = Arc::new(clearinghouse);
    if let Ok(mut cache) = CLEARINGHOUSE_CACHE.lock() {
        cache.insert(cache_key, Arc::clone(&clearinghouse));
    }
    Ok(clearinghouse)
}

#[cfg(feature = "clearinghouse")]
async fn decode_passport_via_clearinghouse(
    token: &str,
    cfg: &AuthMiddlewareConfig,
) -> Result<AuthClaims, jsonwebtoken::errors::Error> {
    let clearinghouse = clearinghouse_for_cfg(cfg).await?;
    let passport = clearinghouse
        .validate_passport(token)
        .await
        .map_err(|err| {
            tracing::warn!(
                error = %err,
                token_iss = peek_jwt_claim_string(token, "iss").as_deref().unwrap_or(""),
                configured_iss = cfg.issuer.as_deref().unwrap_or(""),
                jwks_url = ?cfg.jwks_url,
                "clearinghouse passport validation failed"
            );
            jsonwebtoken::errors::ErrorKind::InvalidToken
        })?;
    let visa_objects = match clearinghouse.extract_visas(&passport).await {
        Ok(visas) => visas
            .into_iter()
            .map(|visa| VisaObject {
                r#type: visa.claim.r#type.to_string(),
                asserted: visa.claim.asserted,
                value: visa.claim.value,
                source: visa.claim.source,
                conditions: visa
                    .claim
                    .conditions
                    .as_ref()
                    .and_then(|c| serde_json::to_value(c).ok())
                    .map(|v| vec![v]),
                by: visa
                    .claim
                    .by
                    .as_ref()
                    .and_then(|b| serde_json::to_value(b).ok())
                    .and_then(|v| v.as_str().map(str::to_string)),
            })
            .collect(),
        Err(err) => {
            tracing::warn!(
                error = %err,
                embedded_visa_count = passport.visa_jwts.len(),
                "clearinghouse visa extract failed; refusing unverified embedded visas"
            );
            Vec::new()
        }
    };
    Ok(AuthClaims::Passport {
        claims: PassportClaims {
            sub: Some(passport.sub),
            iss: Some(passport.iss),
            exp: Some(passport.exp),
            iat: Some(passport.iat),
            jti: Some(passport.jti),
            ga4gh_passport_v1: Some(passport.visa_jwts),
            scope: passport.scope,
            aud: passport.aud,
        },
        visas: visa_objects,
        raw_token: Some(token.to_string()),
    })
}

fn scope_to_passport_claims(claims: &PassportClaims, token: &str) -> AuthClaims {
    let visas = claims
        .scope
        .as_deref()
        .unwrap_or("")
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| s.starts_with("ferrum:"))
        .map(|value| VisaObject {
            r#type: "AffiliationAndRole".to_string(),
            asserted: claims.iat.unwrap_or(0),
            value: value.to_string(),
            source: claims
                .iss
                .clone()
                .unwrap_or_else(|| "ferrum-edge-local".into()),
            conditions: None,
            by: claims.sub.clone(),
        })
        .collect();
    AuthClaims::Passport {
        claims: claims.clone(),
        visas,
        raw_token: Some(token.to_string()),
    }
}

async fn decode_passport_jwt(
    token: &str,
    cfg: &AuthMiddlewareConfig,
) -> Result<PassportClaims, jsonwebtoken::errors::Error> {
    let decoded_header = jsonwebtoken::decode_header(token)?;

    // OWASP A02: pin to RS256/ES256 for Passport; never HS256 or None.
    let alg = decoded_header.alg;
    if alg != Algorithm::RS256 && alg != Algorithm::ES256 {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into());
    }

    let _jwks_ref = cfg
        .jwks_url
        .as_deref()
        .or(cfg.jwks_file.as_deref())
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let kid = decoded_header.kid.unwrap_or_default();
    let validation = passport_decode_validation(alg, cfg);

    for attempt in 0..2 {
        let force_refresh = attempt > 0;
        let set = fetch_jwks_cached(cfg, force_refresh).await?;
        let jwk = if !kid.is_empty() {
            set.find(&kid)
        } else {
            set.keys.first()
        };
        let Some(jwk) = jwk else {
            if force_refresh || cfg.jwks_file.is_some() {
                return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
            }
            continue;
        };

        let key = match jsonwebtoken::DecodingKey::from_jwk(jwk) {
            Ok(key) => key,
            Err(err) => {
                return Err(map_passport_decode_error(
                    err,
                    token,
                    cfg,
                    &kid,
                    "decoding_key_from_jwk",
                ));
            }
        };

        match jsonwebtoken::decode::<PassportClaims>(token, &key, &validation) {
            Ok(data) => return Ok(data.claims),
            Err(err) if attempt == 0 && should_refresh_jwks_after_decode(&err, cfg) => {
                tracing::info!(
                    kind = ?err.kind(),
                    kid = %kid,
                    jwks_url = ?cfg.jwks_url,
                    "Passport JWT verify failed; refreshing JWKS and retrying"
                );
            }
            Err(err) => {
                return Err(map_passport_decode_error(err, token, cfg, &kid, "decode"));
            }
        }
    }

    Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into())
}

fn should_refresh_jwks_after_decode(
    err: &jsonwebtoken::errors::Error,
    cfg: &AuthMiddlewareConfig,
) -> bool {
    if cfg.jwks_file.is_some() {
        return false;
    }
    matches!(
        err.kind(),
        jsonwebtoken::errors::ErrorKind::InvalidSignature
            | jsonwebtoken::errors::ErrorKind::InvalidKeyFormat
    )
}

async fn decode_passport_visas_clearinghouse(
    passport_jwt: &str,
    visa_jwts: &[String],
) -> Vec<VisaObject> {
    #[cfg(feature = "clearinghouse")]
    {
        let _ = visa_jwts;
        use std::time::Duration;

        use ga4gh_clearinghouse::{Clearinghouse, ClearinghouseConfig, TrustedBroker};

        let jwks_url = std::env::var("FERRUM_AUTH__JWKS_URL").ok();
        let issuer = std::env::var("FERRUM_AUTH__ISSUER").ok();

        let trusted = match (issuer, jwks_url) {
            (Some(iss), Some(jwks)) if !iss.is_empty() && !jwks.is_empty() => {
                vec![TrustedBroker::new(iss, jwks)]
            }
            _ => Vec::new(),
        };

        if trusted.is_empty() {
            tracing::warn!(
                "clearinghouse enabled but FERRUM_AUTH__ISSUER/JWKS_URL missing; refusing unverified visas"
            );
            return Vec::new();
        }

        let result = async {
            let clearinghouse =
                Clearinghouse::new(ClearinghouseConfig::new(trusted, Duration::from_secs(300)))
                    .await
                    .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;

            let passport = clearinghouse
                .validate_passport(passport_jwt)
                .await
                .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
            clearinghouse
                .extract_visas(&passport)
                .await
                .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)
        }
        .await;

        match result {
            Ok(visas) => visas
                .into_iter()
                .map(|visa| VisaObject {
                    r#type: visa.claim.r#type.to_string(),
                    asserted: visa.claim.asserted,
                    value: visa.claim.value,
                    source: visa.claim.source,
                    conditions: visa
                        .claim
                        .conditions
                        .as_ref()
                        .and_then(|c| serde_json::to_value(c).ok())
                        .map(|v| vec![v]),
                    by: visa
                        .claim
                        .by
                        .as_ref()
                        .and_then(|b| serde_json::to_value(b).ok())
                        .and_then(|v| v.as_str().map(str::to_string)),
                })
                .collect(),
            Err(_) => {
                tracing::warn!("clearinghouse visa extract failed; refusing unverified visas");
                Vec::new()
            }
        }
    }

    #[cfg(not(feature = "clearinghouse"))]
    {
        let _ = passport_jwt;
        let _ = visa_jwts;
        tracing::warn!(
            "clearinghouse requested but crate built without the feature; refusing unverified visas"
        );
        Vec::new()
    }
}

async fn decode_passport_visas_verified(
    visa_jwts: &[String],
    cfg: &AuthMiddlewareConfig,
) -> Vec<VisaObject> {
    let mut out = Vec::new();
    for token in visa_jwts {
        match verify_visa_jwt(token, cfg).await {
            Ok(Some(v)) => out.push(v),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "dropping visa JWT that failed signature verification");
            }
        }
    }
    out
}

async fn verify_visa_jwt(
    token: &str,
    cfg: &AuthMiddlewareConfig,
) -> Result<Option<VisaObject>, jsonwebtoken::errors::Error> {
    let header = jsonwebtoken::decode_header(token)?;
    let alg = header.alg;
    if alg == Algorithm::HS256 {
        let secret = cfg
            .jwt_secret
            .as_deref()
            .ok_or(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm)?;
        let mut validation = Validation::new(Algorithm::HS256);
        validation.algorithms = vec![Algorithm::HS256];
        validation.validate_exp = true;
        let data = decode::<VisaJwtPayload>(token, &DecodingKey::from_secret(secret), &validation)?;
        return Ok(data.claims.ga4gh_visa_v1);
    }
    if alg != Algorithm::RS256 && alg != Algorithm::ES256 {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into());
    }
    if cfg.jwks_url.is_none() && cfg.jwks_file.is_none() {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }
    let kid = header.kid.unwrap_or_default();
    // Visa issuer is often not the Passport broker; do not pin `iss` to cfg.issuer.
    let mut validation = Validation::new(alg);
    validation.validate_exp = true;
    validation.validate_aud = false;
    validation.algorithms = vec![Algorithm::RS256, Algorithm::ES256];

    for attempt in 0..2 {
        let set = fetch_jwks_cached(cfg, attempt > 0).await?;
        let jwk = if !kid.is_empty() {
            set.find(&kid)
        } else {
            set.keys.first()
        };
        let Some(jwk) = jwk else {
            continue;
        };
        let key = DecodingKey::from_jwk(jwk)?;
        match decode::<VisaJwtPayload>(token, &key, &validation) {
            Ok(data) => return Ok(data.claims.ga4gh_visa_v1),
            Err(e) if attempt == 0 && should_refresh_jwks_after_decode(&e, cfg) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into())
}

#[derive(Debug, Deserialize)]
struct VisaJwtPayload {
    #[serde(rename = "ga4gh_visa_v1")]
    ga4gh_visa_v1: Option<VisaObject>,
}

/// Tower-compatible auth layer.
pub fn auth_layer(config: Option<Arc<AuthMiddlewareConfig>>) -> impl Clone {
    axum::middleware::from_fn::<_, axum::body::Body>(move |req: Request, next: Next| {
        let config = config.clone();
        Box::pin(async move {
            let mut req = req;
            if let Some(cfg) = config {
                req.extensions_mut().insert(cfg);
            }
            auth_middleware(req, next).await
        })
    })
}

#[cfg(test)]
mod published_access_tests {
    use super::{AuthClaims, PassportClaims, VisaObject};

    fn passport_with_grant(value: &str) -> AuthClaims {
        AuthClaims::Passport {
            claims: PassportClaims {
                sub: Some("researcher@example.org".to_string()),
                iss: None,
                exp: None,
                iat: None,
                jti: None,
                ga4gh_passport_v1: None,
                scope: None,
                aud: None,
            },
            visas: vec![VisaObject {
                r#type: "ControlledAccessGrants".to_string(),
                asserted: 0,
                value: value.to_string(),
                source: "ads".to_string(),
                conditions: None,
                by: None,
            }],
            raw_token: None,
        }
    }

    #[test]
    fn published_access_matches_ads_uuid_or_drs_scope() {
        let ads_id = "550e8400-e29b-41d4-a716-446655440000";
        let object_id = "obj-abc";
        let by_uuid = passport_with_grant(ads_id);
        assert!(by_uuid.has_published_dataset_access(ads_id, object_id));

        let by_drs = passport_with_grant("drs:obj-abc");
        assert!(by_drs.has_published_dataset_access(ads_id, object_id));

        let no_grant = passport_with_grant("other-dataset");
        assert!(!no_grant.has_published_dataset_access(ads_id, object_id));
    }

    #[test]
    fn collector_scope_allows_ingest() {
        let claims = AuthClaims::Jwt {
            sub: "alice".into(),
            iss: Some("ferrum-edge-local".into()),
            exp: 0,
            jti: None,
            scope: Some("ferrum:collector".into()),
            raw_token: None,
        };
        assert!(claims.can_ingest());
        assert!(!claims.can_sync());
    }

    #[test]
    fn require_auth_enabled_reads_env() {
        let prev = std::env::var("FERRUM_AUTH__REQUIRE_AUTH").ok();
        let prev_runtime = super::REQUIRE_AUTH_RUNTIME.load(std::sync::atomic::Ordering::Relaxed);
        std::env::remove_var("FERRUM_AUTH__REQUIRE_AUTH");
        super::set_require_auth_runtime(true);
        assert!(super::require_auth_enabled());
        std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "true");
        assert!(super::require_auth_enabled());
        std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "1");
        assert!(super::require_auth_enabled());
        std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
        assert!(!super::require_auth_enabled());
        super::set_require_auth_runtime(prev_runtime);
        match prev {
            Some(v) => std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", v),
            None => std::env::remove_var("FERRUM_AUTH__REQUIRE_AUTH"),
        }
    }

    #[test]
    fn admin_visa_is_exact_match_not_substring() {
        let claims = passport_with_grant("prefix-ferrum:admin-suffix");
        assert!(!claims.is_admin());
        let admin = AuthClaims::Passport {
            claims: PassportClaims {
                sub: Some("admin@example.org".to_string()),
                iss: None,
                exp: None,
                iat: None,
                jti: None,
                ga4gh_passport_v1: None,
                scope: None,
                aud: None,
            },
            visas: vec![VisaObject {
                r#type: "AffiliationAndRole".to_string(),
                asserted: 0,
                value: super::VISA_ADMIN.to_string(),
                source: "test".to_string(),
                conditions: None,
                by: None,
            }],
            raw_token: None,
        };
        assert!(admin.is_admin());
    }

    #[test]
    fn cookie_name_is_exact_not_prefix() {
        use axum::http::Request;
        let req = Request::builder()
            .header("Cookie", "ferrum_token_evil=bad; ferrum_token=good")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            super::extract_token_from_cookie(&req, "ferrum_token").as_deref(),
            Some("good")
        );
        let prefix_only = Request::builder()
            .header("Cookie", "ferrum_token_evil=bad")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            super::extract_token_from_cookie(&prefix_only, "ferrum_token"),
            None
        );
    }

    #[test]
    fn sync_operator_scope_allows_sync() {
        let claims = AuthClaims::Jwt {
            sub: "bob".into(),
            iss: None,
            exp: 0,
            jti: None,
            scope: Some("ferrum:sync_operator".into()),
            raw_token: None,
        };
        assert!(claims.can_sync());
    }
}
