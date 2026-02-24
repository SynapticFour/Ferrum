//! Workspace and collaboration: named project containers grouping DRS, WES, cohorts under shared access control.

pub mod activity;
pub mod email;
pub mod error;
pub mod guard;
pub mod validation;
pub mod handlers;
pub mod repo;
pub mod state;
pub mod types;

use axum::{routing::{delete, get, post, put}, Router};
use std::sync::Arc;

use crate::handlers::{
    accept_invite, add_member, archive_workspace, create_invite, create_workspace, get_activity,
    get_workspace, get_workspace_contents, list_invites, list_members, list_my_workspaces,
    remove_member, revoke_invite, update_member_role, update_workspace,
};
use crate::state::AppState;

/// Build the Workspaces router. Mount at /workspaces/v1.
pub fn router(
    pool: sqlx::PgPool,
    email_sender: Option<Arc<dyn crate::email::EmailSender>>,
    invite_base_url: Option<String>,
) -> Router {
    let state = Arc::new(AppState {
        pool: pool.clone(),
        activity: Arc::new(activity::ActivityLogger::new(pool)),
        email_sender,
        invite_base_url,
    });
    Router::new()
        .route("/workspaces", get(list_my_workspaces).post(create_workspace))
        .route(
            "/workspaces/:id",
            get(get_workspace)
                .put(update_workspace)
                .delete(archive_workspace),
        )
        .route(
            "/workspaces/:id/members",
            get(list_members).post(add_member),
        )
        .route(
            "/workspaces/:id/members/:sub",
            put(update_member_role).delete(remove_member),
        )
        .route(
            "/workspaces/:id/invites",
            get(list_invites).post(create_invite),
        )
        .route(
            "/workspaces/:id/invites/:invite_id",
            delete(revoke_invite),
        )
        .route("/invites/:token/accept", post(accept_invite))
        .route("/workspaces/:id/activity", get(get_activity))
        .route("/workspaces/:id/contents", get(get_workspace_contents))
        .with_state(state)
}

pub fn router_unconfigured() -> Router {
    Router::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Workspaces service not configured",
        )
    })
}
