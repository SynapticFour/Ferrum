//! Embedded backends for offline-first / laptop Ferrum deployments.

mod bootstrap;
mod database;
mod memory;
mod mode;
mod offline;

pub use bootstrap::{default_ferrum_home, ensure_data_dirs, resolve_embed_mode, EmbedBootstrap};
pub use database::{Database, PostgresStorage, SqliteStorage};
pub use memory::{MemoryCapGuard, MemoryCapLevel, MemoryCapState};
pub use mode::EmbedMode;
pub use offline::{probe_auth_endpoints, probe_with_timeout, STARTUP_PROBE_TIMEOUT};

pub use ferrum_storage::LocalStorage as LocalObjectStorage;
