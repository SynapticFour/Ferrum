//! Ferrum API Gateway binary: single entrypoint for all GA4GH services.

use ferrum_gateway::run;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ferrum_gateway=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ferrum_core::FerrumConfig::load().ok();
    let bind: SocketAddr = config
        .as_ref()
        .and_then(|c| c.bind.parse().ok())
        .unwrap_or_else(|| "0.0.0.0:8080".parse().unwrap());

    run(bind, config, None, None, None, None, None, None, None).await?;
    Ok(())
}
