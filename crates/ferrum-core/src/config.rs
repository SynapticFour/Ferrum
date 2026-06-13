//! Layered configuration: defaults, /etc/ferrum, ~/.ferrum, FERRUM_ env, optional --config file.

use serde::Deserialize;
use std::path::{Path, PathBuf};

pub mod watch;

/// Root Ferrum configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct FerrumConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub services: ServicesConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub pricing: PricingConfig,
    /// A05: CORS and security options. If absent, CORS is permissive.
    #[serde(default)]
    pub security: Option<SecurityConfig>,
    /// Workspace invite emails (SMTP). If absent, invites are stored but not emailed.
    #[serde(default)]
    pub email: Option<EmailConfig>,
    /// Lab Kit / machine ingest: upload defaults and limits (`/api/v1/ingest/*`).
    #[serde(default)]
    pub ingest: IngestConfig,
    /// MII Connect (FHIR MII-KDS conformance checks).
    #[serde(default)]
    pub mii_connect: MiiConnectConfig,
    /// Resource-constrained / offline-first deployment profile (Africa laptop mode).
    #[serde(default)]
    pub africa: Option<AfricaProfile>,
    /// Outbreak Mode: policy-based emergency Beacon access (opt-in, disabled by default).
    #[serde(default)]
    pub outbreak: OutbreakConfig,
    /// P2P federated Beacon (opt-in, disabled by default).
    #[serde(default)]
    pub federation: FederationConfig,
    /// Solar/battery-aware operating modes.
    #[serde(default)]
    pub power: PowerConfig,
    /// Bandwidth-adaptive transfer thresholds.
    #[serde(default)]
    pub bandwidth: BandwidthConfig,
    /// GA4GH Service Registry discovery (ga4gh-infra integration).
    #[serde(default)]
    pub discovery: DiscoveryConfig,
}

/// Upload/register ingest limits for [`FerrumConfig::ingest`].
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IngestConfig {
    /// When true, multipart upload defaults to Crypt4GH (Ferrum node public key) if the client omits `encrypt`.
    #[serde(default)]
    pub default_encrypt_upload: bool,
    /// Max accepted upload body size in bytes (0 = use built-in default 1 GiB).
    #[serde(default)]
    pub max_upload_bytes: Option<u64>,
}

impl IngestConfig {
    /// Effective max upload size (defaults to 1 GiB when unset or zero).
    pub fn effective_max_upload_bytes(&self) -> u64 {
        const DEFAULT: u64 = 1 << 30;
        self.max_upload_bytes.filter(|&n| n > 0).unwrap_or(DEFAULT)
    }
}

/// MII Connect configuration for offline-first KDS profile validation.
#[derive(Debug, Clone, Deserialize)]
pub struct MiiConnectConfig {
    /// Enable MII validation checks.
    #[serde(default)]
    pub enabled: bool,
    /// Active module set (MII default-17 profile defaults).
    #[serde(default = "default_mii_modules")]
    pub modules: Vec<String>,
    /// Version tag of vendored profile set.
    #[serde(default = "default_mii_profile_set_version")]
    pub profile_set_version: String,
    /// Strict mode fails on warnings that indicate gaps.
    #[serde(default)]
    pub strict_mode: bool,
    /// Optional cap for maximum per-run errors before short-circuit.
    #[serde(default)]
    pub max_errors: Option<usize>,
    /// Keep validation offline-only unless explicitly disabled.
    #[serde(default = "default_true")]
    pub offline_only: bool,
    /// Path to vendored manifest json.
    #[serde(default = "default_mii_manifest_path")]
    pub manifest_path: String,
}

impl Default for MiiConnectConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            modules: default_mii_modules(),
            profile_set_version: default_mii_profile_set_version(),
            strict_mode: false,
            max_errors: None,
            offline_only: true,
            manifest_path: default_mii_manifest_path(),
        }
    }
}

fn default_mii_modules() -> Vec<String> {
    vec![
        "person".to_string(),
        "encounter".to_string(),
        "consent".to_string(),
        "diagnosis".to_string(),
        "procedure".to_string(),
        "laboratory".to_string(),
        "medication".to_string(),
        "oncology".to_string(),
        "pathology_report".to_string(),
        "molecular_genetic_report".to_string(),
        "molecular_tumor_board".to_string(),
        "microbiology".to_string(),
        "imaging".to_string(),
        "intensive_care".to_string(),
        "biobank".to_string(),
        "document".to_string(),
        "research_study".to_string(),
    ]
}

fn default_mii_profile_set_version() -> String {
    "mii-kds-default17-v1".to_string()
}

fn default_mii_manifest_path() -> String {
    "profiles/mii/manifest.json".to_string()
}

#[cfg(test)]
mod ingest_config_tests {
    use super::{FerrumConfig, IngestConfig, MiiConnectConfig};
    use std::fs;
    use std::io::Write;

    #[test]
    fn effective_max_upload_bytes_default_one_gib() {
        let c = IngestConfig::default();
        assert_eq!(c.effective_max_upload_bytes(), 1 << 30);
    }

    #[test]
    fn effective_max_upload_bytes_custom() {
        let c = IngestConfig {
            default_encrypt_upload: false,
            max_upload_bytes: Some(1024),
        };
        assert_eq!(c.effective_max_upload_bytes(), 1024);
    }

    #[test]
    fn mii_connect_default_is_offline_default17_disabled() {
        let m = MiiConnectConfig::default();
        assert!(!m.enabled);
        assert!(m.offline_only);
        assert_eq!(m.modules.len(), 17);
        assert_eq!(m.profile_set_version, "mii-kds-default17-v1");
        assert_eq!(m.manifest_path, "profiles/mii/manifest.json");
    }

    #[test]
    fn database_url_from_env_enables_postgres_mode() {
        let home = std::env::temp_dir().join("ferrum-config-env-test-home");
        let _ = fs::create_dir_all(&home);
        std::env::set_var("HOME", home.as_os_str());
        std::env::set_var(
            "FERRUM_DATABASE__URL",
            "postgres://ferrum:ferrum@postgres:5432/ferrum",
        );
        std::env::set_var("FERRUM_DATABASE__RUN_MIGRATIONS", "false");
        std::env::set_var("FERRUM_STORAGE__BACKEND", "s3");

        let cfg = FerrumConfig::load().expect("load config from env");
        assert_eq!(
            cfg.database.url.as_deref(),
            Some("postgres://ferrum:ferrum@postgres:5432/ferrum")
        );
        assert!(!cfg.is_offline_first());

        std::env::remove_var("FERRUM_DATABASE__URL");
        std::env::remove_var("FERRUM_DATABASE__RUN_MIGRATIONS");
        std::env::remove_var("FERRUM_STORAGE__BACKEND");
    }

    #[test]
    fn mii_connect_loads_from_file() {
        let file = std::env::temp_dir().join("ferrum-config-mii-test.toml");
        let mut f = fs::File::create(&file).expect("create temp config");
        writeln!(
            f,
            r#"
bind = "0.0.0.0:8080"

[database]
driver = "sqlite"
sqlite_path = "ferrum.db"

[mii_connect]
enabled = true
modules = ["diagnosis", "genomics"]
profile_set_version = "mii-kds-default17-v2"
strict_mode = true
max_errors = 7
offline_only = true
manifest_path = "profiles/mii/custom-manifest.json"
"#
        )
        .expect("write");

        let cfg = FerrumConfig::load_from_path(&file).expect("load config");
        assert!(cfg.mii_connect.enabled);
        assert_eq!(cfg.mii_connect.modules, vec!["diagnosis", "genomics"]);
        assert_eq!(cfg.mii_connect.profile_set_version, "mii-kds-default17-v2");
        assert!(cfg.mii_connect.strict_mode);
        assert_eq!(cfg.mii_connect.max_errors, Some(7));
        assert!(cfg.mii_connect.offline_only);
        assert_eq!(
            cfg.mii_connect.manifest_path,
            "profiles/mii/custom-manifest.json"
        );

        let _ = fs::remove_file(file);
    }
}

/// SMTP configuration for workspace invite emails.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    pub smtp_from: String,
    #[serde(default)]
    pub smtp_username: Option<String>,
    #[serde(default)]
    pub smtp_password: Option<String>,
    /// Base URL for invite links (e.g. https://ferrum.institution.edu). Env: FERRUM_EMAIL__BASE_URL
    #[serde(default)]
    pub base_url: Option<String>,
}

fn default_smtp_port() -> u16 {
    587
}

/// A05: Security / CORS configuration. Never use wildcard (*) in production.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SecurityConfig {
    /// Allowed origins (e.g. ["https://ferrum.institution.edu"]). Empty = permissive.
    #[serde(default)]
    pub allowed_origins: Option<Vec<String>>,
    #[serde(default)]
    pub allow_credentials: Option<bool>,
}

/// Pricing configuration for run cost estimation (WES/TES).
/// No cloud billing API — cost = wall-clock × configured resource price.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PricingConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Display currency label only (e.g. "USD").
    #[serde(default = "default_currency")]
    pub currency: String,
    /// USD per CPU-core-hour (e.g. AWS c6i.large reference).
    #[serde(default = "default_cpu_core_hour")]
    pub cpu_core_hour: f64,
    /// USD per GB-hour (memory).
    #[serde(default = "default_memory_gb_hour")]
    pub memory_gb_hour: f64,
    /// USD per GB-month (for DRS storage estimation).
    #[serde(default = "default_storage_gb_month")]
    pub storage_gb_month: f64,
    /// Named compute tiers (e.g. gpu, highmem) override default rates.
    #[serde(default)]
    pub tiers: std::collections::HashMap<String, PricingTier>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PricingTier {
    #[serde(default)]
    pub cpu_core_hour: Option<f64>,
    #[serde(default)]
    pub memory_gb_hour: Option<f64>,
}

fn default_currency() -> String {
    "USD".to_string()
}
fn default_cpu_core_hour() -> f64 {
    0.048
}
fn default_memory_gb_hour() -> f64 {
    0.006
}
fn default_storage_gb_month() -> f64 {
    0.023
}

fn default_bind() -> String {
    "0.0.0.0:8080".to_string()
}

/// Offline-first / laptop deployment profile for resource-constrained environments.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AfricaProfile {
    /// Enable offline-first mode. Default: false.
    /// When true: SQLite backend, local storage, no external auth probing.
    #[serde(default)]
    pub offline_first: bool,
    /// Maximum RAM Ferrum may use in MB. Default: unlimited.
    #[serde(default)]
    pub max_memory_mb: Option<u64>,
    /// Path for SQLite database file. Default: ~/.ferrum/ferrum.db
    #[serde(default)]
    pub sqlite_path: Option<PathBuf>,
    /// Path for local object storage root. Default: ~/.ferrum/objects/
    #[serde(default)]
    pub objects_path: Option<PathBuf>,
}

/// Outbreak Mode configuration (opt-in; disabled by default).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OutbreakConfig {
    /// Master switch. Must be explicitly enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Policy definitions loaded from config (not stored in DB).
    #[serde(default)]
    pub policies: Vec<OutbreakPolicy>,
}

/// A single outbreak sharing policy (config-driven).
#[derive(Debug, Clone, Deserialize)]
pub struct OutbreakPolicy {
    pub name: String,
    pub trigger_pathogen: String,
    /// Passport issuer domains or explicit recipient identifiers.
    #[serde(default)]
    pub emergency_recipients: Vec<String>,
    /// `beacon_only` or `full` (full still requires per-object download approval).
    #[serde(default = "default_outbreak_access_level")]
    pub access_level: String,
    #[serde(default)]
    pub gisaid_auto_package: bool,
}

fn default_outbreak_access_level() -> String {
    "beacon_only".to_string()
}

impl OutbreakConfig {
    pub fn policy_by_name(&self, name: &str) -> Option<&OutbreakPolicy> {
        self.policies.iter().find(|p| p.name == name)
    }
}

/// P2P federated Beacon configuration (opt-in; disabled by default).
#[derive(Debug, Clone, Deserialize)]
pub struct FederationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub peers: Vec<FerrumPeerConfig>,
    #[serde(default = "default_true")]
    pub fan_out_parallel: bool,
    #[serde(default)]
    pub aggregate_strategy: AggregateStrategy,
    #[serde(default = "default_peer_requests_per_minute")]
    pub peer_requests_per_minute: u32,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            peers: Vec::new(),
            fan_out_parallel: true,
            aggregate_strategy: AggregateStrategy::default(),
            peer_requests_per_minute: default_peer_requests_per_minute(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FerrumPeerConfig {
    pub name: String,
    pub beacon_endpoint: String,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default = "default_peer_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub service_token: Option<String>,
}

fn default_peer_timeout_ms() -> u64 {
    3000
}

fn default_peer_requests_per_minute() -> u32 {
    10
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregateStrategy {
    #[default]
    Union,
    Intersection,
    LocalFirst,
}

/// Solar/battery power monitoring configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PowerConfig {
    #[serde(default = "default_power_enabled")]
    pub enabled: bool,
    #[serde(default = "default_low_power_threshold")]
    pub low_power_threshold: u8,
    #[serde(default = "default_emergency_threshold")]
    pub emergency_threshold: u8,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            enabled: default_power_enabled(),
            low_power_threshold: default_low_power_threshold(),
            emergency_threshold: default_emergency_threshold(),
        }
    }
}

fn default_power_enabled() -> bool {
    cfg!(target_os = "linux")
}

fn default_low_power_threshold() -> u8 {
    50
}

fn default_emergency_threshold() -> u8 {
    10
}

/// Bandwidth classification thresholds (bits per second).
#[derive(Debug, Clone, Deserialize)]
pub struct BandwidthConfig {
    #[serde(default = "default_high_bps")]
    pub high_bps: u64,
    #[serde(default = "default_medium_bps")]
    pub medium_bps: u64,
    #[serde(default = "default_low_bps")]
    pub low_bps: u64,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            high_bps: default_high_bps(),
            medium_bps: default_medium_bps(),
            low_bps: default_low_bps(),
        }
    }
}

fn default_high_bps() -> u64 {
    10_000_000
}

fn default_medium_bps() -> u64 {
    1_000_000
}

fn default_low_bps() -> u64 {
    100_000
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL (overrides driver/params). Env: FERRUM_DATABASE__URL
    pub url: Option<String>,
    /// Max pool size for PostgreSQL (SQLite uses a smaller effective cap).
    /// Default: `max(10, min(100, 2 * available_parallelism))` — production pattern for SSD-backed APIs.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum idle connections to keep open (PostgreSQL). Env: FERRUM_DATABASE__MIN_CONNECTIONS
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// Seconds to wait for a pool connection before error (PostgreSQL). Env: FERRUM_DATABASE__ACQUIRE_TIMEOUT_SECS
    #[serde(default = "default_acquire_timeout_secs")]
    pub acquire_timeout_secs: u64,
    /// Close idle connections after this many seconds (PostgreSQL). Env: FERRUM_DATABASE__IDLE_TIMEOUT_SECS
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    /// Max lifetime of a connection in seconds (PostgreSQL). Env: FERRUM_DATABASE__MAX_LIFETIME_SECS
    #[serde(default = "default_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
    #[serde(default = "default_true")]
    pub run_migrations: bool,
    /// Driver when url not set: "sqlite" | "postgres"
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
    #[serde(default)]
    pub postgres_host: Option<String>,
    #[serde(default = "default_postgres_port")]
    pub postgres_port: u16,
    #[serde(default)]
    pub postgres_db: Option<String>,
    #[serde(default)]
    pub postgres_user: Option<String>,
    #[serde(default)]
    pub postgres_password: Option<String>,
}

fn default_max_connections() -> u32 {
    std::thread::available_parallelism()
        .map(|n| {
            let doubled = (n.get() as u32).saturating_mul(2);
            doubled.clamp(10, 100)
        })
        .unwrap_or(10)
}

fn default_min_connections() -> u32 {
    2
}

fn default_acquire_timeout_secs() -> u64 {
    10
}

fn default_idle_timeout_secs() -> u64 {
    600
}

fn default_max_lifetime_secs() -> u64 {
    1800
}
fn default_driver() -> String {
    "sqlite".to_string()
}
fn default_sqlite_path() -> String {
    "ferrum.db".to_string()
}
fn default_postgres_port() -> u16 {
    5432
}

/// Storage backend configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StorageConfig {
    #[serde(default = "default_storage_backend")]
    pub backend: String,
    /// Base path for Local backend. Env: FERRUM_STORAGE__BASE_PATH
    #[serde(default)]
    pub base_path: Option<String>,
    /// S3-compatible endpoint (e.g. http://minio:9000). Env: FERRUM_STORAGE__S3_ENDPOINT
    #[serde(default)]
    pub s3_endpoint: Option<String>,
    #[serde(default)]
    pub s3_region: Option<String>,
    #[serde(default)]
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub s3_access_key_id: Option<String>,
    #[serde(default)]
    pub s3_secret_access_key: Option<String>,
}

fn default_storage_backend() -> String {
    "local".to_string()
}

/// Auth / JWT configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    /// Authentication backend: `builtin` (ferrum-passports) or `external` (ga4gh-infra broker).
    #[serde(default)]
    pub mode: AuthMode,
    /// HMAC secret for JWT validation (HS256). Env: FERRUM_AUTH__JWT_SECRET. May be file:///path for Docker/K8s secrets.
    pub jwt_secret: Option<String>,
    /// Expected JWT issuer. Env: FERRUM_AUTH__ISSUER
    pub issuer: Option<String>,
    /// JWKS URL for RS256 validation. Env: FERRUM_AUTH__JWKS_URL
    pub jwks_url: Option<String>,
    /// GA4GH Passport / token endpoints to trust. Env: FERRUM_AUTH__PASSPORT_ENDPOINTS
    #[serde(default)]
    pub passport_endpoints: Vec<String>,
    #[serde(default)]
    pub require_auth: bool,
    /// When true and `mode = external`, validate Passports via ga4gh-clearinghouse (visa signature verification).
    #[serde(default)]
    pub clearinghouse: bool,
    /// Optional ADS introspection URL for controlled-access decisions (ga4gh-infra).
    #[serde(default)]
    pub ads_url: Option<String>,
    /// Environment variable holding the DAC API key for ADS introspection.
    #[serde(default = "default_ads_api_key_env")]
    pub ads_api_key_env: String,
    /// A07: Reject tokens older than this many hours (even if not expired). Default 24.
    #[serde(default = "default_max_token_age_hours")]
    pub max_token_age_hours: u32,
}

/// Authentication backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// Built-in ferrum-passports broker (standalone Ferrum deployments).
    #[default]
    Builtin,
    /// External ga4gh-infra AAI broker; disables ferrum-passports when combined with discovery.
    External,
}

fn default_ads_api_key_env() -> String {
    "ADS_DAC_API_KEY".to_string()
}

/// GA4GH Service Registry discovery configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiscoveryConfig {
    /// When true, register enabled Ferrum services and resolve peer URLs via service registry.
    #[serde(default)]
    pub enabled: bool,
    /// Base URL of ga4gh-infra service registry (no trailing slash).
    #[serde(default)]
    pub service_registry_url: Option<String>,
    /// Environment variable for the registry registration API key.
    #[serde(default = "default_registry_key_env")]
    pub registration_api_key_env: String,
    /// Register Ferrum services on startup.
    #[serde(default)]
    pub auto_register: bool,
    /// Public base URL used in service-registry entries (defaults to bind address when unset).
    /// Env: `FERRUM_DISCOVERY__REGISTRATION_BASE_URL`. Use `http://ferrum-gateway:8080` in Docker co-deploy.
    #[serde(default)]
    pub registration_base_url: Option<String>,
    /// Static fallback URLs keyed by GA4GH artifact (`drs`, `beacon`, `wes`, …) when registry is offline.
    #[serde(default)]
    pub fallback_urls: std::collections::HashMap<String, String>,
}

fn default_registry_key_env() -> String {
    "SERVICE_REGISTRY_REGISTRATION_KEY".to_string()
}

impl DiscoveryConfig {
    /// Resolve the registration API key from the configured environment variable.
    pub fn registration_api_key(&self) -> Result<String, std::env::VarError> {
        std::env::var(&self.registration_api_key_env)
    }
}

impl AuthConfig {
    /// Returns true when Ferrum should use an external ga4gh-infra auth plane.
    pub fn is_external(&self) -> bool {
        self.mode == AuthMode::External
    }
}

fn default_max_token_age_hours() -> u32 {
    24
}

/// Encryption (Crypt4GH) and DRS stream behaviour.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EncryptionConfig {
    pub enabled: bool,
    /// Directory containing Crypt4GH node private key `{crypt4gh_master_key_id}.sec` (and `.pub`).
    /// Required for `GET .../objects/{id}/stream` when the object has `is_encrypted = true`.
    /// Env: `FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR`.
    pub crypt4gh_key_dir: Option<String>,
    /// Key id / basename for [LocalKeyStore] (default file `node.sec`). Env: `FERRUM_ENCRYPTION__CRYPT4GH_MASTER_KEY_ID`.
    #[serde(default = "default_crypt4gh_master_key_id")]
    pub crypt4gh_master_key_id: String,
    /// When true, `GET /ga4gh/drs/v1/objects/{id}/stream` decrypts Crypt4GH at-rest objects and streams **plaintext** to the client (no Crypt4GH client library needed).
    #[serde(default = "default_true")]
    pub crypt4gh_decrypt_stream: bool,
}

fn default_crypt4gh_master_key_id() -> String {
    "node".to_string()
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            crypt4gh_key_dir: None,
            crypt4gh_master_key_id: default_crypt4gh_master_key_id(),
            crypt4gh_decrypt_stream: true,
        }
    }
}

/// Per-service enable/disable flags.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServicesConfig {
    #[serde(default = "default_true")]
    pub enable_drs: bool,
    #[serde(default = "default_true")]
    pub enable_trs: bool,
    #[serde(default = "default_true")]
    pub enable_wes: bool,
    #[serde(default = "default_true")]
    pub enable_tes: bool,
    #[serde(default = "default_true")]
    pub enable_passports: bool,
    #[serde(default = "default_true")]
    pub enable_crypt4gh: bool,
    #[serde(default = "default_true")]
    pub enable_beacon: bool,
    #[serde(default = "default_true")]
    pub enable_htsget: bool,
    #[serde(default)]
    pub wes: Option<WesServiceConfig>,
}

/// WES-specific service options (e.g. [services.wes.multiqc], A08 allowed_workflow_sources, A04 limits).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WesServiceConfig {
    #[serde(default)]
    pub multiqc: Option<MultiQCConfig>,
    /// A08: Allowed workflow URL prefixes (e.g. https://github.com/, file://). Empty = allow all.
    #[serde(default)]
    pub allowed_workflow_sources: Vec<String>,
    /// A04: Optional limits (max_workflow_url_length, max_concurrent_runs per owner, etc.). Enforcement is service-level.
    #[serde(default)]
    pub limits: Option<WesLimitsConfig>,
}

/// A04: WES rate/limits configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WesLimitsConfig {
    /// Max length of workflow_url (default no limit when absent).
    pub max_workflow_url_length: Option<u32>,
    /// Max concurrent runs per owner (default no limit when absent).
    pub max_concurrent_runs_per_owner: Option<u32>,
}

/// MultiQC auto-report config: [services.wes.multiqc].
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MultiQCConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Workflow types to run MultiQC for, or ["*"] for all.
    #[serde(default)]
    pub run_for: Vec<String>,
    #[serde(default = "default_multiqc_image")]
    pub image: String,
    #[serde(default = "default_scan_patterns")]
    pub scan_patterns: Vec<String>,
    #[serde(default = "default_report_mime_type")]
    pub report_mime_type: String,
    #[serde(default = "default_report_name_template")]
    pub report_name_template: String,
    #[serde(default = "default_report_tags")]
    pub report_tags: Vec<String>,
}

fn default_multiqc_image() -> String {
    "multiqc/multiqc:v1.21".to_string()
}
fn default_scan_patterns() -> Vec<String> {
    vec![
        "*_fastqc.zip".into(),
        "*.flagstat".into(),
        "*.idxstats".into(),
        "*.stats".into(),
        "*_metrics.txt".into(),
        "*.log".into(),
        "qualimap_report/".into(),
        "dedup_metrics.txt".into(),
    ]
}
fn default_report_mime_type() -> String {
    "text/html".to_string()
}
fn default_report_name_template() -> String {
    "MultiQC Report — {workflow_type} run {run_id}".to_string()
}
fn default_report_tags() -> Vec<String> {
    vec!["multiqc".into(), "qc-report".into(), "automated".into()]
}

impl Default for MultiQCConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            run_for: vec![],
            image: default_multiqc_image(),
            scan_patterns: default_scan_patterns(),
            report_mime_type: default_report_mime_type(),
            report_name_template: default_report_name_template(),
            report_tags: default_report_tags(),
        }
    }
}

fn default_true() -> bool {
    true
}

impl FerrumConfig {
    /// Default config file paths in layer order (later overrides earlier).
    fn default_paths() -> Vec<PathBuf> {
        let mut paths = vec![
            PathBuf::from("/etc/ferrum/config.toml"),
            PathBuf::from("config.toml"),
        ];
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join(".ferrum/config.toml"));
        }
        if let Ok(config_path) = std::env::var("FERRUM_CONFIG") {
            paths.push(PathBuf::from(config_path));
        }
        paths
    }

    /// Build config from layered sources: defaults, then files (if exist), then env.
    fn build_builder(explicit_path: Option<&Path>) -> Result<config::Config, config::ConfigError> {
        use config::Environment;

        let mut builder = config::Config::builder()
            .set_default("bind", "0.0.0.0:8080")?
            .set_default("auth.require_auth", false)?
            .set_default("database.max_connections", default_max_connections() as i64)?
            .set_default("database.min_connections", default_min_connections() as i64)?
            .set_default(
                "database.acquire_timeout_secs",
                default_acquire_timeout_secs() as i64,
            )?
            .set_default(
                "database.idle_timeout_secs",
                default_idle_timeout_secs() as i64,
            )?
            .set_default(
                "database.max_lifetime_secs",
                default_max_lifetime_secs() as i64,
            )?
            .set_default("database.run_migrations", true)?
            .set_default("database.driver", "sqlite")?
            .set_default("database.sqlite_path", "ferrum.db")?
            .set_default("storage.backend", "local")?
            .set_default("services.enable_drs", true)?
            .set_default("services.enable_trs", true)?
            .set_default("services.enable_wes", true)?
            .set_default("services.enable_tes", true)?
            .set_default("services.enable_passports", true)?
            .set_default("services.enable_crypt4gh", true)?
            .set_default("services.enable_beacon", true)?
            .set_default("services.enable_htsget", true)?
            .set_default("encryption.crypt4gh_decrypt_stream", true)?
            .set_default("encryption.crypt4gh_master_key_id", "node")?
            .set_default("ingest.default_encrypt_upload", false)?
            .set_default("mii_connect.enabled", false)?
            .set_default(
                "mii_connect.modules",
                vec![
                    "person",
                    "encounter",
                    "consent",
                    "diagnosis",
                    "procedure",
                    "laboratory",
                    "medication",
                    "oncology",
                    "pathology_report",
                    "molecular_genetic_report",
                    "molecular_tumor_board",
                    "microbiology",
                    "imaging",
                    "intensive_care",
                    "biobank",
                    "document",
                    "research_study",
                ],
            )?
            .set_default("mii_connect.profile_set_version", "mii-kds-default17-v1")?
            .set_default("mii_connect.strict_mode", false)?
            .set_default("mii_connect.offline_only", true)?
            .set_default("mii_connect.manifest_path", "profiles/mii/manifest.json")?
            .set_default("pricing.enabled", false)?
            .set_default("pricing.currency", "USD")?
            .set_default("pricing.cpu_core_hour", 0.048)?
            .set_default("pricing.memory_gb_hour", 0.006)?
            .set_default("pricing.storage_gb_month", 0.023)?;

        let paths: Vec<PathBuf> = if let Some(p) = explicit_path {
            vec![p.to_path_buf()]
        } else {
            Self::default_paths()
        };

        for path in paths {
            if path.exists() {
                builder = builder.add_source(config::File::from(path.clone()).required(false));
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("FERRUM")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        );

        builder.build()
    }

    /// Load config from layered defaults: /etc/ferrum, ~/.ferrum, FERRUM_CONFIG, then FERRUM_* env.
    pub fn load() -> Result<Self, config::ConfigError> {
        let c = Self::build_builder(None)?;
        let mut cfg: Self = c.try_deserialize()?;
        cfg.resolve_file_secrets();
        cfg.apply_embedded_defaults();
        Ok(cfg)
    }

    /// Load config from an explicit path (e.g. --config path.toml), then apply env overrides.
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
        let path = path.as_ref();
        let c = Self::build_builder(Some(path))?;
        let mut cfg: Self = c.try_deserialize()?;
        cfg.resolve_file_secrets();
        cfg.apply_embedded_defaults();
        Ok(cfg)
    }

    /// True when embedded (SQLite + local storage) backends should be used.
    pub fn uses_embedded_backends(&self) -> bool {
        self.is_offline_first()
    }

    /// Offline-first: explicit profile, `FERRUM_OFFLINE=1`, or SQLite without explicit Postgres URL.
    pub fn is_offline_first(&self) -> bool {
        if std::env::var("FERRUM_OFFLINE")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        {
            return true;
        }
        if self.africa.as_ref().is_some_and(|a| a.offline_first) {
            return true;
        }
        if let Some(ref url) = self.database.url {
            let lower = url.split('?').next().unwrap_or(url).to_lowercase();
            return lower.starts_with("sqlite:") || lower.starts_with("sqlite://");
        }
        self.database.driver.eq_ignore_ascii_case("sqlite")
    }

    /// Apply Africa / laptop profile defaults to database and storage when embedded mode is active.
    pub fn apply_embedded_defaults(&mut self) {
        if !self.is_offline_first() {
            return;
        }
        if self.database.url.is_none() {
            self.database.driver = "sqlite".to_string();
            if let Some(ref africa) = self.africa {
                if let Some(ref p) = africa.sqlite_path {
                    self.database.sqlite_path = p.to_string_lossy().into_owned();
                }
            }
            if self.database.sqlite_path == "ferrum.db" {
                if let Some(home) = default_ferrum_home() {
                    self.database.sqlite_path =
                        home.join("ferrum.db").to_string_lossy().into_owned();
                }
            }
        }
        if !self.storage.backend.eq_ignore_ascii_case("s3")
            && !self.storage.backend.eq_ignore_ascii_case("minio")
        {
            self.storage.backend = "local".to_string();
            if self.storage.base_path.is_none() {
                let objects = self
                    .africa
                    .as_ref()
                    .and_then(|a| a.objects_path.clone())
                    .or_else(|| default_ferrum_home().map(|h| h.join("objects")));
                if let Some(p) = objects {
                    self.storage.base_path = Some(p.to_string_lossy().into_owned());
                }
            }
        }
    }

    /// A02: Resolve file:// references in secret fields (Docker/K8s secrets pattern).
    pub fn resolve_file_secrets(&mut self) {
        if let Some(ref s) = self.auth.jwt_secret {
            if let Some(resolved) = resolve_file_secret(s) {
                self.auth.jwt_secret = Some(resolved);
            }
        }
        if let Some(ref url) = self.database.url {
            if let Some(resolved) = resolve_file_secret(url) {
                self.database.url = Some(resolved);
            }
        }
        if let Some(ref s) = self.storage.s3_secret_access_key {
            if let Some(resolved) = resolve_file_secret(s) {
                self.storage.s3_secret_access_key = Some(resolved);
            }
        }
        if let Some(ref mut email) = self.email {
            if let Some(ref s) = email.smtp_password {
                if let Some(resolved) = resolve_file_secret(s) {
                    email.smtp_password = Some(resolved);
                }
            }
        }
    }
}

/// If value is file:///path, read file and return contents (trimmed). Otherwise None.
fn resolve_file_secret(value: &str) -> Option<String> {
    let path = value.strip_prefix("file://")?.trim();
    let path = std::path::Path::new(path);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn default_ferrum_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".ferrum"))
}

/// Backward-compatible alias.
pub type AppConfig = FerrumConfig;
