//! WES 1.1 HTTP handlers.

use crate::error::{Result, WesError};
use crate::state::AppState;
use crate::types::*;
use axum::{
    extract::{Multipart, Path, Query, State},
    response::{sse::{Event, KeepAlive, Sse}, IntoResponse},
    Json,
};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::{IntoParams, ToSchema};

/// GET /service-info with supported workflow types and engines.
#[utoipa::path(get, path = "/service-info", responses((status = 200, body = WesServiceInfo)))]
pub async fn get_service_info(State(app): State<Arc<AppState>>) -> Json<WesServiceInfo> {
    let mut workflow_type_versions = std::collections::HashMap::new();
    let mut workflow_engine_versions = std::collections::HashMap::new();
    for exec in app.run_manager.all_executors() {
        for (name, versions) in exec.supported_languages() {
            workflow_type_versions
                .insert(name.clone(), WorkflowTypeVersion { workflow_type_version: versions.clone() });
            workflow_engine_versions.insert(
                name,
                WorkflowEngineVersion {
                    workflow_engine_version: versions,
                },
            );
        }
    }
    let system_state_counts = app.repo.system_state_counts().await.unwrap_or_default();
    Json(WesServiceInfo {
        id: "ferrum-wes".to_string(),
        name: "Ferrum WES".to_string(),
        r#type: ServiceType {
            group: "org.ga4gh".to_string(),
            artifact: "wes".to_string(),
            version: "1.1.0".to_string(),
        },
        description: Some("GA4GH Workflow Execution Service 1.1".to_string()),
        organization: Organization {
            name: "Ferrum".to_string(),
            url: None,
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        workflow_type_versions,
        supported_wes_versions: vec!["1.1.0".to_string()],
        supported_filesystem_protocols: vec!["file".to_string(), "http".to_string(), "https".to_string()],
        workflow_engine_versions,
        default_workflow_engine_parameters: vec![],
        system_state_counts,
        auth_instructions_url: String::new(),
        tags: std::collections::HashMap::new(),
    })
}

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct ListRunsQuery {
    pub page_size: Option<i64>,
    pub page_token: Option<String>,
    pub state: Option<String>,
}

/// GET /runs
#[utoipa::path(get, path = "/runs", params(ListRunsQuery), responses((status = 200, body = RunListResponse)))]
pub async fn list_runs(
    State(app): State<Arc<AppState>>,
    Query(q): Query<ListRunsQuery>,
) -> Result<Json<RunListResponse>> {
    let page_size = q.page_size.unwrap_or(100).min(1000);
    let state_filter = q.state.as_deref().map(RunState::from_str);
    let (runs, next_page_token) = app
        .repo
        .list_runs(page_size, q.page_token.as_deref(), state_filter)
        .await?;
    Ok(Json(RunListResponse { runs, next_page_token }))
}

/// POST /runs (multipart: workflow_params, workflow_type, workflow_type_version, workflow_url, tags, etc.)
#[utoipa::path(post, path = "/runs", responses((status = 200, body = RunIdResponse)))]
pub async fn post_runs(State(app): State<Arc<AppState>>, mut multipart: Multipart) -> Result<Json<RunIdResponse>> {
    let mut workflow_params = serde_json::Value::Object(serde_json::Map::new());
    let mut workflow_type = None::<String>;
    let mut workflow_type_version = None::<String>;
    let mut workflow_url = None::<String>;
    let mut workflow_engine_params = serde_json::Value::Object(serde_json::Map::new());
    let mut tags = serde_json::Value::Object(serde_json::Map::new());

    while let Some(field) = multipart.next_field().await.map_err(|e| WesError::Other(e.into()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "workflow_params" => {
                if let Ok(text) = field.text().await {
                    workflow_params = serde_json::from_str(&text).unwrap_or(workflow_params);
                }
            }
            "workflow_type" => workflow_type = Some(field.text().await.unwrap_or_default()),
            "workflow_type_version" => workflow_type_version = Some(field.text().await.unwrap_or_default()),
            "workflow_url" => workflow_url = Some(field.text().await.unwrap_or_default()),
            "workflow_engine_parameters" => {
                if let Ok(text) = field.text().await {
                    workflow_engine_params = serde_json::from_str(&text).unwrap_or(workflow_engine_params);
                }
            }
            "tags" => {
                if let Ok(text) = field.text().await {
                    tags = serde_json::from_str(&text).unwrap_or(tags);
                }
            }
            "workflow_attachment" => {
                let _ = field.bytes().await;
            }
            _ => {}
        }
    }

    let workflow_type = workflow_type.ok_or_else(|| WesError::Validation("workflow_type required".into()))?;
    let workflow_type_version = workflow_type_version.ok_or_else(|| WesError::Validation("workflow_type_version required".into()))?;
    let workflow_url = workflow_url.ok_or_else(|| WesError::Validation("workflow_url required".into()))?;

    let run_id = ulid::Ulid::new().to_string();
    app
        .repo
        .create_run(
            &run_id,
            &workflow_url,
            &workflow_type,
            &workflow_type_version,
            &workflow_params,
            &workflow_engine_params,
            &tags,
            None,
        )
        .await?;

    let run = crate::executor::WesRun {
        run_id: run_id.clone(),
        workflow_url,
        workflow_type,
        workflow_type_version,
        workflow_params,
        workflow_engine_params,
        work_dir: None,
    };
    app.run_manager.submit(&run).await?;

    if let Some(ref base) = app.trs_register_url {
        let url = format!("{}/internal/register", base.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "workflow_url": run.workflow_url,
            "workflow_type": run.workflow_type,
            "workflow_type_version": run.workflow_type_version,
        });
        tokio::spawn(async move {
            let _ = client.post(&url).json(&body).send().await;
        });
    }

    Ok(Json(RunIdResponse { run_id }))
}

/// GET /runs/{run_id}/status
#[utoipa::path(get, path = "/runs/{run_id}/status", responses((status = 200, body = RunStatus), (status = 404)))]
pub async fn get_run_status(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<RunStatus>> {
    let state_row = app.run_manager.poll_status(&run_id).await?;
    if state_row == RunState::Unknown {
        if let Some((_, _, _, _, _, _, _, s, _, _, _, _)) = app.repo.get_run(&run_id).await? {
            return Ok(Json(RunStatus {
                run_id,
                state: RunState::from_str(&s),
            }));
        }
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    app.repo.update_state(&run_id, state_row).await?;
    Ok(Json(RunStatus {
        run_id,
        state: state_row,
    }))
}

/// GET /runs/{run_id} (full RunLog)
#[utoipa::path(get, path = "/runs/{run_id}", responses((status = 200, body = RunLog), (status = 404)))]
pub async fn get_run_log(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<RunLog>> {
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (run_id_db, workflow_url, workflow_type, workflow_type_version, _params, _ep, _tags, state_str, start_time, end_time, outputs, _work_dir) = row;
    let run_state = RunState::from_str(&state_str);
    let run_log_row: Option<(String, Vec<String>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<String>, Option<String>, Option<i32>)> =
        app.repo.get_run_log(&run_id).await?;
    let run_log = run_log_row
        .map(|(name, cmd, st, et, stdout, stderr, exit_code)| Log {
            name: Some(name),
            cmd: Some(cmd),
            start_time: st.map(|t| t.to_rfc3339()),
            end_time: et.map(|t| t.to_rfc3339()),
            stdout,
            stderr,
            exit_code,
        })
        .unwrap_or_else(|| Log {
            name: Some("main".to_string()),
            cmd: None,
            start_time: start_time.map(|t| t.to_rfc3339()),
            end_time: end_time.map(|t| t.to_rfc3339()),
            stdout: None,
            stderr: None,
            exit_code: None,
        });
    let task_logs = app.repo.get_task_logs(&run_id, 100, None).await.unwrap_or_default();
    Ok(Json(RunLog {
        run_id: run_id_db,
        request: RunRequestRef {
            workflow_type,
            workflow_type_version,
            workflow_url,
        },
        state: run_state,
        run_log,
        task_logs: Some(task_logs),
        task_logs_url: Some(format!("/runs/{}/tasks", run_id)),
        outputs: Some(outputs),
    }))
}

/// POST /runs/{run_id}/cancel
#[utoipa::path(post, path = "/runs/{run_id}/cancel", responses((status = 200, body = RunIdResponse), (status = 404)))]
pub async fn cancel_run(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<RunIdResponse>> {
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    app.run_manager.cancel(&run_id).await?;
    Ok(Json(RunIdResponse { run_id }))
}

/// GET /runs/{run_id}/tasks (paginated task logs)
#[utoipa::path(get, path = "/runs/{run_id}/tasks", responses((status = 200)))]
pub async fn list_tasks(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<TaskListResponse>> {
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let task_logs = app.repo.get_task_logs(&run_id, 100, None).await?;
    Ok(Json(TaskListResponse {
        task_logs,
        next_page_token: None,
    }))
}

/// GET /runs/{run_id}/logs/stream — Server-Sent Events stream of live stdout/stderr.
pub async fn stream_logs(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>> + Send + 'static>> {
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let rx = app
        .log_registry
        .subscribe(&run_id)
        .await
        .ok_or_else(|| WesError::NotFound(format!("no live stream for run: {}", run_id)))?;
    let stream = BroadcastStream::new(rx).map(|r| {
        Ok::<_, Infallible>(match r {
            Ok(ev) => Event::default().event(ev.stream).data(ev.data),
            Err(_) => Event::default().data("[stream closed]"),
        })
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// GET /runs/{run_id}/logs/stdout — serve stored stdout file.
pub async fn get_stdout(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<axum::response::Response> {
    let row = app.repo.get_run(&run_id).await?.and_then(|r| {
        let (_, _, _, _, _, _, _, _, _, _, _, work_dir) = r;
        work_dir.map(|d| (run_id.clone(), d))
    });
    let (_, work_dir) = row.ok_or_else(|| WesError::NotFound(format!("run or work_dir not found: {}", run_id)))?;
    let path = std::path::Path::new(&work_dir).join("stdout.txt");
    let body = tokio::fs::read_to_string(&path)
        .await
        .map_err(WesError::Io)?;
    Ok((
        [("content-type", "text/plain; charset=utf-8")],
        axum::body::Body::from(body),
    )
        .into_response())
}

/// GET /runs/{run_id}/logs/stderr — serve stored stderr file.
pub async fn get_stderr(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<axum::response::Response> {
    let row = app.repo.get_run(&run_id).await?.and_then(|r| {
        let (_, _, _, _, _, _, _, _, _, _, _, work_dir) = r;
        work_dir.map(|d| (run_id.clone(), d))
    });
    let (_, work_dir) = row.ok_or_else(|| WesError::NotFound(format!("run or work_dir not found: {}", run_id)))?;
    let path = std::path::Path::new(&work_dir).join("stderr.txt");
    let body = tokio::fs::read_to_string(&path)
        .await
        .map_err(WesError::Io)?;
    Ok((
        [("content-type", "text/plain; charset=utf-8")],
        axum::body::Body::from(body),
    )
        .into_response())
}

#[derive(serde::Serialize, ToSchema)]
pub struct TaskListResponse {
    pub task_logs: Vec<TaskLog>,
    pub next_page_token: Option<String>,
}
