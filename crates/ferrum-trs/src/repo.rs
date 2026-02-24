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

    pub async fn get_tool(&self, id: &str) -> Result<Option<(String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>> {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
            "SELECT id, name, description, organization, toolclass, meta_version FROM trs_tools WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_tools(&self, page_size: i64, page_token: Option<&str>) -> Result<(Vec<Tool>, Option<String>)> {
        type ToolRow = (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);
        let offset: i64 = page_token.and_then(|t| t.parse().ok()).unwrap_or(0);
        let rows: Vec<ToolRow> =
            sqlx::query_as(
                "SELECT id, name, description, organization, toolclass, meta_version FROM trs_tools ORDER BY id LIMIT $1 OFFSET $2",
            )
            .bind(page_size + 1)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let has_more = rows.len() as i64 > page_size;
        let tools = rows
            .into_iter()
            .take(page_size as usize)
            .map(|(id, name, description, organization, toolclass, meta_version)| Tool {
                id,
                name,
                description,
                organization,
                toolclass: toolclass.map(|s| crate::types::ToolClass {
                    id: Some(s.clone()),
                    name: Some(s),
                }),
                meta_version,
            })
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

    pub async fn get_versions(&self, tool_id: &str) -> Result<Vec<ToolVersion>> {
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, name, tool_id FROM trs_tool_versions WHERE tool_id = $1 ORDER BY name")
                .bind(tool_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, tool_id)| ToolVersion { id, name, tool_id })
            .collect())
    }

    pub async fn get_descriptor(&self, tool_id: &str, version_id: &str, descriptor_type: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT content FROM trs_files WHERE tool_id = $1 AND version_id = $2 AND file_type = 'DESCRIPTOR' AND descriptor_type = $3",
        )
        .bind(tool_id)
        .bind(version_id)
        .bind(descriptor_type.to_uppercase())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }
}
