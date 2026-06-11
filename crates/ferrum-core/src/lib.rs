//! Ferrum core: config, database, errors, auth, GA4GH types, provenance.

pub mod auth;
pub mod config;
pub mod db;
pub mod dialect;
pub mod error;
pub mod pool;
pub mod health;
pub mod io;
#[cfg(feature = "libdeflate")]
pub use noodles_bgzf;
pub mod outbreak;
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
    AfricaProfile, AppConfig, AuthConfig, DatabaseConfig, EncryptionConfig, FerrumConfig,
    IngestConfig, MultiQCConfig, OutbreakConfig, OutbreakPolicy, PricingConfig, PricingTier,
    SecurityConfig, ServicesConfig, StorageConfig, WesServiceConfig,
};
pub use db::DatabasePool;
pub use dialect::{
    chromosomes_json, empty_json_array, now, sql_alias_lookup, sql_beacon_variant_count_coord,
    sql_beacon_variant_count_exact, sql_beacon_variant_exists_coord, sql_beacon_variant_exists_exact,
    sql_beacon_variant_match_ids, sql_ingest_job_failed, sql_ingest_job_succeeded,
    sql_insert_access_method, sql_insert_drs_object, sql_list_bundle_contents_page, sql_list_objects,
    sql_pathogen_count, sql_pathogen_exists, sql_update_drs_object,
};
pub use error::{FerrumError, Result};
pub use pool::{postgres_pool_from_config, DbDialect, FerrumPool};
pub use health::health_router;
pub use outbreak::{
    build_gisaid_package, ActivateRequest, ApproveDownloadRequest, DeactivateRequest, GisaidEntry,
    OutbreakService, PathogenPackageRow,
};
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
