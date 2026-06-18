//! Field operations: SQLite backup/restore and local object integrity checks (Phase 6).

use crate::config::FerrumConfig;
use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use crate::sync_export::resolve_objects_root;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder, Header};

const BACKUP_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const DB_ENTRY: &str = "ferrum.db";
const OBJECTS_PREFIX: &str = "objects/";

/// Operations & resilience settings for Edge deployments.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct OperationsConfig {
    /// When true, verify local object SHA-256 checksums against DRS metadata on gateway startup.
    pub verify_checksums_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub created_at: String,
    pub includes_objects: bool,
    pub sqlite_entry: String,
    pub objects_prefix: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IntegrityReport {
    pub checked: usize,
    pub ok: usize,
    pub missing_files: usize,
    pub checksum_mismatches: usize,
    pub no_checksum: usize,
    pub errors: Vec<String>,
}

impl IntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.missing_files == 0 && self.checksum_mismatches == 0 && self.errors.is_empty()
    }
}

/// Resolve SQLite database path from layered config.
pub fn resolve_sqlite_path(cfg: &FerrumConfig) -> PathBuf {
    PathBuf::from(&cfg.database.sqlite_path)
}

/// Create a gzip-compressed tar backup of the Edge SQLite DB and optional local objects tree.
pub fn create_field_backup(
    cfg: &FerrumConfig,
    output: &Path,
    include_objects: bool,
) -> Result<BackupManifest> {
    let db_path = resolve_sqlite_path(cfg);
    if !db_path.is_file() {
        return Err(FerrumError::ValidationError(format!(
            "SQLite database not found: {}",
            db_path.display()
        )));
    }
    let objects_root = resolve_objects_root(cfg);
    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        created_at: chrono::Utc::now().to_rfc3339(),
        includes_objects: include_objects,
        sqlite_entry: DB_ENTRY.into(),
        objects_prefix: OBJECTS_PREFIX.into(),
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| FerrumError::StorageError(e.into()))?;
    }
    let file = File::create(output).map_err(|e| FerrumError::StorageError(e.into()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);
    append_file_to_tar(&mut tar, DB_ENTRY, &db_path)?;
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| FerrumError::Internal(e.into()))?;
    append_bytes_to_tar(&mut tar, MANIFEST_NAME, &manifest_bytes)?;
    if include_objects && objects_root.is_dir() {
        append_dir_recursive(&mut tar, &objects_root, OBJECTS_PREFIX)?;
    }
    tar.finish()
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    Ok(manifest)
}

/// Restore a field backup into configured sqlite/objects paths.
pub fn restore_field_backup(cfg: &FerrumConfig, archive: &Path, force: bool) -> Result<()> {
    let file = File::open(archive).map_err(|e| FerrumError::StorageError(e.into()))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut tar = Archive::new(dec);
    let tmp = tempfile::tempdir().map_err(|e| FerrumError::StorageError(e.into()))?;
    tar.unpack(tmp.path())
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    let manifest_path = tmp.path().join(MANIFEST_NAME);
    let raw =
        fs::read_to_string(&manifest_path).map_err(|e| FerrumError::StorageError(e.into()))?;
    let manifest: BackupManifest =
        serde_json::from_str(&raw).map_err(|e| FerrumError::Internal(e.into()))?;
    if manifest.version != BACKUP_VERSION {
        return Err(FerrumError::ValidationError(format!(
            "unsupported backup version {}",
            manifest.version
        )));
    }
    let db_src = tmp.path().join(&manifest.sqlite_entry);
    let db_dest = resolve_sqlite_path(cfg);
    restore_path(&db_src, &db_dest, force)?;
    if manifest.includes_objects {
        let objects_src = tmp
            .path()
            .join(manifest.objects_prefix.trim_end_matches('/'));
        let objects_dest = resolve_objects_root(cfg);
        if objects_src.is_dir() {
            if objects_dest.exists() && !force {
                return Err(FerrumError::ValidationError(format!(
                    "objects directory {} exists; pass --force to overwrite",
                    objects_dest.display()
                )));
            }
            if objects_dest.exists() {
                fs::remove_dir_all(&objects_dest)
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
            }
            copy_dir_all(&objects_src, &objects_dest)?;
        }
    }
    Ok(())
}

/// Verify local object bytes against stored SHA-256 checksums in DRS.
pub async fn verify_local_checksums(
    pool: &FerrumPool,
    objects_root: &Path,
) -> Result<IntegrityReport> {
    let rows = load_local_checksum_rows(pool).await?;
    let mut report = IntegrityReport::default();
    for (object_id, storage_key, expected) in rows {
        report.checked += 1;
        let path = objects_root.join(&storage_key);
        if !path.is_file() {
            report.missing_files += 1;
            report
                .errors
                .push(format!("{object_id}: missing file {}", path.display()));
            continue;
        }
        let Some(expected_sha) = expected else {
            report.no_checksum += 1;
            continue;
        };
        match sha256_file(&path) {
            Ok(actual) => {
                if actual.eq_ignore_ascii_case(&expected_sha) {
                    report.ok += 1;
                } else {
                    report.checksum_mismatches += 1;
                    report.errors.push(format!(
                        "{object_id}: checksum mismatch (expected {expected_sha}, got {actual})"
                    ));
                }
            }
            Err(e) => {
                report.errors.push(format!("{object_id}: read failed: {e}"));
            }
        }
    }
    Ok(report)
}

async fn load_local_checksum_rows(
    pool: &FerrumPool,
) -> Result<Vec<(String, String, Option<String>)>> {
    let sql = "SELECT o.id, r.storage_key, c.checksum
               FROM drs_objects o
               JOIN storage_references r ON r.object_id = o.id
               LEFT JOIN drs_checksums c ON c.object_id = o.id AND c.type = 'sha-256'
               WHERE r.storage_backend = 'local'
               ORDER BY o.created_time ASC";
    match pool {
        FerrumPool::Postgres(p) => {
            let rows: Vec<(String, String, Option<String>)> =
                sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows)
        }
        FerrumPool::Sqlite(p) => {
            let rows: Vec<(String, String, Option<String>)> =
                sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows)
        }
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn restore_path(src: &Path, dest: &Path, force: bool) -> Result<()> {
    if dest.exists() && !force {
        return Err(FerrumError::ValidationError(format!(
            "{} exists; pass --force to overwrite",
            dest.display()
        )));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| FerrumError::StorageError(e.into()))?;
    }
    fs::copy(src, dest).map_err(|e| FerrumError::StorageError(e.into()))?;
    Ok(())
}

fn append_bytes_to_tar<W: Write>(tar: &mut Builder<W>, name: &str, data: &[u8]) -> Result<()> {
    let mut header = Header::new_gnu();
    header
        .set_path(name)
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, data)
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    Ok(())
}

fn append_file_to_tar<W: Write>(tar: &mut Builder<W>, name: &str, src: &Path) -> Result<()> {
    let mut file = File::open(src).map_err(|e| FerrumError::StorageError(e.into()))?;
    let len = file
        .metadata()
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .len();
    let mut header = Header::new_gnu();
    header
        .set_path(name)
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    header.set_size(len);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, &mut file)
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    Ok(())
}

fn append_dir_recursive<W: Write>(tar: &mut Builder<W>, dir: &Path, prefix: &str) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|e| FerrumError::StorageError(e.into()))? {
        let entry = entry.map_err(|e| FerrumError::StorageError(e.into()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(dir)
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        let dest = format!("{}{}", prefix, rel.to_string_lossy());
        if path.is_dir() {
            append_dir_recursive(tar, &path, &format!("{dest}/"))?;
        } else if path.is_file() {
            append_file_to_tar(tar, &dest, &path)?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|e| FerrumError::StorageError(e.into()))?;
    for entry in fs::read_dir(src).map_err(|e| FerrumError::StorageError(e.into()))? {
        let entry = entry.map_err(|e| FerrumError::StorageError(e.into()))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| FerrumError::StorageError(e.into()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FerrumConfig;
    use crate::pool::FerrumPool;
    use sqlx::sqlite::SqlitePoolOptions;

    fn test_config(tmp: &Path, db: &Path, objects: &Path) -> FerrumConfig {
        let cfg_path = tmp.join("config.toml");
        let toml = format!(
            r#"
bind = "127.0.0.1:0"
[database]
driver = "sqlite"
sqlite_path = "{db}"
run_migrations = true
[storage]
backend = "local"
base_path = "{objects}"
[africa]
offline_first = true
sqlite_path = "{db}"
objects_path = "{objects}"
"#,
            db = db.display(),
            objects = objects.display()
        );
        fs::write(&cfg_path, toml).unwrap();
        FerrumConfig::load_from_path(&cfg_path).unwrap()
    }

    async fn seed_object(pool: &FerrumPool, object_id: &str, storage_key: &str, sha256: &str) {
        match pool {
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "INSERT INTO drs_objects (id, name, size, created_time, updated_time, is_bundle)
                     VALUES ($1, 'test.bin', 4, datetime('now'), datetime('now'), 0)",
                )
                .bind(object_id)
                .execute(p)
                .await
                .unwrap();
                sqlx::query(
                    "INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
                     VALUES ($1, 'local', $2, 0)",
                )
                .bind(object_id)
                .bind(storage_key)
                .execute(p)
                .await
                .unwrap();
                sqlx::query(
                    "INSERT INTO drs_checksums (object_id, type, checksum) VALUES ($1, 'sha-256', $2)",
                )
                .bind(object_id)
                .bind(sha256)
                .execute(p)
                .await
                .unwrap();
            }
            FerrumPool::Postgres(_) => panic!("sqlite test only"),
        }
    }

    #[tokio::test]
    async fn backup_restore_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("ferrum.db");
        let objects = tmp.path().join("objects");
        fs::create_dir_all(&objects).unwrap();
        let cfg = test_config(tmp.path(), &db, &objects);
        let pool = SqlitePoolOptions::new()
            .connect(&format!("sqlite:{}?mode=rwc", db.display()))
            .await
            .unwrap();
        sqlx::migrate!("../ferrum-embed/migrations")
            .run(&pool)
            .await
            .unwrap();
        let pool = FerrumPool::Sqlite(pool);
        let key = "drs/obj1";
        let data = b"ACGT";
        fs::create_dir_all(objects.join("drs")).unwrap();
        fs::write(objects.join(key), data).unwrap();
        let sha = sha256_file(&objects.join(key)).unwrap();
        seed_object(&pool, "obj1", key, &sha).await;

        let backup = tmp.path().join("backup.tar.gz");
        create_field_backup(&cfg, &backup, true).unwrap();
        assert!(backup.is_file());

        fs::write(objects.join(key), b"XXXX").unwrap();
        fs::remove_file(&db).unwrap();
        restore_field_backup(&cfg, &backup, true).unwrap();
        assert_eq!(fs::read(objects.join(key)).unwrap(), data);
        assert!(db.is_file());

        let report = verify_local_checksums(&pool, &objects).await.unwrap();
        assert!(report.is_clean());
        assert_eq!(report.ok, 1);
    }

    #[tokio::test]
    async fn verify_detects_checksum_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("ferrum.db");
        let objects = tmp.path().join("objects");
        fs::create_dir_all(&objects).unwrap();
        let pool = SqlitePoolOptions::new()
            .connect(&format!("sqlite:{}?mode=rwc", db.display()))
            .await
            .unwrap();
        sqlx::migrate!("../ferrum-embed/migrations")
            .run(&pool)
            .await
            .unwrap();
        let pool = FerrumPool::Sqlite(pool);
        let key = "drs/bad";
        fs::create_dir_all(objects.join("drs")).unwrap();
        fs::write(objects.join(key), b"data").unwrap();
        seed_object(&pool, "bad1", key, "deadbeef").await;
        let report = verify_local_checksums(&pool, &objects).await.unwrap();
        assert_eq!(report.checksum_mismatches, 1);
        assert!(!report.is_clean());
    }
}
