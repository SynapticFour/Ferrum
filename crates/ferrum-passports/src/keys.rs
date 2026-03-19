//! JWT signing and JWKS export for Passport Broker / Visa Issuer.

use crate::config::PassportConfig;
use crate::error::{PassportError, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey};
use rsa::pkcs8::EncodePublicKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use std::sync::Arc;
use tokio::sync::RwLock;

const KEY_ID: &str = "ferrum-1";

/// Holds the signing key and optional precomputed JWKS JSON.
pub struct SigningKeys {
    encoding_key: EncodingKey,
    /// PEM of the private key (so we can derive public for JWKS).
    private_key_pem: String,
    /// Cached JWKS JSON.
    jwks_json: Arc<RwLock<Option<String>>>,
}

fn base64url_no_pad(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

impl SigningKeys {
    /// Load or generate key from config.
    pub fn from_config(config: &PassportConfig) -> Result<Arc<Self>> {
        let private_key_pem = config
            .signing_key_pem
            .clone()
            .unwrap_or_else(Self::generate_rs256_pem);
        let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
            .map_err(|e| PassportError::Jwt(e.to_string()))?;
        Ok(Arc::new(Self {
            encoding_key,
            private_key_pem,
            jwks_json: Arc::new(RwLock::new(None)),
        }))
    }

    fn generate_rs256_pem() -> String {
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA keygen");
        priv_key
            .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .expect("PEM encode")
            .to_string()
    }

    /// Sign a JWT with RS256. Header will include kid and typ as needed.
    pub fn sign(&self, header: &Header, claims: &impl serde::Serialize) -> Result<String> {
        encode(header, claims, &self.encoding_key).map_err(|e| PassportError::Jwt(e.to_string()))
    }

    /// Build JWKS JSON (one RSA key). Computes from private key PEM.
    pub async fn jwks_json(&self) -> Result<String> {
        {
            let guard = self.jwks_json.read().await;
            if let Some(ref j) = *guard {
                return Ok(j.clone());
            }
        }
        let jwks = self.compute_jwks()?;
        {
            let mut guard = self.jwks_json.write().await;
            *guard = Some(jwks.clone());
        }
        Ok(jwks)
    }

    /// Public key PEM (SPKI) for token verification.
    pub fn public_key_pem(&self) -> Result<String> {
        let priv_key = RsaPrivateKey::from_pkcs1_pem(&self.private_key_pem)
            .map_err(|e| PassportError::Jwt(e.to_string()))?;
        let pub_key = rsa::RsaPublicKey::from(priv_key);
        let pem = pub_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|e| PassportError::Jwt(format!("{:?}", e)))?;
        Ok(pem)
    }

    fn compute_jwks(&self) -> Result<String> {
        let priv_key = RsaPrivateKey::from_pkcs1_pem(&self.private_key_pem)
            .map_err(|e| PassportError::Jwt(e.to_string()))?;
        let pub_key = rsa::RsaPublicKey::from(priv_key);
        let n_bytes = pub_key.n().to_bytes_be();
        let e_bytes = pub_key.e().to_bytes_be();
        let n = base64url_no_pad(&n_bytes);
        let e = base64url_no_pad(&e_bytes);
        let jwk = serde_json::json!({
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": KEY_ID,
            "n": n,
            "e": e
        });
        let jwks = serde_json::json!({ "keys": [ jwk ] });
        Ok(jwks.to_string())
    }
}

pub fn default_passport_header() -> Header {
    Header {
        kid: Some(KEY_ID.to_string()),
        typ: Some("vnd.ga4gh.passport+jwt".to_string()),
        alg: Algorithm::RS256,
        ..Default::default()
    }
}

pub fn default_visa_header() -> Header {
    Header {
        kid: Some(KEY_ID.to_string()),
        typ: Some("JWT".to_string()),
        alg: Algorithm::RS256,
        ..Default::default()
    }
}

pub fn default_access_token_header() -> Header {
    Header {
        kid: Some(KEY_ID.to_string()),
        typ: Some("at+jwt".to_string()),
        alg: Algorithm::RS256,
        ..Default::default()
    }
}
