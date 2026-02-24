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
    pub workflow_url: String,
    pub workflow_type: Option<String>,
    pub workflow_type_version: Option<String>,
}
