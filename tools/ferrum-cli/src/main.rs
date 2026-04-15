//! Ferrum CLI for management and operations.

use clap::{Parser, Subcommand};
use ferrum_mii_connect::{
    build_manifest_from_sync_inputs, download_package_bytes, fhir_package_download_url,
    load_manifest, load_sync_spec, read_payload_from_input, validate_payload, ConformanceReport,
    IssueSeverity, MiiModule, MiiValidationConfig,
};
use std::path::PathBuf;
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
    /// MII-KDS conformance commands
    Mii {
        #[command(subcommand)]
        action: MiiAction,
    },
}

#[derive(Subcommand)]
enum MiiAction {
    /// Regenerate `manifest.json` from pinned FHIR NPM packages (packages.fhir.org or cache)
    SyncManifest {
        /// Pin list (package id + version per module)
        #[arg(long, default_value = "profiles/mii/sync-spec.json")]
        spec: PathBuf,
        /// Output manifest path
        #[arg(long, default_value = "profiles/mii/manifest.json")]
        output: PathBuf,
        /// Directory for downloaded `.tgz` mirrors (audit / air-gapped reuse)
        #[arg(long, default_value = "profiles/mii/package-cache")]
        cache_dir: PathBuf,
        /// Only read from `cache_dir` (no network)
        #[arg(long)]
        offline: bool,
    },
    /// Validate FHIR payload against vendored MII profile metadata
    Validate {
        /// Input JSON / NDJSON / FHIR Bundle path
        #[arg(long)]
        input: PathBuf,
        /// Optional explicit config path
        #[arg(long)]
        config: Option<PathBuf>,
        /// Optional explicit manifest path override
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Optional module list, comma-separated
        #[arg(long, value_delimiter = ',')]
        modules: Option<Vec<String>>,
        /// Treat warning-level gaps as failing conditions
        #[arg(long)]
        strict: bool,
        /// Optional report output path (json)
        #[arg(long)]
        output: Option<PathBuf>,
        /// Report format: text, json, sarif
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match run_cli().await {
        Ok(()) => Ok(()),
        Err(CliExit::ValidationFailed) => {
            std::process::exit(1);
        }
        Err(CliExit::RuntimeFailed(msg)) => {
            eprintln!("ferrum mii validate runtime error: {msg}");
            std::process::exit(2);
        }
    }
}

enum CliExit {
    ValidationFailed,
    RuntimeFailed(String),
}

async fn run_cli() -> Result<(), CliExit> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Health { base_url } => {
            let url = format!("{}/health", base_url.trim_end_matches('/'));
            let res = reqwest::get(&url)
                .await
                .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
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
        Commands::Mii { action } => match action {
            MiiAction::SyncManifest {
                spec,
                output,
                cache_dir,
                offline,
            } => {
                // `download_package_bytes` uses reqwest::blocking; run off the async runtime
                // to avoid "Cannot drop a runtime in a context where blocking is not allowed".
                let r = tokio::task::spawn_blocking(move || {
                    sync_manifest_blocking(spec, output, cache_dir, offline)
                })
                .await
                .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                r?;
            }
            MiiAction::Validate {
                input,
                config,
                manifest,
                modules,
                strict,
                output,
                format,
            } => {
                let cfg_loaded = config
                    .as_ref()
                    .and_then(|p| ferrum_core::FerrumConfig::load_from_path(p).ok())
                    .or_else(|| ferrum_core::FerrumConfig::load().ok());

                let mut cfg = MiiValidationConfig::default();
                if let Some(c) = cfg_loaded.as_ref() {
                    cfg.enabled = c.mii_connect.enabled;
                    cfg.profile_set_version = c.mii_connect.profile_set_version.clone();
                    cfg.strict_mode = c.mii_connect.strict_mode;
                    cfg.max_errors = c.mii_connect.max_errors;
                    cfg.offline_only = c.mii_connect.offline_only;
                    cfg.modules = MiiModule::parse_list(&c.mii_connect.modules)
                        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                }
                if let Some(m) = modules {
                    cfg.modules = MiiModule::parse_list(&m)
                        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                }
                if strict {
                    cfg.strict_mode = true;
                }

                let manifest_path = manifest
                    .or_else(|| {
                        cfg_loaded
                            .as_ref()
                            .map(|c| PathBuf::from(c.mii_connect.manifest_path.clone()))
                    })
                    .unwrap_or_else(|| PathBuf::from("profiles/mii/manifest.json"));

                let (manifest_doc, manifest_sha) = load_manifest(&manifest_path)
                    .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                let payload = read_payload_from_input(&input)
                    .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                let report = validate_payload(&payload, &cfg, &manifest_doc, &manifest_sha)
                    .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;

                let format = format.trim().to_ascii_lowercase();
                if let Some(path) = output {
                    let body = if format == "sarif" {
                        serde_json::to_string_pretty(&to_sarif(&report))
                            .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?
                    } else {
                        serde_json::to_string_pretty(&report)
                            .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?
                    };
                    std::fs::write(path, body)
                        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                } else if format == "json" {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report)
                            .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?
                    );
                } else if format == "sarif" {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&to_sarif(&report))
                            .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?
                    );
                } else {
                    println!(
                        "MII report: total={}, passed={}, failed={}, skipped={}, gaps={}",
                        report.summary.total_resources,
                        report.summary.passed,
                        report.summary.failed,
                        report.summary.skipped,
                        report.gap_list.len()
                    );
                }

                let has_errors = report.summary.failed > 0;
                let has_gaps = !report.gap_list.is_empty();
                if should_fail_validation(has_errors, has_gaps, cfg.strict_mode) {
                    return Err(CliExit::ValidationFailed);
                }
            }
        },
    }
    Ok(())
}

fn sync_manifest_blocking(
    spec_path: PathBuf,
    output: PathBuf,
    cache_dir: PathBuf,
    offline: bool,
) -> Result<(), CliExit> {
    let spec = load_sync_spec(&spec_path)
        .map_err(|e| CliExit::RuntimeFailed(format!("sync spec: {e}")))?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
    let mut blobs: Vec<Vec<u8>> = Vec::with_capacity(spec.packages.len());
    for entry in &spec.packages {
        let cache_name = format!(
            "{}__{}.tgz",
            entry.package_name.replace('.', "_"),
            entry.package_version
        );
        let path = cache_dir.join(cache_name);
        let bytes = if offline {
            std::fs::read(&path).map_err(|e| {
                CliExit::RuntimeFailed(format!("offline package read {}: {e}", path.display()))
            })?
        } else if path.exists() {
            std::fs::read(&path).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?
        } else {
            let url = fhir_package_download_url(
                &spec.registry_base,
                &entry.package_name,
                &entry.package_version,
            );
            tracing::info!(target: "ferrum_cli", %url, "fetching FHIR package");
            let b =
                download_package_bytes(&url).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
            std::fs::write(&path, &b).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
            b
        };
        blobs.push(bytes);
    }
    let manifest = build_manifest_from_sync_inputs(&spec, &blobs)
        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
    std::fs::write(&output, json).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
    println!(
        "Wrote manifest with {} packages to {}",
        manifest.packages.len(),
        output.display()
    );
    Ok(())
}

fn to_sarif(report: &ConformanceReport) -> serde_json::Value {
    let results = report
        .resources
        .iter()
        .flat_map(|r| {
            r.issues.iter().map(|i| {
                let level = match i.severity {
                    IssueSeverity::Error => "error",
                    IssueSeverity::Warning => "warning",
                    IssueSeverity::Info => "note",
                };
                serde_json::json!({
                  "ruleId": i.code,
                  "level": level,
                  "message": {"text": i.message},
                  "locations": [{
                    "physicalLocation": {
                      "artifactLocation": {"uri": format!("fhir://{}/{}", r.resource_type, r.resource_id)}
                    }
                  }]
                })
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
      "version": "2.1.0",
      "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
      "runs": [{
        "tool": {
          "driver": {
            "name": "ferrum-mii-connect",
            "informationUri": "https://github.com/SynapticFour/Ferrum",
            "rules": []
          }
        },
        "results": results
      }]
    })
}

fn should_fail_validation(has_errors: bool, has_gaps: bool, strict_mode: bool) -> bool {
    has_errors || (strict_mode && has_gaps)
}

#[cfg(test)]
mod tests {
    use super::should_fail_validation;

    #[test]
    fn fail_when_errors_present() {
        assert!(should_fail_validation(true, false, false));
    }

    #[test]
    fn fail_when_strict_and_gaps_present() {
        assert!(should_fail_validation(false, true, true));
    }

    #[test]
    fn pass_when_no_errors_and_non_strict_gaps() {
        assert!(!should_fail_validation(false, true, false));
    }
}
