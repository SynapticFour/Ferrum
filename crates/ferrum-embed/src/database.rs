//! [`Database`] trait and embedded / production storage backends.

use async_trait::async_trait;
use ferrum_core::{DbDialect, FerrumConfig, FerrumError, FerrumPool, Result};

/// Storage backend abstraction shared by PostgreSQL and SQLite implementations.
#[async_trait]
pub trait Database: Send + Sync {
    fn pool(&self) -> &FerrumPool;
    fn dialect(&self) -> DbDialect;
    async fn migrate(&self) -> Result<()>;
}

/// SQLite-backed embedded database (laptop / offline mode).
pub struct SqliteStorage {
    pool: FerrumPool,
}

impl SqliteStorage {
    pub async fn connect(cfg: &FerrumConfig) -> Result<Self> {
        let pool = FerrumPool::from_config(&cfg.database).await?;
        if pool.dialect() != DbDialect::Sqlite {
            return Err(FerrumError::ValidationError(
                "SqliteStorage requires SQLite configuration".to_string(),
            ));
        }
        Ok(Self { pool })
    }

    pub async fn connect_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    FerrumError::ValidationError(format!("create db parent dir: {e}"))
                })?;
            }
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        Ok(Self {
            pool: FerrumPool::Sqlite(pool),
        })
    }
}

#[async_trait]
impl Database for SqliteStorage {
    fn pool(&self) -> &FerrumPool {
        &self.pool
    }

    fn dialect(&self) -> DbDialect {
        DbDialect::Sqlite
    }

    async fn migrate(&self) -> Result<()> {
        if let Some(p) = self.pool.as_sqlite() {
            sqlx::migrate!("./migrations").run(p).await?;
        }
        Ok(())
    }
}

/// PostgreSQL production database (unchanged behaviour).
pub struct PostgresStorage {
    pool: FerrumPool,
}

impl PostgresStorage {
    pub async fn connect(cfg: &FerrumConfig) -> Result<Self> {
        let pool = FerrumPool::from_config(&cfg.database).await?;
        if pool.dialect() != DbDialect::Postgres {
            return Err(FerrumError::ValidationError(
                "PostgresStorage requires PostgreSQL configuration".to_string(),
            ));
        }
        Ok(Self { pool })
    }
}

#[async_trait]
impl Database for PostgresStorage {
    fn pool(&self) -> &FerrumPool {
        &self.pool
    }

    fn dialect(&self) -> DbDialect {
        DbDialect::Postgres
    }

    async fn migrate(&self) -> Result<()> {
        if let Some(p) = self.pool.as_postgres() {
            sqlx::migrate!("../ferrum-core/migrations").run(p).await?;
        }
        Ok(())
    }
}
