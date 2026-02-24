//! Standalone WES binary. Build with: cargo run -p ferrum-wes --features standalone
//! Requires DATABASE_URL (PostgreSQL). Optional: WES_WORK_DIR, BIND (default 0.0.0.0:8083).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ferrum_wes=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum".to_string());
    let mut db = ferrum_core::DatabasePool::from_url(&database_url).await?;
    db.run_migrations().await?;
    let pool = match db {
        ferrum_core::DatabasePool::Postgres(p) => p,
        ferrum_core::DatabasePool::Sqlite(_) => {
            return Err(anyhow::anyhow!("WES requires PostgreSQL").into());
        }
    };
    let work_dir_base = std::env::var("WES_WORK_DIR").ok().map(std::path::PathBuf::from);
    let tes_url = std::env::var("WES_TES_URL").ok();
    let trs_register_url = std::env::var("WES_TRS_REGISTER_URL").ok();
    let app = ferrum_wes::router(pool, work_dir_base, tes_url, trs_register_url);

    let bind: SocketAddr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8083".to_string()).parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("WES standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
