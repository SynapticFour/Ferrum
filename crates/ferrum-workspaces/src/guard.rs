use crate::error::{Result, WorkspaceError};
use crate::repo::WorkspaceRepo;
use crate::types::{Workspace, WorkspaceRole};

/// Load workspace and check membership; returns (workspace, role) or error.
pub async fn ensure_workspace_member(
    pool: &sqlx::PgPool,
    workspace_id: &str,
    sub: &str,
) -> Result<(Workspace, WorkspaceRole)> {
    let repo = WorkspaceRepo::new(pool.clone());
    let workspace = repo
        .get_by_id(workspace_id)
        .await?
        .ok_or_else(|| WorkspaceError::NotFound(workspace_id.to_string()))?;

    if workspace.is_archived {
        return Err(WorkspaceError::NotFound(workspace_id.to_string()));
    }

    let role_str = repo
        .get_member_role(&workspace.id, sub)
        .await?
        .ok_or_else(|| WorkspaceError::Forbidden("not a member of this workspace".to_string()))?;

    let role = WorkspaceRole::from_str(&role_str).unwrap_or(WorkspaceRole::Viewer);

    Ok((workspace, role))
}

impl WorkspaceRole {
    pub fn require_read(&self) -> Result<()> {
        if self.can_read() {
            Ok(())
        } else {
            Err(WorkspaceError::Forbidden("read not allowed".to_string()))
        }
    }
    pub fn require_write(&self) -> Result<()> {
        if self.can_write() {
            Ok(())
        } else {
            Err(WorkspaceError::Forbidden("write not allowed".to_string()))
        }
    }
    pub fn require_manage_members(&self) -> Result<()> {
        if self.can_manage_members() {
            Ok(())
        } else {
            Err(WorkspaceError::Forbidden(
                "member management not allowed".to_string(),
            ))
        }
    }
    pub fn require_delete(&self) -> Result<()> {
        if self.can_delete() {
            Ok(())
        } else {
            Err(WorkspaceError::Forbidden("delete not allowed".to_string()))
        }
    }
}
