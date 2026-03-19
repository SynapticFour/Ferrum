//! Standalone TES binary. Build with: cargo run -p ferrum-tes --features standalone
//! Requires DATABASE_URL (PostgreSQL). Optional: TES_BACKEND (podman|slurm), TES_WORK_DIR, BIND.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferrum_tes=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum".to_string());
    let mut db = ferrum_core::DatabasePool::from_url(&database_url).await?;
    db.run_migrations().await?;
    let pool = match db {
        ferrum_core::DatabasePool::Postgres(p) => p,
        ferrum_core::DatabasePool::Sqlite(_) => {
            return Err(anyhow::anyhow!("TES requires PostgreSQL").into())
        }
    };
    let backend = std::env::var("TES_BACKEND").ok();
    let work_dir = std::env::var("TES_WORK_DIR")
        .ok()
        .map(std::path::PathBuf::from);
    let app = ferrum_tes::router(pool, backend, work_dir);

    let bind: SocketAddr = std::env::var("BIND")
        .unwrap_or_else(|_| "0.0.0.0:8084".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("TES standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
