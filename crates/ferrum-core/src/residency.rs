// SPDX-License-Identifier: BUSL-1.1
//! Append-only cryptographically chained data residency audit log.
//! Timestamps in the hash are microsecond Zulu so Postgres timestamptz round-trips
//! match (`to_rfc3339()` nanos / `+00:00` vs `Z` used to break `chain_valid`).
//!
//! Appends take the previous hash and insert the new row in one transaction.
//! A failed `append` returns the error (fail closed for callers that propagate
//! it). `append_warn` logs the error and returns; that is not fail-closed.

use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Connection;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidencyAuditEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub drs_id: Option<String>,
    pub requester: Option<String>,
    pub destination: Option<String>,
    pub data_left_node: bool,
    pub bytes_transferred: Option<i64>,
    pub prev_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResidencyAuditQueryResult {
    pub entries: Vec<ResidencyAuditEntry>,
    pub chain_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResidencyVerifyResult {
    pub chain_valid: bool,
    pub entry_count: i64,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub last_hash: Option<String>,
}

pub struct ResidencyAuditLog {
    pool: FerrumPool,
    /// One in-process writer at a time. The database transaction in `append`
    /// also serializes writers that do not share this mutex.
    write_lock: tokio::sync::Mutex<()>,
}

/// Postgres advisory lock held for the duration of one audit insert.
const RESIDENCY_APPEND_LOCK: i64 = 4_829_100_174;

impl ResidencyAuditLog {
    pub fn new(pool: FerrumPool) -> Self {
        Self {
            pool,
            write_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Append one residency row and return its id.
    ///
    /// The previous hash is read and the new row is inserted in one
    /// transaction (`BEGIN IMMEDIATE` on SQLite, `pg_advisory_xact_lock` on
    /// Postgres). Concurrent appends therefore extend one chain.
    ///
    /// A failed insert returns `Err`. Callers that move or register bytes and
    /// use this method fail the request (fail closed). There is no setting
    /// that turns a failed `append` into success. [`Self::append_warn`] logs
    /// the error and returns; that path is not fail-closed.
    pub async fn append(
        &self,
        event_type: &str,
        drs_id: Option<&str>,
        requester: Option<&str>,
        destination: Option<&str>,
        data_left_node: bool,
        bytes_transferred: Option<i64>,
    ) -> Result<i64> {
        let _guard = self.write_lock.lock().await;
        match &self.pool {
            FerrumPool::Sqlite(pool) => {
                append_sqlite(
                    pool,
                    event_type,
                    drs_id,
                    requester,
                    destination,
                    data_left_node,
                    bytes_transferred,
                )
                .await
            }
            FerrumPool::Postgres(pool) => {
                append_postgres(
                    pool,
                    event_type,
                    drs_id,
                    requester,
                    destination,
                    data_left_node,
                    bytes_transferred,
                )
                .await
            }
        }
    }

    /// Append an audit row and log a warning if the insert fails.
    ///
    /// This is not fail-closed. The caller continues when the insert fails.
    /// `POST /api/v1/ingest/register` does not use this method.
    pub async fn append_warn(
        &self,
        event_type: &str,
        drs_id: Option<&str>,
        requester: Option<&str>,
        destination: Option<&str>,
        data_left_node: bool,
        bytes_transferred: Option<i64>,
    ) {
        if let Err(e) = self
            .append(
                event_type,
                drs_id,
                requester,
                destination,
                data_left_node,
                bytes_transferred,
            )
            .await
        {
            tracing::warn!(
                error = %e,
                event_type,
                drs_id,
                "residency audit append failed"
            );
        }
    }

    pub async fn query_range(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<ResidencyAuditQueryResult> {
        self.query_range_for_requester(from, to, None, true).await
    }

    /// Query audit entries; non-admin callers only see rows matching `requester`.
    pub async fn query_range_for_requester(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        requester: Option<&str>,
        is_admin: bool,
    ) -> Result<ResidencyAuditQueryResult> {
        let entries = self.fetch_filtered(from, to, requester, is_admin).await?;
        let chain_valid = verify_chain(&entries);
        Ok(ResidencyAuditQueryResult {
            entries,
            chain_valid,
        })
    }

    pub async fn verify(&self) -> Result<ResidencyVerifyResult> {
        let entries = self.fetch_all_ordered().await?;
        let chain_valid = verify_chain(&entries);
        Ok(ResidencyVerifyResult {
            chain_valid,
            entry_count: entries.len() as i64,
            first_timestamp: entries.first().map(|e| e.timestamp),
            last_timestamp: entries.last().map(|e| e.timestamp),
            last_hash: entries.last().map(|e| e.entry_hash.clone()),
        })
    }

    async fn fetch_all_ordered(&self) -> Result<Vec<ResidencyAuditEntry>> {
        self.fetch_filtered(None, None, None, true).await
    }

    async fn fetch_filtered(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        requester: Option<&str>,
        is_admin: bool,
    ) -> Result<Vec<ResidencyAuditEntry>> {
        const SELECT: &str = "SELECT id, timestamp, event_type, drs_id, requester, destination, data_left_node, bytes_transferred, prev_hash, entry_hash FROM residency_audit WHERE 1=1";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(SELECT);
                if let Some(f) = from {
                    qb.push(" AND timestamp >= ").push_bind(f);
                }
                if let Some(t) = to {
                    qb.push(" AND timestamp <= ").push_bind(t);
                }
                if !is_admin {
                    if let Some(r) = requester {
                        qb.push(" AND requester = ").push_bind(r);
                    }
                }
                qb.push(" ORDER BY id ASC");
                Ok(qb
                    .build_query_as::<ResidencyRow>()
                    .fetch_all(p)
                    .await?
                    .into_iter()
                    .map(ResidencyAuditEntry::from)
                    .collect())
            }
            FerrumPool::Sqlite(p) => {
                let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(SELECT);
                if let Some(f) = from {
                    qb.push(" AND timestamp >= ").push_bind(f.to_rfc3339());
                }
                if let Some(t) = to {
                    qb.push(" AND timestamp <= ").push_bind(t.to_rfc3339());
                }
                if !is_admin {
                    if let Some(r) = requester {
                        qb.push(" AND requester = ").push_bind(r);
                    }
                }
                qb.push(" ORDER BY id ASC");
                Ok(qb
                    .build_query_as::<ResidencyRowSqlite>()
                    .fetch_all(p)
                    .await?
                    .into_iter()
                    .map(ResidencyAuditEntry::from)
                    .collect())
            }
        }
    }
}

const INSERT_SQL: &str = "INSERT INTO residency_audit
    (timestamp, event_type, drs_id, requester, destination, data_left_node, bytes_transferred, prev_hash, entry_hash)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    RETURNING id";

struct PreparedAudit {
    timestamp: DateTime<Utc>,
    prev_hash: String,
    entry_hash: String,
}

fn prepare_audit(
    prev_hash: String,
    event_type: &str,
    drs_id: Option<&str>,
    requester: Option<&str>,
    destination: Option<&str>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
) -> PreparedAudit {
    let timestamp = timestamp_for_hash(Utc::now());
    let canonical = canonical_json(&CanonicalAuditFields {
        timestamp: &timestamp,
        event_type,
        drs_id,
        requester,
        destination,
        data_left_node,
        bytes_transferred,
        prev_hash: &prev_hash,
    });
    PreparedAudit {
        timestamp,
        entry_hash: sha256_hex(&canonical),
        prev_hash,
    }
}

async fn append_sqlite(
    pool: &sqlx::SqlitePool,
    event_type: &str,
    drs_id: Option<&str>,
    requester: Option<&str>,
    destination: Option<&str>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
) -> Result<i64> {
    let mut conn = pool.acquire().await?;
    sqlx::query("PRAGMA busy_timeout = 8000")
        .execute(&mut *conn)
        .await?;
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await?;
    let prev_hash: String =
        sqlx::query_scalar("SELECT entry_hash FROM residency_audit ORDER BY id DESC LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(|| GENESIS_HASH.to_string());
    let row = prepare_audit(
        prev_hash,
        event_type,
        drs_id,
        requester,
        destination,
        data_left_node,
        bytes_transferred,
    );
    let id: i64 = sqlx::query_scalar(INSERT_SQL)
        .bind(canonical_timestamp(&row.timestamp))
        .bind(event_type)
        .bind(drs_id)
        .bind(requester)
        .bind(destination)
        .bind(data_left_node)
        .bind(bytes_transferred)
        .bind(&row.prev_hash)
        .bind(&row.entry_hash)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(id)
}

async fn append_postgres(
    pool: &sqlx::PgPool,
    event_type: &str,
    drs_id: Option<&str>,
    requester: Option<&str>,
    destination: Option<&str>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(RESIDENCY_APPEND_LOCK)
        .execute(&mut *tx)
        .await?;
    let prev_hash: String =
        sqlx::query_scalar("SELECT entry_hash FROM residency_audit ORDER BY id DESC LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(|| GENESIS_HASH.to_string());
    let row = prepare_audit(
        prev_hash,
        event_type,
        drs_id,
        requester,
        destination,
        data_left_node,
        bytes_transferred,
    );
    let id: i64 = sqlx::query_scalar(INSERT_SQL)
        .bind(row.timestamp)
        .bind(event_type)
        .bind(drs_id)
        .bind(requester)
        .bind(destination)
        .bind(data_left_node)
        .bind(bytes_transferred)
        .bind(&row.prev_hash)
        .bind(&row.entry_hash)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(id)
}

#[derive(sqlx::FromRow)]
struct ResidencyRow {
    id: i64,
    timestamp: DateTime<Utc>,
    event_type: String,
    drs_id: Option<String>,
    requester: Option<String>,
    destination: Option<String>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
    prev_hash: String,
    entry_hash: String,
}

#[derive(sqlx::FromRow)]
struct ResidencyRowSqlite {
    id: i64,
    timestamp: String,
    event_type: String,
    drs_id: Option<String>,
    requester: Option<String>,
    destination: Option<String>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
    prev_hash: String,
    entry_hash: String,
}

impl From<ResidencyRow> for ResidencyAuditEntry {
    fn from(r: ResidencyRow) -> Self {
        Self {
            id: r.id,
            timestamp: r.timestamp,
            event_type: r.event_type,
            drs_id: r.drs_id,
            requester: r.requester,
            destination: r.destination,
            data_left_node: r.data_left_node,
            bytes_transferred: r.bytes_transferred,
            prev_hash: r.prev_hash,
            entry_hash: r.entry_hash,
        }
    }
}

impl From<ResidencyRowSqlite> for ResidencyAuditEntry {
    fn from(r: ResidencyRowSqlite) -> Self {
        Self {
            id: r.id,
            timestamp: DateTime::parse_from_rfc3339(&r.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            event_type: r.event_type,
            drs_id: r.drs_id,
            requester: r.requester,
            destination: r.destination,
            data_left_node: r.data_left_node,
            bytes_transferred: r.bytes_transferred,
            prev_hash: r.prev_hash,
            entry_hash: r.entry_hash,
        }
    }
}

struct CanonicalAuditFields<'a> {
    timestamp: &'a DateTime<Utc>,
    event_type: &'a str,
    drs_id: Option<&'a str>,
    requester: Option<&'a str>,
    destination: Option<&'a str>,
    data_left_node: bool,
    bytes_transferred: Option<i64>,
    prev_hash: &'a str,
}

fn canonical_timestamp(ts: &DateTime<Utc>) -> String {
    timestamp_for_hash(*ts).to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn timestamp_for_hash(ts: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_micros(ts.timestamp_micros()).unwrap_or(ts)
}

fn canonical_json(fields: &CanonicalAuditFields<'_>) -> String {
    serde_json::json!({
        "timestamp": canonical_timestamp(fields.timestamp),
        "event_type": fields.event_type,
        "drs_id": fields.drs_id,
        "requester": fields.requester,
        "destination": fields.destination,
        "data_left_node": fields.data_left_node,
        "bytes_transferred": fields.bytes_transferred,
        "prev_hash": fields.prev_hash,
    })
    .to_string()
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_chain(entries: &[ResidencyAuditEntry]) -> bool {
    let mut expected_prev = GENESIS_HASH.to_string();
    for entry in entries {
        if entry.prev_hash != expected_prev {
            return false;
        }
        let canonical = canonical_json(&CanonicalAuditFields {
            timestamp: &timestamp_for_hash(entry.timestamp),
            event_type: &entry.event_type,
            drs_id: entry.drs_id.as_deref(),
            requester: entry.requester.as_deref(),
            destination: entry.destination.as_deref(),
            data_left_node: entry.data_left_node,
            bytes_transferred: entry.bytes_transferred,
            prev_hash: &entry.prev_hash,
        });
        if sha256_hex(&canonical) != entry.entry_hash {
            return false;
        }
        expected_prev = entry.entry_hash.clone();
    }
    true
}

pub async fn last_transaction_id(pool: &FerrumPool) -> Result<Option<i64>> {
    let sql = "SELECT id FROM residency_audit ORDER BY id DESC LIMIT 1";
    match pool {
        FerrumPool::Postgres(p) => Ok(sqlx::query_scalar(sql).fetch_optional(p).await?),
        FerrumPool::Sqlite(p) => Ok(sqlx::query_scalar(sql).fetch_optional(p).await?),
    }
}

pub fn residency_delete_blocked() -> FerrumError {
    FerrumError::ValidationError("residency_audit is append-only; DELETE not allowed".into())
}
