//! Ferrum API Gateway binary: single entrypoint for all GA4GH services.

use clap::{Parser, Subcommand};
use ferrum_gateway::run;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
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
    /// Start the full demo stack (PostgreSQL + Gateway + UI)
    Start,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Demo { action }) => {
            let demo = demo_dir();
            let status = match action {
                DemoAction::Start => {
                    println!("\n  🧬 Ferrum Demo\n");
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
        Some(Commands::Start) | None => {}
    }

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferrum_gateway=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ferrum_core::FerrumConfig::load().ok();
    let bind: SocketAddr = config
        .as_ref()
        .and_then(|c| c.bind.parse().ok())
        .unwrap_or_else(|| "0.0.0.0:8080".parse().unwrap());

    // When database is configured (from config or FERRUM_DATABASE__URL), create a pool for Cohorts, Workspaces, Beacon, Passports, and Admin.
    let pg_pool: Option<sqlx::PgPool> = if let Some(ref cfg) = config {
        match ferrum_core::DatabasePool::from_config(&cfg.database).await {
            Ok(ferrum_core::DatabasePool::Postgres(p)) => Some(p),
            _ => None,
        }
    } else {
        None
    };
    let pg_pool: Option<sqlx::PgPool> = if pg_pool.is_some() {
        pg_pool
    } else if let Ok(url) = std::env::var("FERRUM_DATABASE__URL") {
        match ferrum_core::DatabasePool::from_url(&url).await {
            Ok(ferrum_core::DatabasePool::Postgres(p)) => Some(p),
            _ => None,
        }
    } else {
        None
    };

    // Startup diagnostics for demo/CI: confirm gateway sees seeded data.
    if let Some(ref pool) = pg_pool {
        let run_migrations_env = std::env::var("FERRUM_DATABASE__RUN_MIGRATIONS")
            .unwrap_or_else(|_| "<unset>".to_string());
        let db_url =
            std::env::var("FERRUM_DATABASE__URL").unwrap_or_else(|_| "<unset>".to_string());
        tracing::info!(run_migrations_env = %run_migrations_env, db_url = %db_url, "Gateway database config (env)");
        let drs_count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM drs_objects")
            .fetch_one(pool)
            .await;
        match drs_count {
            Ok(n) => tracing::info!(drs_objects = n, "Gateway sees drs_objects rows"),
            Err(e) => {
                tracing::warn!(error = %e, "Gateway could not query drs_objects (schema missing?)")
            }
        }
        let has_test_object: Result<bool, _> = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM drs_objects WHERE id = 'test-object-1')",
        )
        .fetch_one(pool)
        .await;
        if let Ok(exists) = has_test_object {
            tracing::info!(test_object_1_exists = exists, "Gateway DRS seed presence");
        }
        let ids: Result<Vec<String>, _> =
            sqlx::query_scalar("SELECT id FROM drs_objects ORDER BY id LIMIT 5")
                .fetch_all(pool)
                .await;
        if let Ok(ids) = ids {
            tracing::info!(sample_drs_ids = ?ids, "Gateway sample DRS IDs");
        }
    }

    // Public URL for htsget tickets (DRS stream links). Override for local HTTP, e.g. http://127.0.0.1:8080
    let drs_hostname =
        std::env::var("FERRUM_DRS_HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let public_base_url = std::env::var("FERRUM_PUBLIC_BASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("https://{}", drs_hostname))
        .trim_end_matches('/')
        .to_string();

    // DRS: when we have a pool, build state so list/get (and ingest when storage is configured) work.
    let drs_state: Option<ferrum_drs::AppState> = if let Some(ref pool) = pg_pool {
        let repo = Arc::new(ferrum_drs::repo::DrsRepo::new(
            pool.clone(),
            drs_hostname.clone(),
        ));
        let object_storage_backend = config
            .as_ref()
            .map(|c| c.storage.backend.clone())
            .unwrap_or_else(|| "local".to_string());

        let ingest = config
            .as_ref()
            .map(|c| c.ingest.clone())
            .unwrap_or_default();

        let storage: Option<Arc<dyn ferrum_storage::ObjectStorage>> = if let Some(ref cfg) = config
        {
            if cfg.storage.backend == "s3" {
                match ferrum_storage::S3Storage::from_config(&cfg.storage).await {
                    Ok(s) => Some(Arc::new(s) as Arc<dyn ferrum_storage::ObjectStorage>),
                    Err(_) => None,
                }
            } else {
                let base = cfg.storage.base_path.as_deref().unwrap_or("./ferrum-blobs");
                match ferrum_storage::LocalStorage::new(base) {
                    Ok(s) => Some(Arc::new(s) as Arc<dyn ferrum_storage::ObjectStorage>),
                    Err(e) => {
                        tracing::warn!(error = %e, "LocalStorage init failed; DRS upload ingest disabled");
                        None
                    }
                }
            }
        } else {
            None
        };
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

    // WES: when we have a pool, enable list/submit with a work dir (demo: /tmp/wes-runs or FERRUM_WES_WORK_DIR).
    // For demo/CI we always route WES runs via the local TES endpoint so that execution can use the configured TES backend
    // (e.g. the noop executor on GitHub Actions) instead of requiring local workflow engines like nextflow or cwltool.
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

    // TES: enable when we have a pool. In demo/CI use a no-op backend (podman isn't available on GitHub runners).
    let tes_params = pg_pool
        .clone()
        .map(|pool| (pool, Some("noop".to_string()), None));

    run(
        bind,
        config,
        drs_state,
        htsget_state,
        wes_params,
        tes_params,
        pg_pool.clone(), // trs_params
        pg_pool.clone(), // beacon_params
        pg_pool.clone(), // passport_params
        pg_pool.clone(), // cohort_params
        pg_pool.clone(), // workspaces_pool
        pg_pool,         // admin_pool
    )
    .await?;
    Ok(())
}
