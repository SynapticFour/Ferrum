//! Outbreak Mode: policy activation, emergency Beacon access, audit trail, GISAID packaging.

use crate::config::{OutbreakConfig, OutbreakPolicy};
use crate::error::{FerrumError, Result};
use crate::gisaid::missing_gisaid_fields;
use crate::pool::FerrumPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Activation request body.
#[derive(Debug, Deserialize)]
pub struct ActivateRequest {
    pub policy: String,
    pub activated_by: String,
}

/// Deactivation request body.
#[derive(Debug, Deserialize)]
pub struct DeactivateRequest {
    pub policy: String,
    pub reason: String,
}

/// Download approval request body.
#[derive(Debug, Deserialize)]
pub struct ApproveDownloadRequest {
    pub recipient: String,
    pub approved_by: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivationRecord {
    pub id: String,
    pub policy_name: String,
    pub trigger_pathogen: String,
    pub activated_by: String,
    pub active: bool,
}

pub struct OutbreakService {
    pool: FerrumPool,
    config: OutbreakConfig,
}

impl OutbreakService {
    pub fn new(pool: FerrumPool, config: OutbreakConfig) -> Self {
        Self { pool, config }
    }

    pub fn config(&self) -> &OutbreakConfig {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn policy(&self, name: &str) -> Option<&OutbreakPolicy> {
        self.config.policy_by_name(name)
    }

    /// Activate a configured policy. Requires outbreak mode enabled in config.
    pub async fn activate(&self, req: &ActivateRequest) -> Result<ActivationRecord> {
        if !self.config.enabled {
            return Err(FerrumError::ValidationError(
                "outbreak mode is disabled in configuration".into(),
            ));
        }
        let policy = self.config.policy_by_name(&req.policy).ok_or_else(|| {
            FerrumError::ValidationError(format!("unknown outbreak policy '{}'", req.policy))
        })?;
        let id = ulid::Ulid::new().to_string();
        let sql = "INSERT INTO outbreak_activations (id, policy_name, trigger_pathogen, activated_by, active)
                   VALUES ($1, $2, $3, $4, TRUE)";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(sql)
                    .bind(&id)
                    .bind(&policy.name)
                    .bind(&policy.trigger_pathogen)
                    .bind(&req.activated_by)
                    .execute(p)
                    .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(sql)
                    .bind(&id)
                    .bind(&policy.name)
                    .bind(&policy.trigger_pathogen)
                    .bind(&req.activated_by)
                    .execute(p)
                    .await?;
            }
        }
        self.audit(
            &policy.name,
            "activate",
            &req.activated_by,
            None,
            Some(&policy.trigger_pathogen),
            None,
            None,
            None,
        )
        .await?;
        Ok(ActivationRecord {
            id,
            policy_name: policy.name.clone(),
            trigger_pathogen: policy.trigger_pathogen.clone(),
            activated_by: req.activated_by.clone(),
            active: true,
        })
    }

    /// Warn when `gisaid_auto_package` is enabled but tagged objects lack required metadata.
    pub async fn gisaid_packaging_warnings(&self, policy: &OutbreakPolicy) -> Result<Vec<String>> {
        if !policy.gisaid_auto_package {
            return Ok(Vec::new());
        }
        let rows = self.pathogen_drs_objects(&policy.trigger_pathogen).await?;
        let mut warnings = Vec::new();
        for row in rows {
            let missing = missing_gisaid_fields(row.gisaid_metadata.as_ref());
            if !missing.is_empty() {
                warnings.push(format!(
                    "DRS object {} missing gisaid_metadata fields: {}",
                    row.drs_object_id,
                    missing.join(", ")
                ));
            }
        }
        Ok(warnings)
    }

    /// Deactivate an active policy.
    pub async fn deactivate(&self, req: &DeactivateRequest, actor: &str) -> Result<()> {
        if !self.config.enabled {
            return Err(FerrumError::ValidationError(
                "outbreak mode is disabled in configuration".into(),
            ));
        }
        let _policy = self.config.policy_by_name(&req.policy).ok_or_else(|| {
            FerrumError::ValidationError(format!("unknown outbreak policy '{}'", req.policy))
        })?;
        let sql = "UPDATE outbreak_activations SET active = FALSE, deactivated_at = NOW(),
                   deactivated_by = $1, deactivation_reason = $2
                   WHERE policy_name = $3 AND active = TRUE";
        let sql_sqlite =
            "UPDATE outbreak_activations SET active = 0, deactivated_at = datetime('now'),
                          deactivated_by = $1, deactivation_reason = $2
                          WHERE policy_name = $3 AND active = 1";
        let affected = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query(sql)
                .bind(actor)
                .bind(&req.reason)
                .bind(&req.policy)
                .execute(p)
                .await?
                .rows_affected(),
            FerrumPool::Sqlite(p) => sqlx::query(sql_sqlite)
                .bind(actor)
                .bind(&req.reason)
                .bind(&req.policy)
                .execute(p)
                .await?
                .rows_affected(),
        };
        if affected == 0 {
            return Err(FerrumError::ValidationError(format!(
                "policy '{}' is not active",
                req.policy
            )));
        }
        self.audit(
            &req.policy,
            "deactivate",
            actor,
            None,
            None,
            None,
            Some(&req.reason),
            None,
        )
        .await?;
        Ok(())
    }

    /// List currently active policy names.
    pub async fn active_policies(&self) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(
                    "SELECT policy_name FROM outbreak_activations WHERE active = TRUE ORDER BY activated_at DESC",
                )
                .fetch_all(p)
                .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(
                    "SELECT policy_name FROM outbreak_activations WHERE active = 1 ORDER BY activated_at DESC",
                )
                .fetch_all(p)
                .await?
            }
        };
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// True when recipient has emergency Beacon access for pathogen under an active policy.
    pub async fn emergency_beacon_access(
        &self,
        recipient_issuer: &str,
        pathogen: &str,
    ) -> Result<bool> {
        if !self.config.enabled {
            return Ok(false);
        }
        let active = self.active_policies().await?;
        for policy_name in active {
            let Some(policy) = self.config.policy_by_name(&policy_name) else {
                continue;
            };
            if !pathogen_matches(&policy.trigger_pathogen, pathogen) {
                continue;
            }
            if policy
                .emergency_recipients
                .iter()
                .any(|r| recipient_matches(r, recipient_issuer))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Record a Beacon query under outbreak mode.
    pub async fn audit_beacon_query(
        &self,
        policy_name: &str,
        actor: &str,
        recipient: &str,
        pathogen: &str,
        summary: &str,
    ) -> Result<()> {
        self.audit(
            policy_name,
            "beacon_query",
            actor,
            Some(recipient),
            Some(pathogen),
            None,
            Some(summary),
            None,
        )
        .await
    }

    /// Approve DRS download for emergency recipient under active policy.
    pub async fn approve_download(
        &self,
        policy_name: &str,
        drs_object_id: &str,
        req: &ApproveDownloadRequest,
    ) -> Result<()> {
        if !self.config.enabled {
            return Err(FerrumError::ValidationError(
                "outbreak mode is disabled".into(),
            ));
        }
        let active = self.active_policies().await?;
        if !active.iter().any(|p| p == policy_name) {
            return Err(FerrumError::ValidationError(format!(
                "policy '{policy_name}' is not active"
            )));
        }
        let sql = "INSERT INTO outbreak_download_approvals (drs_object_id, policy_name, approved_by, recipient)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT (drs_object_id, policy_name, recipient) DO NOTHING";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(sql)
                    .bind(drs_object_id)
                    .bind(policy_name)
                    .bind(&req.approved_by)
                    .bind(&req.recipient)
                    .execute(p)
                    .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO outbreak_download_approvals (drs_object_id, policy_name, approved_by, recipient)
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(drs_object_id)
                .bind(policy_name)
                .bind(&req.approved_by)
                .bind(&req.recipient)
                .execute(p)
                .await?;
            }
        }
        self.audit(
            policy_name,
            "approve_download",
            &req.approved_by,
            Some(&req.recipient),
            None,
            Some(drs_object_id),
            None,
            None,
        )
        .await?;
        Ok(())
    }

    /// Check if download was approved for recipient under policy.
    pub async fn has_download_approval(
        &self,
        policy_name: &str,
        drs_object_id: &str,
        recipient: &str,
    ) -> Result<bool> {
        let row: Option<(bool,)> = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as(
                    "SELECT TRUE FROM outbreak_download_approvals
                     WHERE drs_object_id = $1 AND policy_name = $2 AND recipient = $3",
                )
                .bind(drs_object_id)
                .bind(policy_name)
                .bind(recipient)
                .fetch_optional(p)
                .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as(
                    "SELECT 1 FROM outbreak_download_approvals
                     WHERE drs_object_id = $1 AND policy_name = $2 AND recipient = $3",
                )
                .bind(drs_object_id)
                .bind(policy_name)
                .bind(recipient)
                .fetch_optional(p)
                .await?
            }
        };
        Ok(row.is_some())
    }

    /// Count audit entries (for immutability tests).
    pub async fn audit_count(&self) -> Result<i64> {
        let row: (i64,) = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query_as("SELECT COUNT(*) FROM outbreak_audit")
                    .fetch_one(p)
                    .await?
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query_as("SELECT COUNT(*) FROM outbreak_audit")
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(row.0)
    }

    /// Fetch DRS object IDs tagged with pathogen for GISAID packaging.
    pub async fn pathogen_drs_objects(&self, pathogen: &str) -> Result<Vec<PathogenPackageRow>> {
        match &self.pool {
            FerrumPool::Postgres(p) => {
                let rows: Vec<(String, String, Option<String>, Option<Value>)> = sqlx::query_as(
                    "SELECT pa.drs_object_id, pa.organism, sr.storage_key, d.gisaid_metadata
                     FROM pathogen_annotations pa
                     JOIN storage_references sr ON sr.object_id = pa.drs_object_id
                     JOIN drs_objects d ON d.id = pa.drs_object_id
                     WHERE pa.organism = $1 OR pa.organism ILIKE $2",
                )
                .bind(pathogen)
                .bind(format!("%{pathogen}%"))
                .fetch_all(p)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(drs_object_id, organism, storage_key, gisaid_raw)| PathogenPackageRow {
                            drs_object_id,
                            organism,
                            storage_key,
                            gisaid_metadata: gisaid_raw.filter(|v| !v.is_null()),
                        },
                    )
                    .collect())
            }
            FerrumPool::Sqlite(p) => {
                let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT pa.drs_object_id, pa.organism, sr.storage_key, d.gisaid_metadata
                     FROM pathogen_annotations pa
                     JOIN storage_references sr ON sr.object_id = pa.drs_object_id
                     JOIN drs_objects d ON d.id = pa.drs_object_id
                     WHERE pa.organism = $1 OR pa.organism LIKE $2",
                )
                .bind(pathogen)
                .bind(format!("%{pathogen}%"))
                .fetch_all(p)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(drs_object_id, organism, storage_key, gisaid_raw)| PathogenPackageRow {
                            drs_object_id,
                            organism,
                            storage_key,
                            gisaid_metadata: gisaid_raw
                                .and_then(|raw| serde_json::from_str(&raw).ok()),
                        },
                    )
                    .collect())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn audit(
        &self,
        policy_name: &str,
        action: &str,
        actor: &str,
        recipient: Option<&str>,
        pathogen: Option<&str>,
        drs_object_id: Option<&str>,
        query_summary: Option<&str>,
        details: Option<Value>,
    ) -> Result<()> {
        let details_json = details
            .as_ref()
            .map(|d| serde_json::to_string(d).unwrap_or_default());
        let sql = "INSERT INTO outbreak_audit (policy_name, action, actor, recipient, pathogen, drs_object_id, query_summary, details)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)";
        match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(sql)
                    .bind(policy_name)
                    .bind(action)
                    .bind(actor)
                    .bind(recipient)
                    .bind(pathogen)
                    .bind(drs_object_id)
                    .bind(query_summary)
                    .bind(details)
                    .execute(p)
                    .await?;
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(sql)
                    .bind(policy_name)
                    .bind(action)
                    .bind(actor)
                    .bind(recipient)
                    .bind(pathogen)
                    .bind(drs_object_id)
                    .bind(query_summary)
                    .bind(details_json)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PathogenPackageRow {
    pub drs_object_id: String,
    pub organism: String,
    pub storage_key: Option<String>,
    pub gisaid_metadata: Option<Value>,
}

fn gisaid_field(meta: Option<&Value>, key: &str, fallback: &str) -> String {
    meta.and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn pathogen_matches(policy_pathogen: &str, query_pathogen: &str) -> bool {
    policy_pathogen.eq_ignore_ascii_case(query_pathogen)
        || query_pathogen.contains(policy_pathogen)
        || policy_pathogen.contains(query_pathogen)
}

fn recipient_matches(configured: &str, issuer: &str) -> bool {
    configured.eq_ignore_ascii_case(issuer)
        || issuer.ends_with(configured)
        || issuer.contains(configured)
}

/// Build GISAID EpiCoV-style submission archive (CSV + FASTA) from local FASTA bytes.
pub fn build_gisaid_package(policy_name: &str, entries: &[GisaidEntry]) -> Result<Vec<u8>> {
    let mut csv = String::from(
        "submitter,virus name,type,passage details/history,collection date,location,host,patient age,gender,clade,sequencing technology\n",
    );
    let mut fasta = String::new();
    for (i, e) in entries.iter().enumerate() {
        csv.push_str(&format!(
            "{},{},{},original,{},{},{},unknown,unknown,unknown,ONT\n",
            e.submitting_lab, e.virus_name, e.organism, e.collection_date, e.location, e.host,
        ));
        fasta.push_str(&format!(">{}\n{}\n", e.virus_name, e.sequence));
        let _ = i;
    }
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    let meta = format!(
        "policy={policy_name}\ngenerated=ferrum\nentries={}\n",
        entries.len()
    );
    let meta_bytes = meta.into_bytes();
    let csv_bytes = csv.into_bytes();
    let fasta_bytes = fasta.into_bytes();

    for (name, data) in [
        ("metadata.txt", meta_bytes.as_slice()),
        ("gisaid_submission.csv", csv_bytes.as_slice()),
        ("sequences.fasta", fasta_bytes.as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header
            .set_path(name)
            .map_err(|e| FerrumError::Internal(e.into()))?;
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, data)
            .map_err(|e| FerrumError::Internal(e.into()))?;
    }

    tar.finish().map_err(|e| FerrumError::Internal(e.into()))?;
    let enc = tar
        .into_inner()
        .map_err(|e| FerrumError::Internal(e.into()))?;
    let bytes = enc.finish().map_err(|e| FerrumError::Internal(e.into()))?;
    Ok(bytes)
}

#[derive(Debug, Clone)]
pub struct GisaidEntry {
    pub virus_name: String,
    pub organism: String,
    pub collection_date: String,
    pub location: String,
    pub host: String,
    pub submitting_lab: String,
    pub submitting_lab_address: String,
    pub originating_lab: String,
    pub sequence: String,
}

impl GisaidEntry {
    pub fn from_package_row(row: &PathogenPackageRow, index: usize, sequence: &str) -> Self {
        let meta = row.gisaid_metadata.as_ref();
        Self {
            virus_name: format!(
                "hCoV-19/{}/{}",
                gisaid_field(meta, "location", "Unknown"),
                index + 1
            ),
            organism: row.organism.clone(),
            collection_date: gisaid_field(meta, "collection_date", "2025-01-01"),
            location: gisaid_field(meta, "location", "Unknown/Unknown"),
            host: gisaid_field(meta, "host", "Human"),
            submitting_lab: gisaid_field(meta, "submitting_lab", "Unknown"),
            submitting_lab_address: gisaid_field(meta, "submitting_lab_address", "Unknown"),
            originating_lab: gisaid_field(meta, "originating_lab", "Unknown"),
            sequence: sequence.to_string(),
        }
    }
}
