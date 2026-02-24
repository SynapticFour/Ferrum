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
use std::io::Write;
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

    if let Some(ref store) = app.provenance_store {
        for object_id in crate::provenance_helpers::extract_drs_object_ids_from_json(&workflow_params) {
            let _ = store.record_wes_input(&run_id, &object_id).await;
        }
    }

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

/// GET /runs/{run_id}/provenance — lineage subgraph for this run (inputs + outputs).
#[utoipa::path(
    get,
    path = "/runs/{run_id}/provenance",
    responses((status = 200, description = "Provenance graph"), (status = 404), (status = 503, description = "Provenance not configured"))
)]
pub async fn get_run_provenance(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<RunProvenanceResponse>> {
    let store = app
        .provenance_store
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("provenance not configured")))?;
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let graph = store.run_lineage(&run_id).await?;
    Ok(Json(RunProvenanceResponse {
        run_id: run_id.clone(),
        graph: RunProvenanceGraphResponse {
            nodes: graph.nodes.clone(),
            edges: graph.edges.clone(),
            mermaid: graph.to_mermaid(),
            cytoscape: graph.to_cytoscape_json(),
        },
    }))
}

#[derive(serde::Serialize, ToSchema)]
pub struct RunProvenanceResponse {
    pub run_id: String,
    pub graph: RunProvenanceGraphResponse,
}

#[derive(serde::Serialize, ToSchema)]
pub struct RunProvenanceGraphResponse {
    pub nodes: Vec<ferrum_core::ProvenanceNode>,
    pub edges: Vec<ferrum_core::ProvenanceEdge>,
    pub mermaid: String,
    pub cytoscape: serde_json::Value,
}

/// Query params for GET /provenance/graph
#[derive(Debug, serde::Deserialize, utoipa::IntoParams, ToSchema)]
pub struct ProvenanceGraphQuery {
    pub root_id: String,
    #[serde(default = "default_root_type")]
    pub root_type: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default = "default_depth")]
    pub depth: Option<u32>,
}

fn default_root_type() -> String {
    "drs_object".to_string()
}
fn default_direction() -> String {
    "both".to_string()
}
fn default_depth() -> Option<u32> {
    Some(10)
}

/// GET /provenance/graph — subgraph by root_id and root_type (drs_object | wes_run).
#[utoipa::path(
    get,
    path = "/provenance/graph",
    params(ProvenanceGraphQuery),
    responses((status = 200, description = "Provenance graph"), (status = 503))
)]
pub async fn get_provenance_graph(
    State(app): State<Arc<AppState>>,
    Query(q): Query<ProvenanceGraphQuery>,
) -> Result<Json<RunProvenanceGraphResponse>> {
    let store = app
        .provenance_store
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("provenance not configured")))?;
    let depth = q.depth.unwrap_or(10).min(20).max(1);
    let graph = store.subgraph(&q.root_id, &q.root_type, &q.direction, depth).await?;
    Ok(Json(RunProvenanceGraphResponse {
        nodes: graph.nodes.clone(),
        edges: graph.edges.clone(),
        mermaid: graph.to_mermaid(),
        cytoscape: graph.to_cytoscape_json(),
    }))
}

/// GET /runs/{run_id}/export/ro-crate — export run as RO-Crate (ZIP with ro-crate-metadata.json).
#[utoipa::path(
    get,
    path = "/runs/{run_id}/export/ro-crate",
    responses((status = 200, description = "ZIP file"), (status = 404))
)]
pub async fn export_ro_crate(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<axum::response::Response> {
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (_, workflow_url, workflow_type, _version, _params, _ep, _tags, state_str, start_time, end_time, outputs, _work_dir) = row;
    let date_published = end_time.or(start_time).map(|t| t.to_rfc3339()).unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let mut input_parts: Vec<serde_json::Value> = Vec::new();
    let mut output_parts: Vec<serde_json::Value> = Vec::new();
    if let Some(ref store) = app.provenance_store {
        let graph = store.run_lineage(&run_id).await?;
        for e in &graph.edges {
            if matches!(e.edge_type, ferrum_core::EdgeType::Input) && matches!(e.from_type, ferrum_core::NodeType::DrsObject) {
                input_parts.push(serde_json::json!({
                    "@id": format!("drs://ferrum/{}", e.from_id),
                    "@type": "File",
                    "identifier": e.from_id
                }));
            }
            if matches!(e.edge_type, ferrum_core::EdgeType::Output) && matches!(e.to_type, ferrum_core::NodeType::DrsObject) {
                output_parts.push(serde_json::json!({
                    "@id": format!("drs://ferrum/{}", e.to_id),
                    "@type": "File",
                    "identifier": e.to_id
                }));
            }
        }
    }
    if output_parts.is_empty() {
        if let Some(obj) = outputs.get("output_files").and_then(|v| v.as_array()) {
            for o in obj {
                if let Some(id) = o.get("file_id").and_then(|v| v.as_str()) {
                    output_parts.push(serde_json::json!({
                        "@id": format!("drs://ferrum/{}", id),
                        "@type": "File",
                        "identifier": id
                    }));
                }
            }
        }
    }
    let workflow_app = serde_json::json!({
        "@type": "SoftwareApplication",
        "@id": "#workflow",
        "name": workflow_type,
        "url": workflow_url
    });
    let create_action = serde_json::json!({
        "@type": "CreateAction",
        "@id": format!("#run-{}", run_id),
        "name": format!("WES Run {}", run_id),
        "result": output_parts,
        "instrument": { "@id": "#workflow" }
    });
    let graph_vec = vec![
        serde_json::json!({
            "@type": "CreativeWork",
            "@id": "ro-crate-metadata.json",
            "conformsTo": { "@id": "https://w3id.org/ro/crate/1.1" }
        }),
        serde_json::json!({
            "@type": "Dataset",
            "@id": "./",
            "name": format!("WES Run {}", run_id),
            "datePublished": date_published,
            "hasPart": [input_parts, output_parts].into_iter().flatten().collect::<Vec<_>>(),
            "mainEntity": { "@id": format!("#run-{}", run_id) }
        }),
        workflow_app,
        create_action,
    ];
    let ro_crate = serde_json::json!({
        "@context": "https://w3id.org/ro/crate/1.1/context",
        "@graph": graph_vec
    });
    let json_bytes = serde_json::to_vec_pretty(&ro_crate).map_err(|e| WesError::Other(e.into()))?;
    let mut zip_buf = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let opts = zip::write::FileOptions::default().unix_permissions(0o644);
        zip_writer.start_file("ro-crate-metadata.json", opts).map_err(|e| WesError::Other(e.into()))?;
        zip_writer.write_all(&json_bytes).map_err(|e| WesError::Other(e.into()))?;
        zip_writer.finish().map_err(|e| WesError::Other(e.into()))?;
    }
    Ok((
        [
            ("content-type", "application/zip"),
            ("content-disposition", &format!("attachment; filename=\"run-{}.ro-crate.zip\"", run_id)),
        ],
        axum::body::Body::from(zip_buf),
    )
        .into_response())
}
