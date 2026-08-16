// SPDX-License-Identifier: BUSL-1.1
use crate::error::Result;
use crate::types::{Tool, ToolVersion};
use sqlx::PgPool;

pub struct TrsRepo {
    pool: PgPool,
}

impl TrsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_tool(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        organization: Option<&str>,
        toolclass: Option<&str>,
        meta_version: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO trs_tools (id, name, description, organization, toolclass, meta_version)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(organization)
        .bind(toolclass)
        .bind(meta_version)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_tool(
        &self,
        id: &str,
    ) -> Result<
        Option<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
    > {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
            "SELECT id, name, description, organization, toolclass, meta_version FROM trs_tools WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_tools(
        &self,
        page_size: i64,
        page_token: Option<&str>,
    ) -> Result<(Vec<Tool>, Option<String>)> {
        type ToolRow = (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let offset: i64 = page_token.and_then(|t| t.parse().ok()).unwrap_or(0);
        let rows: Vec<ToolRow> = sqlx::query_as(
            r#"SELECT id, name, description, organization, toolclass, meta_version
                   FROM trs_tools
                   ORDER BY
                     CASE
                       WHEN id = 'demo-bam-to-vcf' THEN 0
                       WHEN id LIKE 'demo-%' OR id LIKE 'tiny-%' THEN 1
                       ELSE 2
                     END,
                     id
                   LIMIT $1 OFFSET $2"#,
        )
        .bind(page_size + 1)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > page_size;
        let tools = rows
            .into_iter()
            .take(page_size as usize)
            .map(
                |(id, name, description, organization, toolclass, meta_version)| {
                    crate::types::tool_from_row(
                        id,
                        name,
                        description,
                        organization,
                        toolclass,
                        meta_version,
                    )
                },
            )
            .collect();
        let next = if has_more {
            Some((offset + page_size).to_string())
        } else {
            None
        };
        Ok((tools, next))
    }

    pub async fn add_version(&self, version_id: &str, tool_id: &str, name: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO trs_tool_versions (id, tool_id, name) VALUES ($1, $2, $3) ON CONFLICT (tool_id, name) DO NOTHING",
        )
        .bind(version_id)
        .bind(tool_id)
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn add_descriptor(
        &self,
        tool_id: &str,
        version_id: &str,
        descriptor_type: &str,
        content: Option<&str>,
        url: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO trs_files (tool_id, version_id, file_type, descriptor_type, content, url)
               VALUES ($1, $2, 'DESCRIPTOR', $3, $4, $5)"#,
        )
        .bind(tool_id)
        .bind(version_id)
        .bind(descriptor_type)
        .bind(content)
        .bind(url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_versions(&self, tool_id: &str) -> Result<Vec<ToolVersion>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, name, tool_id FROM trs_tool_versions WHERE tool_id = $1 ORDER BY name",
        )
        .bind(tool_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, tool_id)| {
                let url = format!("/ga4gh/trs/v2/tools/{}/versions", tool_id);
                ToolVersion {
                    id,
                    name,
                    tool_id,
                    url,
                }
            })
            .collect())
    }

    /// Resolve version_id to the actual version row id. GA4GH TRS allows clients to use
    /// either the version id (e.g. "demo-bam-to-vcf-1.0") or the version name (e.g. "1.0")
    /// in the URL; we store by id, so if the given version_id is a name, look it up.
    async fn resolve_version_id(&self, tool_id: &str, version_id: &str) -> Result<Option<String>> {
        // Already an id if we have a row
        let row: Option<(String,)> =
            sqlx::query_as("SELECT id FROM trs_tool_versions WHERE tool_id = $1 AND id = $2")
                .bind(tool_id)
                .bind(version_id)
                .fetch_optional(&self.pool)
                .await?;
        if row.is_some() {
            return Ok(Some(version_id.to_string()));
        }
        // Try as version name
        let row: Option<(String,)> =
            sqlx::query_as("SELECT id FROM trs_tool_versions WHERE tool_id = $1 AND name = $2")
                .bind(tool_id)
                .bind(version_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn get_descriptor(
        &self,
        tool_id: &str,
        version_id: &str,
        descriptor_type: &str,
    ) -> Result<Option<String>> {
        let resolved = self.resolve_version_id(tool_id, version_id).await?;
        let version_id = match resolved {
            Some(id) => id,
            None => return Ok(None),
        };
        // 1) Prefer exact descriptor_type match (case-insensitive).
        // 2) If nothing matches, return any available DESCRIPTOR for the tool+version.
        let wanted = descriptor_type.trim();
        if !wanted.is_empty() {
            let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
                r#"SELECT content, url
                   FROM trs_files
                   WHERE tool_id = $1
                     AND version_id = $2
                     AND file_type = 'DESCRIPTOR'
                     AND UPPER(descriptor_type) = UPPER($3)
                   ORDER BY created_at DESC"#,
            )
            .bind(tool_id)
            .bind(&version_id)
            .bind(wanted)
            .fetch_optional(&self.pool)
            .await?;
            if let Some((content, url)) = row {
                return Ok(resolve_descriptor_content(content, url).await);
            }
        }

        let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT content, url
               FROM trs_files
               WHERE tool_id = $1
                 AND version_id = $2
                 AND file_type = 'DESCRIPTOR'
               ORDER BY created_at DESC"#,
        )
        .bind(tool_id)
        .bind(&version_id)
        .fetch_optional(&self.pool)
        .await?;
        if let Some((content, url)) = row {
            return Ok(resolve_descriptor_content(content, url).await);
        }
        Ok(None)
    }
}

async fn resolve_descriptor_content(
    content: Option<String>,
    url: Option<String>,
) -> Option<String> {
    if let Some(c) = content.filter(|s| !s.is_empty()) {
        return Some(c);
    }
    let remote = url.filter(|s| s.starts_with("http://") || s.starts_with("https://"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok()?;
    client
        .get(&remote)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()
}
