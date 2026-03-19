use crate::error::{Result, TrsError};
use crate::repo::TrsRepo;
use crate::types::*;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

pub struct AppState {
    pub repo: Arc<TrsRepo>,
}

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct ListToolsQuery {
    pub page_size: Option<i64>,
    pub page_token: Option<String>,
}

/// Query params for GET .../descriptor?type=CWL (some TRS clients use query instead of path).
#[derive(Debug, serde::Deserialize, IntoParams)]
pub struct DescriptorQuery {
    #[serde(alias = "type")]
    pub descriptor_type: Option<String>,
}

#[utoipa::path(get, path = "/service-info", responses((status = 200, body = TrsServiceInfo)))]
pub async fn get_service_info() -> Json<TrsServiceInfo> {
    Json(TrsServiceInfo {
        id: "ferrum-trs".to_string(),
        name: "Ferrum TRS".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /tools returns a root-level JSON array for GA4GH/HelixTest compatibility ("TRS /tools must return array").
#[utoipa::path(get, path = "/tools", params(ListToolsQuery), responses((status = 200, body = Vec<Tool>)))]
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListToolsQuery>,
) -> Result<Json<Vec<Tool>>> {
    let page_size = q.page_size.unwrap_or(100).min(1000);
    let (tools, _next_page_token) = state
        .repo
        .list_tools(page_size, q.page_token.as_deref())
        .await?;
    let mut out = Vec::with_capacity(tools.len());
    for t in tools {
        let versions = state.repo.get_versions(&t.id).await.unwrap_or_default();
        out.push(Tool {
            url: Some(format!("/ga4gh/trs/v2/tools/{}", t.id)),
            versions: Some(versions),
            ..t
        });
    }
    Ok(Json(out))
}

#[utoipa::path(get, path = "/tools/{id}", responses((status = 200, body = Tool), (status = 404)))]
pub async fn get_tool(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Tool>> {
    let row = state
        .repo
        .get_tool(&id)
        .await?
        .ok_or_else(|| TrsError::NotFound(format!("tool not found: {}", id)))?;
    let (id, name, description, organization, toolclass, meta_version) = row;
    let versions = state.repo.get_versions(&id).await.unwrap_or_default();
    let url = format!("/ga4gh/trs/v2/tools/{}", id);
    Ok(Json(Tool {
        id,
        name,
        description,
        organization,
        toolclass: toolclass.map(|s| ToolClass {
            id: Some(s.clone()),
            name: Some(s),
        }),
        meta_version,
        url: Some(url),
        versions: Some(versions),
    }))
}

// HelixTest + GA4GH TRS expect GET /tools/{id}/versions to return a root-level JSON array of ToolVersion.
#[utoipa::path(get, path = "/tools/{id}/versions", responses((status = 200, body = Vec<ToolVersion>), (status = 404)))]
pub async fn get_tool_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ToolVersion>>> {
    if state.repo.get_tool(&id).await?.is_none() {
        return Err(TrsError::NotFound(format!("tool not found: {}", id)));
    }
    let versions = state.repo.get_versions(&id).await?;
    Ok(Json(versions))
}

#[utoipa::path(get, path = "/tools/{id}/versions/{version_id}/descriptor/{descriptor_type}", responses((status = 200, body = String), (status = 404)))]
pub async fn get_descriptor(
    State(state): State<Arc<AppState>>,
    Path((id, version_id, descriptor_type)): Path<(String, String, String)>,
) -> Result<axum::response::Response> {
    let content = state
        .repo
        .get_descriptor(&id, &version_id, &descriptor_type)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                tool_id = %id,
                version_id = %version_id,
                descriptor_type = %descriptor_type,
                "TRS descriptor not found"
            );
            TrsError::NotFound("descriptor not found".into())
        })?;
    Ok(([("content-type", "text/plain; charset=utf-8")], content).into_response())
}

/// GET .../descriptor?type=CWL — same as path form, for clients that pass descriptor type as query param.
pub async fn get_descriptor_query(
    State(state): State<Arc<AppState>>,
    Path((id, version_id)): Path<(String, String)>,
    Query(q): Query<DescriptorQuery>,
) -> Result<axum::response::Response> {
    let descriptor_type = q.descriptor_type.unwrap_or_else(|| "CWL".to_string());
    let content = state
        .repo
        .get_descriptor(&id, &version_id, &descriptor_type)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                tool_id = %id,
                version_id = %version_id,
                descriptor_type = %descriptor_type,
                "TRS descriptor not found (query param)"
            );
            TrsError::NotFound("descriptor not found".into())
        })?;
    Ok(([("content-type", "text/plain; charset=utf-8")], content).into_response())
}

/// Internal: register a tool (e.g. from WES when a workflow is submitted).
#[utoipa::path(post, path = "/internal/register", request_body = RegisterToolRequest, responses((status = 200, body = Tool)))]
pub async fn register_tool(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterToolRequest>,
) -> Result<Json<Tool>> {
    let tool_id = ulid::Ulid::new().to_string();
    let version_id = ulid::Ulid::new().to_string();
    state
        .repo
        .create_tool(
            &tool_id,
            body.name.as_deref(),
            body.description.as_deref(),
            body.organization.as_deref(),
            body.toolclass.as_deref(),
            body.workflow_type_version.as_deref(),
        )
        .await?;
    state
        .repo
        .add_version(
            &version_id,
            &tool_id,
            body.workflow_type_version.as_deref().unwrap_or("1"),
        )
        .await?;
    let row = state.repo.get_tool(&tool_id).await?.unwrap();
    Ok(Json(Tool {
        id: row.0,
        name: row.1,
        description: row.2,
        organization: row.3,
        toolclass: row.4.map(|s| ToolClass {
            id: Some(s.clone()),
            name: Some(s),
        }),
        meta_version: row.5,
        url: None,
        versions: None,
    }))
}
