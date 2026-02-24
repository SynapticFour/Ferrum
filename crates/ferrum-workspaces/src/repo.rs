use crate::error::{Result, WorkspaceError};
use crate::types::{Workspace, WorkspaceActivityItem, WorkspaceInvite, WorkspaceMember};
use chrono::Utc;

pub struct WorkspaceRepo {
    pool: sqlx::PgPool,
}

impl WorkspaceRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        owner_sub: &str,
        slug: &str,
        settings: &serde_json::Value,
    ) -> Result<Workspace> {
        sqlx::query(
            r#"INSERT INTO workspaces (id, name, description, owner_sub, slug, settings)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(owner_sub)
        .bind(slug)
        .bind(settings)
        .execute(&self.pool)
        .await?;
        self.get_by_id(id).await?.ok_or_else(|| WorkspaceError::NotFound(id.to_string()))
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Workspace>> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, bool, serde_json::Value, Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>)>(
            "SELECT id, name, description, owner_sub, slug, is_archived, settings, created_at, updated_at FROM workspaces WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Workspace {
            id: r.0,
            name: r.1,
            description: r.2,
            owner_sub: r.3,
            slug: r.4,
            is_archived: r.5,
            settings: r.6,
            created_at: r.7,
            updated_at: r.8,
        }))
    }

    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Workspace>> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, bool, serde_json::Value, Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>)>(
            "SELECT id, name, description, owner_sub, slug, is_archived, settings, created_at, updated_at FROM workspaces WHERE slug = $1 AND NOT is_archived",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Workspace {
            id: r.0,
            name: r.1,
            description: r.2,
            owner_sub: r.3,
            slug: r.4,
            is_archived: r.5,
            settings: r.6,
            created_at: r.7,
            updated_at: r.8,
        }))
    }

    pub async fn list_by_member(&self, sub: &str) -> Result<Vec<Workspace>> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, bool, serde_json::Value, Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>)>(
            r#"SELECT w.id, w.name, w.description, w.owner_sub, w.slug, w.is_archived, w.settings, w.created_at, w.updated_at
               FROM workspaces w
               JOIN workspace_members m ON w.id = m.workspace_id
               WHERE m.sub = $1 AND NOT w.is_archived
               ORDER BY w.updated_at DESC NULLS LAST"#,
        )
        .bind(sub)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Workspace {
                id: r.0,
                name: r.1,
                description: r.2,
                owner_sub: r.3,
                slug: r.4,
                is_archived: r.5,
                settings: r.6,
                created_at: r.7,
                updated_at: r.8,
            })
            .collect())
    }

    pub async fn update(&self, id: &str, name: Option<&str>, description: Option<&str>, settings: Option<&serde_json::Value>) -> Result<bool> {
        let r = sqlx::query(
            r#"UPDATE workspaces SET updated_at = now(),
               name = COALESCE($2, name),
               description = COALESCE($3, description),
               settings = COALESCE($4, settings)
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(settings)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn archive(&self, id: &str) -> Result<bool> {
        let r = sqlx::query("UPDATE workspaces SET is_archived = true, updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn get_member_role(&self, workspace_id: &str, sub: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT role FROM workspace_members WHERE workspace_id = $1 AND sub = $2")
                .bind(workspace_id)
                .bind(sub)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn add_member(&self, workspace_id: &str, sub: &str, role: &str, invited_by: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO workspace_members (workspace_id, sub, role, invited_by) VALUES ($1, $2, $3, $4) ON CONFLICT (workspace_id, sub) DO UPDATE SET role = $3",
        )
        .bind(workspace_id)
        .bind(sub)
        .bind(role)
        .bind(invited_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<chrono::DateTime<Utc>>)>(
            "SELECT workspace_id, sub, role, invited_by, joined_at FROM workspace_members WHERE workspace_id = $1 ORDER BY joined_at",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| WorkspaceMember {
                workspace_id: r.0,
                sub: r.1,
                role: r.2,
                invited_by: r.3,
                joined_at: r.4,
            })
            .collect())
    }

    pub async fn update_member_role(&self, workspace_id: &str, sub: &str, role: &str) -> Result<bool> {
        let r = sqlx::query("UPDATE workspace_members SET role = $3 WHERE workspace_id = $1 AND sub = $2")
            .bind(workspace_id)
            .bind(sub)
            .bind(role)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn remove_member(&self, workspace_id: &str, sub: &str) -> Result<bool> {
        let r = sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND sub = $2")
            .bind(workspace_id)
            .bind(sub)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn count_owners(&self, workspace_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM workspace_members WHERE workspace_id = $1 AND role = 'owner'")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn create_invite(
        &self,
        id: &str,
        workspace_id: &str,
        email: &str,
        role: &str,
        token: &str,
        invited_by: &str,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<WorkspaceInvite> {
        sqlx::query(
            r#"INSERT INTO workspace_invites (id, workspace_id, email, role, token, invited_by, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(email)
        .bind(role)
        .bind(token)
        .bind(invited_by)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(WorkspaceInvite {
            id: id.to_string(),
            workspace_id: workspace_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            invited_by: invited_by.to_string(),
            expires_at,
            accepted_at: None,
        })
    }

    pub async fn get_invite_by_token(&self, token: &str) -> Result<Option<(String, String, String)>> {
        let row = sqlx::query_as::<_, (String, String, String, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
            "SELECT id, workspace_id, role, expires_at, accepted_at FROM workspace_invites WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| if r.4.is_none() && r.3 > Utc::now() { Some((r.0, r.1, r.2)) } else { None }))
    }

    pub async fn list_invites(&self, workspace_id: &str) -> Result<Vec<WorkspaceInvite>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
            "SELECT id, workspace_id, email, role, invited_by, expires_at, accepted_at FROM workspace_invites WHERE workspace_id = $1 ORDER BY expires_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| WorkspaceInvite {
                id: r.0,
                workspace_id: r.1,
                email: r.2,
                role: r.3,
                invited_by: r.4,
                expires_at: r.5,
                accepted_at: r.6,
            })
            .collect())
    }

    pub async fn revoke_invite(&self, workspace_id: &str, invite_id: &str) -> Result<bool> {
        let r = sqlx::query("DELETE FROM workspace_invites WHERE workspace_id = $1 AND id = $2")
            .bind(workspace_id)
            .bind(invite_id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn accept_invite(&self, invite_id: &str, sub: &str) -> Result<()> {
        let inv: Option<(String, String)> = sqlx::query_as("SELECT workspace_id, role FROM workspace_invites WHERE id = $1")
            .bind(invite_id)
            .fetch_optional(&self.pool)
            .await?;
        let (ws_id, role) = inv.ok_or_else(|| WorkspaceError::NotFound("invite".to_string()))?;
        self.add_member(&ws_id, sub, &role, "invite").await?;
        sqlx::query("UPDATE workspace_invites SET accepted_at = now() WHERE id = $1")
            .bind(invite_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_activity(&self, workspace_id: &str, limit: i64, offset: i64) -> Result<Vec<WorkspaceActivityItem>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>, serde_json::Value, Option<chrono::DateTime<Utc>>)>(
            "SELECT id, workspace_id, sub, action, resource_type, resource_id, details, occurred_at FROM workspace_activity WHERE workspace_id = $1 ORDER BY occurred_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(workspace_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| WorkspaceActivityItem {
                id: r.0,
                workspace_id: r.1,
                sub: r.2,
                action: r.3,
                resource_type: r.4,
                resource_id: r.5,
                details: r.6,
                occurred_at: r.7,
            })
            .collect())
    }

    pub async fn get_contents(&self, workspace_id: &str) -> Result<(i64, usize, usize, usize, usize)> {
        let size: (Option<i64>,) = sqlx::query_as("SELECT COALESCE(sum(size), 0) FROM drs_objects WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let drs_count: (i64,) = sqlx::query_as("SELECT count(*) FROM drs_objects WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let wes_count: (i64,) = sqlx::query_as("SELECT count(*) FROM wes_runs WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let active: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM wes_runs WHERE workspace_id = $1 AND state IN ('RUNNING', 'QUEUED', 'INITIALIZING', 'CANCELING')",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;
        let cohort_count: (i64,) = sqlx::query_as("SELECT count(*) FROM cohorts WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((
            size.0.unwrap_or(0),
            drs_count.0 as usize,
            wes_count.0 as usize,
            cohort_count.0 as usize,
            active.0 as usize,
        ))
    }

    pub async fn slug_exists(&self, slug: &str) -> Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as("SELECT true FROM workspaces WHERE slug = $1 LIMIT 1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}
