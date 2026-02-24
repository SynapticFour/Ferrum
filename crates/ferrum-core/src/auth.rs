//! Auth middleware: JWT validation (jsonwebtoken), GA4GH Passport extraction, Bearer + cookie. A07: revocation check.

use async_trait::async_trait;
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

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
}

/// Claims stored in request extensions (set by auth middleware).
#[derive(Debug, Clone)]
pub enum AuthClaims {
    /// Standard JWT claims (e.g. from access token).
    Jwt { sub: String, iss: Option<String>, exp: i64, jti: Option<String> },
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

    /// True if the token has ControlledAccessGrants visa for the given dataset (DRS access control).
    pub fn has_dataset_grant(&self, dataset_id: &str) -> bool {
        match self {
            AuthClaims::Jwt { .. } => false,
            AuthClaims::Passport { visas, .. } => visas.iter().any(|v| {
                (v.r#type == "ControlledAccessGrants" || v.r#type.contains("ControlledAccessGrants"))
                    && v.value == dataset_id
            }),
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
    /// A07: Max token age in hours (reject if iat too old). 0 = disable.
    pub max_token_age_hours: u32,
    /// A07: If set, token with matching jti is rejected (revoked).
    pub revocation_check: Option<Arc<dyn RevocationCheck + Send + Sync>>,
}

impl AuthMiddlewareConfig {
    pub fn from_crate_config(cfg: &crate::config::AuthConfig) -> Self {
        Self {
            jwt_secret: cfg.jwt_secret.as_deref().map(|s| s.as_bytes().to_vec()),
            issuer: cfg.issuer.clone(),
            jwks_url: cfg.jwks_url.clone(),
            passport_endpoints: cfg.passport_endpoints.clone(),
            max_token_age_hours: cfg.max_token_age_hours,
            revocation_check: None,
        }
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
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    let config = request
        .extensions()
        .get::<Arc<AuthMiddlewareConfig>>()
        .cloned();

    let token = extract_token(&request)
        .or_else(|| extract_token_from_cookie(&request, "ferrum_token"));

    let mut request = request;

    if let Some(token) = token {
        if let Some(ref cfg) = config {
            if let Ok(claims) = decode_jwt_or_passport(&token, cfg) {
                let insert = if let (Some(jti), Some(check)) = (claims.jti(), cfg.revocation_check.as_ref()) {
                    !check.is_revoked(jti).await
                } else {
                    true
                };
                if insert {
                    request.extensions_mut().insert(claims);
                }
            }
        } else {
            // No config: try default HS256 with no issuer check
            if let Ok(claims) = decode_jwt_fallback(&token) {
                request.extensions_mut().insert(claims);
            }
        }
    }

    next.run(request).await
}

/// A07: Reject token if issued more than max_hours ago. 0 = skip check.
fn reject_token_if_too_old(iat: Option<i64>, max_hours: u32) -> Result<(), jsonwebtoken::errors::Error> {
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
fn decode_jwt_or_passport(token: &str, cfg: &AuthMiddlewareConfig) -> Result<AuthClaims, jsonwebtoken::errors::Error> {
    // Try as GA4GH Passport (has ga4gh_passport_v1 claim)
    if let Ok(claims) = decode_passport_jwt(token, cfg) {
        reject_token_if_too_old(claims.iat, cfg.max_token_age_hours)?;
        return Ok(AuthClaims::Passport {
            claims: claims.clone(),
            visas: decode_passport_visas(claims.ga4gh_passport_v1.as_deref().unwrap_or(&[])),
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
    })
}

fn decode_passport_jwt(token: &str, _cfg: &AuthMiddlewareConfig) -> Result<PassportClaims, jsonwebtoken::errors::Error> {
    let decoded = jsonwebtoken::decode_header(token)?;
    // OWASP A02: pin to RS256/ES256 for Passport; never HS256 or None
    let alg = decoded.alg;
    if alg != Algorithm::RS256 && alg != Algorithm::ES256 {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into());
    }
    let key = DecodingKey::from_secret(b""); // TODO: use JWKS from cfg.jwks_url for RS256
    let mut validation = Validation::new(alg);
    validation.validate_exp = true;
    validation.algorithms = vec![Algorithm::RS256, Algorithm::ES256];
    let data = jsonwebtoken::decode::<PassportClaims>(token, &key, &validation)?;
    Ok(data.claims)
}

fn decode_passport_visas(visa_jwts: &[String]) -> Vec<VisaObject> {
    let mut out = Vec::new();
    for s in visa_jwts {
        if let Ok(decoded) = jsonwebtoken::decode_header(s) {
            if decoded.alg != Algorithm::RS256 && decoded.alg != Algorithm::ES256 {
                continue;
            }
            let key = jsonwebtoken::DecodingKey::from_secret(b"");
            let mut val = jsonwebtoken::Validation::new(decoded.alg);
            val.algorithms = vec![Algorithm::RS256, Algorithm::ES256];
            if let Ok(data) = jsonwebtoken::decode::<VisaJwtPayload>(s, &key, &val) {
                if let Some(v) = data.claims.ga4gh_visa_v1 {
                    out.push(v);
                }
            }
        }
    }
    out
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
