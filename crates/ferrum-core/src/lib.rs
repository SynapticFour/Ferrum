//! Ferrum core: config, database, errors, auth, storage, GA4GH types, provenance.

pub mod config;
pub mod db;
pub mod error;
pub mod auth;
pub mod health;
pub mod provenance;
pub mod storage;
pub mod types;

pub use config::{FerrumConfig, AppConfig, DatabaseConfig, StorageConfig, AuthConfig, ServicesConfig, EncryptionConfig};
pub use db::DatabasePool;
pub use error::{FerrumError, Result};
pub use auth::{auth_layer, auth_middleware, AuthClaims, AuthMiddlewareConfig, PassportClaims, VisaObject};
pub use health::health_router;
pub use provenance::{ProvenanceEdge, ProvenanceGraph, ProvenanceNode, ProvenanceStore, NodeType, EdgeType};
pub use storage::{ObjectStorage, LocalStorage, S3Storage};
pub use types::{Checksum, AccessMethod, AccessType, AccessUrl, DrsObject, ServiceInfo, ServiceType, Organization};
