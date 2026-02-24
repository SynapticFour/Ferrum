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
    if let (Some(ref metrics), false) = (
        &app.metrics,
        app.metrics_sampler_started.load(std::sync::atomic::Ordering::Acquire),
    ) {
        if !app.metrics_sampler_started.swap(true, std::sync::atomic::Ordering::SeqCst) {
            crate::spawn_metrics_sampler(Arc::clone(&app.run_manager), Arc::clone(metrics));
        }
    }
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
    let extensions: Option<std::collections::HashMap<String, serde_json::Value>> = outputs
        .as_object()
        .map(|obj| {
            let mut ext = std::collections::HashMap::new();
            for key in ["ferrum:multiqc_status", "ferrum:multiqc_report_drs_id"] {
                if let Some(v) = obj.get(key) {
                    ext.insert(key.to_string(), v.clone());
                }
            }
            ext
        })
        .filter(|m| !m.is_empty());
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
        extensions,
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
    let has_part: Vec<serde_json::Value> = input_parts.into_iter().chain(output_parts.clone()).collect();
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
            "hasPart": has_part,
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
        let opts = zip::write::FileOptions::<()>::default().unix_permissions(0o644);
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

// ---------- Metrics & cost ----------

#[derive(serde::Serialize, ToSchema)]
pub struct RunMetricsResponse {
    pub run_id: String,
    pub summary: RunMetricsSummary,
    pub tasks: Vec<RunMetricsTask>,
    pub timeseries: RunMetricsTimeseries,
}

#[derive(serde::Serialize, ToSchema)]
pub struct RunMetricsSummary {
    pub wall_time: String,
    pub total_cpu_seconds: f64,
    pub peak_memory_mb: i64,
    pub total_read_gb: f64,
    pub total_write_gb: f64,
    pub estimated_cost: EstimatedCost,
}

#[derive(serde::Serialize, ToSchema)]
pub struct EstimatedCost {
    pub amount: f64,
    pub currency: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct RunMetricsTask {
    pub name: String,
    pub wall_seconds: i64,
    pub cpu_peak_pct: f64,
    pub memory_peak_mb: i64,
    pub exit_code: Option<i32>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct RunMetricsTimeseries {
    pub timestamps: Vec<String>,
    pub cpu_pct: Vec<f64>,
    pub memory_mb: Vec<i64>,
}

fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// GET /runs/{run_id}/metrics
pub async fn get_run_metrics(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<Json<RunMetricsResponse>> {
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let summary = match metrics.get_run_cost_summary(&run_id).await? {
        Some((wall, cpu_s, _mem_gb_h, peak, read_gb, write_gb, cost, _snap)) => RunMetricsSummary {
            wall_time: format_duration(wall),
            total_cpu_seconds: cpu_s,
            peak_memory_mb: peak,
            total_read_gb: read_gb,
            total_write_gb: write_gb,
            estimated_cost: EstimatedCost {
                amount: cost,
                currency: metrics.pricing_snapshot().currency,
            },
        },
        None => {
            let computed = metrics.compute_run_summary(&run_id).await?;
            RunMetricsSummary {
                wall_time: format_duration(computed.total_wall_seconds),
                total_cpu_seconds: computed.total_cpu_seconds,
                peak_memory_mb: computed.peak_memory_mb,
                total_read_gb: computed.total_read_gb,
                total_write_gb: computed.total_write_gb,
                estimated_cost: EstimatedCost {
                    amount: computed.estimated_cost_usd,
                    currency: metrics.pricing_snapshot().currency,
                },
            }
        }
    };
    let task_rows = metrics.get_task_metrics_for_run(&run_id).await?;
    let tasks: Vec<RunMetricsTask> = task_rows
        .into_iter()
        .map(|(_, name, wall_seconds, cpu_peak_pct, memory_peak_mb, exit_code, _)| RunMetricsTask {
            name,
            wall_seconds: wall_seconds.unwrap_or(0) as i64,
            cpu_peak_pct: cpu_peak_pct.unwrap_or(0.0),
            memory_peak_mb: memory_peak_mb.unwrap_or(0),
            exit_code,
        })
        .collect();
    let mut combined: Vec<(String, f64, i64)> = Vec::new();
    let task_rows2 = metrics.get_task_metrics_for_run(&run_id).await?;
    for (_, _, _, _, _, _, samples_opt) in task_rows2 {
        if let Some(serde_json::Value::Array(arr)) = samples_opt {
            for s in arr {
                if let (Some(ts), Some(cpu), Some(mem)) = (
                    s.get("ts").and_then(|v| v.as_str()),
                    s.get("cpu_pct").and_then(|v| v.as_f64()),
                    s.get("memory_mb").and_then(|v| v.as_i64()),
                ) {
                    combined.push((ts.to_string(), cpu, mem));
                }
            }
        }
    }
    combined.sort_by(|a, b| a.0.cmp(&b.0));
    let timestamps: Vec<String> = combined.iter().map(|(t, _, _)| t.clone()).collect();
    let cpu_pct: Vec<f64> = combined.iter().map(|(_, c, _)| *c).collect();
    let memory_mb: Vec<i64> = combined.iter().map(|(_, _, m)| *m).collect();
    Ok(Json(RunMetricsResponse {
        run_id: run_id.clone(),
        summary,
        tasks,
        timeseries: RunMetricsTimeseries {
            timestamps,
            cpu_pct,
            memory_mb,
        },
    }))
}

/// GET /runs/{run_id}/metrics/report — standalone HTML report (Chart.js from CDN).
pub async fn get_run_metrics_report(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Result<axum::response::Response> {
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
    if app.repo.get_run(&run_id).await?.is_none() {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let run_row = app.repo.get_run(&run_id).await?.ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (_, _url, workflow_type, _ver, _, _, _, state_str, _, _, _, _) = run_row;
    let (wall, cpu_s, peak_mb, read_gb, write_gb, cost_usd, tasks_for_bar) = match metrics.get_run_cost_summary(&run_id).await? {
        Some((w, c, _, p, r, wr, co, _)) => {
            let task_rows = metrics.get_task_metrics_for_run(&run_id).await?;
            let bar: Vec<(String, i64)> = task_rows
                .into_iter()
                .map(|(_, name, wall, _, _, _, _)| (name, wall.unwrap_or(0) as i64))
                .collect();
            (w, c, p, r, wr, co, bar)
        }
        None => {
            let computed = metrics.compute_run_summary(&run_id).await?;
            let bar: Vec<(String, i64)> = computed
                .breakdown
                .iter()
                .map(|t| (t.task_name.clone(), t.wall_seconds))
                .collect();
            (
                computed.total_wall_seconds,
                computed.total_cpu_seconds,
                computed.peak_memory_mb,
                computed.total_read_gb,
                computed.total_write_gb,
                computed.estimated_cost_usd,
                bar,
            )
        }
    };
    let task_rows = metrics.get_task_metrics_for_run(&run_id).await?;
    let mut combined: Vec<(String, f64, i64)> = Vec::new();
    for (_, _, _, _, _, _, samples_opt) in &task_rows {
        if let Some(serde_json::Value::Array(arr)) = samples_opt {
            for s in arr {
                if let (Some(ts), Some(cpu), Some(mem)) = (
                    s.get("ts").and_then(|v| v.as_str()),
                    s.get("cpu_pct").and_then(|v| v.as_f64()),
                    s.get("memory_mb").and_then(|v| v.as_i64()),
                ) {
                    combined.push((ts.to_string(), cpu, mem));
                }
            }
        }
    }
    combined.sort_by(|a, b| a.0.cmp(&b.0));
    let timestamps_json = serde_json::to_string(&combined.iter().map(|(t, _, _)| t).cloned().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
    let cpu_json = serde_json::to_string(&combined.iter().map(|(_, c, _)| c).cloned().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
    let mem_json = serde_json::to_string(&combined.iter().map(|(_, _, m)| m).cloned().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
    let bar_labels_json = serde_json::to_string(&tasks_for_bar.iter().map(|(n, _)| n).cloned().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
    let bar_data_json = serde_json::to_string(&tasks_for_bar.iter().map(|(_, s)| s).cloned().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
    let snapshot = metrics.pricing_snapshot();
    let pricing_json = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".into());
    let html = metrics_report_html(
        &run_id,
        &workflow_type,
        &state_str,
        format_duration(wall),
        cpu_s,
        peak_mb,
        read_gb,
        write_gb,
        cost_usd,
        &snapshot.currency,
        &timestamps_json,
        &cpu_json,
        &mem_json,
        &bar_labels_json,
        &bar_data_json,
        &pricing_json,
    );
    Ok((
        [("content-type", "text/html; charset=utf-8")],
        axum::body::Body::from(html.into_string()),
    )
        .into_response())
}

#[allow(clippy::too_many_arguments)]
fn metrics_report_html(
    run_id: &str,
    workflow_type: &str,
    state: &str,
    wall_time: String,
    _total_cpu_seconds: f64,
    peak_memory_mb: i64,
    total_read_gb: f64,
    total_write_gb: f64,
    cost_usd: f64,
    currency: &str,
    timestamps_json: &str,
    cpu_json: &str,
    mem_json: &str,
    bar_labels_json: &str,
    bar_data_json: &str,
    pricing_json: &str,
) -> maud::Markup {
    maud::html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                title { "Run Metrics — " (run_id) }
                script src="https://cdn.jsdelivr.net/npm/chart.js" {}
            }
            body {
                h1 { "Run Metrics Report" }
                p { strong { "Run ID: " } (run_id) " | Workflow: " (workflow_type) " | State: " (state) }
                p { strong { "Total cost: " } (format!("{:.2}", cost_usd)) " " (currency) }
                h2 { "Wall time per task" }
                canvas id="barChart" width="400" height="200" {}
                h2 { "CPU % and Memory (MB) over time" }
                canvas id="lineChart" width="400" height="200" {}
                h2 { "Per-task breakdown" }
                table {
                    thead { tr { th { "Task" } th { "Duration (s)" } th { "Est. cost" } } }
                    tbody id="taskTable" {}
                }
                footer { pre { "Pricing config: " (pricing_json) } }
                script {
                    (maud::PreEscaped(format!(r#"
var ts = {};
var cpu = {};
var mem = {};
var barLabels = {};
var barData = {};
new Chart(document.getElementById('barChart'), {{ type: 'bar', data: {{ labels: barLabels, datasets: [{{ label: 'Wall seconds', data: barData }}] }}, options: {{ indexAxis: 'y' }} }});
new Chart(document.getElementById('lineChart'), {{ type: 'line', data: {{ labels: ts, datasets: [
  {{ label: 'CPU %', data: cpu, yAxisID: 'y' }},
  {{ label: 'Memory MB', data: mem, yAxisID: 'y1' }}
] }}, options: {{ scales: {{ y: {{ type: 'linear' }}, y1: {{ type: 'linear', position: 'right' }} }} }} }});
"#, timestamps_json, cpu_json, mem_json, bar_labels_json, bar_data_json)))
                }
            }
        }
    }
}

#[derive(serde::Deserialize, ToSchema)]
pub struct CostEstimateRequest {
    pub workflow_engine_parameters: Option<serde_json::Value>,
}

/// POST /cost/estimate — estimate cost from workflow_engine_params (same shape as POST /runs body).
pub async fn post_cost_estimate(
    State(app): State<Arc<AppState>>,
    Json(body): Json<CostEstimateRequest>,
) -> Result<Json<crate::metrics::CostEstimate>> {
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
    let params = body.workflow_engine_parameters.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    let estimate = metrics.estimate_cost(&params)?;
    Ok(Json(estimate))
}

#[derive(serde::Deserialize, IntoParams, ToSchema)]
pub struct CostSummaryQuery {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub tags: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct CostSummaryResponse {
    pub period: CostSummaryPeriod,
    pub total_runs: u64,
    pub total_estimated_cost: EstimatedCost,
    pub by_workflow_type: std::collections::HashMap<String, f64>,
    pub by_tag: std::collections::HashMap<String, f64>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct CostSummaryPeriod {
    pub from: String,
    pub to: String,
}

/// GET /cost/summary — aggregate costs for chargeback (from_date, to_date, optional tags filter).
pub async fn get_cost_summary(
    State(app): State<Arc<AppState>>,
    Query(q): Query<CostSummaryQuery>,
) -> Result<Json<CostSummaryResponse>> {
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
    let to_date = q.to_date.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc));
    let from_date = q.from_date.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc));
    let runs = app.repo.list_runs_for_cost(from_date, to_date).await?;
    let total_runs = runs.len() as u64;
    let mut total_cost = 0.0;
    let mut by_workflow_type: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut by_tag: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (run_id, workflow_type, _end_time, tags) in runs {
        if let Some(cost) = metrics.get_run_cost_usd(&run_id).await? {
            total_cost += cost;
            *by_workflow_type.entry(workflow_type).or_insert(0.0) += cost;
            if let Some(obj) = tags.as_object() {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        let key = format!("{}:{}", k, s);
                        *by_tag.entry(key).or_insert(0.0) += cost;
                    }
                }
            }
        }
    }
    let period = CostSummaryPeriod {
        from: q.from_date.clone().unwrap_or_else(|| "".to_string()),
        to: q.to_date.clone().unwrap_or_else(|| "".to_string()),
    };
    Ok(Json(CostSummaryResponse {
        period,
        total_runs,
        total_estimated_cost: EstimatedCost {
            amount: total_cost,
            currency: metrics.pricing_snapshot().currency,
        },
        by_workflow_type,
        by_tag,
    }))
}
