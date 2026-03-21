//! Database layer: sqlx pool (SQLite/PostgreSQL auto-detect) and embedded migrations.

use crate::config::DatabaseConfig;
use crate::error::{FerrumError, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use std::time::Duration;

/// Database pool: SQLite (local) or PostgreSQL (production).
#[derive(Clone)]
pub enum DatabasePool {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
}

impl DatabasePool {
    /// Create pool from URL. Auto-detects scheme: `sqlite:` / `sqlite://` -> SQLite; `postgres://` / `postgresql://` -> PostgreSQL.
    pub async fn from_url(url: &str) -> Result<Self> {
        Self::from_url_with_options(url, 10, 5).await
    }

    /// Create pool from URL with given max connections (postgres, sqlite).
    pub async fn from_url_with_options(
        url: &str,
        postgres_max: u32,
        sqlite_max: u32,
    ) -> Result<Self> {
        let url_lower = url.split('?').next().unwrap_or(url).to_lowercase();
        if url_lower.starts_with("postgres://") || url_lower.starts_with("postgresql://") {
            // Lesson 10: pool sizing + timeouts — avoid unbounded waits when the pool is exhausted.
            // Source: sqlx production guidance + high-concurrency GA4GH API patterns.
            let pool = PgPoolOptions::new()
                .max_connections(postgres_max.max(1))
                .min_connections(2.min(postgres_max.max(1)))
                .acquire_timeout(Duration::from_secs(10))
                .idle_timeout(Some(Duration::from_secs(600)))
                .max_lifetime(Some(Duration::from_secs(1800)))
                .connect(url)
                .await?;
            return Ok(DatabasePool::Postgres(pool));
        }
        if url_lower.starts_with("sqlite://") || url_lower.starts_with("sqlite:") {
            let path = url
                .trim_start_matches("sqlite://")
                .trim_start_matches("sqlite:");
            let pool = SqlitePoolOptions::new()
                .max_connections(sqlite_max.max(1))
                .acquire_timeout(Duration::from_secs(10))
                .connect(&format!("sqlite:{}", path))
                .await?;
            return Ok(DatabasePool::Sqlite(pool));
        }
        Err(FerrumError::ValidationError(format!(
            "Unsupported database URL scheme: {}",
            url
        )))
    }

    /// Create pool from [DatabaseConfig]. Uses url if set, else builds URL from driver/params.
    pub async fn from_config(cfg: &DatabaseConfig) -> Result<Self> {
        let url = if let Some(ref u) = cfg.url {
            u.clone()
        } else if cfg.driver.eq_ignore_ascii_case("sqlite") {
            format!("sqlite:{}", cfg.sqlite_path)
        } else if cfg.driver.eq_ignore_ascii_case("postgres")
            || cfg.driver.eq_ignore_ascii_case("postgresql")
        {
            let host = cfg.postgres_host.as_deref().unwrap_or("localhost");
            let port = cfg.postgres_port;
            let db = cfg.postgres_db.as_deref().unwrap_or("ferrum");
            let user = cfg.postgres_user.as_deref().unwrap_or("ferrum");
            let password = cfg.postgres_password.as_deref().unwrap_or("");
            format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, db)
        } else {
            return Err(FerrumError::ValidationError(format!(
                "Unknown driver: {}",
                cfg.driver
            )));
        };

        let url_lower = url.split('?').next().unwrap_or(&url).to_lowercase();
        let mut pool =
            if url_lower.starts_with("postgres://") || url_lower.starts_with("postgresql://") {
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
                DatabasePool::Postgres(pool)
            } else if url_lower.starts_with("sqlite://") || url_lower.starts_with("sqlite:") {
                let path = url
                    .trim_start_matches("sqlite://")
                    .trim_start_matches("sqlite:");
                let sqlite_max = cfg.max_connections.max(1).min(5);
                let pool = SqlitePoolOptions::new()
                    .max_connections(sqlite_max)
                    .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs.max(1)))
                    .connect(&format!("sqlite:{}", path))
                    .await?;
                DatabasePool::Sqlite(pool)
            } else {
                return Err(FerrumError::ValidationError(format!(
                    "Unsupported database URL scheme: {}",
                    url
                )));
            };

        // Env override so demo/CI can force skip (init already ran migrations + seeds)
        let run_migrations = match std::env::var("FERRUM_DATABASE__RUN_MIGRATIONS").as_deref() {
            Ok("false") | Ok("0") | Ok("no") => false,
            Ok("true") | Ok("1") | Ok("yes") => true,
            _ => cfg.run_migrations,
        };
        if run_migrations {
            pool.run_migrations().await?;
        }

        Ok(pool)
    }

    /// Run embedded migrations (PostgreSQL and SQLite; migrations are written for PostgreSQL).
    pub async fn run_migrations(&mut self) -> Result<()> {
        match self {
            DatabasePool::Sqlite(p) => {
                sqlx::migrate!("./migrations").run(&*p).await?;
            }
            DatabasePool::Postgres(p) => {
                sqlx::migrate!("./migrations").run(&*p).await?;
            }
        }
        Ok(())
    }
}
