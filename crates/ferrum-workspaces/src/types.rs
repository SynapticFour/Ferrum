use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceRole {
    Owner,
    Editor,
    Viewer,
}

impl WorkspaceRole {
    pub fn can_read(&self) -> bool {
        true
    }
    pub fn can_write(&self) -> bool {
        matches!(self, WorkspaceRole::Editor | WorkspaceRole::Owner)
    }
    pub fn can_manage_members(&self) -> bool {
        matches!(self, WorkspaceRole::Owner)
    }
    pub fn can_delete(&self) -> bool {
        matches!(self, WorkspaceRole::Owner)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WorkspaceRole::Owner => "owner",
            WorkspaceRole::Editor => "editor",
            WorkspaceRole::Viewer => "viewer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "owner" => Some(WorkspaceRole::Owner),
            "editor" => Some(WorkspaceRole::Editor),
            "viewer" => Some(WorkspaceRole::Viewer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_sub: String,
    pub slug: String,
    pub is_archived: bool,
    pub settings: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub workspace_id: String,
    pub sub: String,
    pub role: String,
    pub invited_by: String,
    pub joined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInvite {
    pub id: String,
    pub workspace_id: String,
    pub email: String,
    pub role: String,
    pub invited_by: String,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceActivityItem {
    pub id: String,
    pub workspace_id: String,
    pub sub: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub details: serde_json::Value,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub label: Option<String>,
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    pub count: usize,
    pub recent: Vec<RecentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContents {
    pub drs_objects: ContentSummary,
    pub wes_runs: ContentSummary,
    pub cohorts: ContentSummary,
    pub total_size_bytes: i64,
    pub active_runs: usize,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub description: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub sub: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub email: String,
    pub role: String,
    #[serde(default)]
    pub message: Option<String>,
}
