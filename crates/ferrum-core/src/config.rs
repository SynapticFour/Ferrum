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
    /// HMAC secret for JWT validation (HS256). Env: FERRUM_AUTH__JWT_SECRET
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
            .set_default("services.enable_beacon", true)?;

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
        c.try_deserialize()
    }

    /// Load config from an explicit path (e.g. --config path.toml), then apply env overrides.
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
        let path = path.as_ref();
        let c = Self::build_builder(Some(path))?;
        c.try_deserialize()
    }
}

/// Backward-compatible alias.
pub type AppConfig = FerrumConfig;
