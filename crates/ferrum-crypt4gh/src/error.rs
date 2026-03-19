//! Errors for ferrum-crypt4gh.

use ferrum_core::FerrumError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Crypt4GHError>;

#[derive(Error, Debug)]
pub enum Crypt4GHError {
    #[error("crypt4gh: {0}")]
    Crypto(#[from] crypt4gh::error::Crypt4GHError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("key error: {0}")]
    KeyError(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl From<Crypt4GHError> for FerrumError {
    fn from(e: Crypt4GHError) -> Self {
        match &e {
            Crypt4GHError::NotFound(_) => FerrumError::NotFound(e.to_string()),
            Crypt4GHError::Forbidden(_) => FerrumError::Forbidden(e.to_string()),
            _ => FerrumError::EncryptionError(e.into()),
        }
    }
}

impl axum::response::IntoResponse for Crypt4GHError {
    fn into_response(self) -> axum::response::Response {
        FerrumError::from(self).into_response()
    }
}
