//! Startup bootstrap: paths, migrations, backend selection.

use crate::mode::EmbedMode;
use ferrum_core::FerrumConfig;
use std::path::{Path, PathBuf};

/// Resolved embed bootstrap state.
pub struct EmbedBootstrap {
    pub mode: EmbedMode,
    pub data_dir: PathBuf,
}

/// Resolve embed mode from config (Auto semantics).
pub fn resolve_embed_mode(cfg: &FerrumConfig) -> EmbedMode {
    EmbedMode::resolve(cfg)
}

/// Ensure ~/.ferrum data directories exist for laptop mode.
pub fn ensure_data_dirs(cfg: &FerrumConfig) -> std::io::Result<PathBuf> {
    let home = default_ferrum_home();
    if let Some(ref base) = home {
        std::fs::create_dir_all(base)?;
        if let Some(ref objects) = cfg.storage.base_path {
            std::fs::create_dir_all(objects)?;
        } else {
            std::fs::create_dir_all(base.join("objects"))?;
        }
        if cfg.database.driver.eq_ignore_ascii_case("sqlite") {
            if let Some(parent) = Path::new(&cfg.database.sqlite_path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
    }
    Ok(home.unwrap_or_else(|| PathBuf::from(".")))
}

pub fn default_ferrum_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".ferrum"))
}
