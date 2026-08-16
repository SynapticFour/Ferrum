// SPDX-License-Identifier: BUSL-1.1
//! Broker JWKS round-trip: mint passport like ga4gh-infra, decode like ferrum-gateway.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use ferrum_core::PassportClaims;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde_json::json;
use sha2::{Digest, Sha256};

fn broker_pem_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ga4gh-infra/docker/secrets/broker_rs256.pem")
}

#[test]
fn broker_jwks_decodes_with_rsa_components_and_from_jwk() {
    let pem = std::fs::read_to_string(broker_pem_path()).expect("broker pem");
    let private_key = RsaPrivateKey::from_pkcs8_pem(&pem).expect("parse pem");
    let public_key = private_key.to_public_key();
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
    let kid = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(public_key.n().to_bytes_be()));

    let issuer = "https://broker.example.org";
    let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("encoding key");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.clone());
    let token = encode(
        &header,
        &json!({
            "sub": "researcher@example.org",
            "iss": issuer,
            "iat": now,
            "exp": now + 3600,
            "jti": "test-jti",
            "ga4gh_passport_v1": [],
            "scope": "openid ga4gh_passport_v1"
        }),
        &encoding_key,
    )
    .expect("mint");

    let jwks = json!({
        "keys": [{
            "kty": "RSA",
            "kid": kid,
            "use": "sig",
            "alg": "RS256",
            "n": n,
            "e": e,
        }]
    });
    let set: jsonwebtoken::jwk::JwkSet = serde_json::from_value(jwks).expect("jwks");
    let jwk = set.find(&kid).expect("kid");
    let key_from_jwk = DecodingKey::from_jwk(jwk).expect("from_jwk");
    let key_from_components =
        DecodingKey::from_rsa_components(&n, &e).expect("from_rsa_components");

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.validate_aud = false;

    for key in [&key_from_jwk, &key_from_components] {
        decode::<PassportClaims>(token.as_str(), key, &validation).expect("decode passport");
    }
}
