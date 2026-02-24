//! Standalone Beacon binary. Build with: cargo run -p ferrum-beacon --features standalone
//! Requires DATABASE_URL (PostgreSQL). Optional: BIND.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ferrum_beacon=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum".to_string());
    let mut db = ferrum_core::DatabasePool::from_url(&database_url).await?;
    db.run_migrations().await?;
    let pool = match db {
        ferrum_core::DatabasePool::Postgres(p) => p,
        ferrum_core::DatabasePool::Sqlite(_) => return Err(anyhow::anyhow!("Beacon requires PostgreSQL").into()),
    };

    let bind: SocketAddr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8087".to_string()).parse()?;
    let app = ferrum_beacon::router(pool);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Beacon standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
