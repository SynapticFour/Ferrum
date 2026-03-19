//! Passport broker configuration from environment.

pub struct PassportConfig {
    /// Base URL of the issuer (e.g. https://ferrum.example.com/passports/v1).
    pub issuer_base_url: String,
    /// RSA private key PEM for signing tokens. If unset, a temporary key is generated (not for production).
    pub signing_key_pem: Option<String>,
    /// Optional external IdP authorization URL (Keycloak, Google, etc.).
    pub oidc_authorization_url: Option<String>,
    pub oidc_token_url: Option<String>,
    pub oidc_userinfo_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_issuer: Option<String>,
}

impl PassportConfig {
    pub fn from_env() -> Self {
        let issuer_base_url = std::env::var("PASSPORT_ISSUER_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080/passports/v1".to_string());
        Self {
            signing_key_pem: std::env::var("PASSPORT_SIGNING_KEY_PEM").ok(),
            oidc_authorization_url: std::env::var("OIDC_AUTHORIZATION_URL").ok(),
            oidc_token_url: std::env::var("OIDC_TOKEN_URL").ok(),
            oidc_userinfo_url: std::env::var("OIDC_USERINFO_URL").ok(),
            oidc_client_id: std::env::var("OIDC_CLIENT_ID").ok(),
            oidc_client_secret: std::env::var("OIDC_CLIENT_SECRET").ok(),
            oidc_issuer: std::env::var("OIDC_ISSUER").ok(),
            issuer_base_url,
        }
    }

    pub fn jwks_uri(&self) -> String {
        format!(
            "{}/.well-known/jwks.json",
            self.issuer_base_url.trim_end_matches('/')
        )
    }

    pub fn authorization_endpoint(&self) -> String {
        format!("{}/authorize", self.issuer_base_url.trim_end_matches('/'))
    }

    pub fn token_endpoint(&self) -> String {
        format!("{}/token", self.issuer_base_url.trim_end_matches('/'))
    }

    pub fn userinfo_endpoint(&self) -> String {
        format!("{}/userinfo", self.issuer_base_url.trim_end_matches('/'))
    }
}
