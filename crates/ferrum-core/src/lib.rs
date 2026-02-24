//! Ferrum core: config, database, errors, auth, storage, GA4GH types, provenance.

pub mod config;
pub mod db;
pub mod error;
pub mod auth;
pub mod health;
pub mod provenance;
pub mod security;
pub mod ssrf;
pub mod storage;
pub mod types;

pub use config::{FerrumConfig, AppConfig, DatabaseConfig, StorageConfig, AuthConfig, SecurityConfig, ServicesConfig, EncryptionConfig, PricingConfig, PricingTier, WesServiceConfig, MultiQCConfig};
pub use db::DatabasePool;
pub use error::{FerrumError, Result};
pub use auth::{auth_layer, auth_middleware, AuthClaims, AuthMiddlewareConfig, PassportClaims, RevocationCheck, RevokedTokensChecker, VisaObject};
pub use security::{safe_join, validate_drs_name, ResourceAuthorizer, SecurityEvent, SecurityEventLogger};
pub use ssrf::{is_private_ip, SafeHttpClient, SsrfPolicy, validate_url_ssrf};
pub use health::health_router;
pub use provenance::{ProvenanceEdge, ProvenanceGraph, ProvenanceNode, ProvenanceStore, NodeType, EdgeType};
pub use storage::{ObjectStorage, LocalStorage, S3Storage};
pub use types::{Checksum, AccessMethod, AccessType, AccessUrl, DrsObject, ServiceInfo, ServiceType, Organization};
