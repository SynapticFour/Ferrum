// SPDX-License-Identifier: BUSL-1.1
//! TES 1.1 HTTP handlers.

use crate::error::{Result, TesError};
use crate::state::AppState;
use crate::types::*;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

fn public_base_url() -> String {
    std::env::var("FERRUM_PUBLIC_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:8080".into())
        .trim_end_matches('/')
        .to_string()
}

/// Deterministic noop TES bytes matching HelixTest `tes_echo_out.txt` (`hello-tes` + newline).
/// Disabled unless `FERRUM_TES_HELIXTEST_STUB` is an explicit opt-in (demo compose only).
pub async fn demo_echo_output() -> axum::response::Response {
    if !ferrum_core::env_flag("FERRUM_TES_HELIXTEST_STUB") {
        return (
            StatusCode::NOT_FOUND,
            "demo echo output is disabled (set FERRUM_TES_HELIXTEST_STUB=1 for NON-PILOT demo only)",
        )
            .into_response();
    }
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        "hello-tes\n",
    )
        .into_response()
}

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct ListTasksQuery {
    pub page_size: Option<i64>,
    pub page_token: Option<String>,
    pub state: Option<String>,
}

/// Fail closed when `FERRUM_AUTH__REQUIRE_AUTH` is set and no Bearer claims were injected.
fn reject_anonymous_when_auth_required(
    auth: &Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<()> {
    if ferrum_core::require_auth_enabled() && auth.is_none() {
        return Err(TesError::Unauthorized(
            "Bearer authentication required when require_auth is enabled".into(),
        ));
    }
    Ok(())
}

#[utoipa::path(get, path = "/service-info", responses((status = 200, body = TesServiceInfo)))]
pub async fn get_service_info() -> Json<TesServiceInfo> {
    Json(TesServiceInfo {
        id: "ferrum-tes".to_string(),
        name: "Ferrum TES".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[utoipa::path(post, path = "/tasks", request_body = CreateTaskRequest, responses((status = 200, body = CreateTaskResponse)))]
pub async fn create_task(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<CreateTaskResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    if body.executors.is_empty() {
        return Err(TesError::Validation("executors required".into()));
    }
    let id = ulid::Ulid::new().to_string();
    let inputs = match body.inputs.as_ref() {
        Some(v) => serde_json::to_value(v).map_err(|e| TesError::Validation(e.to_string()))?,
        None => serde_json::json!([]),
    };
    let outputs = match body.outputs.as_ref() {
        Some(v) => serde_json::to_value(v).map_err(|e| TesError::Validation(e.to_string()))?,
        None => serde_json::json!([]),
    };
    let executors =
        serde_json::to_value(&body.executors).map_err(|e| TesError::Validation(e.to_string()))?;
    let resources = body.resources.as_ref();
    let volumes = body
        .volumes
        .as_ref()
        .map(|v| serde_json::to_value(v).map_err(|e| TesError::Validation(e.to_string())))
        .transpose()?;
    let tags = body
        .tags
        .as_ref()
        .map(|m| serde_json::to_value(m).map_err(|e| TesError::Validation(e.to_string())))
        .transpose()?;
    app.repo
        .create(
            &id,
            body.name.as_deref(),
            body.description.as_deref(),
            &inputs,
            &outputs,
            &executors,
            resources,
            volumes.as_ref(),
            tags.as_ref(),
        )
        .await?;
    app.repo
        .update_state(&id, crate::types::TaskState::Running)
        .await?;
    let external_id = match app.executor.run(&id, &body).await {
        Ok(ext) => ext,
        Err(e) => {
            let _ = app
                .repo
                .update_state(&id, crate::types::TaskState::ExecutorError)
                .await;
            return Err(e);
        }
    };
    if let Some(ref ext) = external_id {
        app.repo
            .set_external_id(&id, ext, app.executor.name())
            .await?;
    }
    // Demo/CI backend: complete immediately. HelixTest checksum stub is opt-in only.
    if app.executor.name() == "noop" {
        app.repo
            .update_state(&id, crate::types::TaskState::Complete)
            .await?;
        let empty_outputs = body.outputs.as_ref().map(|o| o.is_empty()).unwrap_or(true);
        if empty_outputs && ferrum_core::env_flag("FERRUM_TES_HELIXTEST_STUB") {
            let url = format!("{}/ga4gh/tes/v1/demo/echo-output", public_base_url());
            let outputs = serde_json::json!([{
                "path": "/test-data/workflows/outputs/tes_echo_out.txt",
                "url": url,
            }]);
            app.repo.set_outputs(&id, &outputs).await?;
        }
    }
    Ok(Json(CreateTaskResponse { id }))
}

#[utoipa::path(get, path = "/tasks", params(ListTasksQuery), responses((status = 200, body = TaskListResponse)))]
pub async fn list_tasks(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Query(q): Query<ListTasksQuery>,
) -> Result<Json<TaskListResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    let page_size = q.page_size.unwrap_or(100).min(1000);
    let (rows, next_page_token) = app
        .repo
        .list(page_size, q.page_token.as_deref(), q.state.as_deref())
        .await?;
    let tasks = rows
        .into_iter()
        .map(|(id, state)| TaskSummary { id, state })
        .collect();
    Ok(Json(TaskListResponse {
        tasks,
        next_page_token,
    }))
}

#[utoipa::path(get, path = "/tasks/{id}", params(("id" = String, Path, description = "Task ID")), responses((status = 200, body = Task), (status = 404)))]
pub async fn get_task(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Path(id): Path<String>,
) -> Result<Json<Task>> {
    reject_anonymous_when_auth_required(&auth)?;
    let row = app
        .repo
        .get(&id)
        .await?
        .ok_or_else(|| TesError::NotFound(format!("task not found: {}", id)))?;
    let (
        task_id,
        state,
        _name,
        _description,
        _inputs,
        _outputs,
        _executors,
        _resources,
        _volumes,
        _tags,
        _started_at,
        _ended_at,
        _created_at,
        external_id,
        _backend,
        _logs,
    ) = row;
    let state_enum = crate::types::TaskState::from_str(&state);
    if state_enum == crate::types::TaskState::Running {
        let polled = app.executor.poll_state(&id, external_id.as_deref()).await?;
        if polled != crate::types::TaskState::Running && polled != crate::types::TaskState::Unknown
        {
            app.repo.update_state(&id, polled).await?;
        }
    }
    let existing_logs = app.repo.get(&id).await?.and_then(|r| r.15);
    let needs_logs = existing_logs
        .as_ref()
        .map(|v| v.is_null() || v.as_array().is_none_or(|a| a.is_empty()))
        .unwrap_or(true);
    let current_state = app
        .repo
        .get(&id)
        .await?
        .map(|r| crate::types::TaskState::from_str(&r.1))
        .unwrap_or(state_enum);
    if needs_logs && current_state.is_terminal() {
        if let Ok(Some((stdout, stderr))) =
            app.executor.fetch_logs(&id, external_id.as_deref()).await
        {
            let logs = serde_json::json!([{
                "stdout": stdout,
                "stderr": stderr,
            }]);
            let _ = app.repo.set_logs(&id, &logs).await;
        }
    }
    let (
        _,
        state_str,
        name2,
        desc2,
        inputs2,
        outputs2,
        executors2,
        resources2,
        volumes2,
        tags2,
        _,
        _,
        _,
        _,
        _,
        logs2,
    ) = app
        .repo
        .get(&id)
        .await?
        .ok_or_else(|| TesError::NotFound(format!("task not found: {}", id)))?;
    let executors_vec: Option<Vec<TesExecutor>> = serde_json::from_value(executors2).ok();
    let inputs_vec: Option<Vec<TesInput>> = serde_json::from_value(inputs2).ok();
    let outputs_vec: Option<Vec<TesOutput>> = serde_json::from_value(outputs2).ok();
    let tags_map = tags2.and_then(|v| v.as_object().cloned()).map(|m| {
        m.into_iter()
            .filter_map(|(k, v)| Some((k, v.as_str()?.to_string())))
            .collect()
    });
    let logs_vec: Option<Vec<TaskLog>> = logs2.and_then(|v| serde_json::from_value(v).ok());
    Ok(Json(Task {
        id: task_id,
        state: state_str,
        name: name2,
        description: desc2,
        inputs: inputs_vec,
        outputs: outputs_vec,
        executors: executors_vec,
        resources: resources2,
        volumes: volumes2.and_then(|v| v.as_array().cloned()),
        tags: tags_map,
        logs: logs_vec,
    }))
}

#[utoipa::path(post, path = "/tasks/{id}:cancel", params(("id" = String, Path, description = "Task ID")), responses((status = 200, body = CreateTaskResponse), (status = 404)))]
pub async fn cancel_task(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Path(id): Path<String>,
) -> Result<Json<CreateTaskResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    let row = app
        .repo
        .get(&id)
        .await?
        .ok_or_else(|| TesError::NotFound(format!("task not found: {}", id)))?;
    let (_, state, _, _, _, _, _, _, _, _, _, _, _, external_id, _, _) = row;
    if state == "RUNNING" || state == "QUEUED" || state == "INITIALIZING" {
        app.executor.cancel(&id, external_id.as_deref()).await?;
        app.repo
            .update_state(&id, crate::types::TaskState::Canceled)
            .await?;
    }
    Ok(Json(CreateTaskResponse { id }))
}
