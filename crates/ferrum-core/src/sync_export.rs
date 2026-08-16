// SPDX-License-Identifier: BUSL-1.1
//! Sneakernet export bundle: objects + ferrum-meta + audit slice for physical transfer.

use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use crate::residency::ResidencyAuditLog;
use crate::sync_queue::{list_queue_items, SyncQueueItem};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::Builder;

#[derive(Debug, serde::Serialize)]
pub struct SneakernetManifest {
    pub version: String,
    pub exported_at: String,
    pub queue_items: Vec<SyncQueueItem>,
    pub object_ids: Vec<String>,
    pub metadata_refs: Vec<String>,
}

/// Build a gzip-compressed tar export for sneakernet / USB transfer.
pub async fn build_sneakernet_bundle(
    pool: &FerrumPool,
    objects_root: &Path,
    output: &Path,
    gisaid_archive: Option<Vec<u8>>,
) -> Result<SneakernetManifest> {
    let queue_items = list_queue_items(pool, None).await?;
    let object_ids: Vec<String> = if queue_items.is_empty() {
        crate::sync_queue::list_local_object_ids(pool).await?
    } else {
        queue_items.iter().map(|i| i.object_id.clone()).collect()
    };

    let mut metadata_refs = Vec::new();
    for oid in &object_ids {
        if let Some(info) = crate::sync_queue::load_object_sync_info(pool, oid).await? {
            if let Some(m) = info.metadata_ref {
                if !metadata_refs.contains(&m) {
                    metadata_refs.push(m);
                }
            }
        }
    }

    let manifest = SneakernetManifest {
        version: "1".into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        queue_items: queue_items.clone(),
        object_ids: object_ids.clone(),
        metadata_refs: metadata_refs.clone(),
    };

    let audit = ResidencyAuditLog::new(pool.clone());
    let audit_entries = audit
        .query_range(None, None)
        .await
        .map(|r| r.entries)
        .unwrap_or_default();

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| FerrumError::StorageError(e.into()))?;
    }
    let file = std::fs::File::create(output).map_err(|e| FerrumError::StorageError(e.into()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| FerrumError::ValidationError(e.to_string()))?;
    append_tar_bytes(&mut tar, "manifest.json", &manifest_bytes)?;

    for oid in &object_ids {
        if let Some(info) = crate::sync_queue::load_object_sync_info(pool, oid).await? {
            let src = objects_root.join(&info.storage_key);
            if src.is_file() {
                let dest = format!("objects/{}/{}", oid, info.name);
                append_tar_file(&mut tar, &dest, &src)?;
            }
        }
    }

    for alias in &metadata_refs {
        if let Some(doc) = crate::sync_queue::load_metadata_document(pool, alias).await? {
            let bytes = serde_json::to_vec_pretty(&doc)
                .map_err(|e| FerrumError::ValidationError(e.to_string()))?;
            append_tar_bytes(&mut tar, &format!("meta/{alias}.json"), &bytes)?;
        }
    }

    let audit_json = serde_json::to_vec_pretty(&json!({ "entries": audit_entries }))
        .map_err(|e| FerrumError::ValidationError(e.to_string()))?;
    append_tar_bytes(&mut tar, "audit/residency_slice.json", &audit_json)?;

    if let Some(gisaid) = gisaid_archive {
        append_tar_bytes(&mut tar, "gisaid/package.tar", &gisaid)?;
    }

    tar.finish()
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    Ok(manifest)
}

fn append_tar_bytes<W: Write>(tar: &mut Builder<W>, path: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, data)
        .map_err(|e| FerrumError::StorageError(e.into()))
}

fn append_tar_file<W: Write>(tar: &mut Builder<W>, dest: &str, src: &Path) -> Result<()> {
    let mut file = std::fs::File::open(src).map_err(|e| FerrumError::StorageError(e.into()))?;
    tar.append_file(dest, &mut file)
        .map_err(|e| FerrumError::StorageError(e.into()))
}

/// Resolve local object storage root from config paths.
pub fn resolve_objects_root(cfg: &crate::config::FerrumConfig) -> PathBuf {
    if let Some(ref africa) = cfg.africa {
        if let Some(ref p) = africa.objects_path {
            return p.clone();
        }
    }
    if let Some(ref p) = cfg.storage.base_path {
        return PathBuf::from(p);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".ferrum/objects");
    }
    PathBuf::from("objects")
}
