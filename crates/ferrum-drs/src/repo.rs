//! Database repository for DRS objects (PostgreSQL).

use crate::error::{DrsError, Result};
use crate::types::{
    AccessUrl, ContentsObject, CreateObjectRequest, DrsObject, UpdateObjectRequest,
};
use base64::Engine;
use ferrum_core::{AccessMethod, AccessType, Checksum};
use sqlx::PgPool;

pub struct DrsRepo {
    pool: PgPool,
    hostname: String,
}

impl DrsRepo {
    const CHECKSUM_STATUS_META_KEY: &'static str = "checksum_status";

    pub fn new(pool: PgPool, hostname: String) -> Self {
        Self { pool, hostname }
    }

    /// Hostname for DRS URIs (drs://hostname/object_id).
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
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
        let row: Option<(String,)> = sqlx::query_as("SELECT id FROM drs_objects WHERE id = $1")
            .bind(id_or_alias)
            .fetch_optional(&self.pool)
            .await?;
        if let Some((id,)) = row {
            return Ok(Some(id));
        }
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM drs_objects WHERE aliases @> jsonb_build_array($1::text) LIMIT 1",
        )
        .bind(id_or_alias)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Dataset ID for access control (ControlledAccessGrants visa). None = no restriction.
    pub async fn get_dataset_id(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT dataset_id FROM drs_objects WHERE id = $1")
                .bind(object_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|r| r.0))
    }

    /// Get object by canonical ID, optionally expand bundle contents.
    pub async fn get_object(&self, id: &str, expand: bool) -> Result<Option<DrsObject>> {
        let row: Option<DrsObjectRow> = sqlx::query_as(
            r#"SELECT id, name, description, created_time, updated_time, version, mime_type, size, is_bundle, aliases, dataset_id
               FROM drs_objects WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let checksums = self.get_checksums(id).await?;
        let access_methods = self.get_access_methods(id).await?;
        let contents = if row.is_bundle && expand {
            Some(self.get_bundle_contents_expanded(id).await?)
        } else {
            None
        };
        let aliases: Option<Vec<String>> = row
            .aliases
            .as_ref()
            .and_then(|a| serde_json::from_value(a.clone()).ok());

        Ok(Some(DrsObject {
            id: row.id.clone(),
            self_uri: self.self_uri(&row.id),
            size: row.size,
            created_time: row.created_time.to_string(),
            checksums,
            name: row.name,
            updated_time: row.updated_time.map(|t| t.to_string()),
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
        }))
    }

    async fn get_checksums(&self, object_id: &str) -> Result<Vec<Checksum>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT type, checksum FROM drs_checksums WHERE object_id = $1")
                .bind(object_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(r#type, checksum)| Checksum { r#type, checksum })
            .collect())
    }

    async fn get_access_methods(&self, object_id: &str) -> Result<Vec<AccessMethod>> {
        let rows: Vec<AccessMethodRow> = sqlx::query_as(
            r#"SELECT type, access_id, access_url, region, headers FROM drs_access_methods WHERE object_id = $1"#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;
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
        let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
            "SELECT access_url, headers FROM drs_access_methods WHERE object_id = $1 AND access_id = $2",
        )
        .bind(object_id)
        .bind(access_id)
        .fetch_optional(&self.pool)
        .await?;
        let (access_url, headers) = match row {
            Some(r) => r,
            None => return Ok(None),
        };
        let url = access_url
            .as_ref()
            .and_then(|v| crate::access_url::parse_stored_access_url(v));
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
        sqlx::query(
            r#"INSERT INTO drs_objects (id, name, description, version, mime_type, size, is_bundle, aliases, workspace_id)
               VALUES ($1, $2, $3, NULL, $4, $5, FALSE, COALESCE($6, '[]'::jsonb), $7)"#,
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.mime_type)
        .bind(req.size)
        .bind(aliases)
        .bind(req.workspace_id.as_deref())
        .execute(&self.pool)
        .await?;
        for c in &req.checksums {
            sqlx::query(
                "INSERT INTO drs_checksums (object_id, type, checksum) VALUES ($1, $2, $3)",
            )
            .bind(&id)
            .bind(&c.r#type)
            .bind(&c.checksum)
            .execute(&self.pool)
            .await?;
        }
        sqlx::query(
            "INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted) VALUES ($1, $2, $3, $4)",
        )
        .bind(&id)
        .bind(&req.storage_backend)
        .bind(&req.storage_key)
        .bind(req.is_encrypted.unwrap_or(false))
        .execute(&self.pool)
        .await?;
        let access_id = format!("access-{}", id);
        let access_url_json = serde_json::json!({"url": format!("https://{}/ga4gh/drs/v1/objects/{}/access/{}", self.hostname, id, access_id)});
        sqlx::query(
            "INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers) VALUES ($1, 'https', $2, $3, '[]'::jsonb)",
        )
        .bind(&id)
        .bind(&access_id)
        .bind(access_url_json)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Update object (admin).
    pub async fn update_object(&self, id: &str, req: &UpdateObjectRequest) -> Result<bool> {
        let aliases_json = req
            .aliases
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Array(vec![])));
        let r = sqlx::query(
            r#"UPDATE drs_objects SET updated_time = NOW(), name = COALESCE($2, name), description = COALESCE($3, description),
               mime_type = COALESCE($4, mime_type), size = COALESCE($5, size), aliases = COALESCE($6, aliases) WHERE id = $1"#,
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.mime_type)
        .bind(req.size)
        .bind(aliases_json)
        .execute(&self.pool)
        .await?;
        if let Some(checksums) = &req.checksums {
            sqlx::query("DELETE FROM drs_checksums WHERE object_id = $1")
                .bind(id)
                .execute(&self.pool)
                .await?;
            for c in checksums {
                sqlx::query(
                    "INSERT INTO drs_checksums (object_id, type, checksum) VALUES ($1, $2, $3)",
                )
                .bind(id)
                .bind(&c.r#type)
                .bind(&c.checksum)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(r.rows_affected() > 0)
    }

    /// Delete object (admin).
    pub async fn delete_object(&self, id: &str) -> Result<bool> {
        let r = sqlx::query("DELETE FROM drs_objects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
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
        let rows: Vec<DrsObjectRow> = sqlx::query_as(
            r#"SELECT id, name, description, created_time, updated_time, version, mime_type, size, is_bundle, aliases, dataset_id
               FROM drs_objects
               WHERE ($1::text IS NULL OR mime_type = $1)
                 AND ($2::bigint IS NULL OR size >= $2)
                 AND ($3::bigint IS NULL OR size <= $3)
                 AND ($6::text IS NULL OR workspace_id = $6)
               ORDER BY created_time DESC LIMIT $4 OFFSET $5"#,
        )
        .bind(mime_type)
        .bind(min_size)
        .bind(max_size)
        .bind(limit as i64)
        .bind(offset as i64)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::new();
        for row in rows {
            if let Some(obj) = self.get_object(&row.id, false).await? {
                out.push(obj);
            }
        }
        Ok(out)
    }

    /// Storage ref for object (backend, key, is_encrypted).
    pub async fn get_storage_ref(&self, object_id: &str) -> Result<Option<(String, String, bool)>> {
        let row: Option<(String, String, bool)> = sqlx::query_as(
            "SELECT storage_backend, storage_key, is_encrypted FROM storage_references WHERE object_id = $1",
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;
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
        sqlx::query(
            "INSERT INTO drs_access_log (object_id, access_id, method, status, client_ip) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(object_id)
        .bind(access_id)
        .bind(method)
        .bind(status as i32)
        .bind(client_ip)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // --- Ingest jobs (`/api/v1/ingest`, Lab Kit) ---

    /// Find ingest job by idempotent client key (if set).
    pub async fn ingest_job_by_client_request_id(
        &self,
        client_request_id: &str,
    ) -> Result<Option<DrsIngestJobRow>> {
        let row: Option<DrsIngestJobRow> = sqlx::query_as(
            r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
               FROM drs_ingest_jobs WHERE client_request_id = $1"#,
        )
        .bind(client_request_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn ingest_job_get(&self, id: &str) -> Result<Option<DrsIngestJobRow>> {
        let row: Option<DrsIngestJobRow> = sqlx::query_as(
            r#"SELECT id, client_request_id, job_type, status, created_at, updated_at, result_json, error_json
               FROM drs_ingest_jobs WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn ingest_job_insert(
        &self,
        id: &str,
        client_request_id: Option<&str>,
        job_type: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO drs_ingest_jobs (id, client_request_id, job_type, status)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(id)
        .bind(client_request_id)
        .bind(job_type)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn ingest_job_finish_success(
        &self,
        id: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE drs_ingest_jobs SET status = 'succeeded', result_json = $2, error_json = NULL, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(result)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn ingest_job_finish_failed(
        &self,
        id: &str,
        error: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE drs_ingest_jobs SET status = 'failed', error_json = $2, updated_at = NOW() WHERE id = $1"#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
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
            let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
                r#"SELECT c.object_id, c.name, c.drs_uri, o.is_bundle
                   FROM drs_bundle_contents c
                   JOIN drs_objects o ON o.id = c.object_id
                   WHERE c.bundle_id = $1"#,
            )
            .bind(&bid)
            .fetch_all(&self.pool)
            .await?;
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

        // Fetch page_size + 1 to decide whether a next token exists.
        let limit = (page_size as i64) + 1;

        let mut rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT c.object_id, c.name, c.drs_uri
            FROM drs_bundle_contents c
            WHERE c.bundle_id = $1
              AND ($2::text IS NULL OR c.object_id > $2::text)
            ORDER BY c.object_id
            LIMIT $3
            "#,
        )
        .bind(bundle_id)
        .bind(last_seen.as_deref())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let next_page_token = if rows.len() as u32 > page_size {
            let last_in_page = rows[page_size as usize - 1].clone();
            // Drop the extra element; `contents` should contain exactly `page_size` rows.
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
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT key, value FROM drs_object_metadata WHERE object_id = $1")
                .bind(object_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect())
    }

    /// Set a single metadata key-value for an object (upsert).
    pub async fn set_metadata(&self, object_id: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO drs_object_metadata (object_id, key, value) VALUES ($1, $2, $3)
             ON CONFLICT (object_id, key) DO UPDATE SET value = $3",
        )
        .bind(object_id)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns checksum compute status stored in `drs_object_metadata`.
    ///
    /// Values follow Ferrum's async checksum model:
    /// - `pending`
    /// - `computed`
    /// - `failed:<reason>` (best-effort)
    pub async fn get_checksum_status(&self, object_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM drs_object_metadata WHERE object_id = $1 AND key = $2",
        )
        .bind(object_id)
        .bind(Self::CHECKSUM_STATUS_META_KEY)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Set checksum status for an object.
    pub async fn set_checksum_status(&self, object_id: &str, status: &str) -> Result<()> {
        self.set_metadata(object_id, Self::CHECKSUM_STATUS_META_KEY, status)
            .await
    }

    /// Upsert checksums in `drs_checksums`.
    pub async fn upsert_checksums(
        &self,
        object_id: &str,
        checksums: &[(&str, &str)],
    ) -> Result<()> {
        for (typ, checksum) in checksums {
            sqlx::query(
                "INSERT INTO drs_checksums (object_id, type, checksum)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (object_id, type)
                 DO UPDATE SET checksum = EXCLUDED.checksum",
            )
            .bind(object_id)
            .bind(typ)
            .bind(checksum)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}
