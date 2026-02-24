//! Ferrum CLI for management and operations.

use clap::{Parser, Subcommand};
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "ferrum")]
#[command(about = "GA4GH Ferrum management CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show service health
    Health {
        #[arg(short, long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
    },
    /// Run database migrations (when applicable)
    Migrate {
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
    /// Print resolved configuration
    Config {
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Health { base_url } => {
            let url = format!("{}/health", base_url.trim_end_matches('/'));
            let res = reqwest::get(&url).await?;
            let status = res.status();
            let body: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
            println!("{} {}", status, body);
        }
        Commands::Migrate { config } => {
            let path = config.or_else(|| Some(std::path::PathBuf::from("config.toml")));
            println!("Migrate (config: {:?}) - not yet implemented", path);
        }
        Commands::Config { path } => {
            let cfg = path
                .as_ref()
                .and_then(|p| ferrum_core::FerrumConfig::load_from_path(p).ok())
                .or_else(|| ferrum_core::FerrumConfig::load().ok());
            match cfg {
                Some(c) => println!("{:#?}", c),
                None => println!("No config found"),
            }
        }
    }
    Ok(())
}
