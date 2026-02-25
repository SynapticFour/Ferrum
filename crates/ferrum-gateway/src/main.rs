//! Ferrum API Gateway binary: single entrypoint for all GA4GH services.

use clap::{Parser, Subcommand};
use ferrum_gateway::run;
use std::path::PathBuf;
use std::process::Command;
use std::net::SocketAddr;
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

    run(bind, config, None, None, None, None, None, None, None, None, None).await?;
    Ok(())
}
