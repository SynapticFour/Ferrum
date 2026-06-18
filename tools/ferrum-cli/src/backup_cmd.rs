//! Field backup and integrity CLI (Phase 6).

use ferrum_core::{
    create_field_backup, restore_field_backup, verify_local_checksums, DatabasePool, FerrumConfig,
    FerrumPool,
};
use ferrum_core::{resolve_objects_root, IntegrityReport};
use std::path::{Path, PathBuf};

async fn edge_pool(config: Option<&PathBuf>) -> Result<(FerrumConfig, FerrumPool), String> {
    let mut cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found (pass --config or set FERRUM_CONFIG)".to_string())?;
    cfg.database.run_migrations = false;
    let db = DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    let pool = match db {
        DatabasePool::Postgres(p) => FerrumPool::Postgres(p),
        DatabasePool::Sqlite(p) => FerrumPool::Sqlite(p),
    };
    Ok((cfg, pool))
}

pub fn backup_create(
    output: &Path,
    include_objects: bool,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found".to_string())?;
    let manifest = create_field_backup(&cfg, output, include_objects).map_err(|e| e.to_string())?;
    println!(
        "Wrote backup to {} (objects={})",
        output.display(),
        manifest.includes_objects
    );
    Ok(())
}

pub fn backup_restore(archive: &Path, force: bool, config: Option<&PathBuf>) -> Result<(), String> {
    let cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found".to_string())?;
    restore_field_backup(&cfg, archive, force).map_err(|e| e.to_string())?;
    println!("Restored backup from {}", archive.display());
    Ok(())
}

pub async fn backup_verify(config: Option<&PathBuf>) -> Result<IntegrityReport, String> {
    let (cfg, pool) = edge_pool(config).await?;
    let objects_root = resolve_objects_root(&cfg);
    let report = verify_local_checksums(&pool, &objects_root)
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "Integrity: checked={} ok={} mismatches={} missing={} no_checksum={}",
        report.checked,
        report.ok,
        report.checksum_mismatches,
        report.missing_files,
        report.no_checksum
    );
    for err in &report.errors {
        eprintln!("  {err}");
    }
    if !report.is_clean() {
        return Err(format!(
            "integrity check failed ({} issue(s))",
            report.errors.len()
        ));
    }
    Ok(report)
}
