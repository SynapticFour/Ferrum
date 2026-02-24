use crate::error::{Result, WorkspaceError};
use crate::guard;
use crate::repo::WorkspaceRepo;
use crate::state::AppState;
use crate::types::*;
use axum::{
    extract::{Path, State},
    Extension,
    Json,
};
use ferrum_core::AuthClaims;
use std::sync::Arc;

fn slug_from_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(64)
        .collect()
}

pub async fn list_my_workspaces(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Vec<Workspace>>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let list = repo.list_by_member(sub).await?;
    Ok(Json(list))
}

pub async fn create_workspace(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthClaims>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<Json<Workspace>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(WorkspaceError::Validation("name required".to_string()));
    }
    let slug = req
        .slug
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slug_from_name(name));
    let repo = WorkspaceRepo::new(state.pool.clone());
    if repo.slug_exists(&slug).await? {
        return Err(WorkspaceError::Conflict("slug already in use".to_string()));
    }
    let id = ulid::Ulid::new().to_string();
    let settings = serde_json::json!({});
    let workspace = repo
        .create(
            &id,
            name,
            req.description.as_deref(),
            sub,
            &slug,
            &settings,
        )
        .await?;
    repo.add_member(&id, sub, "owner", sub).await?;
    state
        .activity
        .log(&id, sub, "created", None, None, serde_json::json!({ "name": name }))
        .await?;
    Ok(Json(workspace))
}

pub async fn get_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Workspace>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_read()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let workspace = repo.get_by_id(&id).await?.ok_or_else(|| WorkspaceError::NotFound(id))?;
    Ok(Json(workspace))
}

pub async fn update_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<Json<Workspace>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_write()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.update(
        &id,
        req.name.as_deref(),
        req.description.as_deref(),
        req.settings.as_ref(),
    )
    .await?;
    let workspace = repo.get_by_id(&id).await?.ok_or_else(|| WorkspaceError::NotFound(id))?;
    Ok(Json(workspace))
}

pub async fn archive_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_delete()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.archive(&id).await?;
    Ok(Json(serde_json::json!({ "archived": true })))
}

pub async fn list_members(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Vec<WorkspaceMember>>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_read()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let list = repo.list_members(&id).await?;
    Ok(Json(list))
}

pub async fn add_member(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<WorkspaceMember>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    let role_parsed = WorkspaceRole::from_str(req.role.trim()).unwrap_or(WorkspaceRole::Viewer);
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.add_member(&id, &req.sub, role_parsed.as_str(), sub).await?;
    let members = repo.list_members(&id).await?;
    let member = members.into_iter().find(|m| m.sub == req.sub).ok_or_else(|| WorkspaceError::Internal(anyhow::anyhow!("member not found")))?;
    state.activity.log(&id, sub, "added_member", Some("workspace_member"), Some(&req.sub), serde_json::json!({ "role": role_parsed.as_str() })).await?;
    Ok(Json(member))
}

pub async fn update_member_role(
    State(state): State<Arc<AppState>>,
    Path((id, member_sub)): Path<(String, String)>,
    Extension(auth): Extension<AuthClaims>,
    Json(req): Json<UpdateMemberRoleRequest>,
) -> Result<Json<WorkspaceMember>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    let role_parsed = WorkspaceRole::from_str(req.role.trim()).unwrap_or(WorkspaceRole::Viewer);
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.update_member_role(&id, &member_sub, role_parsed.as_str()).await?;
    let members = repo.list_members(&id).await?;
    let member = members.into_iter().find(|m| m.sub == member_sub).ok_or_else(|| WorkspaceError::NotFound("member".to_string()))?;
    Ok(Json(member))
}

pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    Path((id, member_sub)): Path<(String, String)>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    if member_sub == sub {
        let repo = WorkspaceRepo::new(state.pool.clone());
        let owners = repo.count_owners(&id).await?;
        if owners <= 1 {
            return Err(WorkspaceError::Forbidden("cannot remove the last owner".to_string()));
        }
    }
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.remove_member(&id, &member_sub).await?;
    Ok(Json(serde_json::json!({ "removed": true })))
}

pub async fn list_invites(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Vec<WorkspaceInvite>>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let list = repo.list_invites(&id).await?;
    Ok(Json(list))
}

pub async fn create_invite(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
    Json(req): Json<CreateInviteRequest>,
) -> Result<Json<WorkspaceInvite>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    let email = req.email.trim();
    if email.is_empty() {
        return Err(WorkspaceError::Validation("email required".to_string()));
    }
    let role_parsed = WorkspaceRole::from_str(req.role.trim()).unwrap_or(WorkspaceRole::Viewer);
    let token: String = (0..32).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
    let invite_id = ulid::Ulid::new().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    let repo = WorkspaceRepo::new(state.pool.clone());
    let invite = repo
        .create_invite(&invite_id, &id, email, role_parsed.as_str(), &token, sub, expires_at)
        .await?;
    Ok(Json(invite))
}

pub async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    Path((id, invite_id)): Path<(String, String)>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<serde_json::Value>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_manage_members()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    repo.revoke_invite(&id, &invite_id).await?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}

pub async fn accept_invite(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Workspace>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let (invite_id, workspace_id, _role) = repo
        .get_invite_by_token(&token)
        .await?
        .ok_or_else(|| WorkspaceError::NotFound("invite expired or invalid".to_string()))?;
    repo.accept_invite(&invite_id, sub).await?;
    state.activity.log(&workspace_id, sub, "joined", None, None, serde_json::json!({})).await?;
    let workspace = repo.get_by_id(&workspace_id).await?.ok_or_else(|| WorkspaceError::NotFound(workspace_id))?;
    Ok(Json(workspace))
}

pub async fn get_activity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<Vec<WorkspaceActivityItem>>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_read()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let list = repo.get_activity(&id, 100, 0).await?;
    Ok(Json(list))
}

pub async fn get_workspace_contents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthClaims>,
) -> Result<Json<WorkspaceContents>> {
    let sub = auth.sub().ok_or_else(|| WorkspaceError::Forbidden("authentication required".to_string()))?;
    let (_ws, role) = guard::ensure_workspace_member(&state.pool, &id, sub).await?;
    role.require_read()?;
    let repo = WorkspaceRepo::new(state.pool.clone());
    let (total_size_bytes, drs_count, wes_count, cohort_count, active_runs) = repo.get_contents(&id).await?;
    Ok(Json(WorkspaceContents {
        drs_objects: ContentSummary {
            count: drs_count,
            recent: vec![],
        },
        wes_runs: ContentSummary {
            count: wes_count,
            recent: vec![],
        },
        cohorts: ContentSummary {
            count: cohort_count,
            recent: vec![],
        },
        total_size_bytes,
        active_runs,
    }))
}
