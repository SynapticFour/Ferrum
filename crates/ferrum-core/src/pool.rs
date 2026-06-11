//! Unified database pool for PostgreSQL and SQLite (laptop / offline mode).

use crate::config::DatabaseConfig;
use crate::error::{FerrumError, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{PgPool, SqlitePool};
use std::time::Duration;

/// SQL dialect for portable query fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbDialect {
    Postgres,
    Sqlite,
}

/// Unified pool backing both production PostgreSQL and embedded SQLite.
#[derive(Clone)]
pub enum FerrumPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

impl FerrumPool {
    pub fn dialect(&self) -> DbDialect {
        match self {
            Self::Postgres(_) => DbDialect::Postgres,
            Self::Sqlite(_) => DbDialect::Sqlite,
        }
    }

    pub fn as_postgres(&self) -> Option<&PgPool> {
        match self {
            Self::Postgres(p) => Some(p),
            Self::Sqlite(_) => None,
        }
    }

    pub fn as_sqlite(&self) -> Option<&SqlitePool> {
        match self {
            Self::Sqlite(p) => Some(p),
            Self::Postgres(_) => None,
        }
    }

    pub async fn from_config(cfg: &DatabaseConfig) -> Result<Self> {
        let url = database_url(cfg)?;
        let dialect = detect_dialect(&url, &cfg.driver);
        match dialect {
            DbDialect::Postgres => {
                let max_c = cfg.max_connections.max(1);
                let min_c = cfg.min_connections.min(max_c).max(1).min(max_c);
                let pool = PgPoolOptions::new()
                    .max_connections(max_c)
                    .min_connections(min_c)
                    .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs.max(1)))
                    .idle_timeout(Some(Duration::from_secs(cfg.idle_timeout_secs.max(1))))
                    .max_lifetime(Some(Duration::from_secs(cfg.max_lifetime_secs.max(60))))
                    .connect(&url)
                    .await?;
                Ok(Self::Postgres(pool))
            }
            DbDialect::Sqlite => {
                use sqlx::sqlite::SqliteConnectOptions;
                let sqlite_max = cfg.max_connections.clamp(1, 5);
                if let Some(parent) = std::path::Path::new(&cfg.sqlite_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                }
                let options = SqliteConnectOptions::new()
                    .filename(&cfg.sqlite_path)
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(sqlite_max)
                    .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs.max(1)))
                    .connect_with(options)
                    .await?;
                Ok(Self::Sqlite(pool))
            }
        }
    }

    pub async fn from_url(url: &str) -> Result<Self> {
        let dialect = detect_dialect(url, "");
        match dialect {
            DbDialect::Postgres => {
                let pool = PgPoolOptions::new()
                    .max_connections(10)
                    .connect(url)
                    .await?;
                Ok(Self::Postgres(pool))
            }
            DbDialect::Sqlite => {
                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect(url)
                    .await?;
                Ok(Self::Sqlite(pool))
            }
        }
    }

    pub async fn run_migrations(&self, migrator: sqlx::migrate::Migrator) -> Result<()> {
        match self {
            Self::Postgres(p) => {
                migrator.run(p).await?;
            }
            Self::Sqlite(p) => {
                migrator.run(p).await?;
            }
        }
        Ok(())
    }
}

/// Dispatch the same sqlx query to Postgres or SQLite.
#[macro_export]
macro_rules! ferrum_db {
    ($pool:expr, |$p:ident| $body:expr) => {
        match $pool {
            $crate::FerrumPool::Postgres($p) => $body,
            $crate::FerrumPool::Sqlite($p) => $body,
        }
    };
}

fn database_url(cfg: &DatabaseConfig) -> Result<String> {
    if let Some(ref u) = cfg.url {
        return Ok(u.clone());
    }
    if cfg.driver.eq_ignore_ascii_case("sqlite") {
        let path = std::path::Path::new(&cfg.sqlite_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        return Ok(format!("sqlite://{}", cfg.sqlite_path));
    }
    if cfg.driver.eq_ignore_ascii_case("postgres") || cfg.driver.eq_ignore_ascii_case("postgresql")
    {
        let host = cfg.postgres_host.as_deref().unwrap_or("localhost");
        let port = cfg.postgres_port;
        let db = cfg.postgres_db.as_deref().unwrap_or("ferrum");
        let user = cfg.postgres_user.as_deref().unwrap_or("ferrum");
        let password = cfg.postgres_password.as_deref().unwrap_or("");
        return Ok(format!(
            "postgres://{}:{}@{}:{}/{}",
            user, password, host, port, db
        ));
    }
    Err(FerrumError::ValidationError(format!(
        "Unknown database driver: {}",
        cfg.driver
    )))
}

fn detect_dialect(url: &str, driver: &str) -> DbDialect {
    let lower = url.split('?').next().unwrap_or(url).to_lowercase();
    if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
        DbDialect::Postgres
    } else if lower.starts_with("sqlite:") || lower.starts_with("sqlite://") {
        DbDialect::Sqlite
    } else if driver.eq_ignore_ascii_case("postgres") || driver.eq_ignore_ascii_case("postgresql") {
        DbDialect::Postgres
    } else {
        DbDialect::Sqlite
    }
}

/// Connect a typed PostgreSQL pool for services that still require `PgPool`.
pub async fn postgres_pool_from_config(cfg: &DatabaseConfig) -> Result<PgPool> {
    let url = database_url(cfg)?;
    if detect_dialect(&url, &cfg.driver) != DbDialect::Postgres {
        return Err(FerrumError::ValidationError(
            "expected PostgreSQL database URL".to_string(),
        ));
    }
    let max_c = cfg.max_connections.max(1);
    let min_c = cfg.min_connections.min(max_c).max(1).min(max_c);
    PgPoolOptions::new()
        .max_connections(max_c)
        .min_connections(min_c)
        .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs.max(1)))
        .idle_timeout(Some(Duration::from_secs(cfg.idle_timeout_secs.max(1))))
        .max_lifetime(Some(Duration::from_secs(cfg.max_lifetime_secs.max(60))))
        .connect(&url)
        .await
        .map_err(Into::into)
}
