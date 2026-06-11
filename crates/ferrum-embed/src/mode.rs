//! Backend selection: full production stack vs embedded laptop mode.

use ferrum_core::FerrumConfig;

/// Deployment backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedMode {
    /// PostgreSQL + S3/MinIO (existing production behaviour).
    Full,
    /// SQLite + local filesystem storage.
    Sqlite,
    /// Detect from config: SQLite when offline-first / no Postgres URL, else Full.
    Auto,
}

impl EmbedMode {
    pub fn resolve(cfg: &FerrumConfig) -> Self {
        if cfg.is_offline_first() {
            return EmbedMode::Sqlite;
        }
        if let Some(ref url) = cfg.database.url {
            let lower = url.split('?').next().unwrap_or(url).to_lowercase();
            if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
                return EmbedMode::Full;
            }
            if lower.starts_with("sqlite:") || lower.starts_with("sqlite://") {
                return EmbedMode::Sqlite;
            }
        }
        if cfg.database.driver.eq_ignore_ascii_case("postgres")
            || cfg.database.driver.eq_ignore_ascii_case("postgresql")
        {
            EmbedMode::Full
        } else {
            EmbedMode::Sqlite
        }
    }
}
