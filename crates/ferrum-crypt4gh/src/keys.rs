//! Layer 4: Key distribution — node keypair, POST /keys/exchange for header-only delivery.

use crate::encryption::{recipient_keys_from_pubkey, reencrypt_bytes, KeyStore};
use crate::error::{Crypt4GHError, Result};
use crate::policy::PolicyEngine;
use axum::{extract::State, routing::post, Json, Router};
use base64::Engine;
use ferrum_core::auth::{AuthClaims, VisaObject};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Node keypair identity (master key for decrypting stored objects).
pub struct NodeKeypair {
    pub key_id: String,
}

/// Request body for key exchange.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct KeyExchangeRequest {
    /// DRS object ID the client wants access to.
    pub object_id: String,
    /// Optional: client's Crypt4GH public key (base64). If omitted, must be in Passport/visa.
    pub public_key: Option<String>,
}

/// Response: encrypted header (and optionally first segment) so client can decrypt with byte-range.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct KeyExchangeResponse {
    /// Base64-encoded Crypt4GH blob re-encrypted for the client's key (header + body or header-only).
    pub encrypted_header: String,
    /// Object size in bytes (if known).
    pub object_size: Option<u64>,
}

/// Fetches object bytes by object_id (e.g. from storage). Returns full encrypted object.
pub type ObjectFetcher = Arc<
    dyn Fn(
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>>> + Send>>
        + Send
        + Sync,
>;

/// State for the key exchange handler.
pub struct KeyExchangeState {
    pub key_store: Arc<dyn KeyStore>,
    pub policy_engine: Arc<PolicyEngine>,
    pub master_key_id: String,
    /// Fetches encrypted object bytes by object_id. If None, returns 404 for exchange.
    pub object_fetcher: Option<ObjectFetcher>,
}

/// Build router for /crypt4gh/v1/keys (mount at /keys).
pub fn keys_router(state: KeyExchangeState) -> Router {
    Router::new()
        .route("/exchange", post(key_exchange_handler))
        .with_state(Arc::new(state))
}

async fn key_exchange_handler(
    State(state): State<Arc<KeyExchangeState>>,
    axum::extract::Extension(claims): axum::extract::Extension<AuthClaims>,
    axum::extract::Json(body): axum::extract::Json<KeyExchangeRequest>,
) -> Result<Json<KeyExchangeResponse>> {
    let visas: Vec<VisaObject> = match &claims {
        AuthClaims::Jwt { .. } => {
            return Err(Crypt4GHError::Forbidden(
                "GA4GH Passport required".to_string(),
            ))
        }
        AuthClaims::Passport { visas, .. } => visas.clone(),
    };

    let subject_id = match &claims {
        AuthClaims::Passport { claims: c, .. } => c.sub.as_deref().unwrap_or(""),
        _ => "",
    };

    if !state
        .policy_engine
        .check(&body.object_id, &visas, subject_id)
    {
        return Err(Crypt4GHError::Forbidden(
            "No valid visa for this object".to_string(),
        ));
    }

    let pubkey_b64 = body.public_key.as_deref().ok_or(Crypt4GHError::Forbidden(
        "public_key in body or X-Crypt4GH-Public-Key header required".to_string(),
    ))?;
    let pubkey = base64::engine::general_purpose::STANDARD
        .decode(pubkey_b64.trim())
        .map_err(|_| Crypt4GHError::KeyError("Invalid base64 public key".to_string()))?;

    let master_keys = state
        .key_store
        .get_private_key(&state.master_key_id)
        .await?
        .ok_or(Crypt4GHError::KeyError("Master key not found".to_string()))?;

    let recipient_keys = std::collections::HashSet::from([recipient_keys_from_pubkey(&pubkey)]);

    let object_bytes = state
        .object_fetcher
        .as_ref()
        .ok_or(Crypt4GHError::NotFound(
            "Object fetcher not configured".to_string(),
        ))?(body.object_id.clone())
    .await?
    .ok_or(Crypt4GHError::NotFound("Object not found".to_string()))?;

    let reencrypted = reencrypt_bytes(&master_keys, &recipient_keys, &object_bytes, true)
        .map_err(Crypt4GHError::Crypto)?;

    let encrypted_header = base64::engine::general_purpose::STANDARD.encode(&reencrypted);
    Ok(Json(KeyExchangeResponse {
        encrypted_header,
        object_size: Some(object_bytes.len() as u64),
    }))
}
