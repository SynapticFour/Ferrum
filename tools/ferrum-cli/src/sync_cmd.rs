//! Field sync queue CLI (ADR-019 / Phase 4).

use ferrum_core::{
    build_gisaid_package, build_sneakernet_bundle, enqueue_all_local, enqueue_object,
    list_queue_items, normalize_target_url, push_pending_items, resolve_objects_root, DatabasePool,
    FerrumConfig, FerrumPool, GisaidEntry, OutbreakService, PushOptions, STATE_COMPLETED,
    STATE_FAILED, STATE_PENDING,
};
use std::path::{Path, PathBuf};

async fn edge_pool(config: Option<&PathBuf>) -> Result<(FerrumConfig, FerrumPool), String> {
    let mut cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found (pass --config or set FERRUM_CONFIG)".to_string())?;
    // Edge gateway already applied embed migrations; avoid re-running core migrations on the same DB.
    cfg.database.run_migrations = false;
    let db = DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    let pool = match db {
        DatabasePool::Postgres(p) => FerrumPool::Postgres(p),
        DatabasePool::Sqlite(p) => FerrumPool::Sqlite(p),
    };
    Ok((cfg, pool))
}

fn resolve_target(cfg: &FerrumConfig, target: Option<&str>) -> Result<String, String> {
    target
        .map(str::to_string)
        .or_else(|| cfg.sync.default_target_url.clone())
        .map(|t| normalize_target_url(&t))
        .ok_or_else(|| {
            "target URL required (--target or [sync] default_target_url in config)".into()
        })
}

pub async fn sync_status(config: Option<&PathBuf>) -> Result<(), String> {
    let (_cfg, pool) = edge_pool(config).await?;
    let items = list_queue_items(&pool, None)
        .await
        .map_err(|e| e.to_string())?;
    if items.is_empty() {
        println!("Sync queue is empty.");
        return Ok(());
    }
    let pending = items.iter().filter(|i| i.state == STATE_PENDING).count();
    let failed = items.iter().filter(|i| i.state == STATE_FAILED).count();
    let done = items.iter().filter(|i| i.state == STATE_COMPLETED).count();
    for item in &items {
        let err = item
            .error_message
            .as_deref()
            .map(|m| format!(" — {m}"))
            .unwrap_or_default();
        println!(
            "{} {} → {} [{}] {}/{} bytes{}",
            item.id,
            item.object_id,
            item.target_url,
            item.state,
            item.bytes_sent,
            item.bytes_total,
            err
        );
    }
    println!("Summary: {pending} pending, {failed} failed, {done} completed");
    Ok(())
}

pub async fn sync_enqueue(
    object_id: Option<&str>,
    all_local: bool,
    target: Option<&str>,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, pool) = edge_pool(config).await?;
    let target_url = resolve_target(&cfg, target)?;
    let policy = cfg.sync.clone();

    if all_local {
        let items = enqueue_all_local(&pool, &target_url, &policy)
            .await
            .map_err(|e| e.to_string())?;
        println!("Enqueued {} object(s) for {target_url}", items.len());
        return Ok(());
    }
    let oid = object_id.ok_or_else(|| "pass --object-id or --all-local".to_string())?;
    let item = enqueue_object(&pool, oid, &target_url, &policy)
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "Enqueued {} ({}) for {}",
        item.object_id, item.id, item.target_url
    );
    Ok(())
}

pub async fn sync_push(
    target: Option<&str>,
    dry_run: bool,
    bearer: Option<&str>,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, pool) = edge_pool(config).await?;
    let target_url = resolve_target(&cfg, target)?;

    if cfg.sync.register_on_push && cfg.discovery.enabled {
        maybe_register_services(&cfg).await;
    }

    let objects_root = resolve_objects_root(&cfg);
    let opts = PushOptions {
        dry_run,
        bearer_token: bearer.map(str::to_string),
        requester: std::env::var("FERRUM_COLLECTOR").ok(),
    };
    let results = push_pending_items(&pool, &objects_root, &target_url, &opts)
        .await
        .map_err(|e| e.to_string())?;

    if results.is_empty() {
        println!("No pending sync items for {target_url}");
        return Ok(());
    }
    let mut ok = 0;
    for r in &results {
        let mark = if r.success { "OK" } else { "FAIL" };
        println!("{mark} {} — {}", r.object_id, r.message);
        if r.success {
            ok += 1;
        }
    }
    println!("Push complete: {ok}/{} succeeded", results.len());
    if ok < results.len() {
        return Err("one or more sync push items failed".into());
    }
    Ok(())
}

pub async fn sync_export(
    output: &Path,
    policy: Option<&str>,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let (cfg, pool) = edge_pool(config).await?;
    let objects_root = resolve_objects_root(&cfg);

    let gisaid = if let Some(policy_name) = policy.or(cfg.sync.outbreak_policy_on_export.as_deref())
    {
        Some(build_gisaid_for_policy(&cfg, &pool, policy_name).await?)
    } else {
        None
    };

    let manifest = build_sneakernet_bundle(&pool, &objects_root, output, gisaid)
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "Wrote sneakernet bundle to {} ({} objects, {} metadata refs)",
        output.display(),
        manifest.object_ids.len(),
        manifest.metadata_refs.len()
    );
    Ok(())
}

async fn build_gisaid_for_policy(
    cfg: &FerrumConfig,
    pool: &FerrumPool,
    policy_name: &str,
) -> Result<Vec<u8>, String> {
    let policy = cfg
        .outbreak
        .policy_by_name(policy_name)
        .ok_or_else(|| format!("unknown outbreak policy '{policy_name}'"))?;
    let svc = OutbreakService::new(pool.clone(), cfg.outbreak.clone());
    let rows = svc
        .pathogen_drs_objects(&policy.trigger_pathogen)
        .await
        .map_err(|e| e.to_string())?;
    let entries: Vec<GisaidEntry> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| GisaidEntry::from_package_row(&row, i, "NNNNATCGATCG"))
        .collect();
    if entries.is_empty() {
        return Err(format!(
            "no pathogen objects for GISAID export (policy {policy_name})"
        ));
    }
    build_gisaid_package(policy_name, &entries).map_err(|e| e.to_string())
}

async fn maybe_register_services(cfg: &FerrumConfig) {
    use ferrum_discovery::{register_ferrum_services, ServiceRegistryClient};

    let Ok(client) = ServiceRegistryClient::from_config(&cfg.discovery) else {
        tracing::warn!("sync push: discovery enabled but registry client unavailable");
        return;
    };
    let base = cfg
        .discovery
        .registration_base_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", cfg.bind));
    let env = cfg
        .discovery
        .preferred_environment
        .clone()
        .unwrap_or_else(|| "africa".into());
    if let Err(e) = register_ferrum_services(&client, &base, &cfg.services, &env).await {
        tracing::warn!("sync push: service registration failed: {e}");
    } else {
        println!("Registered Ferrum services with ga4gh-infra registry ({base})");
    }
}
