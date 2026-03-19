//! Standalone TRS binary. Build with: cargo run -p ferrum-trs --features standalone

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::net::SocketAddr;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferrum_trs=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind: SocketAddr = "0.0.0.0:8082".parse()?;
    let app = ferrum_trs::router();
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("TRS standalone listening on {}", bind);
    axum::serve(listener, app).await?;
    Ok(())
}
