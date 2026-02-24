//! Database layer: sqlx pool (SQLite/PostgreSQL auto-detect) and embedded migrations.

use crate::config::DatabaseConfig;
use crate::error::{FerrumError, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;

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
    pub async fn from_url_with_options(url: &str, postgres_max: u32, sqlite_max: u32) -> Result<Self> {
        let url_lower = url.split('?').next().unwrap_or(url).to_lowercase();
        if url_lower.starts_with("postgres://") || url_lower.starts_with("postgresql://") {
            let pool = PgPoolOptions::new()
                .max_connections(postgres_max)
                .connect(url)
                .await?;
            return Ok(DatabasePool::Postgres(pool));
        }
        if url_lower.starts_with("sqlite://") || url_lower.starts_with("sqlite:") {
            let path = url
                .trim_start_matches("sqlite://")
                .trim_start_matches("sqlite:");
            let pool = SqlitePoolOptions::new()
                .max_connections(sqlite_max)
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
        } else if cfg.driver.eq_ignore_ascii_case("postgres") || cfg.driver.eq_ignore_ascii_case("postgresql") {
            let host = cfg.postgres_host.as_deref().unwrap_or("localhost");
            let port = cfg.postgres_port;
            let db = cfg.postgres_db.as_deref().unwrap_or("ferrum");
            let user = cfg.postgres_user.as_deref().unwrap_or("ferrum");
            let password = cfg.postgres_password.as_deref().unwrap_or("");
            format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, db)
        } else {
            return Err(FerrumError::ValidationError(format!("Unknown driver: {}", cfg.driver)));
        };

        let max_conn = cfg.max_connections;
        let mut pool = Self::from_url_with_options(&url, max_conn, max_conn.min(5)).await?;

        if cfg.run_migrations {
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
