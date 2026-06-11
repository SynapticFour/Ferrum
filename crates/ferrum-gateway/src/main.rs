//! Ferrum API Gateway binary: single entrypoint for all GA4GH services.

use clap::{Parser, Subcommand};
use ferrum_embed::{
    ensure_data_dirs, log_platform_startup, probe_auth_endpoints, Database, EmbedMode,
    MemoryCapGuard, MemoryCapState, SqliteStorage,
};
#[cfg(feature = "full")]
use ferrum_embed::PostgresStorage;
use ferrum_gateway::run;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "ferrum", about = "Ferrum GA4GH Bioinformatics Platform")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage the demo stack
    Demo {
        #[command(subcommand)]
        action: DemoAction,
    },
    /// Start the gateway server (default)
    Start,
}

#[derive(Subcommand)]
enum DemoAction {
    /// Start the full demo stack (PostgreSQL + Gateway + UI), or Laptop Mode when unavailable
    Start {
        /// Force embedded SQLite + local storage (no Docker)
        #[arg(long)]
        offline: bool,
    },
    /// Stop the demo stack
    Stop,
    /// Show demo stack status
    Status,
}

fn demo_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let relative = exe.parent().unwrap_or(&exe).join("..").join("demo");
    if relative.exists() {
        return relative;
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("demo")
}

fn postgres_available() -> bool {
    std::process::Command::new("pg_isready")
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg("5432")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn minio_available() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9000".parse().unwrap(),
        Duration::from_secs(2),
    )
    .is_ok()
}

async fn start_laptop_mode() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("[ferrum] PostgreSQL not detected. Starting in Laptop Mode (SQLite + local storage).");
    if let Some(home) = ferrum_embed::default_ferrum_home() {
        println!("[ferrum] Data will be stored at {}/", home.display());
    }
    println!(
        "[ferrum] To use production backends, set FERRUM_CONFIG=/path/to/config.toml"
    );
    std::env::set_var("FERRUM_OFFLINE", "1");
    run_gateway_server().await
}

/// Merge `FERRUM_STORAGE__*` env into storage config so Docker/CI never lose nested fields
fn merged_storage_config(base: Option<&ferrum_core::StorageConfig>) -> ferrum_core::StorageConfig {
    let mut s = base.cloned().unwrap_or_default();
    if let Ok(v) = std::env::var("FERRUM_STORAGE__BACKEND") {
        let v = v.trim();
        if !v.is_empty() {
            s.backend = v.to_string();
        }
    }
    if s.s3_endpoint
        .as_ref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true)
    {
        if let Ok(v) = std::env::var("FERRUM_STORAGE__S3_ENDPOINT") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                s.s3_endpoint = Some(v);
            }
        }
    }
    if s.s3_bucket
        .as_ref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true)
    {
        if let Ok(v) = std::env::var("FERRUM_STORAGE__S3_BUCKET") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                s.s3_bucket = Some(v);
            }
        }
    }
    if s.s3_region
        .as_ref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true)
    {
        if let Ok(v) = std::env::var("FERRUM_STORAGE__S3_REGION") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                s.s3_region = Some(v);
            }
        }
    }
    if s.s3_access_key_id
        .as_ref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true)
    {
        if let Ok(v) = std::env::var("FERRUM_STORAGE__S3_ACCESS_KEY_ID") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                s.s3_access_key_id = Some(v);
            }
        }
    }
    if s.s3_secret_access_key
        .as_ref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true)
    {
        if let Ok(v) = std::env::var("FERRUM_STORAGE__S3_SECRET_ACCESS_KEY") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                s.s3_secret_access_key = Some(v);
            }
        }
    }
    s
}

fn storage_backend_is_s3_like(backend: &str) -> bool {
    matches!(
        backend.trim().to_ascii_lowercase().as_str(),
        "s3" | "minio" | "s3-compatible"
    )
}

async fn build_object_storage(
    storage_cfg: &ferrum_core::StorageConfig,
) -> Option<std::sync::Arc<dyn ferrum_storage::ObjectStorage>> {
    if storage_backend_is_s3_like(&storage_cfg.backend) {
        if storage_cfg
            .s3_endpoint
            .as_ref()
            .map(|e| e.trim().is_empty())
            .unwrap_or(true)
        {
            tracing::warn!(
                "storage.backend is S3-compatible but s3_endpoint is empty — AWS SDK will use default AWS S3; set FERRUM_STORAGE__S3_ENDPOINT for MinIO (DRS /stream will 404 if the object only exists on MinIO)"
            );
        }
        match ferrum_storage::S3Storage::from_config(storage_cfg).await {
            Ok(s) => {
                tracing::info!(
                    endpoint = ?storage_cfg.s3_endpoint,
                    bucket = ?storage_cfg.s3_bucket,
                    backend = %storage_cfg.backend,
                    "S3-compatible object storage initialized for DRS"
                );
                Some(std::sync::Arc::new(s) as std::sync::Arc<dyn ferrum_storage::ObjectStorage>)
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    endpoint = ?storage_cfg.s3_endpoint,
                    bucket = ?storage_cfg.s3_bucket,
                    "S3Storage::from_config failed; DRS stream/ingest to object storage disabled"
                );
                None
            }
        }
    } else {
        let base = storage_cfg.base_path.as_deref().unwrap_or("./ferrum-blobs");
        match ferrum_storage::LocalStorage::new(base) {
            Ok(s) => {
                tracing::info!(base_path = %base, "Local object storage initialized for DRS");
                Some(std::sync::Arc::new(s) as std::sync::Arc<dyn ferrum_storage::ObjectStorage>)
            }
            Err(e) => {
                tracing::warn!(error = %e, "LocalStorage init failed; DRS upload ingest disabled");
                None
            }
        }
    }
}

async fn run_gateway_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const LAPTOP_BUILD: bool = !cfg!(feature = "full");

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferrum_gateway=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut config = ferrum_core::FerrumConfig::load().ok();
    if let Some(ref mut cfg) = config {
        cfg.apply_embedded_defaults();
        let _ = ensure_data_dirs(cfg);
        if LAPTOP_BUILD || cfg.is_offline_first() {
            if cfg.africa.as_ref().and_then(|a| a.max_memory_mb).is_none() {
                if let Some(cap_mb) = ferrum_embed::suggested_memory_cap_mb() {
                    let africa = cfg.africa.get_or_insert_with(Default::default);
                    africa.max_memory_mb = Some(cap_mb);
                    tracing::info!(
                        cap_mb,
                        "auto-set memory cap to 80% of detected RAM (override with [africa] max_memory_mb)"
                    );
                }
            }
        }
    }

    log_platform_startup(LAPTOP_BUILD);

    let offline_first = config.as_ref().is_some_and(|c| c.is_offline_first());
    let embed_mode = config
        .as_ref()
        .map(EmbedMode::resolve)
        .unwrap_or(EmbedMode::Sqlite);

    if embed_mode == EmbedMode::Sqlite {
        tracing::info!("starting in embedded laptop mode (SQLite + local storage)");
    }

    if let Some(ref cfg) = config {
        probe_auth_endpoints(cfg, offline_first).await;
    }

    let _memory_guard = config
        .as_ref()
        .and_then(|c| c.africa.as_ref())
        .and_then(|a| a.max_memory_mb)
        .map(|mb| MemoryCapGuard::spawn_monitor(MemoryCapState::new(mb)));

    let bind: SocketAddr = config
        .as_ref()
        .and_then(|c| c.bind.parse().ok())
        .unwrap_or_else(|| "0.0.0.0:8080".parse().unwrap());

    let ferrum_pool: Option<ferrum_core::FerrumPool> = if let Some(ref cfg) = config {
        let result = match embed_mode {
            #[cfg(feature = "full")]
            EmbedMode::Full => {
                let storage = PostgresStorage::connect(cfg).await?;
                if cfg.database.run_migrations {
                    storage.migrate().await?;
                }
                Some(storage.pool().clone())
            }
            EmbedMode::Sqlite | EmbedMode::Auto => {
                let storage = SqliteStorage::connect(cfg).await?;
                if cfg.database.run_migrations {
                    storage.migrate().await?;
                }
                Some(storage.pool().clone())
            }
            #[cfg(not(feature = "full"))]
            EmbedMode::Full => {
                tracing::warn!(
                    "postgres/full embed mode requested but this binary was built with --features laptop; using SQLite"
                );
                let storage = SqliteStorage::connect(cfg).await?;
                if cfg.database.run_migrations {
                    storage.migrate().await?;
                }
                Some(storage.pool().clone())
            }
        };
        result
    } else {
        None
    };

    #[cfg(feature = "full")]
    let pg_pool: Option<sqlx::PgPool> = if embed_mode == EmbedMode::Full {
        if let Some(ref cfg) = config {
            ferrum_core::postgres_pool_from_config(&cfg.database).await.ok()
        } else if let Ok(url) = std::env::var("FERRUM_DATABASE__URL") {
            sqlx::PgPool::connect(&url).await.ok()
        } else {
            None
        }
    } else {
        None
    };

    if let Some(ref pool) = ferrum_pool {
        let drs_count: Result<i64, _> = ferrum_core::ferrum_db!(pool, |p| {
            sqlx::query_scalar("SELECT COUNT(*) FROM drs_objects")
                .fetch_one(p)
                .await
        });
        if let Ok(n) = drs_count {
            tracing::info!(drs_objects = n, dialect = ?pool.dialect(), "Gateway database ready");
        }
    }

    let drs_hostname =
        std::env::var("FERRUM_DRS_HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let public_base_url = std::env::var("FERRUM_PUBLIC_BASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("https://{}", drs_hostname))
        .trim_end_matches('/')
        .to_string();

    let drs_state: Option<ferrum_drs::AppState> = if let Some(ref pool) = ferrum_pool {
        let repo = Arc::new(ferrum_drs::repo::DrsRepo::new(
            pool.clone(),
            drs_hostname.clone(),
        ));
        let merged_storage = merged_storage_config(config.as_ref().map(|c| &c.storage));
        let object_storage_backend = merged_storage.backend.clone();
        let ingest = config
            .as_ref()
            .map(|c| c.ingest.clone())
            .unwrap_or_default();
        let storage: Option<Arc<dyn ferrum_storage::ObjectStorage>> =
            build_object_storage(&merged_storage).await;
        let crypt4gh_key_dir = std::env::var("FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                config
                    .as_ref()
                    .and_then(|c| c.encryption.crypt4gh_key_dir.as_ref())
                    .map(std::path::PathBuf::from)
            });
        let crypt4gh_master_key_id = config
            .as_ref()
            .map(|c| c.encryption.crypt4gh_master_key_id.clone())
            .unwrap_or_else(|| "node".to_string());
        let crypt4gh_decrypt_stream = config
            .as_ref()
            .map(|c| c.encryption.crypt4gh_decrypt_stream)
            .unwrap_or(true);

        let bandwidth_cfg = config
            .as_ref()
            .map(|c| c.bandwidth.clone())
            .unwrap_or_default();
        let bandwidth = Arc::new(ferrum_storage::BandwidthMonitor::new(bandwidth_cfg));
        let transfer_queue = Arc::new(ferrum_storage::TransferQueue::new(300));
        let residency_audit = Arc::new(ferrum_core::ResidencyAuditLog::new(pool.clone()));
        let background_gate = Arc::new(ferrum_core::BackgroundWorkGate::default());

        Some(ferrum_drs::AppState {
            repo,
            storage,
            s3_presigner: None,
            provenance_store: None,
            crypt4gh_key_dir,
            crypt4gh_master_key_id,
            crypt4gh_decrypt_stream,
            ingest,
            object_storage_backend,
            outbreak: config.as_ref().filter(|c| c.outbreak.enabled).map(|c| {
                Arc::new(ferrum_core::OutbreakService::new(
                    pool.clone(),
                    c.outbreak.clone(),
                ))
            }),
            bandwidth: Some(bandwidth),
            transfer_queue: Some(transfer_queue),
            residency_audit: Some(residency_audit),
            background_gate: Some(background_gate),
        })
    } else {
        None
    };

    let htsget_state = drs_state.as_ref().map(|s| {
        Arc::new(ferrum_htsget::HtsgetState {
            repo: s.repo.clone(),
            public_base_url: public_base_url.clone(),
        })
    });

    #[cfg(feature = "full")]
    let wes_params = pg_pool.clone().map(|pool| {
        let work_dir = std::env::var("FERRUM_WES_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("wes-runs"));
        let tes_url = std::env::var("FERRUM_WES_TES_URL")
            .unwrap_or_else(|_| "http://localhost:8080/ga4gh/tes/v1".to_string());
        (
            pool,
            Some(work_dir),
            Some(tes_url),
            None,
            None,
            None,
            None,
            None,
            vec![],
        )
    });

    #[cfg(feature = "full")]
    let tes_params = pg_pool
        .clone()
        .map(|pool| (pool, Some("noop".to_string()), None));

    #[cfg(feature = "full")]
    {
        run(
            bind,
            config,
            drs_state,
            htsget_state,
            wes_params,
            tes_params,
            pg_pool.clone(),
            ferrum_pool.clone(),
            pg_pool.clone(),
            pg_pool.clone(),
            pg_pool.clone(),
            pg_pool,
        )
        .await?;
    }

    #[cfg(not(feature = "full"))]
    {
        run(
            bind,
            config,
            drs_state,
            htsget_state,
            None,
            None,
            None,
            ferrum_pool.clone(),
            None,
            None,
            None,
        )
        .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Demo { action }) => {
            let demo = demo_dir();
            let status = match action {
                DemoAction::Start { offline } => {
                    if offline || (!postgres_available() && !minio_available()) {
                        if !offline && !postgres_available() {
                            return start_laptop_mode().await;
                        }
                        if offline {
                            return start_laptop_mode().await;
                        }
                    }
                    println!("\n  🧬 Ferrum Demo\n");
                    if !postgres_available() {
                        return start_laptop_mode().await;
                    }
                    Command::new("sh").arg(demo.join("start.sh")).status()?
                }
                DemoAction::Stop => Command::new("sh").arg(demo.join("stop.sh")).status()?,
                DemoAction::Status => Command::new("docker")
                    .arg("compose")
                    .arg("-f")
                    .arg(demo.join("docker-compose.demo.yml"))
                    .arg("ps")
                    .status()?,
            };
            std::process::exit(status.code().unwrap_or(1));
        }
        Some(Commands::Start) | None => {
            run_gateway_server().await?;
        }
    }
    Ok(())
}
