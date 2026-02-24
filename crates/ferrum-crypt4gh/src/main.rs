//! Standalone Crypt4GH binary. Build with: cargo run -p ferrum-crypt4gh --features standalone

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ferrum_crypt4gh=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind: SocketAddr = "0.0.0.0:8086".parse()?;
    let app = ferrum_crypt4gh::router();
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Crypt4GH standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
