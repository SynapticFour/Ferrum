// SPDX-License-Identifier: BUSL-1.1
//! Edge sync queue: enqueue DRS objects for upstream hub upload (ADR-019).
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::config::SyncConfig;
use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STATE_PENDING: &str = "pending";
pub const STATE_IN_PROGRESS: &str = "in_progress";
pub const STATE_COMPLETED: &str = "completed";
pub const STATE_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncQueueItem {
    pub id: String,
    pub object_id: String,
    pub target_url: String,
    pub state: String,
    pub bytes_total: i64,
    pub bytes_sent: i64,
    pub resume_token: Option<String>,
    pub crypt4gh: bool,
    pub metadata_ref: Option<String>,
    pub created_at: String,
    pub last_attempt_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyncObjectInfo {
    pub object_id: String,
    pub name: String,
    pub size: i64,
    pub mime_type: Option<String>,
    pub metadata_ref: Option<String>,
    pub is_encrypted: bool,
    pub storage_key: String,
}

pub fn normalize_target_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Returns human-readable reason when ferrum-meta fails consent/DUO policy.
pub fn submission_passes_policy(
    doc: &Value,
    policy: &SyncConfig,
) -> std::result::Result<(), String> {
    if policy.allowed_duo_codes.is_empty() && policy.allowed_consent_types.is_empty() {
        return Ok(());
    }

    let duo = extract_duo_codes(doc);
    if !policy.allowed_duo_codes.is_empty() {
        if duo.is_empty() {
            return Err("no data_use_conditions (DUO) in ferrum-meta submission".into());
        }
        if !duo
            .iter()
            .any(|c| policy.allowed_duo_codes.iter().any(|a| codes_match(a, c)))
        {
            return Err(format!(
                "DUO codes {:?} not in allowed list {:?}",
                duo, policy.allowed_duo_codes
            ));
        }
    }

    if !policy.allowed_consent_types.is_empty() {
        let consents = extract_consent_types(doc);
        if consents.is_empty() {
            return Err("no individual consent_type in ferrum-meta submission".into());
        }
        if !consents.iter().any(|c| {
            policy
                .allowed_consent_types
                .iter()
                .any(|a| a.eq_ignore_ascii_case(c))
        }) {
            return Err(format!(
                "consent types {:?} not in allowed list {:?}",
                consents, policy.allowed_consent_types
            ));
        }
    }
    Ok(())
}

fn codes_match(allowed: &str, actual: &str) -> bool {
    let a = allowed.trim().to_ascii_uppercase();
    let b = actual.trim().to_ascii_uppercase();
    a == b || b.starts_with(&format!("{a}:")) || a.starts_with(&format!("{b}:"))
}

fn extract_duo_codes(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(study) = doc.get("study") {
        if let Some(arr) = study.get("data_use_conditions").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    if let Some(arr) = doc.get("data_use_conditions").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(s) = v.as_str() {
                out.push(s.to_string());
            }
        }
    }
    out
}

fn extract_consent_types(doc: &Value) -> Vec<String> {
    doc.get("individuals")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|ind| ind.get("consent_type").and_then(|v| v.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub async fn load_metadata_document(pool: &FerrumPool, alias: &str) -> Result<Option<Value>> {
    let sql = "SELECT document FROM metadata_submissions WHERE alias = $1 LIMIT 1";
    let raw: Option<String> = match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query_scalar(sql)
                .bind(alias)
                .fetch_optional(p)
                .await?
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query_scalar(sql)
                .bind(alias)
                .fetch_optional(p)
                .await?
        }
    };
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

pub async fn consent_allows_sync(
    pool: &FerrumPool,
    metadata_ref: Option<&str>,
    policy: &SyncConfig,
) -> std::result::Result<(), String> {
    if policy.require_metadata_ref && metadata_ref.is_none() {
        return Err("object has no metadata_ref; sync blocked by policy".into());
    }
    let Some(alias) = metadata_ref else {
        return Ok(());
    };
    let doc = load_metadata_document(pool, alias)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("metadata_ref '{alias}' not found in metadata_submissions"))?;
    submission_passes_policy(&doc, policy)
}

pub async fn load_object_sync_info(
    pool: &FerrumPool,
    object_id: &str,
) -> Result<Option<SyncObjectInfo>> {
    let sql = "SELECT o.id, o.name, o.size, o.mime_type, o.metadata_ref, COALESCE(r.is_encrypted, 0), r.storage_key
               FROM drs_objects o
               LEFT JOIN storage_references r ON r.object_id = o.id
               WHERE o.id = $1 LIMIT 1";
    match pool {
        FerrumPool::Postgres(p) => {
            let row: Option<(
                String,
                String,
                i64,
                Option<String>,
                Option<String>,
                bool,
                Option<String>,
            )> = sqlx::query_as(sql)
                .bind(object_id)
                .fetch_optional(p)
                .await?;
            Ok(row.and_then(
                |(id, name, size, mime_type, metadata_ref, is_encrypted, storage_key)| {
                    storage_key.map(|storage_key| SyncObjectInfo {
                        object_id: id,
                        name,
                        size,
                        mime_type,
                        metadata_ref,
                        is_encrypted,
                        storage_key,
                    })
                },
            ))
        }
        FerrumPool::Sqlite(p) => {
            let row: Option<(
                String,
                String,
                i64,
                Option<String>,
                Option<String>,
                i32,
                Option<String>,
            )> = sqlx::query_as(sql)
                .bind(object_id)
                .fetch_optional(p)
                .await?;
            Ok(row.and_then(
                |(id, name, size, mime_type, metadata_ref, is_encrypted, storage_key)| {
                    storage_key.map(|storage_key| SyncObjectInfo {
                        object_id: id,
                        name,
                        size,
                        mime_type,
                        metadata_ref,
                        is_encrypted: is_encrypted != 0,
                        storage_key,
                    })
                },
            ))
        }
    }
}

pub async fn list_local_object_ids(pool: &FerrumPool) -> Result<Vec<String>> {
    let sql = "SELECT id FROM drs_objects ORDER BY created_time ASC";
    match pool {
        FerrumPool::Postgres(p) => {
            let rows: Vec<(String,)> = sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows.into_iter().map(|(id,)| id).collect())
        }
        FerrumPool::Sqlite(p) => {
            let rows: Vec<(String,)> = sqlx::query_as(sql).fetch_all(p).await?;
            Ok(rows.into_iter().map(|(id,)| id).collect())
        }
    }
}

async fn active_queue_exists(pool: &FerrumPool, object_id: &str, target_url: &str) -> Result<bool> {
    let sql = "SELECT 1 FROM sync_queue WHERE object_id = $1 AND target_url = $2 AND state IN ('pending', 'in_progress') LIMIT 1";
    let row: Option<(i32,)> = match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query_as(sql)
                .bind(object_id)
                .bind(target_url)
                .fetch_optional(p)
                .await?
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query_as(sql)
                .bind(object_id)
                .bind(target_url)
                .fetch_optional(p)
                .await?
        }
    };
    Ok(row.is_some())
}

pub async fn enqueue_object(
    pool: &FerrumPool,
    object_id: &str,
    target_url: &str,
    policy: &SyncConfig,
) -> Result<SyncQueueItem> {
    let target_url = normalize_target_url(target_url);
    if active_queue_exists(pool, object_id, &target_url).await? {
        return Err(FerrumError::ValidationError(format!(
            "object {object_id} already queued for {target_url}"
        )));
    }
    let info = load_object_sync_info(pool, object_id)
        .await?
        .ok_or_else(|| {
            FerrumError::NotFound(format!("object {object_id} not found or has no storage"))
        })?;
    consent_allows_sync(pool, info.metadata_ref.as_deref(), policy)
        .await
        .map_err(FerrumError::ValidationError)?;

    let id = ulid::Ulid::new().to_string();
    let sql = "INSERT INTO sync_queue
        (id, object_id, target_url, state, bytes_total, bytes_sent, crypt4gh, metadata_ref)
        VALUES ($1, $2, $3, 'pending', $4, 0, $5, $6)";
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(object_id)
                .bind(&target_url)
                .bind(info.size)
                .bind(info.is_encrypted)
                .bind(info.metadata_ref.as_deref())
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(object_id)
                .bind(&target_url)
                .bind(info.size)
                .bind(info.is_encrypted as i32)
                .bind(info.metadata_ref.as_deref())
                .execute(p)
                .await?;
        }
    }
    list_queue_item(pool, &id)
        .await?
        .ok_or_else(|| FerrumError::Internal(anyhow::anyhow!("enqueue insert failed")))
}

pub async fn enqueue_all_local(
    pool: &FerrumPool,
    target_url: &str,
    policy: &SyncConfig,
) -> Result<Vec<SyncQueueItem>> {
    let ids = list_local_object_ids(pool).await?;
    let mut out = Vec::new();
    for id in ids {
        match enqueue_object(pool, &id, target_url, policy).await {
            Ok(item) => out.push(item),
            Err(FerrumError::ValidationError(msg)) if msg.contains("already queued") => {}
            Err(FerrumError::ValidationError(msg)) => {
                tracing::warn!(object_id = %id, reason = %msg, "sync enqueue skipped by policy");
            }
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

fn row_to_item(
    id: String,
    object_id: String,
    target_url: String,
    state: String,
    bytes_total: i64,
    bytes_sent: i64,
    resume_token: Option<String>,
    crypt4gh: bool,
    metadata_ref: Option<String>,
    created_at: String,
    last_attempt_at: Option<String>,
    error_message: Option<String>,
) -> SyncQueueItem {
    SyncQueueItem {
        id,
        object_id,
        target_url,
        state,
        bytes_total,
        bytes_sent,
        resume_token,
        crypt4gh,
        metadata_ref,
        created_at,
        last_attempt_at,
        error_message,
    }
}

pub async fn list_queue_items(
    pool: &FerrumPool,
    state_filter: Option<&str>,
) -> Result<Vec<SyncQueueItem>> {
    let sql = if state_filter.is_some() {
        "SELECT id, object_id, target_url, state, bytes_total, bytes_sent, resume_token, crypt4gh, metadata_ref, created_at, last_attempt_at, error_message
         FROM sync_queue WHERE state = $1 ORDER BY created_at ASC"
    } else {
        "SELECT id, object_id, target_url, state, bytes_total, bytes_sent, resume_token, crypt4gh, metadata_ref, created_at, last_attempt_at, error_message
         FROM sync_queue ORDER BY created_at ASC"
    };
    match pool {
        FerrumPool::Postgres(p) => {
            let rows: Vec<(
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<String>,
                bool,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
                Option<chrono::DateTime<chrono::Utc>>,
                Option<String>,
            )> = if let Some(st) = state_filter {
                sqlx::query_as(sql).bind(st).fetch_all(p).await?
            } else {
                sqlx::query_as(sql).fetch_all(p).await?
            };
            Ok(rows
                .into_iter()
                .map(
                    |(
                        id,
                        object_id,
                        target_url,
                        state,
                        bytes_total,
                        bytes_sent,
                        resume_token,
                        crypt4gh,
                        metadata_ref,
                        created_at,
                        last_attempt_at,
                        error_message,
                    )| {
                        row_to_item(
                            id,
                            object_id,
                            target_url,
                            state,
                            bytes_total,
                            bytes_sent,
                            resume_token,
                            crypt4gh,
                            metadata_ref,
                            created_at.to_rfc3339(),
                            last_attempt_at.map(|t| t.to_rfc3339()),
                            error_message,
                        )
                    },
                )
                .collect())
        }
        FerrumPool::Sqlite(p) => {
            let rows: Vec<(
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<String>,
                i32,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
            )> = if let Some(st) = state_filter {
                sqlx::query_as(sql).bind(st).fetch_all(p).await?
            } else {
                sqlx::query_as(sql).fetch_all(p).await?
            };
            Ok(rows
                .into_iter()
                .map(
                    |(
                        id,
                        object_id,
                        target_url,
                        state,
                        bytes_total,
                        bytes_sent,
                        resume_token,
                        crypt4gh,
                        metadata_ref,
                        created_at,
                        last_attempt_at,
                        error_message,
                    )| {
                        row_to_item(
                            id,
                            object_id,
                            target_url,
                            state,
                            bytes_total,
                            bytes_sent,
                            resume_token,
                            crypt4gh != 0,
                            metadata_ref,
                            created_at,
                            last_attempt_at,
                            error_message,
                        )
                    },
                )
                .collect())
        }
    }
}

async fn list_queue_item(pool: &FerrumPool, id: &str) -> Result<Option<SyncQueueItem>> {
    let items = list_queue_items(pool, None).await?;
    Ok(items.into_iter().find(|i| i.id == id))
}

pub async fn list_pending_for_target(
    pool: &FerrumPool,
    target_url: &str,
) -> Result<Vec<SyncQueueItem>> {
    let target_url = normalize_target_url(target_url);
    let sql = "SELECT id, object_id, target_url, state, bytes_total, bytes_sent, resume_token, crypt4gh, metadata_ref, created_at, last_attempt_at, error_message
               FROM sync_queue WHERE target_url = $1 AND state IN ('pending', 'failed') ORDER BY created_at ASC";
    match pool {
        FerrumPool::Postgres(p) => {
            let rows: Vec<(
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<String>,
                bool,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
                Option<chrono::DateTime<chrono::Utc>>,
                Option<String>,
            )> = sqlx::query_as(sql).bind(&target_url).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(
                    |(
                        id,
                        object_id,
                        target_url,
                        state,
                        bytes_total,
                        bytes_sent,
                        resume_token,
                        crypt4gh,
                        metadata_ref,
                        created_at,
                        last_attempt_at,
                        error_message,
                    )| {
                        row_to_item(
                            id,
                            object_id,
                            target_url,
                            state,
                            bytes_total,
                            bytes_sent,
                            resume_token,
                            crypt4gh,
                            metadata_ref,
                            created_at.to_rfc3339(),
                            last_attempt_at.map(|t| t.to_rfc3339()),
                            error_message,
                        )
                    },
                )
                .collect())
        }
        FerrumPool::Sqlite(p) => {
            let rows: Vec<(
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<String>,
                i32,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
            )> = sqlx::query_as(sql).bind(&target_url).fetch_all(p).await?;
            Ok(rows
                .into_iter()
                .map(
                    |(
                        id,
                        object_id,
                        target_url,
                        state,
                        bytes_total,
                        bytes_sent,
                        resume_token,
                        crypt4gh,
                        metadata_ref,
                        created_at,
                        last_attempt_at,
                        error_message,
                    )| {
                        row_to_item(
                            id,
                            object_id,
                            target_url,
                            state,
                            bytes_total,
                            bytes_sent,
                            resume_token,
                            crypt4gh != 0,
                            metadata_ref,
                            created_at,
                            last_attempt_at,
                            error_message,
                        )
                    },
                )
                .collect())
        }
    }
}

pub async fn mark_in_progress(pool: &FerrumPool, id: &str) -> Result<()> {
    let sql = if matches!(pool, FerrumPool::Postgres(_)) {
        "UPDATE sync_queue SET state = 'in_progress', last_attempt_at = NOW(), error_message = NULL WHERE id = $1"
    } else {
        "UPDATE sync_queue SET state = 'in_progress', last_attempt_at = datetime('now'), error_message = NULL WHERE id = $1"
    };
    exec_update(pool, sql, id).await
}

pub async fn mark_completed(pool: &FerrumPool, id: &str, bytes_sent: i64) -> Result<()> {
    let sql = if matches!(pool, FerrumPool::Postgres(_)) {
        "UPDATE sync_queue SET state = 'completed', bytes_sent = $2, last_attempt_at = NOW(), error_message = NULL WHERE id = $1"
    } else {
        "UPDATE sync_queue SET state = 'completed', bytes_sent = $2, last_attempt_at = datetime('now'), error_message = NULL WHERE id = $1"
    };
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(id)
                .bind(bytes_sent)
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql)
                .bind(id)
                .bind(bytes_sent)
                .execute(p)
                .await?;
        }
    }
    Ok(())
}

pub async fn mark_failed(
    pool: &FerrumPool,
    id: &str,
    bytes_sent: i64,
    resume_token: Option<&str>,
    message: &str,
) -> Result<()> {
    let msg = truncate_err(message, 500);
    let sql = if matches!(pool, FerrumPool::Postgres(_)) {
        "UPDATE sync_queue SET state = 'failed', bytes_sent = $2, resume_token = $3, last_attempt_at = NOW(), error_message = $4 WHERE id = $1"
    } else {
        "UPDATE sync_queue SET state = 'failed', bytes_sent = $2, resume_token = $3, last_attempt_at = datetime('now'), error_message = $4 WHERE id = $1"
    };
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(id)
                .bind(bytes_sent)
                .bind(resume_token)
                .bind(&msg)
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql)
                .bind(id)
                .bind(bytes_sent)
                .bind(resume_token)
                .bind(&msg)
                .execute(p)
                .await?;
        }
    }
    Ok(())
}

async fn exec_update(pool: &FerrumPool, sql: &str, id: &str) -> Result<()> {
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql).bind(id).execute(p).await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql).bind(id).execute(p).await?;
        }
    }
    Ok(())
}

fn truncate_err(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

/// Map hub HTTP status to operator-facing sync failure (409 = conflict).
pub fn hub_push_error_message(status: u16, body: &str) -> String {
    if status == 409 {
        format!(
            "hub conflict (409): duplicate sample or object id — {body}. \
             Hub policy: reject or version suffix; edge does not auto-merge. \
             See docs/FIELD-SYNC-HUB.md"
        )
    } else {
        format!("hub returned HTTP {status}: {body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::FerrumPool;

    async fn sqlite_pool() -> FerrumPool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../ferrum-embed/migrations")
            .run(&pool)
            .await
            .unwrap();
        FerrumPool::Sqlite(pool)
    }

    async fn seed_object(pool: &FerrumPool, id: &str, metadata_ref: Option<&str>) {
        let sql_obj =
            "INSERT INTO drs_objects (id, name, size, created_time, updated_time, metadata_ref)
                       VALUES ($1, $2, 100, datetime('now'), datetime('now'), $3)";
        let sql_ref =
            "INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
                       VALUES ($1, 'local', $2, 0)";
        match pool {
            FerrumPool::Sqlite(p) => {
                sqlx::query(sql_obj)
                    .bind(id)
                    .bind(format!("{id}.fastq"))
                    .bind(metadata_ref)
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query(sql_ref)
                    .bind(id)
                    .bind(format!("objects/{id}"))
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("sqlite only"),
        }
    }

    #[test]
    fn duo_policy_allows_matching_code() {
        let doc = serde_json::json!({
            "study": { "data_use_conditions": ["DUO:0000006"] }
        });
        let policy = SyncConfig {
            allowed_duo_codes: vec!["DUO:0000006".into()],
            ..SyncConfig::default()
        };
        submission_passes_policy(&doc, &policy).expect("allowed");
    }

    #[test]
    fn duo_policy_blocks_missing() {
        let doc = serde_json::json!({ "study": {} });
        let policy = SyncConfig {
            allowed_duo_codes: vec!["DUO:0000006".into()],
            ..SyncConfig::default()
        };
        assert!(submission_passes_policy(&doc, &policy).is_err());
    }

    #[test]
    fn hub_conflict_message_mentions_409() {
        let msg = hub_push_error_message(409, "duplicate sample_id");
        assert!(msg.contains("409"));
        assert!(msg.contains("FIELD-SYNC-HUB"));
    }

    #[tokio::test]
    async fn enqueue_and_list() {
        let pool = sqlite_pool().await;
        seed_object(&pool, "obj1", None).await;
        let policy = SyncConfig::default();
        let item = enqueue_object(&pool, "obj1", "https://hub.example.org", &policy)
            .await
            .expect("enqueue");
        assert_eq!(item.state, STATE_PENDING);
        let all = list_queue_items(&pool, None).await.expect("list");
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn enqueue_respects_duo_policy() {
        let pool = sqlite_pool().await;
        seed_object(&pool, "obj2", Some("ds1")).await;
        sqlx::query(
            "INSERT INTO metadata_submissions (id, alias, profile, document) VALUES ('ds1', 'ds1', 'pathogen', $1)",
        )
        .bind(r#"{"study":{"data_use_conditions":["DUO:0000007"]}}"#)
        .execute(match &pool {
            FerrumPool::Sqlite(p) => p,
            _ => panic!(),
        })
        .await
        .unwrap();
        let policy = SyncConfig {
            allowed_duo_codes: vec!["DUO:0000006".into()],
            ..SyncConfig::default()
        };
        let err = enqueue_object(&pool, "obj2", "https://hub.example.org", &policy)
            .await
            .expect_err("blocked");
        assert!(matches!(err, FerrumError::ValidationError(_)));
    }
}
