//! Post-ingest pipeline hooks: htsget metadata + optional Beacon VCF indexing.

use crate::repo::DrsRepo;
use crate::state::AppState;
use ferrum_beacon::repo::BeaconRepo;
use ferrum_core::{classify_htsget_file, is_htsget_supported, is_vcf_like, PipelineConfig};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tracing::warn;

const HTSGET_INDEX_META_KEY: &str = "htsget_index_status";

/// Run configured post-ingest hooks (non-blocking background work where applicable).
pub fn schedule_post_ingest_hooks(
    state: Arc<AppState>,
    object_id: String,
    name: Option<String>,
    mime_type: Option<String>,
) {
    let cfg = state.pipeline.clone();
    if !cfg.auto_htsget_index && !cfg.auto_index_beacon {
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = run_post_ingest_hooks(
            state,
            &object_id,
            name.as_deref(),
            mime_type.as_deref(),
            &cfg,
        )
        .await
        {
            warn!(object_id = %object_id, error = %e, "post-ingest pipeline hook failed");
        }
    });
}

async fn run_post_ingest_hooks(
    state: Arc<AppState>,
    object_id: &str,
    name: Option<&str>,
    mime_type: Option<&str>,
    cfg: &PipelineConfig,
) -> Result<(), String> {
    if cfg.auto_htsget_index {
        let kind = classify_htsget_file(mime_type, name);
        if is_htsget_supported(kind) {
            state
                .repo
                .set_metadata(object_id, HTSGET_INDEX_META_KEY, "ready")
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if cfg.auto_index_beacon && is_vcf_like(name, mime_type) {
        spawn_vcf_beacon_index(
            state,
            object_id.to_string(),
            cfg.default_beacon_dataset.clone(),
        )
        .await;
    }
    Ok(())
}

async fn spawn_vcf_beacon_index(state: Arc<AppState>, object_id: String, dataset_id: String) {
    let _ = state.repo.set_vcf_index_status(&object_id, "pending").await;
    let storage = match state.storage.clone() {
        Some(s) => s,
        None => {
            let _ = state
                .repo
                .set_vcf_index_status(&object_id, "failed:no_storage")
                .await;
            return;
        }
    };
    if state
        .background_gate
        .as_ref()
        .is_some_and(|g| !g.allows_background_work())
    {
        let _ = state
            .repo
            .set_vcf_index_status(&object_id, "deferred_low_power")
            .await;
        return;
    }

    let repo = state.repo.clone();
    let _ = repo.set_vcf_index_status(&object_id, "running").await;

    let status = match index_vcf_object(&repo, &storage, &object_id, &dataset_id).await {
        Ok(n) if n > 0 => format!("completed:{n}"),
        Ok(_) => "completed:0".into(),
        Err(e) => format!("failed:{e}"),
    };
    let _ = repo.set_vcf_index_status(&object_id, &status).await;
}

async fn index_vcf_object(
    repo: &DrsRepo,
    storage: &Arc<dyn ferrum_storage::ObjectStorage>,
    object_id: &str,
    dataset_id: &str,
) -> Result<usize, String> {
    let Some((backend, storage_key, _enc)) = repo
        .get_storage_ref(object_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Err("no storage ref".into());
    };
    if backend != "local" {
        return Err("beacon auto-index only supports local storage on Edge".into());
    }
    let mut reader = storage.get(&storage_key).await.map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    let pool = repo.pool().clone();
    let beacon = BeaconRepo::new(pool.clone());
    beacon
        .ensure_dataset(dataset_id, object_id, None, "GRCh38")
        .await
        .map_err(|e| e.to_string())?;
    ferrum_beacon::vcf_index::index_vcf_bytes(&pool, dataset_id, &bytes)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn htsget_meta_key_is_stable() {
        assert_eq!(HTSGET_INDEX_META_KEY, "htsget_index_status");
    }
}
