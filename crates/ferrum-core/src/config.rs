//! Layered configuration: defaults, /etc/ferrum, ~/.ferrum, FERRUM_ env, optional --config file.

use serde::Deserialize;
use std::path::{Path, PathBuf};

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

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL (overrides driver/params). Env: FERRUM_DATABASE__URL
    pub url: Option<String>,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
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
    10
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
    /// A07: Reject tokens older than this many hours (even if not expired). Default 24.
    #[serde(default = "default_max_token_age_hours")]
    pub max_token_age_hours: u32,
}

fn default_max_token_age_hours() -> u32 {
    24
}

/// Placeholder for encryption (e.g. Crypt4GH) settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default)]
    pub enabled: bool,
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
            .set_default("database.max_connections", 10i64)?
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
                builder = builder.add_source(
                    config::File::from(path.clone()).required(false),
                );
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("FERRUM").separator("__").try_parsing(true),
        );

        Ok(builder.build()?)
    }

    /// Load config from layered defaults: /etc/ferrum, ~/.ferrum, FERRUM_CONFIG, then FERRUM_* env.
    pub fn load() -> Result<Self, config::ConfigError> {
        let c = Self::build_builder(None)?;
        let mut cfg: Self = c.try_deserialize()?;
        cfg.resolve_file_secrets();
        Ok(cfg)
    }

    /// Load config from an explicit path (e.g. --config path.toml), then apply env overrides.
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
        let path = path.as_ref();
        let c = Self::build_builder(Some(path))?;
        let mut cfg: Self = c.try_deserialize()?;
        cfg.resolve_file_secrets();
        Ok(cfg)
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
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Backward-compatible alias.
pub type AppConfig = FerrumConfig;
