//! Ferrum API Gateway binary: single entrypoint for all GA4GH services.

use clap::{Parser, Subcommand};
use ferrum_gateway::run;
use std::path::PathBuf;
use std::process::Command;
use std::net::SocketAddr;
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
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("demo")
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
                    Command::new("sh")
                        .arg(demo.join("start.sh"))
                        .status()?
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
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ferrum_gateway=info,tower_http=info".into()))
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

    // DRS: when we have a pool, build state so list/get (and ingest when storage is configured) work.
    let drs_state: Option<ferrum_drs::AppState> = if let Some(ref pool) = pg_pool {
        let hostname = std::env::var("FERRUM_DRS_HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        let repo = Arc::new(ferrum_drs::repo::DrsRepo::new(pool.clone(), hostname));
        let storage: Option<Arc<dyn ferrum_core::ObjectStorage>> = if let Some(ref cfg) = config {
            if cfg.storage.backend == "s3" {
                match ferrum_core::S3Storage::from_config(&cfg.storage).await {
                    Ok(s) => Some(Arc::new(s) as Arc<dyn ferrum_core::ObjectStorage>),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        Some(ferrum_drs::AppState {
            repo,
            storage,
            s3_presigner: None,
            provenance_store: None,
        })
    } else {
        None
    };

    // WES: when we have a pool, enable list/submit with a work dir (demo: /tmp/wes-runs or FERRUM_WES_WORK_DIR).
    let wes_params = pg_pool.clone().map(|pool| {
        let work_dir = std::env::var("FERRUM_WES_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("wes-runs"));
        (pool, Some(work_dir), None, None, None, None, None, None, vec![])
    });

    // TES: enable when we have a pool (backend defaults to "podman"; work_dir defaults under /tmp).
    let tes_params = pg_pool.clone().map(|pool| (pool, None, None));

    run(
        bind,
        config,
        drs_state,
        wes_params,
        tes_params,
        pg_pool.clone(),   // trs_params
        pg_pool.clone(),   // beacon_params
        pg_pool.clone(),   // passport_params
        pg_pool.clone(),   // cohort_params
        pg_pool.clone(),   // workspaces_pool
        pg_pool,           // admin_pool
    )
    .await?;
    Ok(())
}
