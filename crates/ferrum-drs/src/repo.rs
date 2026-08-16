// SPDX-License-Identifier: BUSL-1.1
//! Database repository for DRS objects (PostgreSQL and SQLite).

use crate::error::{DrsError, Result};
use crate::types::{
    AccessUrl, ContentsObject, CreateObjectRequest, DrsObject, UpdateObjectRequest,
};
use base64::Engine;
use ferrum_core::{
    sql_alias_lookup, sql_ingest_job_failed, sql_ingest_job_succeeded, sql_insert_access_method,
    sql_insert_drs_object, sql_list_bundle_contents_page, sql_list_objects, sql_update_drs_object,
    AccessMethod, AccessType, Checksum, DbDialect, FerrumPool,
};

macro_rules! pool_query {
    ($self:expr, |$p:ident| $body:expr) => {
        match &$self.pool {
            FerrumPool::Postgres($p) => $body,
            FerrumPool::Sqlite($p) => $body,
        }
    };
}

pub struct DrsRepo {
    pool: FerrumPool,
    dialect: DbDialect,
    hostname: String,
}

/// Full ferrum-meta submission row from `metadata_submissions`.
#[derive(Debug, Clone)]
pub struct MetadataSubmissionRow {
    pub id: String,
    pub alias: String,
    pub profile: String,
    pub document: String,
    pub version: i64,
    pub content_sha256: String,
    pub updated_time: Option<String>,
    pub created_time: Option<String>,
}

/// Summary row for Metadata Store list endpoints.
#[derive(Debug, Clone)]
pub struct MetadataSubmissionSummary {
    pub alias: String,
    pub profile: String,
    pub version: i64,
    pub content_sha256: String,
    pub created_time: String,
    pub updated_time: Option<String>,
}

/// Result of a versioned upsert.
#[derive(Debug, Clone)]
pub struct MetadataUpsertResult {
    pub alias: String,
    pub profile: String,
    pub version: i64,
    pub content_sha256: String,
    pub unchanged: bool,
}

/// Historical version row.
#[derive(Debug, Clone)]
pub struct MetadataSubmissionVersionRow {
    pub alias: String,
    pub version: i64,
    pub profile: String,
    pub document: String,
    pub content_sha256: String,
    pub created_time: String,
    pub is_current: bool,
}

impl DrsRepo {
    const CHECKSUM_STATUS_META_KEY: &'static str = "checksum_status";
    const VCF_INDEX_STATUS_META_KEY: &'static str = "vcf_index_status";
    const VARIANTS_INDEXED_META_KEY: &'static str = "variants_indexed";

    pub fn new(pool: FerrumPool, hostname: String) -> Self {
        let dialect = pool.dialect();
        Self {
            pool,
            dialect,
            hostname,
        }
    }

    /// Hostname for DRS URIs (drs://hostname/object_id).
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn pool(&self) -> &FerrumPool {
        &self.pool
    }

    pub fn postgres_pool(&self) -> Option<&sqlx::PgPool> {
        self.pool.as_postgres()
    }

    fn self_uri(&self, id: &str) -> String {
        format!("drs://{}/{}", self.hostname, id)
    }

    /// Resolve a DRS URI or plain ID to a canonical object ID.
    /// If `id_or_uri` is drs://hostname/id and hostname matches this repo, uses id; otherwise treats as alias/id.
    pub async fn resolve_id_or_uri(&self, id_or_uri: &str) -> Result<Option<String>> {
        let to_resolve = if id_or_uri.starts_with("drs://") {
            if let Some((host, id)) = crate::uri::parse_drs_uri(id_or_uri) {
                if host == self.hostname {
                    id
                } else {
                    return Ok(None);
                }
            } else {
                id_or_uri.to_string()
            }
        } else {
            id_or_uri.to_string()
        };
        self.resolve_id(&to_resolve).await
    }

    /// Resolve alias or ID to canonical object ID.
    pub async fn resolve_id(&self, id_or_alias: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT id FROM drs_objects WHERE id = $1")
                .bind(id_or_alias)
                .fetch_optional(p)
                .await
        })?;
        if let Some((id,)) = row {
            return Ok(Some(id));
        }
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as(sql_alias_lookup(self.dialect))
                .bind(id_or_alias)
                .fetch_optional(p)
                .await
        })?;
        Ok(row.map(|r| r.0))
    }

    /// Dataset ID for access control (ControlledAccessGrants visa). None = no restriction.
    pub async fn get_dataset_id(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT dataset_id FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(p)
                .await
        })?;
        Ok(row.and_then(|r| r.0))
    }

    /// Workspace scope for private (pre-publish) objects. None = not workspace-scoped.
    pub async fn get_workspace_id(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT workspace_id FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(p)
                .await
        })?;
        Ok(row.and_then(|r| r.0))
    }

    /// Link a DRS object to a published ADS dataset id.
    pub async fn set_dataset_id(&self, object_id: &str, dataset_id: &str) -> Result<()> {
        let rows = match &self.pool {
            FerrumPool::Postgres(p) => {
                sqlx::query(
                    "UPDATE drs_objects SET dataset_id = $1, updated_time = NOW() WHERE id = $2",
                )
                .bind(dataset_id)
                .bind(object_id)
                .execute(p)
                .await?
                .rows_affected()
            }
            FerrumPool::Sqlite(p) => {
                sqlx::query(
                    "UPDATE drs_objects SET dataset_id = ?1, updated_time = datetime('now') WHERE id = ?2",
                )
                .bind(dataset_id)
                .bind(object_id)
                .execute(p)
                .await?
                .rows_affected()
            }
        };
        if rows == 0 {
            return Err(DrsError::NotFound(object_id.to_string()));
        }
        Ok(())
    }

    /// Get object by canonical ID, optionally expand bundle contents.
    pub async fn get_object(&self, id: &str, expand: bool) -> Result<Option<DrsObject>> {
        let row: Option<DrsObjectRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT id, name, description, created_time, updated_time, version, mime_type, size, is_bundle, aliases, dataset_id, workspace_id
                   FROM drs_objects WHERE id = $1"#,
            )
            .bind(id)
            .fetch_optional(p)
            .await
        })?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let checksums = self.get_checksums(id).await?;
        let access_methods = self.get_access_methods(id).await?;
        let ont_metrics = self.get_ont_metrics(id).await?;
        let gisaid_metadata = self.get_gisaid_metadata(id).await?;
        let metadata_ref = self.get_metadata_ref(id).await?;
        let contents = if row.is_bundle && expand {
            Some(self.get_bundle_contents_expanded(id).await?)
        } else {
            None
        };
        let aliases: Option<Vec<String>> = row
            .aliases
            .as_ref()
            .and_then(|a| serde_json::from_value(a.clone()).ok());

        let (storage_backend, is_encrypted) = match self.get_storage_ref(id).await? {
            Some((backend, _, encrypted)) => (Some(backend), Some(encrypted)),
            None => (None, None),
        };
        let checksum_status = self.get_checksum_status(id).await?;

        Ok(Some(DrsObject {
            id: row.id.clone(),
            self_uri: self.self_uri(&row.id),
            size: row.size,
            created_time: row
                .created_time
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            checksums,
            name: row.name,
            updated_time: row
                .updated_time
                .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            version: row.version,
            mime_type: row.mime_type,
            access_methods: if access_methods.is_empty() {
                None
            } else {
                Some(access_methods)
            },
            contents,
            description: row.description,
            aliases,
            ont_metrics,
            gisaid_metadata,
            metadata_ref,
            storage_backend,
            is_encrypted,
            workspace_id: row.workspace_id,
            checksum_status,
        }))
    }

    async fn get_gisaid_metadata(&self, object_id: &str) -> Result<Option<serde_json::Value>> {
        if self.dialect == DbDialect::Postgres {
            let row: Option<(Option<serde_json::Value>,)> = pool_query!(self, |p| {
                sqlx::query_as("SELECT gisaid_metadata FROM drs_objects WHERE id = $1")
                    .bind(object_id)
                    .fetch_optional(p)
                    .await
            })?;
            return Ok(row.and_then(|r| r.0));
        }

        let row: Option<(Option<String>,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT gisaid_metadata FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(p)
                .await
        })?;
        Ok(row
            .and_then(|r| r.0)
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    async fn get_metadata_ref(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT metadata_ref FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(p)
                .await
        })?;
        Ok(row.and_then(|r| r.0))
    }

    /// Upsert a ferrum-meta submission document keyed by alias (versioned).
    ///
    /// When `expected_version` is set and does not match the current head, returns
    /// [`DrsError::Conflict`]. Identical content is a no-op (`unchanged = true`).
    pub async fn upsert_metadata_submission(
        &self,
        alias: &str,
        profile: &str,
        document: &str,
        expected_version: Option<i64>,
    ) -> Result<MetadataUpsertResult> {
        use sha2::{Digest, Sha256};
        let content_sha256 = hex::encode(Sha256::digest(document.as_bytes()));
        let current = self.get_metadata_submission(alias).await?;

        if let Some(cur) = current.as_ref() {
            if let Some(expected) = expected_version {
                if expected != cur.version {
                    return Err(DrsError::Conflict(format!(
                        "metadata submission '{alias}' version mismatch: expected {expected}, current {}",
                        cur.version
                    )));
                }
            }
            if cur.content_sha256 == content_sha256 && !cur.content_sha256.is_empty() {
                return Ok(MetadataUpsertResult {
                    alias: cur.alias.clone(),
                    profile: cur.profile.clone(),
                    version: cur.version,
                    content_sha256: cur.content_sha256.clone(),
                    unchanged: true,
                });
            }
        } else if let Some(expected) = expected_version {
            return Err(DrsError::Conflict(format!(
                "metadata submission '{alias}' does not exist (If-Match version {expected})"
            )));
        }

        let new_version = current.as_ref().map(|c| c.version + 1).unwrap_or(1);
        let id = alias.to_string();

        if self.dialect == DbDialect::Postgres {
            pool_query!(self, |p| {
                sqlx::query(
                    "INSERT INTO metadata_submissions
                        (id, alias, profile, document, version, content_sha256, updated_time)
                     VALUES ($1, $2, $3, $4, $5, $6, NOW())
                     ON CONFLICT(alias) DO UPDATE SET
                        profile = EXCLUDED.profile,
                        document = EXCLUDED.document,
                        version = EXCLUDED.version,
                        content_sha256 = EXCLUDED.content_sha256,
                        updated_time = NOW()",
                )
                .bind(&id)
                .bind(alias)
                .bind(profile)
                .bind(document)
                .bind(new_version)
                .bind(&content_sha256)
                .execute(p)
                .await?;
                Ok::<(), DrsError>(())
            })?;
        } else {
            pool_query!(self, |p| {
                sqlx::query(
                    "INSERT INTO metadata_submissions
                        (id, alias, profile, document, version, content_sha256, updated_time)
                     VALUES ($1, $2, $3, $4, $5, $6, datetime('now'))
                     ON CONFLICT(alias) DO UPDATE SET
                        profile = excluded.profile,
                        document = excluded.document,
                        version = excluded.version,
                        content_sha256 = excluded.content_sha256,
                        updated_time = datetime('now')",
                )
                .bind(&id)
                .bind(alias)
                .bind(profile)
                .bind(document)
                .bind(new_version)
                .bind(&content_sha256)
                .execute(p)
                .await?;
                Ok::<(), DrsError>(())
            })?;
        }

        self.insert_metadata_version_row(alias, new_version, profile, document, &content_sha256)
            .await?;

        Ok(MetadataUpsertResult {
            alias: alias.to_string(),
            profile: profile.to_string(),
            version: new_version,
            content_sha256,
            unchanged: false,
        })
    }

    async fn insert_metadata_version_row(
        &self,
        alias: &str,
        version: i64,
        profile: &str,
        document: &str,
        content_sha256: &str,
    ) -> Result<()> {
        let id = format!("{alias}:v{version}");
        if self.dialect == DbDialect::Postgres {
            pool_query!(self, |p| {
                sqlx::query(
                    "INSERT INTO metadata_submission_versions
                        (id, alias, version, profile, document, content_sha256, created_time)
                     VALUES ($1, $2, $3, $4, $5, $6, NOW())
                     ON CONFLICT (alias, version) DO NOTHING",
                )
                .bind(&id)
                .bind(alias)
                .bind(version)
                .bind(profile)
                .bind(document)
                .bind(content_sha256)
                .execute(p)
                .await?;
                Ok::<(), DrsError>(())
            })?;
        } else {
            pool_query!(self, |p| {
                sqlx::query(
                    "INSERT OR IGNORE INTO metadata_submission_versions
                        (id, alias, version, profile, document, content_sha256, created_time)
                     VALUES ($1, $2, $3, $4, $5, $6, datetime('now'))",
                )
                .bind(&id)
                .bind(alias)
                .bind(version)
                .bind(profile)
                .bind(document)
                .bind(content_sha256)
                .execute(p)
                .await?;
                Ok::<(), DrsError>(())
            })?;
        }
        Ok(())
    }

    /// Fetch a stored ferrum-meta submission by alias (current head).
    pub async fn get_metadata_submission(
        &self,
        alias: &str,
    ) -> Result<Option<MetadataSubmissionRow>> {
        type MetaHeadRow = (
            String,
            String,
            String,
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
        );
        let row: Option<MetaHeadRow> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT id, alias, profile, document,
                        COALESCE(version, 1),
                        COALESCE(content_sha256, ''),
                        CAST(updated_time AS TEXT),
                        CAST(created_time AS TEXT)
                 FROM metadata_submissions WHERE alias = $1 LIMIT 1",
            )
            .bind(alias)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.map(
            |(
                id,
                alias,
                profile,
                document,
                version,
                content_sha256,
                updated_time,
                created_time,
            )| {
                MetadataSubmissionRow {
                    id,
                    alias,
                    profile,
                    document,
                    version,
                    content_sha256,
                    updated_time,
                    created_time,
                }
            },
        ))
    }

    /// List version history for an alias (newest first).
    pub async fn list_metadata_submission_versions(
        &self,
        alias: &str,
    ) -> Result<Vec<MetadataSubmissionVersionRow>> {
        let current = self.get_metadata_submission(alias).await?;
        let current_version = current.as_ref().map(|c| c.version);
        let rows: Vec<(i64, String, String, String, String)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT version, profile, document, content_sha256, CAST(created_time AS TEXT)
                 FROM metadata_submission_versions
                 WHERE alias = $1
                 ORDER BY version DESC",
            )
            .bind(alias)
            .fetch_all(p)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(
                |(version, profile, document, content_sha256, created_time)| {
                    MetadataSubmissionVersionRow {
                        alias: alias.to_string(),
                        version,
                        profile,
                        document,
                        content_sha256,
                        created_time,
                        is_current: current_version == Some(version),
                    }
                },
            )
            .collect())
    }

    /// Fetch a specific historical (or current) version by number.
    pub async fn get_metadata_submission_version(
        &self,
        alias: &str,
        version: i64,
    ) -> Result<Option<MetadataSubmissionVersionRow>> {
        let current = self.get_metadata_submission(alias).await?;
        if let Some(cur) = current.as_ref() {
            if cur.version == version {
                return Ok(Some(MetadataSubmissionVersionRow {
                    alias: cur.alias.clone(),
                    version: cur.version,
                    profile: cur.profile.clone(),
                    document: cur.document.clone(),
                    content_sha256: cur.content_sha256.clone(),
                    created_time: cur
                        .updated_time
                        .clone()
                        .or_else(|| cur.created_time.clone())
                        .unwrap_or_default(),
                    is_current: true,
                }));
            }
        }
        let row: Option<(i64, String, String, String, String)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT version, profile, document, content_sha256, CAST(created_time AS TEXT)
                 FROM metadata_submission_versions
                 WHERE alias = $1 AND version = $2
                 LIMIT 1",
            )
            .bind(alias)
            .bind(version)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.map(
            |(version, profile, document, content_sha256, created_time)| {
                MetadataSubmissionVersionRow {
                    alias: alias.to_string(),
                    version,
                    profile,
                    document,
                    content_sha256,
                    created_time,
                    is_current: false,
                }
            },
        ))
    }

    /// List stored ferrum-meta submissions (newest first by `updated_time` / `created_time`).
    pub async fn list_metadata_submissions(
        &self,
        profile: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<MetadataSubmissionSummary>> {
        let limit = limit.clamp(1, 500);
        let offset = offset.max(0);
        let rows: Vec<(String, String, i64, String, String, Option<String>)> =
            if let Some(profile) = profile {
                pool_query!(self, |p| {
                    sqlx::query_as(
                        "SELECT alias, profile, COALESCE(version, 1), COALESCE(content_sha256, ''),
                                CAST(created_time AS TEXT), CAST(updated_time AS TEXT)
                         FROM metadata_submissions
                         WHERE profile = $1
                         ORDER BY COALESCE(updated_time, created_time) DESC
                         LIMIT $2 OFFSET $3",
                    )
                    .bind(profile)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(p)
                    .await
                })?
            } else {
                pool_query!(self, |p| {
                    sqlx::query_as(
                        "SELECT alias, profile, COALESCE(version, 1), COALESCE(content_sha256, ''),
                                CAST(created_time AS TEXT), CAST(updated_time AS TEXT)
                         FROM metadata_submissions
                         ORDER BY COALESCE(updated_time, created_time) DESC
                         LIMIT $1 OFFSET $2",
                    )
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(p)
                    .await
                })?
            };
        Ok(rows
            .into_iter()
            .map(
                |(alias, profile, version, content_sha256, created_time, updated_time)| {
                    MetadataSubmissionSummary {
                        alias,
                        profile,
                        version,
                        content_sha256,
                        created_time,
                        updated_time,
                    }
                },
            )
            .collect())
    }

    /// Set `metadata_ref` on an existing DRS object.
    pub async fn set_object_metadata_ref(&self, object_id: &str, metadata_ref: &str) -> Result<()> {
        let sql = if self.dialect == DbDialect::Postgres {
            "UPDATE drs_objects SET metadata_ref = $1, updated_time = NOW() WHERE id = $2"
        } else {
            "UPDATE drs_objects SET metadata_ref = $1, updated_time = datetime('now') WHERE id = $2"
        };
        let affected = pool_query!(self, |p| {
            sqlx::query(sql)
                .bind(metadata_ref)
                .bind(object_id)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        })?;
        if affected == 0 {
            return Err(DrsError::NotFound(object_id.to_string()));
        }
        Ok(())
    }

    /// Clear `metadata_ref` on an existing DRS object.
    pub async fn clear_object_metadata_ref(&self, object_id: &str) -> Result<()> {
        let sql = if self.dialect == DbDialect::Postgres {
            "UPDATE drs_objects SET metadata_ref = NULL, updated_time = NOW() WHERE id = $1"
        } else {
            "UPDATE drs_objects SET metadata_ref = NULL, updated_time = datetime('now') WHERE id = $1"
        };
        let affected = pool_query!(self, |p| {
            sqlx::query(sql)
                .bind(object_id)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        })?;
        if affected == 0 {
            return Err(DrsError::NotFound(object_id.to_string()));
        }
        Ok(())
    }

    async fn get_ont_metrics(&self, object_id: &str) -> Result<Option<serde_json::Value>> {
        if self.dialect == DbDialect::Postgres {
            let row: Option<(Option<serde_json::Value>,)> = pool_query!(self, |p| {
                sqlx::query_as("SELECT ont_metrics FROM drs_objects WHERE id = $1")
                    .bind(object_id)
                    .fetch_optional(p)
                    .await
            })?;
            return Ok(row.and_then(|r| r.0));
        }

        let row: Option<(Option<String>,)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT ont_metrics FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(p)
                .await
        })?;
        Ok(row
            .and_then(|r| r.0)
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    async fn get_checksums(&self, object_id: &str) -> Result<Vec<Checksum>> {
        let rows: Vec<(String, String)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT type, checksum FROM drs_checksums WHERE object_id = $1")
                .bind(object_id)
                .fetch_all(p)
                .await
        })?;
        Ok(rows
            .into_iter()
            .map(|(r#type, checksum)| Checksum { r#type, checksum })
            .collect())
    }

    async fn get_access_methods(&self, object_id: &str) -> Result<Vec<AccessMethod>> {
        let rows: Vec<AccessMethodRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT type, access_id, access_url, region, headers FROM drs_access_methods WHERE object_id = $1"#,
            )
            .bind(object_id)
            .fetch_all(p)
            .await
        })?;
        let mut out = Vec::new();
        for r in rows {
            let access_type = match r.r#type.as_str() {
                "s3" => AccessType::S3,
                "gs" => AccessType::Gs,
                "ftp" => AccessType::Ftp,
                "gsiftp" => AccessType::Gsiftp,
                "globus" => AccessType::Globus,
                "htsget" => AccessType::Htsget,
                "https" => AccessType::Https,
                "file" => AccessType::File,
                _ => AccessType::Https,
            };
            let access_url = r
                .access_url
                .as_ref()
                .and_then(crate::access_url::jsonb_to_core_access_url_for_listing);
            out.push(AccessMethod {
                access_type,
                access_url,
                access_id: r.access_id,
                region: r.region,
            });
        }
        Ok(out)
    }

    /// Get access URL by access_id (for signed URL etc.).
    pub async fn get_access_url(
        &self,
        object_id: &str,
        access_id: &str,
    ) -> Result<Option<AccessUrl>> {
        let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> = pool_query!(
            self,
            |p| {
                sqlx::query_as(
                    "SELECT access_url, headers FROM drs_access_methods WHERE object_id = $1 AND access_id = $2",
                )
                .bind(object_id)
                .bind(access_id)
                .fetch_optional(p)
                .await
            }
        )?;
        let (access_url, headers) = match row {
            Some(r) => r,
            None => return Ok(None),
        };
        let url = access_url
            .as_ref()
            .and_then(crate::access_url::parse_stored_access_url);
        let url = url.ok_or_else(|| {
            DrsError::Validation(
                "access_url missing or unsupported shape (expected JSON string or object with url)"
                    .into(),
            )
        })?;
        let headers: Option<Vec<String>> = headers.and_then(|h| serde_json::from_value(h).ok());
        Ok(Some(AccessUrl {
            url,
            headers,
            expires_at: None,
            resume_token: None,
            bytes_completed: None,
        }))
    }

    /// Create object (admin). If optional_id is Some, use it (e.g. from ingest).
    pub async fn create_object(&self, req: &CreateObjectRequest) -> Result<String> {
        self.create_object_with_id(req, None).await
    }

    pub async fn create_object_with_id(
        &self,
        req: &CreateObjectRequest,
        optional_id: Option<String>,
    ) -> Result<String> {
        let id = optional_id.unwrap_or_else(|| ulid::Ulid::new().to_string());
        let aliases = req
            .aliases
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Array(vec![])));
        pool_query!(self, |p| {
            sqlx::query(&sql_insert_drs_object(self.dialect))
                .bind(&id)
                .bind(&req.name)
                .bind(&req.description)
                .bind(&req.mime_type)
                .bind(req.size)
                .bind(aliases)
                .bind(req.workspace_id.as_deref())
                .bind(req.ont_metrics.as_ref())
                .bind(req.gisaid_metadata.as_ref())
                .bind(req.metadata_ref.as_deref())
                .execute(p)
                .await?;
            for c in &req.checksums {
                sqlx::query(
                    "INSERT INTO drs_checksums (object_id, type, checksum) VALUES ($1, $2, $3)",
                )
                .bind(&id)
                .bind(&c.r#type)
                .bind(&c.checksum)
                .execute(p)
                .await?;
            }
            sqlx::query(
                "INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted) VALUES ($1, $2, $3, $4)",
            )
            .bind(&id)
            .bind(&req.storage_backend)
            .bind(&req.storage_key)
            .bind(req.is_encrypted.unwrap_or(false))
            .execute(p)
            .await?;
            let access_id = format!("access-{}", id);
            let access_url_json = serde_json::json!({"url": format!("https://{}/ga4gh/drs/v1/objects/{}/access/{}", self.hostname, id, access_id)});
            sqlx::query(&sql_insert_access_method(self.dialect))
                .bind(&id)
                .bind(&access_id)
                .bind(access_url_json)
                .execute(p)
                .await?;
            Ok::<(), DrsError>(())
        })?;
        Ok(id)
    }

    /// Update object (admin).
    pub async fn update_object(&self, id: &str, req: &UpdateObjectRequest) -> Result<bool> {
        let aliases_json = req
            .aliases
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Array(vec![])));
        let affected = pool_query!(self, |p| {
            sqlx::query(&sql_update_drs_object(self.dialect))
                .bind(id)
                .bind(&req.name)
                .bind(&req.description)
                .bind(&req.mime_type)
                .bind(req.size)
                .bind(aliases_json)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        })?;
        if let Some(checksums) = &req.checksums {
            pool_query!(self, |p| {
                sqlx::query("DELETE FROM drs_checksums WHERE object_id = $1")
                    .bind(id)
                    .execute(p)
                    .await?;
                for c in checksums {
                    sqlx::query(
                        "INSERT INTO drs_checksums (object_id, type, checksum) VALUES ($1, $2, $3)",
                    )
                    .bind(id)
                    .bind(&c.r#type)
                    .bind(&c.checksum)
                    .execute(p)
                    .await?;
                }
                Ok::<(), DrsError>(())
            })?;
        }
        Ok(affected > 0)
    }

    /// Delete object (admin).
    pub async fn delete_object(&self, id: &str) -> Result<bool> {
        let affected = pool_query!(self, |p| {
            sqlx::query("DELETE FROM drs_objects WHERE id = $1")
                .bind(id)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        })?;
        Ok(affected > 0)
    }

    /// List objects with pagination and filters.
    pub async fn list_objects(
        &self,
        limit: u32,
        offset: u32,
        mime_type: Option<&str>,
        min_size: Option<i64>,
        max_size: Option<i64>,
        workspace_id: Option<&str>,
    ) -> Result<Vec<DrsObject>> {
        let limit = limit.min(1000);
        let rows: Vec<DrsObjectRow> = pool_query!(self, |p| {
            sqlx::query_as(&sql_list_objects(self.dialect))
                .bind(mime_type)
                .bind(min_size)
                .bind(max_size)
                .bind(limit as i64)
                .bind(offset as i64)
                .bind(workspace_id)
                .fetch_all(p)
                .await
        })?;
        let mut out = Vec::new();
        for row in rows {
            match self.get_object(&row.id, false).await {
                Ok(Some(obj)) => out.push(obj),
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(object_id = %row.id, error = %e, "list_objects: skipping object");
                }
            }
        }
        Ok(out)
    }

    /// Storage ref for object (backend, key, is_encrypted).
    pub async fn get_storage_ref(&self, object_id: &str) -> Result<Option<(String, String, bool)>> {
        let row: Option<(String, String, bool)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT storage_backend, storage_key, is_encrypted FROM storage_references WHERE object_id = $1",
            )
            .bind(object_id)
            .fetch_optional(p)
            .await
        })?;
        Ok(row)
    }

    /// Log access for auditing.
    pub async fn log_access(
        &self,
        object_id: &str,
        access_id: Option<&str>,
        method: &str,
        status: u16,
        client_ip: Option<&str>,
    ) -> Result<()> {
        pool_query!(self, |p| {
            sqlx::query(
                "INSERT INTO drs_access_log (object_id, access_id, method, status, client_ip) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(object_id)
            .bind(access_id)
            .bind(method)
            .bind(status as i32)
            .bind(client_ip)
            .execute(p)
            .await
            .map(|_| ())
        })?;
        Ok(())
    }

    // --- Ingest jobs (`/api/v1/ingest`, Lab Kit) ---

    /// Find ingest job by idempotent client key (if set).
    pub async fn ingest_job_by_client_request_id(
        &self,
        client_request_id: &str,
    ) -> Result<Option<DrsIngestJobRow>> {
        let row: Option<DrsIngestJobRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
                   FROM drs_ingest_jobs WHERE client_request_id = $1"#,
            )
            .bind(client_request_id)
            .fetch_optional(p)
            .await
        })?;
        Ok(row)
    }

    pub async fn ingest_job_get(&self, id: &str) -> Result<Option<DrsIngestJobRow>> {
        let row: Option<DrsIngestJobRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
                   FROM drs_ingest_jobs WHERE id = $1"#,
            )
            .bind(id)
            .fetch_optional(p)
            .await
        })?;
        Ok(row)
    }

    /// Recent ingest jobs for UI status banners (newest first).
    pub async fn ingest_job_list_recent(&self, limit: i64) -> Result<Vec<DrsIngestJobRow>> {
        let rows: Vec<DrsIngestJobRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
                   FROM drs_ingest_jobs ORDER BY created_at DESC LIMIT $1"#,
            )
            .bind(limit)
            .fetch_all(p)
            .await
        })?;
        Ok(rows)
    }

    /// Ingest jobs whose `client_request_id` was scoped to this Passport `sub` (Ferrum UI prefix).
    pub async fn ingest_job_list_for_subject(
        &self,
        subject_sub: &str,
        limit: i64,
    ) -> Result<Vec<DrsIngestJobRow>> {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let token = URL_SAFE_NO_PAD.encode(subject_sub.as_bytes());
        let upload_pat = format!("ferrum-ui:{}:%", token);
        let register_pat = format!("ferrum-ui-register:{}:%", token);
        let rows: Vec<DrsIngestJobRow> = pool_query!(self, |p| {
            sqlx::query_as(
                r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
                   FROM drs_ingest_jobs
                   WHERE client_request_id LIKE $1 OR client_request_id LIKE $2
                   ORDER BY created_at DESC LIMIT $3"#,
            )
            .bind(&upload_pat)
            .bind(&register_pat)
            .bind(limit)
            .fetch_all(p)
            .await
        })?;
        Ok(rows)
    }

    pub async fn ingest_job_insert(
        &self,
        id: &str,
        client_request_id: Option<&str>,
        job_type: &str,
        status: &str,
    ) -> Result<()> {
        pool_query!(self, |p| {
            sqlx::query(
                r#"INSERT INTO drs_ingest_jobs (id, client_request_id, job_type, status)
                   VALUES ($1, $2, $3, $4)"#,
            )
            .bind(id)
            .bind(client_request_id)
            .bind(job_type)
            .bind(status)
            .execute(p)
            .await
            .map(|_| ())
        })?;
        Ok(())
    }

    pub async fn ingest_job_finish_success(
        &self,
        id: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        pool_query!(self, |p| {
            sqlx::query(&sql_ingest_job_succeeded(self.dialect))
                .bind(id)
                .bind(result)
                .execute(p)
                .await
                .map(|_| ())
        })?;
        Ok(())
    }

    pub async fn ingest_job_finish_failed(
        &self,
        id: &str,
        error: &serde_json::Value,
    ) -> Result<()> {
        pool_query!(self, |p| {
            sqlx::query(&sql_ingest_job_failed(self.dialect))
                .bind(id)
                .bind(error)
                .execute(p)
                .await
                .map(|_| ())
        })?;
        Ok(())
    }
}

/// Row for `drs_ingest_jobs` (machine ingest / Lab Kit polling).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct DrsIngestJobRow {
    pub id: String,
    pub client_request_id: Option<String>,
    pub job_type: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub result_json: Option<serde_json::Value>,
    pub error_json: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct DrsObjectRow {
    id: String,
    name: Option<String>,
    description: Option<String>,
    created_time: chrono::DateTime<chrono::Utc>,
    updated_time: Option<chrono::DateTime<chrono::Utc>>,
    version: Option<String>,
    mime_type: Option<String>,
    size: i64,
    is_bundle: bool,
    aliases: Option<serde_json::Value>,
    #[allow(dead_code)]
    dataset_id: Option<String>,
    workspace_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AccessMethodRow {
    r#type: String,
    access_id: Option<String>,
    access_url: Option<serde_json::Value>,
    region: Option<String>,
    #[allow(dead_code)]
    headers: Option<serde_json::Value>,
}

impl DrsRepo {
    /// Get bundle contents with recursive expansion of nested bundles (iterative to avoid async recursion).
    async fn get_bundle_contents_expanded(&self, bundle_id: &str) -> Result<Vec<ContentsObject>> {
        const MAX_BUNDLE_DEPTH: usize = 5;
        #[derive(Clone)]
        struct Item {
            object_id: String,
            name: String,
            drs_uri: Option<String>,
            is_bundle: bool,
        }
        let mut to_expand: Vec<(String, usize)> = vec![(bundle_id.to_string(), 1)];
        let mut by_bundle: std::collections::HashMap<String, Vec<Item>> =
            std::collections::HashMap::new();
        while let Some((bid, depth)) = to_expand.pop() {
            let rows: Vec<(String, String, Option<String>, bool)> = pool_query!(self, |p| {
                sqlx::query_as(
                    r#"SELECT c.object_id, c.name, c.drs_uri, o.is_bundle
                       FROM drs_bundle_contents c
                       JOIN drs_objects o ON o.id = c.object_id
                       WHERE c.bundle_id = $1"#,
                )
                .bind(&bid)
                .fetch_all(p)
                .await
            })?;
            let items: Vec<Item> = rows
                .into_iter()
                .map(|(object_id, name, drs_uri, is_bundle)| Item {
                    object_id,
                    name,
                    drs_uri,
                    is_bundle,
                })
                .collect();
            for item in &items {
                if item.is_bundle {
                    let child_depth = depth + 1;
                    if child_depth > MAX_BUNDLE_DEPTH {
                        return Err(DrsError::Validation(format!(
                            "Bundle nesting exceeds maximum depth of {}",
                            MAX_BUNDLE_DEPTH
                        )));
                    }
                    to_expand.push((item.object_id.clone(), child_depth));
                }
            }
            by_bundle.insert(bid, items);
        }
        fn build_contents(
            bundle_id: &str,
            by_bundle: &std::collections::HashMap<String, Vec<Item>>,
            hostname: &str,
        ) -> Vec<ContentsObject> {
            let items = match by_bundle.get(bundle_id) {
                Some(i) => i,
                None => return vec![],
            };
            items
                .iter()
                .map(|r| {
                    let uri = format!("drs://{}/{}", hostname, r.object_id);
                    let drs_uri = r
                        .drs_uri
                        .as_ref()
                        .map(|u| vec![u.clone()])
                        .or_else(|| Some(vec![uri]));
                    let contents = if r.is_bundle {
                        Some(build_contents(&r.object_id, by_bundle, hostname))
                    } else {
                        None
                    };
                    ContentsObject {
                        name: r.name.clone(),
                        id: Some(r.object_id.clone()),
                        drs_uri,
                        contents,
                    }
                })
                .collect()
        }
        let hostname = self.hostname().to_string();
        Ok(build_contents(bundle_id, &by_bundle, &hostname))
    }

    /// List direct bundle members with cursor-based pagination.
    /// Cursor is an opaque base64 string encoding (bundle_id, last_seen_child_id).
    pub async fn list_bundle_contents_page(
        &self,
        bundle_id: &str,
        page_token: Option<&str>,
        page_size: u32,
    ) -> Result<(Vec<ContentsObject>, Option<String>)> {
        const DEFAULT_PAGE_SIZE: u32 = 100;
        const MAX_PAGE_SIZE: u32 = 1000;

        let page_size = if page_size == 0 {
            DEFAULT_PAGE_SIZE
        } else {
            page_size
        };
        if page_size > MAX_PAGE_SIZE {
            return Err(DrsError::Validation(format!(
                "page_size exceeds maximum of {}",
                MAX_PAGE_SIZE
            )));
        }

        #[derive(serde::Deserialize, serde::Serialize)]
        struct Cursor {
            bundle_id: String,
            last_seen_child_id: String,
        }

        let last_seen: Option<String> = if let Some(token) = page_token {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(token)
                .map_err(|e| DrsError::Validation(format!("invalid page_token: {e}")))?;
            let cursor: Cursor = serde_json::from_slice(&decoded)
                .map_err(|e| DrsError::Validation(format!("invalid page_token payload: {e}")))?;
            if cursor.bundle_id != bundle_id {
                return Err(DrsError::Validation(
                    "page_token does not match requested bundle_id".into(),
                ));
            }
            Some(cursor.last_seen_child_id)
        } else {
            None
        };

        let limit = (page_size as i64) + 1;

        let mut rows: Vec<(String, String, Option<String>)> = pool_query!(self, |p| {
            sqlx::query_as(&sql_list_bundle_contents_page(self.dialect))
                .bind(bundle_id)
                .bind(last_seen.as_deref())
                .bind(limit)
                .fetch_all(p)
                .await
        })?;

        let next_page_token = if rows.len() as u32 > page_size {
            let last_in_page = rows[page_size as usize - 1].clone();
            let _extra = rows.pop();
            let cursor = Cursor {
                bundle_id: bundle_id.to_string(),
                last_seen_child_id: last_in_page.0,
            };
            let payload = serde_json::to_vec(&cursor)
                .map_err(|e| DrsError::Other(anyhow::anyhow!(e.to_string())))?;
            Some(base64::engine::general_purpose::STANDARD.encode(payload))
        } else {
            None
        };

        let contents = rows
            .into_iter()
            .map(|(object_id, name, drs_uri)| {
                let uri =
                    drs_uri.or_else(|| Some(format!("drs://{}/{}", self.hostname(), object_id)));
                ContentsObject {
                    name,
                    id: Some(object_id.clone()),
                    drs_uri: uri.map(|u| vec![u]),
                    contents: None,
                }
            })
            .collect();

        Ok((contents, next_page_token))
    }

    /// Get metadata key-value pairs for an object.
    pub async fn get_metadata(&self, object_id: &str) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, Option<String>)> = pool_query!(self, |p| {
            sqlx::query_as("SELECT key, value FROM drs_object_metadata WHERE object_id = $1")
                .bind(object_id)
                .fetch_all(p)
                .await
        })?;
        Ok(rows
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect())
    }

    /// Set a single metadata key-value for an object (upsert).
    pub async fn set_metadata(&self, object_id: &str, key: &str, value: &str) -> Result<()> {
        pool_query!(self, |p| {
            sqlx::query(
                "INSERT INTO drs_object_metadata (object_id, key, value) VALUES ($1, $2, $3)
                 ON CONFLICT (object_id, key) DO UPDATE SET value = $3",
            )
            .bind(object_id)
            .bind(key)
            .bind(value)
            .execute(p)
            .await
            .map(|_| ())
        })?;
        Ok(())
    }

    /// Returns checksum compute status stored in `drs_object_metadata`.
    pub async fn get_checksum_status(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT value FROM drs_object_metadata WHERE object_id = $1 AND key = $2",
            )
            .bind(object_id)
            .bind(Self::CHECKSUM_STATUS_META_KEY)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.map(|r| r.0))
    }

    /// Set checksum status for an object.
    pub async fn set_checksum_status(&self, object_id: &str, status: &str) -> Result<()> {
        self.set_metadata(object_id, Self::CHECKSUM_STATUS_META_KEY, status)
            .await
    }

    /// Returns VCF → Beacon indexing status stored in `drs_object_metadata`.
    pub async fn get_vcf_index_status(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT value FROM drs_object_metadata WHERE object_id = $1 AND key = $2",
            )
            .bind(object_id)
            .bind(Self::VCF_INDEX_STATUS_META_KEY)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.map(|r| r.0))
    }

    /// Set VCF indexing status for a published object.
    pub async fn set_vcf_index_status(&self, object_id: &str, status: &str) -> Result<()> {
        self.set_metadata(object_id, Self::VCF_INDEX_STATUS_META_KEY, status)
            .await
    }

    /// Record how many variants were indexed into Beacon for this object.
    pub async fn set_variants_indexed_count(&self, object_id: &str, count: usize) -> Result<()> {
        self.set_metadata(
            object_id,
            Self::VARIANTS_INDEXED_META_KEY,
            &count.to_string(),
        )
        .await
    }

    /// Returns the number of variants indexed into Beacon (if recorded).
    pub async fn get_variants_indexed_count(&self, object_id: &str) -> Result<Option<usize>> {
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT value FROM drs_object_metadata WHERE object_id = $1 AND key = $2",
            )
            .bind(object_id)
            .bind(Self::VARIANTS_INDEXED_META_KEY)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.and_then(|r| r.0.parse().ok()))
    }

    /// Upsert checksums in `drs_checksums`.
    pub async fn upsert_checksums(
        &self,
        object_id: &str,
        checksums: &[(&str, &str)],
    ) -> Result<()> {
        for (typ, checksum) in checksums {
            pool_query!(self, |p| {
                sqlx::query(
                    "INSERT INTO drs_checksums (object_id, type, checksum)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (object_id, type)
                     DO UPDATE SET checksum = EXCLUDED.checksum",
                )
                .bind(object_id)
                .bind(typ)
                .bind(checksum)
                .execute(p)
                .await
                .map(|_| ())
            })?;
        }
        Ok(())
    }

    /// Insert pathogen annotation linked to a DRS object (multi-pathogen Beacon).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_pathogen_annotation(
        &self,
        drs_object_id: &str,
        organism: &str,
        amr_genes: &[String],
        serotype: Option<&str>,
        virulence_factors: &[String],
        ont_qscore_min: Option<f32>,
        dataset_id: Option<&str>,
    ) -> Result<String> {
        let id = ulid::Ulid::new().to_string();
        let amr_json = serde_json::to_value(amr_genes).unwrap_or(serde_json::json!([]));
        let vf_json = serde_json::to_value(virulence_factors).unwrap_or(serde_json::json!([]));
        pool_query!(self, |p| {
            sqlx::query(
                "INSERT INTO pathogen_annotations (id, dataset_id, drs_object_id, organism, amr_genes, serotype, virulence_factors, ont_qscore_min)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&id)
            .bind(dataset_id)
            .bind(drs_object_id)
            .bind(organism)
            .bind(amr_json)
            .bind(serotype)
            .bind(vf_json)
            .bind(ont_qscore_min)
            .execute(p)
            .await?;
            Ok::<(), DrsError>(())
        })?;
        Ok(id)
    }

    /// Object metadata needed for publish-side Beacon indexing.
    pub async fn get_object_publish_info(
        &self,
        object_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>, String, String)>> {
        let row: Option<(Option<String>, Option<String>, String, String)> =
            pool_query!(self, |p| {
                sqlx::query_as(
                    "SELECT o.name, o.mime_type, sr.storage_backend, sr.storage_key
                 FROM drs_objects o
                 JOIN storage_references sr ON sr.object_id = o.id
                 WHERE o.id = $1",
                )
                .bind(object_id)
                .fetch_optional(p)
                .await
            })?;
        Ok(row)
    }

    /// Link existing pathogen annotations to a published ADS/Beacon dataset id.
    pub async fn link_pathogen_to_dataset(
        &self,
        drs_object_id: &str,
        dataset_id: &str,
    ) -> Result<u64> {
        let rows = match &self.pool {
            FerrumPool::Postgres(p) => sqlx::query(
                "UPDATE pathogen_annotations SET dataset_id = $1 WHERE drs_object_id = $2",
            )
            .bind(dataset_id)
            .bind(drs_object_id)
            .execute(p)
            .await?
            .rows_affected(),
            FerrumPool::Sqlite(p) => sqlx::query(
                "UPDATE pathogen_annotations SET dataset_id = ?1 WHERE drs_object_id = ?2",
            )
            .bind(dataset_id)
            .bind(drs_object_id)
            .execute(p)
            .await?
            .rows_affected(),
        };
        Ok(rows)
    }

    /// Update ONT metrics JSON on an existing DRS object (e.g. from ont-qc workflow).
    pub async fn update_ont_metrics(
        &self,
        object_id: &str,
        metrics: &serde_json::Value,
    ) -> Result<bool> {
        let sql = if self.dialect == DbDialect::Postgres {
            "UPDATE drs_objects SET ont_metrics = $1, updated_time = NOW() WHERE id = $2"
        } else {
            "UPDATE drs_objects SET ont_metrics = $1, updated_time = datetime('now') WHERE id = $2"
        };
        let affected = pool_query!(self, |p| {
            sqlx::query(sql)
                .bind(metrics)
                .bind(object_id)
                .execute(p)
                .await
                .map(|r| r.rows_affected())
        })?;
        Ok(affected > 0)
    }

    /// Pathogen organism tag for a DRS object (if any).
    pub async fn pathogen_organism(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = pool_query!(self, |p| {
            sqlx::query_as(
                "SELECT organism FROM pathogen_annotations WHERE drs_object_id = $1 LIMIT 1",
            )
            .bind(object_id)
            .fetch_optional(p)
            .await
        })?;
        Ok(row.map(|r| r.0))
    }

    /// Mark object as a bundle and set aggregate size.
    pub async fn mark_as_bundle(&self, bundle_id: &str, total_size: i64) -> Result<()> {
        let sql = if self.dialect == DbDialect::Postgres {
            "UPDATE drs_objects SET is_bundle = TRUE, size = $1, updated_time = NOW() WHERE id = $2"
        } else {
            "UPDATE drs_objects SET is_bundle = 1, size = $1, updated_time = datetime('now') WHERE id = $2"
        };
        pool_query!(self, |p| {
            sqlx::query(sql)
                .bind(total_size)
                .bind(bundle_id)
                .execute(p)
                .await?;
            Ok::<(), DrsError>(())
        })?;
        Ok(())
    }

    /// Add a member object to a bundle.
    pub async fn add_bundle_member(
        &self,
        bundle_id: &str,
        object_id: &str,
        name: &str,
    ) -> Result<()> {
        let drs_uri = self.self_uri(object_id);
        pool_query!(self, |p| {
            sqlx::query(
                "INSERT INTO drs_bundle_contents (bundle_id, object_id, name, drs_uri) VALUES ($1, $2, $3, $4)",
            )
            .bind(bundle_id)
            .bind(object_id)
            .bind(name)
            .bind(drs_uri)
            .execute(p)
            .await?;
            Ok::<(), DrsError>(())
        })?;
        Ok(())
    }

    /// Create an ONT bundle DRS object wrapping raw (+ optional FASTQ) members.
    pub async fn create_ont_bundle(
        &self,
        bundle_id: &str,
        name: Option<String>,
        description: Option<String>,
        ont_metrics: Option<serde_json::Value>,
        metadata_ref: Option<String>,
        members: &[(String, String, i64)],
    ) -> Result<()> {
        let total_size: i64 = members.iter().map(|(_, _, s)| s).sum();
        let req = CreateObjectRequest {
            name,
            description,
            mime_type: Some("application/x-ont-bundle".into()),
            size: total_size,
            checksums: vec![],
            aliases: None,
            storage_backend: "bundle".into(),
            storage_key: bundle_id.to_string(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics,
            gisaid_metadata: None,
            metadata_ref,
        };
        self.create_object_with_id(&req, Some(bundle_id.to_string()))
            .await?;
        self.mark_as_bundle(bundle_id, total_size).await?;
        for (object_id, member_name, _) in members {
            self.add_bundle_member(bundle_id, object_id, member_name)
                .await?;
        }
        Ok(())
    }
}
