//! Ferrum core: config, database, errors, auth, GA4GH types, provenance.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod health;
pub mod io;
#[cfg(feature = "libdeflate")]
pub use noodles_bgzf;
pub mod provenance;
pub mod security;
pub mod ssrf;
pub mod types;
pub mod workspace;

pub use auth::{
    auth_layer, auth_middleware, auth_middleware_with_config, AuthClaims, AuthMiddlewareConfig,
    PassportClaims, RevocationCheck, RevokedTokensChecker, VisaObject,
};
pub use config::{
    AppConfig, AuthConfig, DatabaseConfig, EncryptionConfig, FerrumConfig, MultiQCConfig,
    PricingConfig, PricingTier, SecurityConfig, ServicesConfig, StorageConfig, WesServiceConfig,
};
pub use db::DatabasePool;
pub use error::{FerrumError, Result};
pub use health::health_router;
pub use provenance::{
    EdgeType, NodeType, ProvenanceEdge, ProvenanceGraph, ProvenanceNode, ProvenanceStore,
};
pub use security::{
    safe_join, validate_drs_name, ResourceAuthorizer, SecurityEvent, SecurityEventLogger,
};
pub use ssrf::{is_private_ip, validate_url_ssrf, SafeHttpClient, SsrfPolicy};
pub use types::{
    AccessMethod, AccessType, AccessUrl, Checksum, DrsObject, Organization, ServiceInfo,
    ServiceType,
};
pub use workspace::{get_workspace_member_role, is_workspace_editor_or_owner};
