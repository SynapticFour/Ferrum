//! Ferrum core: config, database, errors, auth, GA4GH types, provenance.

pub mod ads;
pub mod auth;
pub mod clock;
pub mod config;
pub mod db;
pub mod dialect;
pub mod disk;
pub mod edge_accounts;
pub mod error;
pub mod gisaid;
pub mod health;
pub mod io;
pub mod pool;
#[cfg(feature = "libdeflate")]
pub use noodles_bgzf;
pub mod ops;
pub mod outbreak;
pub mod pipeline;
pub mod power;
pub mod provenance;
pub mod residency;
pub mod security;
pub mod ssrf;
pub mod sync_export;
pub mod sync_push;
pub mod sync_queue;
pub mod types;
pub mod workspace;

pub use ads::{AdsIntrospectClient, AdsIntrospectError};
pub use auth::{
    auth_layer, auth_middleware, auth_middleware_with_config, warm_jwks_cache, AuthClaims,
    AuthMiddlewareConfig, PassportClaims, RevocationCheck, RevokedTokensChecker, VisaObject,
};
pub use clock::{clock_status, ClockStatus, DEFAULT_MAX_SKEW_SECS, DEFAULT_NTP_HOST};
pub use config::{
    AfricaProfile, AggregateStrategy, AppConfig, AuthConfig, AuthMode, BandwidthConfig,
    DatabaseConfig, DiscoveryConfig, EncryptionConfig, FederationConfig, FerrumConfig,
    FerrumPeerConfig, IngestConfig, MultiQCConfig, OperationsConfig, OutbreakConfig,
    OutbreakPolicy, PowerConfig, PricingConfig, PricingTier, SecurityConfig, ServicesConfig,
    StorageConfig, SyncConfig, WesServiceConfig,
};
pub use db::DatabasePool;
pub use dialect::{
    chromosomes_json, empty_json_array, now, sql_alias_lookup, sql_beacon_variant_count_coord,
    sql_beacon_variant_count_exact, sql_beacon_variant_exists_coord,
    sql_beacon_variant_exists_exact, sql_beacon_variant_match_ids, sql_ingest_job_failed,
    sql_ingest_job_succeeded, sql_insert_access_method, sql_insert_drs_object,
    sql_list_bundle_contents_page, sql_list_objects, sql_pathogen_count, sql_pathogen_exists,
    sql_update_drs_object,
};
pub use edge_accounts::{
    create_account, list_accounts, mint_local_token, normalize_role, verify_account_pin,
    visa_for_role, EdgeOperatorAccount, ROLE_ANALYST, ROLE_COLLECTOR, ROLE_SYNC_OPERATOR,
};
pub use error::{FerrumError, Result};
pub use gisaid::{missing_gisaid_fields, validate_gisaid_metadata, GISAID_REQUIRED_FIELDS};
pub use health::{health_router, set_health_clock_config, set_health_data_path};
pub use ops::{
    create_field_backup, resolve_sqlite_path, restore_field_backup, verify_local_checksums,
    BackupManifest, IntegrityReport,
};
pub use outbreak::{
    build_gisaid_package, ActivateRequest, ApproveDownloadRequest, DeactivateRequest, GisaidEntry,
    OutbreakService, PathogenPackageRow,
};
pub use pipeline::{
    classify_htsget_file, is_htsget_supported, is_vcf_like, HtsgetFileKind, PipelineConfig,
};
pub use pool::{postgres_pool_from_config, DbDialect, FerrumPool};
pub use power::{
    allows_background_work, checkpoint_path, default_power_monitor, max_concurrent_requests,
    resolve_power_mode, write_emergency_checkpoint, AcpiPowerMonitor, BackgroundWorkGate,
    FerrumPowerMode, LinuxPowerMonitor, MacOsPowerMonitor, PowerLevel, PowerMonitor, PowerSource,
    StubPowerMonitor,
};
pub use provenance::{
    EdgeType, NodeType, ProvenanceEdge, ProvenanceGraph, ProvenanceNode, ProvenanceStore,
};
pub use residency::{
    last_transaction_id, residency_delete_blocked, verify_chain, ResidencyAuditEntry,
    ResidencyAuditLog, ResidencyAuditQueryResult, ResidencyVerifyResult, GENESIS_HASH,
};
pub use security::{
    safe_join, validate_drs_name, ResourceAuthorizer, SecurityEvent, SecurityEventLogger,
};
pub use ssrf::{is_private_ip, validate_url_ssrf, SafeHttpClient, SsrfPolicy};
pub use sync_export::{build_sneakernet_bundle, resolve_objects_root, SneakernetManifest};
pub use sync_push::{push_pending_items, PushItemResult, PushOptions};
pub use sync_queue::{
    consent_allows_sync, enqueue_all_local, enqueue_object, hub_push_error_message,
    list_local_object_ids, list_pending_for_target, list_queue_items, load_metadata_document,
    load_object_sync_info, mark_completed, mark_failed, mark_in_progress, normalize_target_url,
    submission_passes_policy, SyncObjectInfo, SyncQueueItem, STATE_COMPLETED, STATE_FAILED,
    STATE_IN_PROGRESS, STATE_PENDING,
};
pub use types::{
    AccessMethod, AccessType, AccessUrl, Checksum, DrsObject, Organization, ServiceInfo,
    ServiceType,
};
pub use workspace::{get_workspace_member_role, is_workspace_editor_or_owner, is_workspace_member};
