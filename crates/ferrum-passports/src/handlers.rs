//! OIDC discovery, token, userinfo, JWKS, and admin handlers.

use crate::config::PassportConfig;
use crate::error::{PassportError, Result};
use crate::keys::{default_access_token_header, default_passport_header, default_visa_header, SigningKeys};
use crate::repo::PassportRepo;
use crate::types::VisaObject;
use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::response::Redirect;
use axum::{Form, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

pub struct AppState {
    pub config: PassportConfig,
    pub keys: Arc<SigningKeys>,
    pub repo: Arc<PassportRepo>,
}

#[derive(Serialize, ToSchema)]
pub struct OidcConfiguration {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
}

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    pub response_type: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
}

#[derive(Deserialize)]
pub struct TokenForm {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub subject_token: Option<String>,
    pub subject_token_type: Option<String>,
    pub requested_token_type: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_token_type: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: Option<Vec<String>>,
    pub iat: i64,
    pub exp: i64,
    pub jti: Option<String>,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idp: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct PassportClaims {
    pub iss: String,
    pub sub: String,
    pub aud: Option<Vec<String>>,
    pub iat: i64,
    pub exp: i64,
    pub jti: Option<String>,
    #[serde(rename = "ga4gh_passport_v1")]
    pub ga4gh_passport_v1: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct VisaJwtClaims {
    pub iss: String,
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
    #[serde(rename = "ga4gh_visa_v1")]
    pub ga4gh_visa_v1: VisaObject,
}

#[derive(Serialize, ToSchema)]
pub struct UserinfoResponse {
    pub sub: String,
    #[serde(rename = "ga4gh_passport_v1", skip_serializing_if = "Option::is_none")]
    pub ga4gh_passport_v1: Option<Vec<String>>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateVisaGrantRequest {
    pub user_sub: String,
    pub user_iss: String,
    pub dataset_id: String,
    pub visa_type: String,
    pub value: String,
    pub source: String,
    #[serde(default)]
    pub conditions: Option<serde_json::Value>,
    /// ISO8601/RFC3339 datetime string, e.g. "2025-12-31T23:59:59Z"
    pub expires_at: Option<String>,
}

#[utoipa::path(get, path = "/.well-known/openid-configuration", responses((status = 200, body = OidcConfiguration)))]
pub async fn get_oidc_configuration(State(state): State<Arc<AppState>>) -> Json<OidcConfiguration> {
    let c = &state.config;
    Json(OidcConfiguration {
        issuer: c.issuer_base_url.trim_end_matches('/').to_string(),
        authorization_endpoint: c.authorization_endpoint(),
        token_endpoint: c.token_endpoint(),
        userinfo_endpoint: c.userinfo_endpoint(),
        jwks_uri: c.jwks_uri(),
        scopes_supported: vec!["openid".to_string(), "ga4gh_passport_v1".to_string()],
        response_types_supported: vec!["code".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["RS256".to_string()],
    })
}

/// GET /authorize - redirect to login or external IdP. For MVP we redirect to a simple login page (same origin) that posts back with code.
pub async fn authorize(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthorizeQuery>,
) -> Result<Redirect> {
    let redirect_uri = q.redirect_uri.as_deref().unwrap_or("http://localhost:8080/callback");
    let client_id = q.client_id.as_deref().unwrap_or("ferrum");
    let scope = q.scope.as_deref().unwrap_or("openid ga4gh_passport_v1");
    let state_param = q.state.as_deref().unwrap_or("");
    if let Some(ref auth_url) = state.config.oidc_authorization_url {
        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            auth_url,
            urlencoding::encode(state.config.oidc_client_id.as_deref().unwrap_or("")),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(scope),
            urlencoding::encode(state_param)
        );
        return Ok(Redirect::to(&url));
    }
    let login_url = format!(
        "{}/login?redirect_uri={}&client_id={}&scope={}&state={}",
        state.config.issuer_base_url.trim_end_matches('/'),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(client_id),
        urlencoding::encode(scope),
        urlencoding::encode(state_param)
    );
    Ok(Redirect::to(&login_url))
}

/// POST /token - authorization_code grant and token exchange (subject_token_type=access_token, requested_token_type=passport).
pub async fn token(
    State(state): State<Arc<AppState>>,
    Form(form): Form<TokenForm>,
) -> Result<(axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 2], Json<TokenResponse>)> {
    let no_cache = [
        (CACHE_CONTROL, "no-cache, no-store"),
        (PRAGMA, "no-cache"),
    ];
    if form.grant_type == "urn:ietf:params:oauth:grant-type:token-exchange"
        && form.requested_token_type.as_deref() == Some("urn:ga4gh:params:oauth:token-type:passport")
        && form.subject_token_type.as_deref() == Some("urn:ietf:params:oauth:token-type:access_token")
    {
        let access_token = form
            .subject_token
            .as_deref()
            .ok_or_else(|| PassportError::Validation("subject_token required".into()))?;
        let public_pem = state.keys.public_key_pem()?;
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[state.config.issuer_base_url.trim_end_matches('/')]);
        validation.validate_exp = true;
        validation.required_spec_claims.clear();
        let decoded = jsonwebtoken::decode::<AccessTokenClaims>(
            access_token,
            &jsonwebtoken::DecodingKey::from_rsa_pem(public_pem.as_bytes())
                .map_err(|e| PassportError::Jwt(e.to_string()))?,
            &validation,
        )
        .map_err(|e| PassportError::Unauthorized(e.to_string()))?;
        let passport_jwt = build_passport_jwt(&state, &decoded.claims.sub, &decoded.claims.iss).await?;
        return Ok((
            axum::http::StatusCode::OK,
            no_cache,
            Json(TokenResponse {
                access_token: passport_jwt,
                token_type: "Bearer".to_string(),
                expires_in: 3600,
                refresh_token: None,
                scope: None,
                issued_token_type: Some("urn:ga4gh:params:oauth:token-type:passport".to_string()),
            }),
        ));
    }
    if form.grant_type == "authorization_code" {
        let code = form.code.as_deref().ok_or_else(|| PassportError::Validation("code required".into()))?;
        let (sub, iss, scope, _redirect_uri) = state
            .repo
            .consume_auth_code(code)
            .await?
            .ok_or_else(|| PassportError::Unauthorized("invalid or expired code".into()))?;
        let exp = (Utc::now() + chrono::Duration::seconds(3600)).timestamp();
        let iat = Utc::now().timestamp();
        let claims = AccessTokenClaims {
            iss: state.config.issuer_base_url.trim_end_matches('/').to_string(),
            sub: sub.clone(),
            aud: form.client_id.as_ref().map(|c| vec![c.clone()]),
            iat,
            exp,
            jti: Some(uuid::Uuid::new_v4().to_string()),
            scope,
            idp: Some(iss.clone()),
        };
        let token = state
            .keys
            .sign(&default_access_token_header(), &claims)
            .map_err(|e| PassportError::Jwt(e.to_string()))?;
        return Ok((
            axum::http::StatusCode::OK,
            no_cache,
            Json(TokenResponse {
                access_token: token,
                token_type: "Bearer".to_string(),
                expires_in: 3600,
                refresh_token: None,
                scope: Some(claims.scope),
                issued_token_type: None,
            }),
        ));
    }
    Err(PassportError::Validation("unsupported grant_type".into()))
}

async fn build_passport_jwt(state: &AppState, sub: &str, iss: &str) -> Result<String> {
    let visa_jwts = build_visa_jwts(state, sub, iss).await?;
    let issuer = state.config.issuer_base_url.trim_end_matches('/').to_string();
    let now = Utc::now();
    let iat = now.timestamp();
    let exp = (now + chrono::Duration::seconds(3600)).timestamp();
    let passport_claims = PassportClaims {
        iss: issuer,
        sub: sub.to_string(),
        aud: None,
        iat,
        exp,
        jti: Some(uuid::Uuid::new_v4().to_string()),
        ga4gh_passport_v1: visa_jwts,
    };
    state
        .keys
        .sign(&default_passport_header(), &passport_claims)
        .map_err(|e| PassportError::Jwt(e.to_string()))
}

/// GET /userinfo - requires Bearer access token; returns ga4gh_passport_v1 with visas.
pub async fn userinfo(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<(axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 2], Json<UserinfoResponse>)> {
    let no_cache = [
        (CACHE_CONTROL, "no-cache, no-store"),
        (PRAGMA, "no-cache"),
    ];
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .filter(|s| s.starts_with("Bearer "))
        .ok_or_else(|| PassportError::Unauthorized("missing or invalid Authorization".into()))?;
    let token = bearer.trim_start_matches("Bearer ");
    let public_pem = state.keys.public_key_pem()?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[state.config.issuer_base_url.trim_end_matches('/')]);
    validation.validate_exp = true;
    validation.required_spec_claims.clear();
    let decoded = jsonwebtoken::decode::<AccessTokenClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| PassportError::Jwt(e.to_string()))?,
        &validation,
    )
    .map_err(|_| PassportError::Unauthorized("invalid access token".into()))?;
    if !decoded.claims.scope.contains("ga4gh_passport_v1") {
        return Err(PassportError::Unauthorized("insufficient scope".into()));
    }
    let visa_jwts = build_visa_jwts(&state, &decoded.claims.sub, &decoded.claims.iss).await?;
    Ok((
        axum::http::StatusCode::OK,
        no_cache,
        Json(UserinfoResponse {
            sub: decoded.claims.sub,
            ga4gh_passport_v1: Some(visa_jwts),
        }),
    ))
}

async fn build_visa_jwts(state: &AppState, sub: &str, _iss: &str) -> Result<Vec<String>> {
    let grants = state.repo.list_visa_grants(sub, state.config.issuer_base_url.trim_end_matches('/')).await?;
    let issuer = state.config.issuer_base_url.trim_end_matches('/').to_string();
    let now = Utc::now();
    let iat = now.timestamp();
    let exp = (now + chrono::Duration::seconds(3600)).timestamp();
    let mut visas: Vec<String> = Vec::new();
    for g in &grants {
        let asserted = g.expires_at.map(|e| e.timestamp()).unwrap_or_else(|| iat);
        let visa_obj = VisaObject {
            r#type: g.visa_type.clone(),
            asserted,
            value: g.value.clone(),
            source: g.source.clone(),
            conditions: g.conditions.clone(),
        };
        let visa_claims = VisaJwtClaims {
            iss: issuer.clone(),
            sub: sub.to_string(),
            iat,
            exp: g.expires_at.map(|e| e.timestamp()).unwrap_or(exp),
            ga4gh_visa_v1: visa_obj,
        };
        let visa_jwt = state
            .keys
            .sign(&default_visa_header(), &visa_claims)
            .map_err(|e| PassportError::Jwt(e.to_string()))?;
        visas.push(visa_jwt);
    }
    Ok(visas)
}

/// GET /.well-known/jwks.json
pub async fn jwks(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    let json = state.keys.jwks_json().await?;
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| PassportError::Jwt(e.to_string()))?;
    Ok(Json(v))
}

/// GET /admin/visa_grants - list visa grants (optional ?user_sub= &dataset_id=)
#[utoipa::path(get, path = "/admin/visa_grants", responses((status = 200)))]
pub async fn admin_list_visa_grants(
    State(state): State<Arc<AppState>>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>> {
    let user_sub = q.get("user_sub").map(String::as_str);
    let dataset_id = q.get("dataset_id").map(String::as_str);
    let rows = state.repo.list_all_visa_grants(user_sub, dataset_id).await?;
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id.to_string(),
                "user_sub": r.user_sub,
                "user_iss": r.user_iss,
                "dataset_id": r.dataset_id,
                "visa_type": r.visa_type,
                "value": r.value,
                "source": r.source,
                "conditions": r.conditions,
                "expires_at": r.expires_at.map(|d| d.to_rfc3339()),
            })
        })
        .collect();
    Ok(Json(list))
}

/// POST /admin/visa_grants - create a visa grant
#[utoipa::path(post, path = "/admin/visa_grants", request_body = CreateVisaGrantRequest, responses((status = 201)))]
pub async fn admin_create_visa_grant(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateVisaGrantRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let expires_at = body
        .expires_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));
    let id = state
        .repo
        .create_visa_grant(
            &body.user_sub,
            &body.user_iss,
            &body.dataset_id,
            &body.visa_type,
            &body.value,
            &body.source,
            body.conditions,
            expires_at,
        )
        .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id.to_string() })),
    ))
}

#[derive(serde::Deserialize)]
pub struct VisaGrantIdPath {
    pub id: uuid::Uuid,
}

/// DELETE /admin/visa_grants/:id
pub async fn admin_delete_visa_grant(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<VisaGrantIdPath>,
) -> Result<axum::http::StatusCode> {
    let deleted = state.repo.delete_visa_grant(path.id).await?;
    if deleted {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(PassportError::NotFound("visa grant not found".into()))
    }
}