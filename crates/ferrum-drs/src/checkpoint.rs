//! Chunked transfer checkpointing for bandwidth-adaptive DRS transfers.

use ferrum_core::pool::FerrumPool;
use ferrum_core::Result;
use ferrum_storage::BandwidthClass;

/// Abandon incomplete browser/API upload sessions after this many seconds.
pub const UPLOAD_SESSION_TTL_SECS: i64 = 86_400;

#[derive(Debug, Clone)]
pub struct TransferCheckpoint {
    pub id: String,
    pub object_id: String,
    pub direction: String,
    pub total_bytes: i64,
    pub chunk_size: i64,
    pub completed_bytes: i64,
    pub resume_token: String,
    pub checksum_sha256: Option<String>,
}

pub async fn create_checkpoint(
    pool: &FerrumPool,
    object_id: &str,
    direction: &str,
    total_bytes: i64,
    class: BandwidthClass,
) -> Result<TransferCheckpoint> {
    let id = ulid::Ulid::new().to_string();
    let resume_token = ulid::Ulid::new().to_string();
    let chunk_size = class.chunk_size_bytes() as i64;
    let sql = "INSERT INTO transfer_checkpoints
        (id, object_id, direction, total_bytes, chunk_size, completed_bytes, resume_token)
        VALUES ($1, $2, $3, $4, $5, 0, $6)";
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(object_id)
                .bind(direction)
                .bind(total_bytes)
                .bind(chunk_size)
                .bind(&resume_token)
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql)
                .bind(&id)
                .bind(object_id)
                .bind(direction)
                .bind(total_bytes)
                .bind(chunk_size)
                .bind(&resume_token)
                .execute(p)
                .await?;
        }
    }
    Ok(TransferCheckpoint {
        id,
        object_id: object_id.to_string(),
        direction: direction.to_string(),
        total_bytes,
        chunk_size,
        completed_bytes: 0,
        resume_token,
        checksum_sha256: None,
    })
}

pub async fn load_checkpoint(
    pool: &FerrumPool,
    resume_token: &str,
) -> Result<Option<TransferCheckpoint>> {
    let sql = "SELECT id, object_id, direction, total_bytes, chunk_size, completed_bytes, resume_token, checksum_sha256
               FROM transfer_checkpoints WHERE resume_token = $1";
    let row = match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query_as::<_, CheckpointRow>(sql)
                .bind(resume_token)
                .fetch_optional(p)
                .await?
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query_as::<_, CheckpointRow>(sql)
                .bind(resume_token)
                .fetch_optional(p)
                .await?
        }
    };
    Ok(row.map(Into::into))
}

pub async fn update_checkpoint_progress(
    pool: &FerrumPool,
    resume_token: &str,
    completed_bytes: i64,
) -> Result<()> {
    let sql = "UPDATE transfer_checkpoints SET completed_bytes = $1, updated_at = NOW() WHERE resume_token = $2";
    let sql_sqlite =
        "UPDATE transfer_checkpoints SET completed_bytes = $1, updated_at = datetime('now') WHERE resume_token = $2";
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql)
                .bind(completed_bytes)
                .bind(resume_token)
                .execute(p)
                .await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql_sqlite)
                .bind(completed_bytes)
                .bind(resume_token)
                .execute(p)
                .await?;
        }
    }
    Ok(())
}

pub async fn delete_checkpoint(pool: &FerrumPool, resume_token: &str) -> Result<()> {
    let sql = "DELETE FROM transfer_checkpoints WHERE resume_token = $1";
    match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query(sql).bind(resume_token).execute(p).await?;
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query(sql).bind(resume_token).execute(p).await?;
        }
    }
    Ok(())
}

/// Returns resume tokens for stale incomplete upload sessions (for assembly + storage cleanup).
pub async fn purge_stale_upload_sessions(
    pool: &FerrumPool,
    max_age_secs: i64,
) -> Result<Vec<String>> {
    let pg_sql = "DELETE FROM transfer_checkpoints
        WHERE direction = 'upload'
          AND object_id = 'pending-upload'
          AND completed_bytes < total_bytes
          AND updated_at < NOW() - ($1::bigint * INTERVAL '1 second')
        RETURNING resume_token";
    let sqlite_sql = "DELETE FROM transfer_checkpoints
        WHERE direction = 'upload'
          AND object_id = 'pending-upload'
          AND completed_bytes < total_bytes
          AND datetime(updated_at) < datetime('now', '-' || $1 || ' seconds')
        RETURNING resume_token";

    let rows: Vec<(String,)> = match pool {
        FerrumPool::Postgres(p) => {
            sqlx::query_as(pg_sql)
                .bind(max_age_secs)
                .fetch_all(p)
                .await?
        }
        FerrumPool::Sqlite(p) => {
            sqlx::query_as(sqlite_sql)
                .bind(max_age_secs)
                .fetch_all(p)
                .await?
        }
    };
    Ok(rows.into_iter().map(|(t,)| t).collect())
}

#[derive(sqlx::FromRow)]
struct CheckpointRow {
    id: String,
    object_id: String,
    direction: String,
    total_bytes: i64,
    chunk_size: i64,
    completed_bytes: i64,
    resume_token: String,
    checksum_sha256: Option<String>,
}

impl From<CheckpointRow> for TransferCheckpoint {
    fn from(r: CheckpointRow) -> Self {
        Self {
            id: r.id,
            object_id: r.object_id,
            direction: r.direction,
            total_bytes: r.total_bytes,
            chunk_size: r.chunk_size,
            completed_bytes: r.completed_bytes,
            resume_token: r.resume_token,
            checksum_sha256: r.checksum_sha256,
        }
    }
}
