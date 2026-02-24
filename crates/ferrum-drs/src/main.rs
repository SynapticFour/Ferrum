//! Standalone DRS service binary. Build with: cargo run -p ferrum-drs --features standalone

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ferrum_drs=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind: SocketAddr = "0.0.0.0:8081".parse()?;
    let app = ferrum_drs::router();
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("DRS standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
