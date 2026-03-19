//! TES 1.1 HTTP handlers.

use crate::error::{Result, TesError};
use crate::state::AppState;
use crate::types::*;
use axum::extract::{Path, Query, State};
use axum::Json;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct ListTasksQuery {
    pub page_size: Option<i64>,
    pub page_token: Option<String>,
    pub state: Option<String>,
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
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<CreateTaskResponse>> {
    if body.executors.is_empty() {
        return Err(TesError::Validation("executors required".into()));
    }
    let id = ulid::Ulid::new().to_string();
    let inputs = body
        .inputs
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap())
        .unwrap_or(serde_json::json!([]));
    let outputs = body
        .outputs
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap())
        .unwrap_or(serde_json::json!([]));
    let executors = serde_json::to_value(&body.executors).unwrap();
    let resources = body.resources.as_ref();
    let volumes = body
        .volumes
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap());
    let tags = body.tags.as_ref().map(|m| serde_json::to_value(m).unwrap());
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
    let external_id = app.executor.run(&id, &body).await?;
    if let Some(ref ext) = external_id {
        app.repo
            .set_external_id(&id, ext, app.executor.name())
            .await?;
    }
    // Demo/CI backend: complete immediately so conformance checks can validate lifecycle.
    if app.executor.name() == "noop" {
        app.repo
            .update_state(&id, crate::types::TaskState::Complete)
            .await?;
    }
    Ok(Json(CreateTaskResponse { id }))
}

#[utoipa::path(get, path = "/tasks", params(ListTasksQuery), responses((status = 200, body = TaskListResponse)))]
pub async fn list_tasks(
    State(app): State<Arc<AppState>>,
    Query(q): Query<ListTasksQuery>,
) -> Result<Json<TaskListResponse>> {
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
    Path(id): Path<String>,
) -> Result<Json<Task>> {
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
    ) = app.repo.get(&id).await?.unwrap();
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
    Path(id): Path<String>,
) -> Result<Json<CreateTaskResponse>> {
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
