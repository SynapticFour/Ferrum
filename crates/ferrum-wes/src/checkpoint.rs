//! Workflow resume and checkpointing: task hash, cache lookup, and DRS-backed checkpoint storage.

use crate::error::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;

/// Compute a deterministic hash for a workflow task.
/// Hash inputs: task name + sorted input checksums + container image digest + script content.
pub fn compute_task_hash(
    task_name: &str,
    input_checksums: &[String],
    container_image: &str,
    script_content: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(task_name.as_bytes());
    let mut sorted = input_checksums.to_vec();
    sorted.sort();
    for checksum in &sorted {
        hasher.update(checksum.as_bytes());
    }
    hasher.update(container_image.as_bytes());
    hasher.update(script_content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Info about a checkpointed task (for resume).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckpointInfo {
    pub task_name: String,
    pub task_hash: String,
    pub status: String,
    pub drs_object_ids: Vec<String>,
}

/// Store for checkpoints and cross-run cache. Optionally uses DRS ingest for output storage.
pub struct CheckpointStore {
    pool: PgPool,
    /// Base URL for DRS ingest (e.g. https://host/ga4gh/drs/v1). When None, save_checkpoint will fail with "checkpoint storage not configured".
    drs_ingest_base_url: Option<String>,
}

impl CheckpointStore {
    pub fn new(pool: PgPool, drs_ingest_base_url: Option<String>) -> Self {
        Self {
            pool,
            drs_ingest_base_url,
        }
    }

    /// Before running a task: check if we have a valid cache entry. Returns Some(drs_object_ids) on hit and increments hit_count.
    pub async fn check_cache(&self, task_hash: &str) -> Result<Option<Vec<String>>> {
        let row: Option<(Value,)> =
            sqlx::query_as("SELECT drs_object_ids FROM wes_cache_entries WHERE task_hash = $1")
                .bind(task_hash)
                .fetch_optional(&self.pool)
                .await?;
        let Some((ids_val,)) = row else {
            return Ok(None);
        };
        let ids: Vec<String> = ids_val
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return Ok(None);
        }
        sqlx::query(
            "UPDATE wes_cache_entries SET hit_count = hit_count + 1, last_used_at = now() WHERE task_hash = $1",
        )
        .bind(task_hash)
        .execute(&self.pool)
        .await?;
        Ok(Some(ids))
    }

    /// After a task completes successfully: store outputs as DRS objects and record checkpoint + cache entry.
    /// When drs_ingest_base_url is None, returns error. Caller may pass auth_header for DRS ingest.
    pub async fn save_checkpoint(
        &self,
        run_id: &str,
        task_name: &str,
        task_hash: &str,
        output_paths: &[std::path::PathBuf],
        auth_header: Option<&str>,
    ) -> Result<Vec<String>> {
        let base = self.drs_ingest_base_url.as_deref().ok_or_else(|| {
            crate::error::WesError::Validation(
                "checkpoint storage not configured (no DRS ingest URL)".into(),
            )
        })?;
        let mut drs_ids = Vec::with_capacity(output_paths.len());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .build()
            .map_err(|e| crate::error::WesError::Other(e.into()))?;
        let ingest_url = format!("{}/ingest/file", base.trim_end_matches('/'));
        for path in output_paths {
            if !path.exists() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str().map(String::from))
                .unwrap_or_else(|| "output".to_string());
            let data = tokio::fs::read(path)
                .await
                .map_err(crate::error::WesError::Io)?;
            let part = reqwest::multipart::Part::bytes(data)
                .file_name(name.clone())
                .mime_str("application/octet-stream")
                .map_err(|e| crate::error::WesError::Other(e.into()))?;
            let form = reqwest::multipart::Form::new()
                .part("file", part)
                .text("name", name)
                .text("encrypt", "true");
            let mut req = client.post(&ingest_url).multipart(form);
            if let Some(h) = auth_header {
                req = req.header("Authorization", h);
            }
            let res = req
                .send()
                .await
                .map_err(|e| crate::error::WesError::Other(e.into()))?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(crate::error::WesError::Other(anyhow::anyhow!(
                    "DRS ingest failed {}: {}",
                    status,
                    body
                )));
            }
            let json: serde_json::Value = res
                .json()
                .await
                .map_err(|e| crate::error::WesError::Other(e.into()))?;
            let id = json
                .get("object_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| {
                    crate::error::WesError::Other(anyhow::anyhow!(
                        "DRS ingest response missing object_id"
                    ))
                })?;
            drs_ids.push(id);
        }
        let ids_json = serde_json::to_value(&drs_ids).unwrap_or(Value::Array(vec![]));
        let id = ulid::Ulid::new().to_string();
        sqlx::query(
            r#"INSERT INTO wes_checkpoints (id, run_id, task_name, task_hash, status, drs_object_ids)
               VALUES ($1, $2, $3, $4, 'complete', $5)
               ON CONFLICT (run_id, task_name, task_hash) DO UPDATE SET status = 'complete', drs_object_ids = $5, created_at = now()"#,
        )
        .bind(&id)
        .bind(run_id)
        .bind(task_name)
        .bind(task_hash)
        .bind(&ids_json)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            r#"INSERT INTO wes_cache_entries (task_hash, drs_object_ids, hit_count, last_used_at, created_at)
               VALUES ($1, $2, 0, now(), now())
               ON CONFLICT (task_hash) DO UPDATE SET drs_object_ids = $2, last_used_at = now()"#,
        )
        .bind(task_hash)
        .bind(&ids_json)
        .execute(&self.pool)
        .await?;
        Ok(drs_ids)
    }

    /// For a resume run: list all tasks with valid checkpoints (status = 'complete').
    pub async fn get_resumable_tasks(&self, run_id: &str) -> Result<Vec<CheckpointInfo>> {
        let rows: Vec<(String, String, String, Value)> = sqlx::query_as(
            r#"SELECT task_name, task_hash, status, drs_object_ids FROM wes_checkpoints
               WHERE run_id = $1 AND status = 'complete'"#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(task_name, task_hash, status, drs_object_ids)| {
                let ids: Vec<String> = drs_object_ids
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                CheckpointInfo {
                    task_name,
                    task_hash,
                    status,
                    drs_object_ids: ids,
                }
            })
            .collect())
    }

    /// Evict old cache entries. Returns number of entries removed.
    pub async fn evict_stale_entries(
        &self,
        max_age_days: u32,
        max_entries: Option<usize>,
    ) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let deleted = if let Some(limit) = max_entries {
            let ids: Vec<String> = sqlx::query_scalar(
                r#"SELECT task_hash FROM wes_cache_entries WHERE last_used_at < $1 ORDER BY last_used_at ASC LIMIT $2"#,
            )
            .bind(cutoff)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;
            for id in &ids {
                sqlx::query("DELETE FROM wes_cache_entries WHERE task_hash = $1")
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            ids.len()
        } else {
            let r = sqlx::query("DELETE FROM wes_cache_entries WHERE last_used_at < $1")
                .bind(cutoff)
                .execute(&self.pool)
                .await?;
            r.rows_affected() as usize
        };
        Ok(deleted)
    }

    /// Cache statistics for GET /cache/stats.
    pub async fn cache_stats(&self) -> Result<CacheStats> {
        let total_entries: (i64,) =
            sqlx::query_as("SELECT COUNT(*)::bigint FROM wes_cache_entries")
                .fetch_one(&self.pool)
                .await?;
        let total_hits: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(hit_count), 0)::bigint FROM wes_cache_entries")
                .fetch_one(&self.pool)
                .await?;
        let entries_7d: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM wes_cache_entries WHERE last_used_at >= now() - interval '7 days'",
        )
        .fetch_one(&self.pool)
        .await?;
        let hit_rate_7d = if entries_7d > 0 {
            (total_hits as f64) / (entries_7d as f64).max(1.0)
        } else {
            0.0
        };
        Ok(CacheStats {
            total_entries: total_entries.0 as u64,
            total_size_bytes: 0,
            hit_rate_7d,
            top_cached_tasks: vec![],
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub hit_rate_7d: f64,
    pub top_cached_tasks: Vec<TopCachedTask>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TopCachedTask {
    pub task_name: String,
    pub hits: u64,
    pub size_gb: f64,
}
