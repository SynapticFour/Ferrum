//! Minimal workspace helpers for DRS/WES integration: membership check only.

use crate::error::Result;

/// Returns the member's role if they belong to the workspace, None otherwise.
pub async fn get_workspace_member_role(
    pool: &sqlx::PgPool,
    workspace_id: &str,
    sub: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT role FROM workspace_members WHERE workspace_id = $1 AND sub = $2")
            .bind(workspace_id)
            .bind(sub)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// Returns true if sub is a member with editor or owner role (can write).
pub async fn is_workspace_editor_or_owner(
    pool: &sqlx::PgPool,
    workspace_id: &str,
    sub: &str,
) -> Result<bool> {
    let role = get_workspace_member_role(pool, workspace_id, sub).await?;
    Ok(role
        .as_deref()
        .map(|r| r == "owner" || r == "editor")
        .unwrap_or(false))
}
