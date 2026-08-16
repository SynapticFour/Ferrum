// SPDX-License-Identifier: BUSL-1.1
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrsServiceInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Tool {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub organization: Option<String>,
    pub toolclass: Option<ToolClass>,
    pub meta_version: Option<String>,
    /// GA4GH TRS: URL to fetch this tool (e.g. /ga4gh/trs/v2/tools/{id}).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// GA4GH TRS: versions list (populated in list response for schema compliance).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<ToolVersion>>,
}

/// Map a DB row to API `Tool` with GA4GH-required string fields (never JSON `null`).
pub fn tool_from_row(
    id: String,
    name: Option<String>,
    description: Option<String>,
    organization: Option<String>,
    toolclass: Option<String>,
    meta_version: Option<String>,
) -> Tool {
    Tool {
        id: id.clone(),
        name: Some(name.unwrap_or_else(|| id.clone())),
        description: Some(description.unwrap_or_default()),
        organization: Some(organization.unwrap_or_else(|| "Ferrum".into())),
        toolclass: toolclass.map(|s| ToolClass {
            id: Some(s.clone()),
            name: Some(s),
        }),
        meta_version,
        url: None,
        versions: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ToolClass {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ToolVersion {
    pub id: String,
    pub name: String,
    pub tool_id: String,
    /// GA4GH TRS: URL to fetch this tool version (e.g. /ga4gh/trs/v2/tools/{id}/versions).
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ToolVersionsResponse {
    pub versions: Vec<ToolVersion>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ToolListResponse {
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RegisterToolRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub organization: Option<String>,
    pub toolclass: Option<String>,
    /// Remote descriptor URL (WDL/CWL/NFL/SMK). Required unless `workflow_content` is set.
    pub workflow_url: Option<String>,
    /// Inline workflow file body (alternative to `workflow_url`).
    pub workflow_content: Option<String>,
    pub workflow_type: Option<String>,
    pub workflow_type_version: Option<String>,
}
