//! Hub upload adapter for Edge sync push (multipart + chunked resume).
#![allow(clippy::too_many_arguments)]

use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use crate::residency::ResidencyAuditLog;
use crate::sync_queue::{
    hub_push_error_message, load_metadata_document, load_object_sync_info, mark_completed,
    mark_failed, mark_in_progress, normalize_target_url, SyncQueueItem,
};
use reqwest::multipart::{Form, Part};
use std::io::SeekFrom;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const CHUNK_SIZE: usize = 256 * 1024;

pub struct PushOptions {
    pub dry_run: bool,
    pub bearer_token: Option<String>,
    pub requester: Option<String>,
}

pub struct PushItemResult {
    pub item_id: String,
    pub object_id: String,
    pub success: bool,
    pub message: String,
}

/// Push all pending/failed items for `target_url` to the hub ingest API.
pub async fn push_pending_items(
    pool: &FerrumPool,
    objects_root: &Path,
    target_url: &str,
    opts: &PushOptions,
) -> Result<Vec<PushItemResult>> {
    let target = normalize_target_url(target_url);
    let items = crate::sync_queue::list_pending_for_target(pool, &target).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| FerrumError::Internal(e.into()))?;
    let audit = ResidencyAuditLog::new(pool.clone());

    let mut results = Vec::new();
    for item in items {
        match push_one_item(pool, &client, &audit, objects_root, &target, &item, opts).await {
            Ok(r) => results.push(r),
            Err(e) => {
                let msg = e.to_string();
                let _ = mark_failed(
                    pool,
                    &item.id,
                    item.bytes_sent,
                    item.resume_token.as_deref(),
                    &msg,
                )
                .await;
                results.push(PushItemResult {
                    item_id: item.id.clone(),
                    object_id: item.object_id.clone(),
                    success: false,
                    message: msg,
                });
            }
        }
    }
    Ok(results)
}

async fn push_one_item(
    pool: &FerrumPool,
    client: &reqwest::Client,
    audit: &ResidencyAuditLog,
    objects_root: &Path,
    target: &str,
    item: &SyncQueueItem,
    opts: &PushOptions,
) -> Result<PushItemResult> {
    if opts.dry_run {
        return Ok(PushItemResult {
            item_id: item.id.clone(),
            object_id: item.object_id.clone(),
            success: true,
            message: format!("dry-run: would push {} bytes to {target}", item.bytes_total),
        });
    }

    mark_in_progress(pool, &item.id).await?;
    let info = load_object_sync_info(pool, &item.object_id)
        .await?
        .ok_or_else(|| FerrumError::NotFound(item.object_id.clone()))?;
    let file_path = objects_root.join(&info.storage_key);
    if !file_path.is_file() {
        return Err(FerrumError::NotFound(format!(
            "object file missing: {}",
            file_path.display()
        )));
    }

    let upload_url = format!("{target}/api/v1/ingest/upload");
    let client_request_id = format!("ferrum-sync-{}", item.id);

    if item.bytes_sent > 0 && item.bytes_sent < item.bytes_total {
        push_chunked(client, pool, target, &file_path, &info.name, item, opts).await?;
    } else {
        push_full_multipart(
            client,
            pool,
            &upload_url,
            &file_path,
            &info.name,
            &client_request_id,
            item.metadata_ref.as_deref(),
            opts,
        )
        .await?;
    }

    mark_completed(pool, &item.id, info.size).await?;
    audit
        .append_warn(
            "sync_push_completed",
            Some(&item.object_id),
            opts.requester.as_deref(),
            Some(target),
            true,
            Some(info.size),
        )
        .await;

    Ok(PushItemResult {
        item_id: item.id.clone(),
        object_id: item.object_id.clone(),
        success: true,
        message: format!("pushed {} bytes to {target}", info.size),
    })
}

async fn push_full_multipart(
    client: &reqwest::Client,
    pool: &FerrumPool,
    url: &str,
    file_path: &Path,
    file_name: &str,
    client_request_id: &str,
    metadata_ref: Option<&str>,
    opts: &PushOptions,
) -> Result<()> {
    let part = Part::file(file_path)
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .file_name(file_name.to_string());

    let mut form = Form::new()
        .part("file", part)
        .text("client_request_id", client_request_id.to_string())
        .text("name", file_name.to_string());

    if let Some(mref) = metadata_ref {
        if let Some(doc) = load_metadata_document(pool, mref).await? {
            let raw = serde_json::to_string(&doc)
                .map_err(|e| FerrumError::ValidationError(e.to_string()))?;
            form = form.text("ferrum_meta", raw);
        } else {
            form = form.text("metadata_ref", mref.to_string());
        }
    }

    send_multipart(client, url, form, opts).await
}

async fn push_chunked(
    client: &reqwest::Client,
    pool: &FerrumPool,
    target: &str,
    file_path: &Path,
    file_name: &str,
    item: &SyncQueueItem,
    opts: &PushOptions,
) -> Result<()> {
    let url = format!("{target}/api/v1/ingest/upload/chunk");
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?;
    let total = file
        .metadata()
        .await
        .map_err(|e| FerrumError::StorageError(e.into()))?
        .len() as i64;
    let mut offset = item.bytes_sent.max(0) as u64;
    if offset > 0 {
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
    }
    let mut upload_token = item.resume_token.clone();
    let mut buf = vec![0u8; CHUNK_SIZE];

    while (offset as i64) < total {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| FerrumError::StorageError(e.into()))?;
        if n == 0 {
            break;
        }
        let part = Part::bytes(buf[..n].to_vec());
        let mut form = Form::new()
            .part("file", part)
            .text("total_bytes", total.to_string())
            .text("chunk_offset", offset.to_string())
            .text("name", file_name.to_string());
        if let Some(ref token) = upload_token {
            form = form.text("upload_token", token.clone());
        }
        if let Some(mref) = item.metadata_ref.as_deref() {
            if let Some(doc) = load_metadata_document(pool, mref).await? {
                let raw = serde_json::to_string(&doc)
                    .map_err(|e| FerrumError::ValidationError(e.to_string()))?;
                form = form.text("ferrum_meta", raw);
            }
        }
        let body = send_multipart_raw(client, &url, form, opts).await?;
        let sent_end = offset + n as u64;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if upload_token.is_none() {
                upload_token = json
                    .get("upload_token")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            if json.get("complete").and_then(|v| v.as_bool()) == Some(true) {
                break;
            }
            let new_offset = json
                .get("completed_bytes")
                .and_then(|v| v.as_i64())
                .map(|v| v.max(0) as u64)
                .unwrap_or(sent_end);
            if new_offset != sent_end {
                file.seek(SeekFrom::Start(new_offset))
                    .await
                    .map_err(|e| FerrumError::StorageError(e.into()))?;
            }
            offset = new_offset;
        } else {
            offset = sent_end;
        }
    }
    Ok(())
}

async fn send_multipart(
    client: &reqwest::Client,
    url: &str,
    form: Form,
    opts: &PushOptions,
) -> Result<()> {
    let body = send_multipart_raw(client, url, form, opts).await?;
    let _: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
    Ok(())
}

async fn send_multipart_raw(
    client: &reqwest::Client,
    url: &str,
    form: Form,
    opts: &PushOptions,
) -> Result<String> {
    let mut req = client.post(url).multipart(form);
    if let Some(ref token) = opts.bearer_token {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| FerrumError::Internal(e.into()))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if (200..300).contains(&status) {
        return Ok(body);
    }
    if status == 409 {
        return Err(FerrumError::Conflict(hub_push_error_message(status, &body)));
    }
    Err(FerrumError::ValidationError(hub_push_error_message(
        status, &body,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const _: () = {
        assert!(CHUNK_SIZE >= 64 * 1024);
        assert!(CHUNK_SIZE <= 1024 * 1024);
    };
}
