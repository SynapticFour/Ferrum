// SPDX-License-Identifier: BUSL-1.1
use thiserror::Error;

pub type Result<T> = std::result::Result<T, OntError>;

#[derive(Debug, Error)]
pub enum OntError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("{0}")]
    Other(String),
}
