// SPDX-License-Identifier: BUSL-1.1
use crate::error::{Result, TrsError};
use crate::repo::TrsRepo;
use crate::types::*;
use axum::extract::{Extension, Path, Query, State};
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
    let mut tool =
        crate::types::tool_from_row(id, name, description, organization, toolclass, meta_version);
    tool.url = Some(url);
    tool.versions = Some(versions);
    Ok(Json(tool))
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(body): Json<RegisterToolRequest>,
) -> Result<Json<Tool>> {
    if ferrum_core::require_auth_enabled() && auth.is_none() {
        return Err(TrsError::Unauthorized(
            "Bearer authentication required when require_auth is enabled".into(),
        ));
    }
    let workflow_url = body
        .workflow_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let workflow_content = body
        .workflow_content
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if workflow_url.is_none() && workflow_content.is_none() {
        return Err(TrsError::Validation(
            "workflow_url or workflow_content required".into(),
        ));
    }

    let tool_id = ulid::Ulid::new().to_string();
    let version_id = ulid::Ulid::new().to_string();
    let version_name = body
        .workflow_type_version
        .as_deref()
        .unwrap_or("1.0")
        .to_string();
    let descriptor_type = normalize_descriptor_type(body.workflow_type.as_deref());

    state
        .repo
        .create_tool(
            &tool_id,
            Some(
                body.name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Unnamed workflow"),
            ),
            Some(
                body.description
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Registered from WES"),
            ),
            Some(
                body.organization
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Ferrum"),
            ),
            body.toolclass.as_deref().or(Some("Workflow")),
            body.workflow_type_version.as_deref(),
        )
        .await?;
    state
        .repo
        .add_version(&version_id, &tool_id, &version_name)
        .await?;
    state
        .repo
        .add_descriptor(
            &tool_id,
            &version_id,
            &descriptor_type,
            workflow_content,
            workflow_url,
        )
        .await?;

    let row = state
        .repo
        .get_tool(&tool_id)
        .await?
        .ok_or_else(|| TrsError::NotFound("registered tool missing after insert".into()))?;
    let versions = state.repo.get_versions(&tool_id).await.unwrap_or_default();
    let mut tool = crate::types::tool_from_row(row.0, row.1, row.2, row.3, row.4, row.5);
    tool.url = Some(format!("/ga4gh/trs/v2/tools/{}", tool_id));
    tool.versions = Some(versions);
    Ok(Json(tool))
}

fn normalize_descriptor_type(workflow_type: Option<&str>) -> String {
    match workflow_type.map(|s| s.to_lowercase()).as_deref() {
        Some("nextflow") | Some("nxf") | Some("nfl") => "NFL".to_string(),
        Some("cwl") => "CWL".to_string(),
        Some("wdl") => "WDL".to_string(),
        Some("snakemake") | Some("smk") => "SMK".to_string(),
        Some(other) if !other.is_empty() => other.to_uppercase(),
        _ => "WDL".to_string(),
    }
}
