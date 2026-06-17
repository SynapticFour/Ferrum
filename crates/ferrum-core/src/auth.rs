//! Auth middleware: JWT validation (jsonwebtoken), GA4GH Passport extraction, Bearer + cookie. A07: revocation check.

use async_trait::async_trait;
use axum::{extract::Request, middleware::Next, response::Response};
use base64::Engine;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

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
    },
    /// GA4GH Passport with decoded passport claims and optional decoded visas.
    Passport {
        claims: PassportClaims,
        visas: Vec<VisaObject>,
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
            AuthClaims::Passport { visas, .. } => visas
                .iter()
                .any(|v| v.value == "ferrum:admin" || v.value.contains("ferrum:admin")),
        }
    }

    /// True if the token has the ferrum:outbreak_activator role (Passport visa).
    pub fn is_outbreak_activator(&self) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| {
                v.value == "ferrum:outbreak_activator" || v.value.contains("outbreak_activator")
            }),
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
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| {
                (v.r#type == "ControlledAccessGrants"
                    || v.r#type.contains("ControlledAccessGrants"))
                    && v.value == dataset_id
            }),
        }
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
        let row: Option<(bool,)> = sqlx::query_as("SELECT true FROM revoked_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .and_then(|r| r);
        row.is_some()
    }
}

/// Auth config used by the middleware (from FerrumConfig).
#[derive(Clone)]
pub struct AuthMiddlewareConfig {
    pub jwt_secret: Option<Vec<u8>>,
    pub issuer: Option<String>,
    pub jwks_url: Option<String>,
    pub passport_endpoints: Vec<String>,
    /// When false, requests without a token get synthetic "demo-user" claims (for demo mode).
    pub require_auth: bool,
    /// A07: Max token age in hours (reject if iat too old). 0 = disable.
    pub max_token_age_hours: u32,
    /// A07: If set, token with matching jti is rejected (revoked).
    pub revocation_check: Option<Arc<dyn RevocationCheck + Send + Sync>>,
    /// When true, Passport visas are verified via ga4gh-clearinghouse (requires `clearinghouse` feature).
    pub use_clearinghouse: bool,
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
        Self {
            jwt_secret: cfg.jwt_secret.as_deref().map(|s| s.as_bytes().to_vec()),
            issuer,
            jwks_url,
            passport_endpoints: cfg.passport_endpoints.clone(),
            require_auth: cfg.require_auth,
            max_token_age_hours: cfg.max_token_age_hours,
            revocation_check: None,
            use_clearinghouse: cfg.clearinghouse || cfg.is_external(),
        }
    }

    /// Config for demo/unauthenticated mode: no JWT required, inject "demo-user" when no token present.
    pub fn demo() -> Self {
        Self {
            jwt_secret: None,
            issuer: None,
            jwks_url: None,
            passport_endpoints: Vec::new(),
            require_auth: false,
            max_token_age_hours: 24,
            revocation_check: None,
            use_clearinghouse: false,
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
            passport_endpoints: Vec::new(),
            require_auth: true,
            max_token_age_hours: 0,
            revocation_check: None,
            use_clearinghouse: false,
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
        if part.starts_with(cookie_name) {
            let rest = part.strip_prefix(cookie_name)?.trim_start_matches('=');
            return Some(rest.to_string());
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
                    let insert = if let (Some(jti), Some(check)) =
                        (claims.jti(), cfg.revocation_check.as_ref())
                    {
                        !check.is_revoked(jti).await
                    } else {
                        true
                    };
                    if insert {
                        request.extensions_mut().insert(claims);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        jwks_url = ?cfg.jwks_url,
                        "JWT/Passport validation failed"
                    );
                }
            }
        } else {
            // No config: try default HS256 with no issuer check
            if let Ok(claims) = decode_jwt_fallback(&token) {
                request.extensions_mut().insert(claims);
            }
        }
    }

    // Demo mode: when no claims were set, inject demo-user if auth is not required (config absent = treat as demo; config.require_auth false = demo).
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
    // Try as GA4GH Passport (has ga4gh_passport_v1 claim)
    if let Ok(claims) = decode_passport_jwt(token, cfg).await {
        reject_token_if_too_old(claims.iat, cfg.max_token_age_hours)?;
        let visa_jwts = claims.ga4gh_passport_v1.as_deref().unwrap_or(&[]);
        let visas = if cfg.use_clearinghouse {
            decode_passport_visas_clearinghouse(token, visa_jwts)
        } else {
            decode_passport_visas(visa_jwts)
        };
        return Ok(AuthClaims::Passport {
            claims: claims.clone(),
            visas,
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
        let data = decode::<PassportClaims>(token, &key, &validation)?;
        reject_token_if_too_old(data.claims.iat, cfg.max_token_age_hours)?;
        return Ok(AuthClaims::Jwt {
            sub: data.claims.sub.unwrap_or_default(),
            iss: data.claims.iss,
            exp: data.claims.exp.unwrap_or(0),
            jti: data.claims.jti,
            scope: data.claims.scope,
        });
    }

    Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into())
}

fn decode_jwt_fallback(token: &str) -> Result<AuthClaims, jsonwebtoken::errors::Error> {
    let decoded = jsonwebtoken::decode_header(token)?;
    // OWASP A02: only allow HS256 in fallback; never accept Algorithm::None or algorithm confusion
    if decoded.alg != Algorithm::HS256 {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into());
    }
    let claims = jsonwebtoken::decode::<PassportClaims>(
        token,
        &DecodingKey::from_secret(b""),
        &Validation::new(Algorithm::HS256),
    )?;
    reject_token_if_too_old(claims.claims.iat, 24)?; // A07: default 24h when no config
    Ok(AuthClaims::Jwt {
        sub: claims.claims.sub.unwrap_or_default(),
        iss: claims.claims.iss,
        exp: claims.claims.exp.unwrap_or(0),
        jti: claims.claims.jti,
        scope: claims.claims.scope,
    })
}

struct CachedJwks {
    fetched_at: Instant,
    set: jsonwebtoken::jwk::JwkSet,
}

static JWKS_CACHE: LazyLock<Mutex<HashMap<String, CachedJwks>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const JWKS_CACHE_TTL: Duration = Duration::from_secs(300);

async fn fetch_jwks_cached(
    jwks_url: &str,
) -> Result<jsonwebtoken::jwk::JwkSet, jsonwebtoken::errors::Error> {
    if let Ok(cache) = JWKS_CACHE.lock() {
        if let Some(entry) = cache.get(jwks_url) {
            if entry.fetched_at.elapsed() < JWKS_CACHE_TTL {
                return Ok(entry.set.clone());
            }
        }
    }

    let jwks_value = reqwest::Client::new()
        .get(jwks_url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(jwks_url = %jwks_url, error = %e, "JWKS fetch failed");
            jsonwebtoken::errors::ErrorKind::InvalidToken
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let set: jsonwebtoken::jwk::JwkSet = serde_json::from_value(jwks_value)
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    if let Ok(mut cache) = JWKS_CACHE.lock() {
        cache.insert(
            jwks_url.to_string(),
            CachedJwks {
                fetched_at: Instant::now(),
                set: set.clone(),
            },
        );
    }

    Ok(set)
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

    let jwks_url = cfg
        .jwks_url
        .as_deref()
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let set = fetch_jwks_cached(jwks_url).await?;
    let kid = decoded_header.kid.unwrap_or_default();

    let jwk = if !kid.is_empty() {
        set.find(&kid)
    } else {
        set.keys.first()
    }
    .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let key = jsonwebtoken::DecodingKey::from_jwk(jwk)
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let mut validation = Validation::new(alg);
    validation.validate_exp = true;
    validation.algorithms = vec![Algorithm::RS256, Algorithm::ES256];
    if let Some(ref iss) = cfg.issuer {
        validation.set_issuer(&[iss.as_str()]);
    }

    let data = jsonwebtoken::decode::<PassportClaims>(token, &key, &validation)
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    Ok(data.claims)
}

fn decode_passport_visas_clearinghouse(
    passport_jwt: &str,
    visa_jwts: &[String],
) -> Vec<VisaObject> {
    #[cfg(feature = "clearinghouse")]
    {
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
                "clearinghouse enabled but FERRUM_AUTH__ISSUER/JWKS_URL missing; falling back to unverified visa parse"
            );
            return decode_passport_visas(visa_jwts);
        }

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
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
            })
        });

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
            Err(_) => decode_passport_visas(visa_jwts),
        }
    }

    #[cfg(not(feature = "clearinghouse"))]
    {
        let _ = passport_jwt;
        decode_passport_visas(visa_jwts)
    }
}

fn decode_passport_visas(visa_jwts: &[String]) -> Vec<VisaObject> {
    let mut out = Vec::new();
    for s in visa_jwts {
        if let Some(v) = parse_visa_claim_without_verification(s) {
            out.push(v);
        }
    }
    out
}

/// Parse ga4gh_visa_v1 from a compact JWT payload without signature verification.
///
/// Security model: Passport JWT signature is verified upstream (`decode_passport_jwt`) before
/// its embedded visa JWT strings are accepted. Here we extract visa claims to avoid invalid
/// RS/ES verification with placeholder keys.
fn parse_visa_claim_without_verification(token: &str) -> Option<VisaObject> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _sig = parts.next()?;

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: VisaJwtPayload = serde_json::from_slice(&payload_bytes).ok()?;
    claims.ga4gh_visa_v1
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
