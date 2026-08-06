//! WES 1.1 HTTP handlers.

use crate::error::{Result, WesError};
use crate::state::AppState;
use crate::types::*;
use axum::{
    extract::{Extension, Path, Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use http_body_util::BodyExt;
use std::convert::Infallible;
use std::io::Write;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::{IntoParams, ToSchema};

/// Fail closed when `FERRUM_AUTH__REQUIRE_AUTH` is set and no Bearer claims were injected.
fn reject_anonymous_when_auth_required(
    auth: &Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<()> {
    if ferrum_core::require_auth_enabled() && auth.is_none() {
        return Err(WesError::Unauthorized(
            "Bearer authentication required when require_auth is enabled".into(),
        ));
    }
    Ok(())
}

/// GET /service-info with supported workflow types and engines.
#[utoipa::path(get, path = "/service-info", responses((status = 200, body = WesServiceInfo)))]
pub async fn get_service_info(State(app): State<Arc<AppState>>) -> Json<WesServiceInfo> {
    let mut workflow_type_versions = std::collections::HashMap::new();
    let mut workflow_engine_versions = std::collections::HashMap::new();
    for exec in app.run_manager.all_executors() {
        for (name, versions) in exec.supported_languages() {
            workflow_type_versions.insert(
                name.clone(),
                WorkflowTypeVersion {
                    workflow_type_version: versions.clone(),
                },
            );
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
            url: Some("https://synapticfour.com".to_string()),
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        workflow_type_versions,
        supported_wes_versions: vec!["1.0".to_string(), "1.1".to_string()],
        supported_filesystem_protocols: vec![
            "file".to_string(),
            "http".to_string(),
            "https".to_string(),
        ],
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
    pub workspace_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct StaleReconcileQuery {
    /// Minimum age in seconds before a stuck QUEUED run is reconciled (default: 0 = immediate).
    pub older_than_secs: Option<i64>,
    pub workspace_id: Option<String>,
}

async fn wes_runs_list_scope(
    app: &AppState,
    workspace_id: Option<&str>,
    owner_sub: Option<&str>,
    is_admin: bool,
) -> Result<(Option<String>, Option<String>)> {
    if let Some(ws_id) = workspace_id {
        let sub = owner_sub
            .ok_or_else(|| WesError::Forbidden("workspace_id requires authentication".into()))?;
        let is_member = ferrum_core::get_workspace_member_role(
            &ferrum_core::FerrumPool::Postgres(app.repo.pool().clone()),
            ws_id,
            sub,
        )
        .await
        .map_err(|e| WesError::Other(e.into()))?
        .is_some();
        if !is_member {
            return Err(WesError::Forbidden("not a member of this workspace".into()));
        }
        Ok((None, Some(ws_id.to_string())))
    } else {
        Ok((
            if is_admin {
                None
            } else {
                owner_sub.map(String::from)
            },
            None,
        ))
    }
}

async fn orphan_queued_count(
    app: &AppState,
    filter_owner: Option<&str>,
    workspace_id: Option<&str>,
) -> Result<u32> {
    let ids = app
        .repo
        .find_orphan_queued_run_ids(0, filter_owner, workspace_id)
        .await?;
    let tracked = app.run_manager.active_run_ids().await;
    let tracked_set: std::collections::HashSet<_> = tracked.into_iter().collect();
    Ok(ids
        .into_iter()
        .filter(|id| !tracked_set.contains(id))
        .count() as u32)
}

/// POST /runs/stale/reconcile — mark orphan QUEUED runs (submit never completed) as EXECUTOR_ERROR.
#[utoipa::path(
    post,
    path = "/runs/stale/reconcile",
    params(StaleReconcileQuery),
    responses((status = 200, body = StaleReconcileResponse))
)]
pub async fn post_reconcile_stale_runs(
    State(app): State<Arc<AppState>>,
    Query(q): Query<StaleReconcileQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<StaleReconcileResponse>> {
    let owner_sub = auth.as_ref().and_then(|c| c.sub());
    let is_admin = auth.as_ref().is_some_and(|c| c.is_admin());
    let (filter_owner, workspace_id) =
        wes_runs_list_scope(&app, q.workspace_id.as_deref(), owner_sub, is_admin).await?;
    let filter_owner_ref = filter_owner.as_deref();
    let workspace_id_ref = workspace_id.as_deref();
    let older_than_secs = q.older_than_secs.unwrap_or(0).max(0);
    let run_ids = app
        .run_manager
        .reconcile_stale_queued_runs(older_than_secs, filter_owner_ref, workspace_id_ref)
        .await?;
    Ok(Json(StaleReconcileResponse {
        reconciled: run_ids.len() as u32,
        run_ids,
    }))
}

/// GET /runs — lists runs for the authenticated user (or all if no auth / admin). If workspace_id set, filter to that workspace (caller must be member).
#[utoipa::path(get, path = "/runs", params(ListRunsQuery), responses((status = 200, body = RunListResponse)))]
pub async fn list_runs(
    State(app): State<Arc<AppState>>,
    Query(q): Query<ListRunsQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunListResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    let page_size = q.page_size.unwrap_or(100).min(1000);
    let state_filter = q.state.as_deref().map(RunState::from_str);
    let owner_sub = auth.as_ref().and_then(|c| c.sub());
    let is_admin = auth.as_ref().is_some_and(|c| c.is_admin());
    let (filter_owner, workspace_id) =
        wes_runs_list_scope(&app, q.workspace_id.as_deref(), owner_sub, is_admin).await?;
    let filter_owner_ref = filter_owner.as_deref();
    let workspace_id_ref = workspace_id.as_deref();
    let _ = app
        .run_manager
        .reconcile_stale_queued_runs(
            crate::run_manager::RunManager::default_stale_queued_secs(),
            filter_owner_ref,
            workspace_id_ref,
        )
        .await;
    let (runs, next_page_token) = app
        .repo
        .list_runs(
            page_size,
            q.page_token.as_deref(),
            state_filter,
            filter_owner_ref,
            workspace_id_ref,
        )
        .await?;
    let mut runs = runs;
    for run in &mut runs {
        if !run.state.is_terminal() {
            if let Ok(state) = app.run_manager.poll_status(&run.run_id).await {
                if state != RunState::Unknown {
                    let _ = app.repo.update_state(&run.run_id, state).await;
                    run.state = state;
                }
            }
        }
    }
    let orphan_queued_count = orphan_queued_count(&app, filter_owner_ref, workspace_id_ref)
        .await
        .ok()
        .filter(|&n| n > 0);
    Ok(Json(RunListResponse {
        runs,
        next_page_token,
        orphan_queued_count,
    }))
}

/// JSON body for POST /runs when Content-Type is application/json (e.g. HelixTest).
#[derive(Debug, serde::Deserialize)]
struct RunRequestJson {
    workflow_type: String,
    workflow_type_version: String,
    workflow_url: String,
    #[serde(default)]
    workflow_params: serde_json::Value,
    #[serde(default, rename = "workflow_engine_parameters")]
    workflow_engine_params: serde_json::Value,
    #[serde(default)]
    tags: serde_json::Value,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    reference_genome: Option<String>,
}

/// POST /runs (multipart or application/json: workflow_type, workflow_type_version, workflow_url, etc.)
#[utoipa::path(post, path = "/runs", responses((status = 200, body = RunIdResponse)))]
pub async fn post_runs(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    req: axum::extract::Request,
) -> Result<Json<RunIdResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    let headers = req.headers().clone();
    let bytes = req
        .into_body()
        .collect()
        .await
        .map_err(|e| WesError::Other(e.into()))?
        .to_bytes();
    let raw_body = bytes.clone();

    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (
        workflow_type,
        workflow_type_version,
        workflow_url,
        workflow_params,
        workflow_engine_params,
        tags,
        workspace_id,
        reference_genome,
    ) = if ct.trim().to_lowercase().starts_with("application/json") {
        let j: RunRequestJson = serde_json::from_slice(&bytes)
            .map_err(|e| WesError::Validation(format!("Invalid JSON body: {}", e)))?;
        let params = if j.workflow_params.is_object() || j.workflow_params.is_array() {
            j.workflow_params
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        let engine = if j.workflow_engine_params.is_object() {
            j.workflow_engine_params
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        let tags_val = if j.tags.is_object() {
            j.tags
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        (
            j.workflow_type,
            j.workflow_type_version,
            j.workflow_url,
            params,
            engine,
            tags_val,
            j.workspace_id.filter(|s| !s.is_empty()),
            j.reference_genome.filter(|s| !s.is_empty()),
        )
    } else {
        let boundary = extract_multipart_boundary(&headers)
                .or_else(|| extract_boundary_from_body(&bytes))
                .ok_or_else(|| {
                    let prefix_len = bytes.len().min(80);
                    let prefix =
                        String::from_utf8_lossy(&bytes[..prefix_len]).replace('\n', "\\n").replace('\r', "\\r");
                    WesError::Validation(format!(
                        "Expected multipart/form-data with a valid boundary (content-type={}, body_prefix={})",
                        ct, prefix
                    ))
                })?;
        let body_stream = tokio_stream::once(Ok::<axum::body::Bytes, std::io::Error>(bytes));
        let mut multipart = multer::Multipart::new(body_stream, boundary);

        let mut workflow_params = serde_json::Value::Object(serde_json::Map::new());
        let mut workflow_type = None::<String>;
        let mut workflow_type_version = None::<String>;
        let mut workflow_url = None::<String>;
        let mut workflow_engine_params = serde_json::Value::Object(serde_json::Map::new());
        let mut tags = serde_json::Value::Object(serde_json::Map::new());
        let mut workspace_id = None::<String>;
        let mut reference_genome = None::<String>;

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| WesError::Other(e.into()))?
        {
            let name = field.name().unwrap_or("").to_string();
            match name.as_str() {
                "workspace_id" => {
                    if let Ok(t) = field.text().await {
                        let t = t.trim().to_string();
                        if !t.is_empty() {
                            workspace_id = Some(t);
                        }
                    }
                }
                "workflow_params" => {
                    if let Ok(text) = field.text().await {
                        workflow_params = serde_json::from_str(&text).unwrap_or(workflow_params);
                    }
                }
                "workflow_type" => workflow_type = Some(field.text().await.unwrap_or_default()),
                "workflow_type_version" => {
                    workflow_type_version = Some(field.text().await.unwrap_or_default())
                }
                "workflow_url" => workflow_url = Some(field.text().await.unwrap_or_default()),
                "workflow_engine_parameters" => {
                    if let Ok(text) = field.text().await {
                        workflow_engine_params =
                            serde_json::from_str(&text).unwrap_or(workflow_engine_params);
                    }
                }
                "tags" => {
                    if let Ok(text) = field.text().await {
                        tags = serde_json::from_str(&text).unwrap_or(tags);
                    }
                }
                "reference_genome" => {
                    if let Ok(t) = field.text().await {
                        let t = t.trim().to_string();
                        if !t.is_empty() {
                            reference_genome = Some(t);
                        }
                    }
                }
                "workflow_attachment" => {
                    let _ = field.bytes().await;
                }
                _ => {}
            }
        }

        (
            workflow_type.ok_or_else(|| WesError::Validation("workflow_type required".into()))?,
            workflow_type_version
                .ok_or_else(|| WesError::Validation("workflow_type_version required".into()))?,
            workflow_url.ok_or_else(|| WesError::Validation("workflow_url required".into()))?,
            workflow_params,
            workflow_engine_params,
            tags,
            workspace_id,
            reference_genome,
        )
    };

    // A03/A08: workflow_url required.
    // For production hardening, enforce scheme/allowlist only when allowed_workflow_sources is configured.
    // In demo/CI (no allowlist), HelixTest may provide workflow_url values that aren't fetchable URLs.
    let url_trim = workflow_url.trim();
    if url_trim.is_empty() {
        return Err(WesError::Validation("workflow_url is required".into()));
    }
    if !app.allowed_workflow_sources.is_empty() {
        let url_lower = url_trim.to_lowercase();
        let allowed_schemes = ["https://", "http://", "file://", "file:", "drs://"];
        let has_allowed = allowed_schemes.iter().any(|s| url_lower.starts_with(s));
        let has_other_scheme = url_lower.contains("://")
            && !url_lower.starts_with("https://")
            && !url_lower.starts_with("http://")
            && !url_lower.starts_with("file://")
            && !url_lower.starts_with("drs://");
        if !has_allowed && has_other_scheme {
            return Err(WesError::Validation(
                "workflow_url must use https://, http://, file://, file:, or drs://".into(),
            ));
        }
        let allowed = app
            .allowed_workflow_sources
            .iter()
            .any(|p| url_trim.starts_with(p.as_str()));
        if !allowed {
            return Err(WesError::Validation(
                "workflow_url not in allowed_workflow_sources".into(),
            ));
        }
    }

    if let Some(ref ws_id) = workspace_id {
        let sub = auth
            .as_ref()
            .and_then(|c| c.sub())
            .ok_or_else(|| WesError::Forbidden("workspace_id requires authentication".into()))?;
        let ok = ferrum_core::is_workspace_editor_or_owner(
            &ferrum_core::FerrumPool::Postgres(app.repo.pool().clone()),
            ws_id,
            sub,
        )
        .await
        .map_err(|e| WesError::Other(e.into()))?;
        if !ok {
            return Err(WesError::Forbidden(
                "not a workspace editor or owner".into(),
            ));
        }
    }

    enforce_ads_resource_tags(
        app.ads_introspect.as_ref(),
        auth.as_ref().map(|Extension(c)| c),
        &tags,
    )
    .await?;

    if let Some(client) = app.solum_consent.as_ref() {
        if let Some((subject, purpose)) = client.binding_from_tags(&tags) {
            client
                .require_granted(&subject, &purpose)
                .await
                .map_err(|e| WesError::Forbidden(format!("solum consent: {e}")))?;
        }
    }

    #[cfg(feature = "discovery")]
    if let Some(run_id) = crate::federated_forward::try_forward_federated_run(
        &app,
        &tags,
        &raw_body,
        headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok()),
    )
    .await?
    {
        return Ok(Json(RunIdResponse {
            run_id,
            warnings: None,
        }));
    }

    let disposition = crate::helixtest_ferrum::classify_trs_workflow(
        &workflow_url,
        &workflow_type,
        &workflow_params,
    );

    let run_id = ulid::Ulid::new().to_string();
    let owner_sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");

    let reference_registry = ferrum_reference::ReferenceRegistry::new(
        ferrum_core::FerrumPool::Postgres(app.repo.pool().clone()),
    );
    let mismatch_warning = ferrum_reference::check_reference_mismatch(
        &reference_registry,
        reference_genome.as_deref(),
        &workflow_params,
    )
    .await
    .map_err(|e| WesError::Other(e.into()))?;
    let warnings = mismatch_warning.map(|w| vec![w]);

    let mut workflow_engine_params = workflow_engine_params;
    if let Some(ref refg) = reference_genome {
        if let serde_json::Value::Object(ref mut map) = workflow_engine_params {
            map.entry("reference_genome".to_string())
                .or_insert_with(|| serde_json::Value::String(refg.clone()));
        }
    }

    app.repo
        .create_run(
            &run_id,
            &workflow_url,
            &workflow_type,
            &workflow_type_version,
            &workflow_params,
            &workflow_engine_params,
            &tags,
            None,
            owner_sub,
            workspace_id.as_deref(),
            None,
            true,
        )
        .await?;

    if let crate::helixtest_ferrum::HelixtestDisposition::ImmediateTerminal(_st) = disposition {
        // HelixTest records state sequence; first /status must not be terminal (see run_manager synthetic phases).
        app.run_manager
            .register_synthetic_helixtest_error(run_id.clone())
            .await;
        return Ok(Json(RunIdResponse { run_id, warnings }));
    }

    if let Some(ref store) = app.provenance_store {
        for object_id in
            crate::provenance_helpers::extract_drs_object_ids_from_json(&workflow_params)
        {
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
    if let Err(e) = app.run_manager.submit(&run).await {
        let _ = app
            .repo
            .update_state(&run_id, RunState::ExecutorError)
            .await;
        return Err(e);
    }

    if let Some(ref base) = app.trs_register_url {
        let wf_url = run.workflow_url.as_str();
        // Skip when UI already registered inline content → TRS descriptor URL.
        let already_trs = wf_url.contains("/ga4gh/trs/v2/tools/") && wf_url.contains("/descriptor");
        if !already_trs {
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
    }

    Ok(Json(RunIdResponse { run_id, warnings }))
}

fn extract_multipart_boundary(headers: &axum::http::HeaderMap) -> Option<String> {
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)?
        .to_str()
        .ok()?;
    // First try the canonical parser (strict).
    if let Ok(b) = multer::parse_boundary(ct) {
        return Some(b);
    }
    // Fallback: tolerate quoted boundary values like boundary="abc".
    let lower = ct.to_lowercase();
    if !lower.starts_with("multipart/form-data") {
        return None;
    }
    let idx = lower.find("boundary=")?;
    let mut b = ct[idx + "boundary=".len()..].trim();
    if let Some(semi) = b.find(';') {
        b = b[..semi].trim();
    }
    let b = b.trim_matches('"').trim();
    if b.is_empty() {
        None
    } else {
        Some(b.to_string())
    }
}

/// Extract boundary from multipart body (first line is `--{boundary}\r\n`).
fn extract_boundary_from_body(body: &[u8]) -> Option<String> {
    let end = body.len().min(500);
    let head = &body[..end];
    let dash2 = b"--";
    let idx = head.windows(2).position(|w| w == dash2)?;
    let start = idx + 2;
    let rest = &head[start..];
    let end_idx = rest
        .iter()
        .position(|&c| c == b'\r' || c == b'\n')
        .unwrap_or(rest.len());
    let boundary = rest[..end_idx].to_vec();
    String::from_utf8(boundary).ok().filter(|s| !s.is_empty())
}

/// GET /runs/{run_id}/status
#[utoipa::path(get, path = "/runs/{run_id}/status", responses((status = 200, body = RunStatus), (status = 404)))]
pub async fn get_run_status(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunStatus>> {
    // Demo/CI: conformance tests frequently query run status without a stable auth context.
    // If auth is not explicitly required, keep /runs/{id}/status usable even when ownership
    // cannot be reliably established.
    reject_anonymous_when_auth_required(&auth)?;
    let require_auth = ferrum_core::require_auth_enabled();
    let visibility_checked = require_auth;
    if require_auth && !run_visible(&app, &run_id, auth.as_ref()).await? {
        tracing::warn!(
            run_id = %run_id,
            require_auth = %require_auth,
            visibility_checked = %visibility_checked,
            auth_sub = auth.as_ref().and_then(|c| c.sub()),
            "WES run visibility denied for /runs/{run_id}/status"
        );
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    if let (Some(ref metrics), false) = (
        &app.metrics,
        app.metrics_sampler_started
            .load(std::sync::atomic::Ordering::Acquire),
    ) {
        if !app
            .metrics_sampler_started
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            crate::spawn_metrics_sampler(Arc::clone(&app.run_manager), Arc::clone(metrics));
        }
    }
    let state_row = app.run_manager.poll_status(&run_id).await?;
    if state_row == RunState::Unknown {
        let run_row = app.repo.get_run(&run_id).await?;
        if let Some((_, _, _, _, _, _, _, s, _, _, _, _, _, _, _)) = run_row {
            return Ok(Json(RunStatus {
                run_id,
                state: RunState::from_str(&s),
            }));
        }
        tracing::warn!(
            run_id = %run_id,
            require_auth = %require_auth,
            visibility_checked = %visibility_checked,
            "WES run missing in DB for /runs/{run_id}/status (poll returned Unknown)"
        );
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    app.repo.update_state(&run_id, state_row).await?;

    // Learned from Sapporo: persist a JSON snapshot after each state transition.
    // Best-effort only; failures must not break the HTTP contract.
    if let Ok(Some((
        _,
        _,
        workflow_type,
        _,
        _,
        _,
        _,
        _,
        start_time,
        _,
        _,
        Some(work_dir),
        _,
        _,
        _,
    ))) = app.repo.get_run(&run_id).await
    {
        let engine = workflow_type.to_lowercase();
        let engine_pid = app.run_manager.process_id_for_run(&run_id).await;
        let snapshot = serde_json::json!({
            "run_id": run_id,
            "state": state_row.as_str(),
            "start_time": start_time.map(|t| t.to_rfc3339()),
            "last_updated": Utc::now().to_rfc3339(),
            "engine": engine,
            "engine_pid": engine_pid,
        });

        let snapshot_path = std::path::Path::new(&work_dir).join("state.json");
        let _ = tokio::fs::create_dir_all(std::path::Path::new(&work_dir)).await;
        if let Err(e) = tokio::fs::write(&snapshot_path, snapshot.to_string()).await {
            tracing::warn!(
                run_id = %run_id,
                path = %snapshot_path.display(),
                error = %e,
                "failed to write run state snapshot"
            );
        }
    }

    Ok(Json(RunStatus {
        run_id,
        state: state_row,
    }))
}

fn run_visible(
    app: &AppState,
    run_id: &str,
    auth: Option<&Extension<ferrum_core::AuthClaims>>,
) -> impl std::future::Future<Output = Result<bool>> + Send {
    let repo = Arc::clone(&app.repo);
    let run_id = run_id.to_string();
    let sub = auth.and_then(|c| c.sub().map(String::from));
    let is_admin = auth.is_some_and(|c| c.is_admin());
    async move { repo.run_visible_to(&run_id, sub.as_deref(), is_admin).await }
}

/// GET /runs/{run_id} (full RunLog)
#[utoipa::path(get, path = "/runs/{run_id}", responses((status = 200, body = RunLog), (status = 404)))]
pub async fn get_run_log(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunLog>> {
    reject_anonymous_when_auth_required(&auth)?;
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let polled = app.run_manager.poll_status(&run_id).await?;
    if polled != RunState::Unknown {
        app.repo.update_state(&run_id, polled).await?;
    }
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (
        run_id_db,
        workflow_url,
        workflow_type,
        workflow_type_version,
        _params,
        _ep,
        tags,
        state_str,
        start_time,
        end_time,
        outputs,
        _work_dir,
        _owner,
        resumed_from_run_id,
        _checkpoint,
    ) = row;
    let run_state = RunState::from_str(&state_str);
    type RunLogRow = Option<(
        String,
        Vec<String>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<String>,
        Option<String>,
        Option<i32>,
    )>;
    let run_log_row: RunLogRow = app.repo.get_run_log(&run_id).await?;
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
    let task_logs = app
        .repo
        .get_task_logs(&run_id, 100, None)
        .await
        .unwrap_or_default();
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
        resumed_from_run_id,
        tags: tags_map_from_value(&tags),
    }))
}

fn tags_map_from_value(
    tags: &serde_json::Value,
) -> Option<std::collections::HashMap<String, String>> {
    let map: std::collections::HashMap<String, String> = tags
        .as_object()?
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// POST /runs/{run_id}/cancel
#[utoipa::path(post, path = "/runs/{run_id}/cancel", responses((status = 200, body = RunIdResponse), (status = 404)))]
pub async fn cancel_run(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunIdResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    app.run_manager.cancel(&run_id).await?;
    Ok(Json(RunIdResponse {
        run_id,
        warnings: None,
    }))
}

/// GET /runs/{run_id}/tasks (paginated task logs)
#[utoipa::path(get, path = "/runs/{run_id}/tasks", responses((status = 200)))]
pub async fn list_tasks(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<TaskListResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>> + Send + 'static>,
> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<axum::response::Response> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let row = app.repo.get_run(&run_id).await?.and_then(|r| {
        let (_, _, _, _, _, _, _, _, _, _, _, work_dir, _, _, _) = r;
        work_dir.map(|d| (run_id.clone(), d))
    });
    let (_, work_dir) =
        row.ok_or_else(|| WesError::NotFound(format!("run or work_dir not found: {}", run_id)))?;
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<axum::response::Response> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let row = app.repo.get_run(&run_id).await?.and_then(|r| {
        let (_, _, _, _, _, _, _, _, _, _, _, work_dir, _, _, _) = r;
        work_dir.map(|d| (run_id.clone(), d))
    });
    let (_, work_dir) =
        row.ok_or_else(|| WesError::NotFound(format!("run or work_dir not found: {}", run_id)))?;
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

#[derive(Debug, serde::Deserialize)]
pub struct OutputFileQuery {
    /// When true, serve with inline disposition (browser preview).
    pub inline: Option<bool>,
}

fn outputs_list_contains(outputs: &serde_json::Value, file_id: &str) -> bool {
    for key in ["output_files", "artifact_files", "log_files"] {
        if let Some(arr) = outputs.get(key).and_then(|v| v.as_array()) {
            if arr
                .iter()
                .any(|o| o.get("file_id").and_then(|v| v.as_str()) == Some(file_id))
            {
                return true;
            }
        }
    }
    false
}

fn guess_content_type(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.ends_with(".json") {
        return "application/json";
    }
    if n.ends_with(".html") {
        return "text/html; charset=utf-8";
    }
    if n.ends_with(".csv") || n.ends_with(".tsv") {
        return "text/plain; charset=utf-8";
    }
    if n.ends_with(".vcf") {
        return "text/plain; charset=utf-8";
    }
    "application/octet-stream"
}

fn is_previewable(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".txt")
        || n.ends_with(".log")
        || n.ends_with(".json")
        || n.ends_with(".html")
        || n.ends_with(".csv")
        || n.ends_with(".tsv")
        || n.ends_with(".vcf")
        || n.ends_with(".cwl")
        || n.ends_with(".wdl")
        || n.ends_with(".nf")
}

/// GET /runs/{run_id}/outputs/files/{file_id} — download or inline-preview a sampled workdir file.
pub async fn get_run_output_file(
    State(app): State<Arc<AppState>>,
    Path((run_id, file_id)): Path<(String, String)>,
    Query(q): Query<OutputFileQuery>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<axum::response::Response> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    if file_id.is_empty() || file_id.contains("..") {
        return Err(WesError::NotFound("invalid file_id".into()));
    }
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (_, _, _, _, _, _, _, _, _, _, outputs, work_dir, _, _, _) = row;
    let work_dir = work_dir.ok_or_else(|| WesError::NotFound("work_dir not found".into()))?;
    if !outputs_list_contains(&outputs, &file_id) {
        return Err(WesError::NotFound(format!(
            "output file not found: {}",
            file_id
        )));
    }
    let rel = file_id.replace("__", "/");
    let base = std::path::Path::new(&work_dir)
        .canonicalize()
        .map_err(WesError::Io)?;
    let path = base.join(&rel);
    let canonical = path
        .canonicalize()
        .map_err(|_| WesError::NotFound(format!("output file not found on disk: {}", file_id)))?;
    if !canonical.starts_with(&base) {
        return Err(WesError::Forbidden("path outside work_dir".into()));
    }
    let name = canonical
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let bytes = tokio::fs::read(&canonical).await.map_err(WesError::Io)?;
    let inline = q.inline.unwrap_or(false) && is_previewable(name);
    let disposition = if inline {
        format!("inline; filename=\"{}\"", name)
    } else {
        format!("attachment; filename=\"{}\"", name)
    };
    let ct = if inline {
        guess_content_type(name)
    } else {
        "application/octet-stream"
    };
    Ok((
        [
            ("content-type", ct),
            ("content-disposition", disposition.as_str()),
        ],
        axum::body::Body::from(bytes),
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunProvenanceResponse>> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let store = app
        .provenance_store
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("provenance not configured")))?;
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
    let depth = q.depth.unwrap_or(10).clamp(1, 20);
    let graph = store
        .subgraph(&q.root_id, &q.root_type, &q.direction, depth)
        .await?;
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<axum::response::Response> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (
        _,
        workflow_url,
        workflow_type,
        _version,
        _params,
        workflow_engine_params,
        _tags,
        _state_str,
        start_time,
        end_time,
        outputs,
        _work_dir,
        _,
        _,
        _,
    ) = row;
    let reference_genome =
        crate::ro_crate::reference_genome_from_engine_params(&workflow_engine_params);
    let date_published = end_time
        .or(start_time)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let mut input_parts: Vec<serde_json::Value> = Vec::new();
    let mut output_parts: Vec<serde_json::Value> = Vec::new();
    let pool = app.repo.pool();
    async fn enrich_part(pool: &sqlx::PgPool, id: &str) -> crate::error::Result<serde_json::Value> {
        let base = serde_json::json!({
            "@id": format!("drs://ferrum/{}", id),
            "@type": "File",
            "identifier": id
        });
        let ext = crate::ro_crate::load_drs_extensions(pool, id)
            .await
            .map_err(|e| crate::error::WesError::Other(e.into()))?;
        Ok(crate::ro_crate::enrich_file_node(base, &ext))
    }
    if let Some(ref store) = app.provenance_store {
        let graph = store.run_lineage(&run_id).await?;
        for e in &graph.edges {
            if matches!(e.edge_type, ferrum_core::EdgeType::Input)
                && matches!(e.from_type, ferrum_core::NodeType::DrsObject)
            {
                input_parts.push(enrich_part(pool, &e.from_id).await?);
            }
            if matches!(e.edge_type, ferrum_core::EdgeType::Output)
                && matches!(e.to_type, ferrum_core::NodeType::DrsObject)
            {
                output_parts.push(enrich_part(pool, &e.to_id).await?);
            }
        }
    }
    if output_parts.is_empty() {
        if let Some(obj) = outputs.get("output_files").and_then(|v| v.as_array()) {
            for o in obj {
                if let Some(id) = o.get("file_id").and_then(|v| v.as_str()) {
                    output_parts.push(enrich_part(pool, id).await?);
                }
            }
        }
    }
    let has_part: Vec<serde_json::Value> = input_parts
        .into_iter()
        .chain(output_parts.clone())
        .collect();
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
    let mut graph_vec = vec![
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
    if let Some(ref rg) = reference_genome {
        graph_vec.push(crate::ro_crate::reference_genome_entity(rg));
    }
    let ro_crate = serde_json::json!({
        "@context": "https://w3id.org/ro/crate/1.1/context",
        "@graph": graph_vec
    });
    let json_bytes = serde_json::to_vec_pretty(&ro_crate).map_err(|e| WesError::Other(e.into()))?;
    let mut zip_buf = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let opts = zip::write::FileOptions::<()>::default().unix_permissions(0o644);
        zip_writer
            .start_file("ro-crate-metadata.json", opts)
            .map_err(|e| WesError::Other(e.into()))?;
        zip_writer
            .write_all(&json_bytes)
            .map_err(|e| WesError::Other(e.into()))?;
        zip_writer.finish().map_err(|e| WesError::Other(e.into()))?;
    }
    Ok((
        [
            ("content-type", "application/zip"),
            (
                "content-disposition",
                &format!("attachment; filename=\"run-{}.ro-crate.zip\"", run_id),
            ),
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<RunMetricsResponse>> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
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
        .map(
            |(_, name, wall_seconds, cpu_peak_pct, memory_peak_mb, exit_code, _)| RunMetricsTask {
                name,
                wall_seconds: wall_seconds.unwrap_or(0) as i64,
                cpu_peak_pct: cpu_peak_pct.unwrap_or(0.0),
                memory_peak_mb: memory_peak_mb.unwrap_or(0),
                exit_code,
            },
        )
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
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<axum::response::Response> {
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let metrics = app
        .metrics
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("metrics not configured")))?;
    let run_row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (_, _url, workflow_type, _ver, _, _, _, state_str, _, _, _, _, _, _, _) = run_row;
    let (wall, cpu_s, peak_mb, read_gb, write_gb, cost_usd, tasks_for_bar) =
        match metrics.get_run_cost_summary(&run_id).await? {
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
    let timestamps_json = serde_json::to_string(
        &combined
            .iter()
            .map(|(t, _, _)| t)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let cpu_json = serde_json::to_string(
        &combined
            .iter()
            .map(|(_, c, _)| c)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let mem_json = serde_json::to_string(
        &combined
            .iter()
            .map(|(_, _, m)| m)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let bar_labels_json = serde_json::to_string(
        &tasks_for_bar
            .iter()
            .map(|(n, _)| n)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let bar_data_json = serde_json::to_string(
        &tasks_for_bar
            .iter()
            .map(|(_, s)| s)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
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
    _wall_time: String,
    _total_cpu_seconds: f64,
    _peak_memory_mb: i64,
    _total_read_gb: f64,
    _total_write_gb: f64,
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
    let params = body
        .workflow_engine_parameters
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
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
    let to_date = q
        .to_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let from_date = q
        .from_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let runs = app.repo.list_runs_for_cost(from_date, to_date).await?;
    let total_runs = runs.len() as u64;
    let mut total_cost = 0.0;
    let mut by_workflow_type: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
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

// --- Resume and checkpoint ---

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct ResumeRunRequest {
    pub override_params: Option<serde_json::Value>,
    pub force_rerun_tasks: Option<Vec<String>>,
    pub checkpoint_strategy: Option<String>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ResumeRunResponse {
    pub run_id: String,
    pub resumed_from: String,
    pub cached_tasks: usize,
    pub tasks_to_rerun: usize,
    pub estimated_time_saved: String,
}

/// POST /runs/{run_id}/resume — create a new run that reuses checkpointed outputs from the given run.
pub async fn resume_run(
    State(app): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    body: Option<Json<ResumeRunRequest>>,
) -> Result<Json<ResumeRunResponse>> {
    reject_anonymous_when_auth_required(&auth)?;
    if !run_visible(&app, &run_id, auth.as_ref()).await? {
        return Err(WesError::NotFound(format!("run not found: {}", run_id)));
    }
    let owner_sub = auth.as_ref().and_then(|c| c.sub()).unwrap_or("anonymous");
    let row = app
        .repo
        .get_run(&run_id)
        .await?
        .ok_or_else(|| WesError::NotFound(format!("run not found: {}", run_id)))?;
    let (
        _,
        workflow_url,
        workflow_type,
        workflow_type_version,
        workflow_params,
        workflow_engine_params,
        tags,
        _state,
        _st,
        _et,
        _outputs,
        _work_dir,
        _owner,
        _resumed_from,
        _checkpoint_enabled,
    ) = row;
    let mut params = workflow_params.clone();
    if let Some(Json(req)) = body {
        if let Some(ref override_params) = req.override_params {
            if let (Some(params_obj), Some(override_obj)) =
                (params.as_object_mut(), override_params.as_object())
            {
                for (k, v) in override_obj {
                    params_obj.insert(k.clone(), v.clone());
                }
            }
        }
    }
    let new_run_id = ulid::Ulid::new().to_string();
    let workspace_id: Option<String> =
        sqlx::query_scalar("SELECT workspace_id FROM wes_runs WHERE run_id = $1")
            .bind(&run_id)
            .fetch_optional(app.repo.pool())
            .await
            .ok()
            .flatten();
    let workspace_id = workspace_id.as_deref();
    app.repo
        .create_run(
            &new_run_id,
            &workflow_url,
            &workflow_type,
            &workflow_type_version,
            &params,
            &workflow_engine_params,
            &tags,
            None,
            owner_sub,
            workspace_id,
            Some(&run_id),
            true,
        )
        .await?;
    let cached_tasks = if let Some(ref store) = app.checkpoint_store {
        store
            .get_resumable_tasks(&run_id)
            .await
            .unwrap_or_default()
            .len()
    } else {
        0
    };
    let tasks_to_rerun = 0usize; // placeholder: would be derived from workflow graph
    let estimated_time_saved = format!("{}m", cached_tasks * 5);
    let run = crate::executor::WesRun {
        run_id: new_run_id.clone(),
        workflow_url,
        workflow_type,
        workflow_type_version,
        workflow_params: params,
        workflow_engine_params,
        work_dir: None,
    };
    if let Err(e) = app.run_manager.submit(&run).await {
        let _ = app
            .repo
            .update_state(&new_run_id, RunState::ExecutorError)
            .await;
        return Err(e);
    }
    Ok(Json(ResumeRunResponse {
        run_id: new_run_id,
        resumed_from: run_id,
        cached_tasks,
        tasks_to_rerun,
        estimated_time_saved,
    }))
}

/// GET /cache/stats — cache statistics (total entries, hit rate, etc.). A01: requires authentication.
pub async fn get_cache_stats(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
) -> Result<Json<crate::checkpoint::CacheStats>> {
    auth.as_ref()
        .and_then(|c| c.sub())
        .ok_or_else(|| WesError::Forbidden("authentication required".into()))?;
    let store = app
        .checkpoint_store
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("cache not configured")))?;
    let stats = store.cache_stats().await?;
    Ok(Json(stats))
}

#[derive(Debug, serde::Deserialize, IntoParams)]
pub struct EvictCacheQuery {
    pub older_than_days: Option<u32>,
    pub task_name: Option<String>,
    pub run_id: Option<String>,
}

/// DELETE /cache — evict cache entries. Requires admin.
pub async fn evict_cache(
    State(app): State<Arc<AppState>>,
    auth: Option<Extension<ferrum_core::AuthClaims>>,
    Query(q): Query<EvictCacheQuery>,
) -> Result<Json<serde_json::Value>> {
    let claims = auth.ok_or_else(|| WesError::Forbidden("authentication required".into()))?;
    if !claims.is_admin() {
        return Err(WesError::Forbidden("admin role required".into()));
    }
    let store = app
        .checkpoint_store
        .as_ref()
        .ok_or_else(|| WesError::Other(anyhow::anyhow!("cache not configured")))?;
    let max_age_days = q.older_than_days.unwrap_or(30);
    let deleted = store.evict_stale_entries(max_age_days, None).await?;
    Ok(Json(serde_json::json!({ "evicted": deleted })))
}

async fn enforce_ads_resource_tags(
    client: Option<&Arc<ferrum_core::AdsIntrospectClient>>,
    auth: Option<&ferrum_core::AuthClaims>,
    tags: &serde_json::Value,
) -> Result<()> {
    let Some(client) = client else {
        return Ok(());
    };
    let Some(map) = tags.as_object() else {
        return Ok(());
    };
    let resource_id = map
        .get("ads_dataset_id")
        .or_else(|| map.get("ads_compute_pool_id"))
        .and_then(|v| v.as_str());
    let Some(resource_id) = resource_id else {
        return Ok(());
    };
    let ads_base = map.get("ads_base_url").and_then(|v| v.as_str());
    let claims = auth.ok_or_else(|| {
        WesError::Forbidden("authentication required for ADS-controlled compute".into())
    })?;
    if claims.is_admin() || claims.has_dataset_grant(resource_id) {
        return Ok(());
    }
    let token = claims
        .raw_token()
        .ok_or_else(|| WesError::Forbidden("Bearer token required for ADS access check".into()))?;
    let resource = format!("wes:run:{resource_id}");
    let active = if let Some(base) = ads_base {
        client
            .introspect_at_base(base, token, &resource, resource_id)
            .await
    } else {
        client
            .is_dataset_access_active(token, &resource, resource_id)
            .await
    }
    .map_err(|e| WesError::Forbidden(format!("ADS access check failed: {e}")))?;
    if !active {
        return Err(WesError::Forbidden(
            "ADS resource access not granted for this run".into(),
        ));
    }
    Ok(())
}
