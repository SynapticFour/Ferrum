//! Resumable chunked upload ingest (`POST /api/v1/ingest/upload/chunk`).

use crate::checkpoint::{create_checkpoint, load_checkpoint, update_checkpoint_progress};
use crate::error::{DrsError, Result};
use crate::ingest::ParsedMultipartUpload;
use crate::state::AppState;
use ferrum_storage::{BandwidthClass, TransferDirection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct ChunkUploadResponse {
    pub upload_token: String,
    pub chunk_offset: i64,
    pub completed_bytes: i64,
    pub total_bytes: i64,
    pub complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
}

fn upload_temp_key(token: &str) -> String {
    format!("drs/uploads/{token}")
}

/// Lab/browser ingest chunks may be larger than field-sync slices; floor avoids DRS stream
/// telemetry (small localhost downloads) forcing 512 KiB caps on `/api/v1/ingest/upload/chunk`.
pub const INGEST_CHUNK_CEILING_BYTES: u64 = 4 * 1024 * 1024;

pub fn effective_ingest_chunk_max_bytes(bandwidth: Option<&ferrum_storage::BandwidthMonitor>) -> u64 {
    let adaptive = bandwidth
        .map(|b| b.classify().chunk_size_bytes())
        .unwrap_or(BandwidthClass::Medium.chunk_size_bytes());
    adaptive.max(INGEST_CHUNK_CEILING_BYTES)
}

pub async fn process_chunked_upload_from_parts(
    state: Arc<AppState>,
    auth: Option<&ferrum_core::AuthClaims>,
    mut parsed: ParsedMultipartUpload,
) -> Result<ChunkUploadResponse> {
    let storage = state
        .storage
        .clone()
        .ok_or_else(|| DrsError::Validation("ingest not configured: no storage".into()))?;
    if parsed.data.is_empty() {
        return Err(DrsError::Validation("no chunk data in multipart".into()));
    }
    let total_bytes = parsed.total_bytes.ok_or_else(|| {
        DrsError::Validation("chunked upload requires total_bytes on every chunk".into())
    })?;
    if total_bytes <= 0 {
        return Err(DrsError::Validation("total_bytes must be positive".into()));
    }
    let chunk_offset = parsed.chunk_offset.unwrap_or(0);
    if chunk_offset < 0 {
        return Err(DrsError::Validation("chunk_offset must be >= 0".into()));
    }

    let bw = state.bandwidth.as_ref();
    let max_chunk = effective_ingest_chunk_max_bytes(bw.map(|m| m.as_ref())) as i64;
    if parsed.data.len() as i64 > max_chunk {
        return Err(DrsError::Validation(format!(
            "chunk exceeds max ingest chunk size ({max_chunk} bytes)"
        )));
    }

    let class = bw
        .map(|b| b.classify())
        .unwrap_or(BandwidthClass::Medium);

    if let (Some(ref tq), Some(ref bw)) = (&state.transfer_queue, &state.bandwidth) {
        if tq.should_queue(total_bytes as u64, bw.as_ref()) {
            tq.enqueue(
                parsed
                    .upload_token
                    .clone()
                    .or(parsed.explicit_name.clone())
                    .or(parsed.file_name.clone())
                    .unwrap_or_else(|| "pending-chunk-upload".into()),
                total_bytes as u64,
                TransferDirection::Upload,
            );
            return Err(DrsError::TransferQueued(format!(
                "large chunked upload deferred on very low bandwidth (size={total_bytes} bytes)"
            )));
        }
    }

    let (upload_token, completed_before) = if let Some(ref token) = parsed.upload_token {
        let cp = load_checkpoint(state.repo.pool(), token)
            .await?
            .ok_or_else(|| DrsError::NotFound(format!("unknown upload_token: {token}")))?;
        if cp.direction != "upload" {
            return Err(DrsError::Validation(
                "resume_token is not an upload checkpoint".into(),
            ));
        }
        if cp.total_bytes != total_bytes {
            return Err(DrsError::Validation(
                "total_bytes does not match upload session".into(),
            ));
        }
        (token.clone(), cp.completed_bytes)
    } else if chunk_offset == 0 {
        let cp = create_checkpoint(
            state.repo.pool(),
            "pending-upload",
            "upload",
            total_bytes,
            class,
        )
        .await?;
        (cp.resume_token, 0)
    } else {
        return Err(DrsError::Validation(
            "chunk_offset > 0 requires upload_token from the first chunk".into(),
        ));
    };

    if chunk_offset != completed_before {
        return Err(DrsError::Validation(format!(
            "chunk_offset {chunk_offset} does not match checkpoint completed_bytes {completed_before}"
        )));
    }

    let temp_key = upload_temp_key(&upload_token);
    storage
        .append_bytes(&temp_key, &parsed.data)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;

    let completed = completed_before + parsed.data.len() as i64;
    update_checkpoint_progress(state.repo.pool(), &upload_token, completed).await?;

    if completed < total_bytes {
        return Ok(ChunkUploadResponse {
            upload_token,
            chunk_offset,
            completed_bytes: completed,
            total_bytes,
            complete: false,
            object_id: None,
            size: None,
        });
    }

    if completed > total_bytes {
        return Err(DrsError::Validation(format!(
            "upload exceeded declared total_bytes ({total_bytes})"
        )));
    }

    let mut reader = storage
        .get(&temp_key)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;
    let mut body = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut body)
        .await
        .map_err(|e| DrsError::Other(e.into()))?;
    if body.len() as i64 != total_bytes {
        return Err(DrsError::Validation(format!(
            "assembled upload size {} != declared total_bytes {total_bytes}",
            body.len()
        )));
    }
    if let Some(ref expected) = parsed.expected_sha256 {
        let digest = hex::encode(Sha256::digest(&body));
        if expected.to_lowercase() != digest {
            return Err(DrsError::Validation(format!(
                "checksum mismatch: expected sha-256 {expected}"
            )));
        }
    }

    parsed.data = body;
    parsed.upload_token = None;
    parsed.chunk_offset = None;
    parsed.total_bytes = None;
    let upload = crate::ingest::process_upload_from_parts(state.clone(), auth, parsed).await?;
    let _ = storage.delete(&temp_key).await;

    Ok(ChunkUploadResponse {
        upload_token,
        chunk_offset,
        completed_bytes: total_bytes,
        total_bytes,
        complete: true,
        object_id: Some(upload.id),
        size: Some(upload.size),
    })
}
