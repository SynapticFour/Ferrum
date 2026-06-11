//! Append-only cryptographically chained data residency audit log.

use crate::error::{FerrumError, Result};
use crate::pool::FerrumPool;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
}

impl ResidencyAuditLog {
    pub fn new(pool: FerrumPool) -> Self {
        Self { pool }
    }

    pub async fn append(
        &self,
        event_type: &str,
        drs_id: Option<&str>,
        requester: Option<&str>,
        destination: Option<&str>,
        data_left_node: bool,
        bytes_transferred: Option<i64>,
    ) -> Result<i64> {
        let prev_hash = self.last_entry_hash().await?;
        let timestamp = Utc::now();
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
        let entry_hash = sha256_hex(&canonical);

        let sql = "INSERT INTO residency_audit
            (timestamp, event_type, drs_id, requester, destination, data_left_node, bytes_transferred, prev_hash, entry_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id";
        let id: i64 = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_scalar(sql)
                    .bind(timestamp)
                    .bind(event_type)
                    .bind(drs_id)
                    .bind(requester)
                    .bind(destination)
                    .bind(data_left_node)
                    .bind(bytes_transferred)
                    .bind(&prev_hash)
                    .bind(&entry_hash)
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_scalar(sql)
                    .bind(timestamp.to_rfc3339())
                    .bind(event_type)
                    .bind(drs_id)
                    .bind(requester)
                    .bind(destination)
                    .bind(data_left_node)
                    .bind(bytes_transferred)
                    .bind(&prev_hash)
                    .bind(&entry_hash)
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(id)
    }

    pub async fn query_range(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<ResidencyAuditQueryResult> {
        let entries = self.fetch_all_ordered().await?;
        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| from.map(|f| e.timestamp >= f).unwrap_or(true))
            .filter(|e| to.map(|t| e.timestamp <= t).unwrap_or(true))
            .collect();
        let chain_valid = verify_chain(&filtered);
        Ok(ResidencyAuditQueryResult {
            entries: filtered,
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

    async fn last_entry_hash(&self) -> Result<String> {
        let sql = "SELECT entry_hash FROM residency_audit ORDER BY id DESC LIMIT 1";
        let hash: Option<String> = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query_scalar(sql).fetch_optional(p).await?,
            FerrumPool::Sqlite(p) => sqlx::query_scalar(sql).fetch_optional(p).await?,
        };
        Ok(hash.unwrap_or_else(|| GENESIS_HASH.to_string()))
    }

    async fn fetch_all_ordered(&self) -> Result<Vec<ResidencyAuditEntry>> {
        let sql = "SELECT id, timestamp, event_type, drs_id, requester, destination, data_left_node, bytes_transferred, prev_hash, entry_hash
                   FROM residency_audit ORDER BY id ASC";
        let rows: Vec<ResidencyAuditEntry> = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query_as::<_, ResidencyRow>(sql)
                .fetch_all(p)
                .await?
                .into_iter()
                .map(ResidencyAuditEntry::from)
                .collect(),
            FerrumPool::Sqlite(p) => sqlx::query_as::<_, ResidencyRowSqlite>(sql)
                .fetch_all(p)
                .await?
                .into_iter()
                .map(ResidencyAuditEntry::from)
                .collect(),
        };
        Ok(rows)
    }
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

fn canonical_json(fields: &CanonicalAuditFields<'_>) -> String {
    serde_json::json!({
        "timestamp": fields.timestamp.to_rfc3339(),
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
            timestamp: &entry.timestamp,
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
