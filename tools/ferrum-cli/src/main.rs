//! Ferrum CLI for management and operations.

mod auth_cmd;
mod backup_cmd;
mod edge_update;
mod i18n;
mod ingest_watch;
mod meta_import;
mod meta_init;
mod pipeline_cmd;
mod reference_cmd;
mod sync_cmd;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
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
    /// Demo / Edge mode helpers
    Demo {
        #[command(subcommand)]
        action: DemoAction,
    },
    /// ferrum-meta offline validation (ferrum-core v0.1)
    Meta {
        #[command(subcommand)]
        action: MetaAction,
    },
    /// Field sync queue (ADR-019; Edge → hub upload)
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
    /// Watch MinKNOW output directory and ingest new reads (Edge mode)
    Ingest {
        #[command(subcommand)]
        action: IngestAction,
    },
    /// Offline Edge binary update bundles
    Update {
        #[command(subcommand)]
        action: UpdateAction,
    },
    /// MII-KDS conformance commands
    Mii {
        #[command(subcommand)]
        action: MiiAction,
    },
    /// Outbreak Mode: GISAID packages and policy helpers
    Outbreak {
        #[command(subcommand)]
        action: OutbreakAction,
    },
    /// Edge operator accounts and local login (shared device)
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Field analysis pipeline (QC, Beacon index, WES forward)
    Pipeline {
        #[command(subcommand)]
        action: PipelineAction,
    },
    /// Reference genome registry helpers
    Reference {
        #[command(subcommand)]
        action: ReferenceAction,
    },
    /// Field backup and integrity (SQLite + local objects)
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },
}

#[derive(Subcommand)]
enum DemoAction {
    /// Start Ferrum (Docker demo or Edge mode fallback)
    Start {
        /// Force embedded SQLite + local storage
        #[arg(long, alias = "offline")]
        edge: bool,
        /// Fail hard when PostgreSQL/MinIO are unavailable
        #[arg(long)]
        force_production: bool,
    },
    /// Seed demo DRS + Beacon data against a running gateway (Edge mode friendly)
    Seed {
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
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

#[derive(Subcommand)]
enum MetaAction {
    /// Validate a ferrum-core YAML/JSON submission offline
    Validate {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "text")]
        format: String,
        /// Profile: core, pathogen, h3africa (auto-detected when omitted)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Generate a ferrum-meta submission template (interactive or via flags)
    Init {
        /// Profile: pathogen or h3africa (core also supported)
        #[arg(long)]
        profile: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        study_title: Option<String>,
        #[arg(long)]
        sample_alias: Option<String>,
        #[arg(long)]
        collection_site: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        pathogen_organism: Option<String>,
        #[arg(long)]
        non_interactive: bool,
    },
    /// Import paper form CSV into ferrum-meta YAML
    Import {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        csv: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Write a GHGA or EGA starter bundle (validate with ferrum-meta afterwards)
    Export {
        /// Profile: ghga or ega
        #[arg(long)]
        profile: String,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum SyncAction {
    /// List pending/completed sync queue items
    Status,
    /// Upload pending objects to a hub
    Push {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        bearer: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Mark DRS objects for upstream sync
    Enqueue {
        #[arg(long)]
        object_id: Option<String>,
        #[arg(long)]
        all_local: bool,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Sneakernet export bundle (objects + meta + audit slice)
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        policy: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum IngestAction {
    /// Poll a directory for new ONT files and POST to a running gateway
    Watch {
        /// MinKNOW / Dorado output directory
        dir: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        gateway: String,
        #[arg(long, default_value = "30")]
        poll_secs: u64,
        #[arg(long)]
        dry_run: bool,
        /// Optional ferrum-meta YAML bundle attached to each ingest
        #[arg(long)]
        meta_bundle: Option<PathBuf>,
        /// Field collector name (provenance); set FERRUM_COLLECTOR env or pass --collector
        #[arg(long)]
        collector: Option<String>,
    },
}

#[derive(Subcommand)]
enum UpdateAction {
    /// Install a signed offline update bundle (tar.gz with manifest.json)
    Install {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value = "~/.ferrum/bin")]
        install_dir: PathBuf,
        #[arg(long)]
        sha256: Option<String>,
        #[arg(long)]
        jwks_dir: Option<PathBuf>,
    },
    /// Create an offline update bundle from a built ferrum-gateway binary
    Pack {
        #[arg(long)]
        gateway: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "0.2.0")]
        version: String,
        /// JWKS files as kid:path (repeatable) for offline key rotation
        #[arg(long = "jwks")]
        jwks: Vec<String>,
        #[arg(long)]
        active_jwks_kid: Option<String>,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Create a local Edge operator account (PIN + field role)
    AccountAdd {
        #[arg(long)]
        username: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        pin: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// List Edge operator accounts
    AccountList {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Login and print a local bearer token (HS256)
    Login {
        #[arg(long)]
        username: String,
        #[arg(long)]
        pin: String,
        #[arg(long, default_value = "12")]
        ttl_hours: u64,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PipelineAction {
    /// Run NanoStat (or stub) and POST metrics to /api/v1/ingest/ont-metrics
    Qc {
        #[arg(long)]
        object_id: String,
        #[arg(long)]
        fastq: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        gateway: String,
        #[arg(long)]
        allow_stub: bool,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Index a local VCF object into Beacon (field dataset)
    IndexBeacon {
        #[arg(long)]
        object_id: String,
        #[arg(long)]
        dataset: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Show htsget index metadata for an object
    HtsgetStatus {
        #[arg(long)]
        object_id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Forward a variant-calling WES run to hub (when online)
    ForwardWes {
        #[arg(long)]
        object_id: String,
        #[arg(long, default_value = "tools/workflows/ont-qc.wdl")]
        workflow: String,
        #[arg(long)]
        wes_url: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ReferenceAction {
    /// Ingest stub FASTAs from profiles/references/field-bundle and link registry entries
    InstallFieldBundle {
        #[arg(long, default_value = "profiles/references/field-bundle")]
        bundle_dir: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        gateway: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BackupAction {
    /// Create a gzip tar backup of SQLite metadata and optional local objects
    Create {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = true)]
        include_objects: bool,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Restore a field backup (stop gateway first)
    Restore {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Verify local object SHA-256 checksums against DRS metadata
    Verify {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum OutbreakAction {
    /// Build a GISAID-compatible submission archive for an outbreak policy
    Package {
        #[arg(long)]
        policy: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
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
    let lang = i18n::current_lang();
    let matches = Cli::command().about(i18n::about(lang)).get_matches();
    let cli = Cli::from_arg_matches(&matches).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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
            let cfg = config
                .as_ref()
                .and_then(|p| ferrum_core::FerrumConfig::load_from_path(p).ok())
                .or_else(|| ferrum_core::FerrumConfig::load().ok())
                .ok_or_else(|| {
                    CliExit::RuntimeFailed(
                        "no config found; pass --config or set FERRUM_CONFIG".to_string(),
                    )
                })?;

            let mut db = ferrum_core::DatabasePool::from_config(&cfg.database)
                .await
                .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
            db.run_migrations()
                .await
                .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
            println!("{}", i18n::migrations_ok(i18n::current_lang()));
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
        Commands::Demo { action } => match action {
            DemoAction::Start {
                edge,
                force_production,
            } => {
                demo_start(edge, force_production)
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
            DemoAction::Seed { base_url } => {
                demo_seed(&base_url).await.map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Meta { action } => match action {
            MetaAction::Validate {
                input,
                output,
                format,
                profile,
            } => {
                let parsed_profile = profile
                    .as_deref()
                    .and_then(ferrum_meta_connect::MetaProfile::parse);
                let report = ferrum_meta_connect::validate_submission_file_with_profile(
                    &input,
                    parsed_profile,
                )
                .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                let body = if format == "json" {
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                } else {
                    report.to_string()
                };
                if let Some(path) = output {
                    std::fs::write(&path, &body)
                        .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                } else {
                    print!("{body}");
                }
                if !report.valid {
                    return Err(CliExit::ValidationFailed);
                }
            }
            MetaAction::Init {
                profile,
                output,
                study_title,
                sample_alias,
                collection_site,
                country,
                pathogen_organism,
                non_interactive,
            } => {
                let parsed =
                    ferrum_meta_connect::MetaProfile::parse(&profile).ok_or_else(|| {
                        CliExit::RuntimeFailed(format!(
                            "unknown profile `{profile}` (use core, pathogen, or h3africa)"
                        ))
                    })?;
                let params = ferrum_meta_connect::InitParams {
                    study_title,
                    study_alias: None,
                    sample_alias,
                    individual_alias: None,
                    collection_site,
                    collection_date: None,
                    country,
                    consent_type: None,
                    pathogen_organism,
                    data_use_conditions: vec![],
                };
                meta_init::run_meta_init(parsed, &output, params, !non_interactive)
                    .map_err(CliExit::RuntimeFailed)?;
            }
            MetaAction::Import {
                profile,
                csv,
                output,
            } => {
                let parsed =
                    ferrum_meta_connect::MetaProfile::parse(&profile).ok_or_else(|| {
                        CliExit::RuntimeFailed(format!(
                            "unknown profile `{profile}` (use pathogen or h3africa)"
                        ))
                    })?;
                meta_import::run_meta_import(parsed, &csv, &output)
                    .map_err(CliExit::RuntimeFailed)?;
            }
            MetaAction::Export { profile, output } => {
                let body = match profile.to_ascii_lowercase().as_str() {
                    "ghga" => {
                        include_str!("../../../profiles/meta/fixtures/ghga-minimal-submission.yaml")
                    }
                    "ega" => {
                        include_str!("../../../profiles/meta/fixtures/ega-minimal-submission.yaml")
                    }
                    other => {
                        return Err(CliExit::RuntimeFailed(format!(
                            "unknown export profile `{other}` (use ghga or ega). core/pathogen/h3africa: ferrum meta init"
                        )));
                    }
                };
                if let Some(parent) = output.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                    }
                }
                std::fs::write(&output, body).map_err(|e| CliExit::RuntimeFailed(e.to_string()))?;
                eprintln!(
                    "wrote {output} — replace aliases/checksums, then: ferrum-meta/scripts/validate-fixture.sh {output}",
                    output = output.display()
                );
            }
        },
        Commands::Sync { action } => match action {
            SyncAction::Status => {
                sync_cmd::sync_status(None)
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
            SyncAction::Push {
                target,
                dry_run,
                bearer,
                config,
            } => {
                sync_cmd::sync_push(
                    target.as_deref(),
                    dry_run,
                    bearer.as_deref(),
                    config.as_ref(),
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
            SyncAction::Enqueue {
                object_id,
                all_local,
                target,
                config,
            } => {
                sync_cmd::sync_enqueue(
                    object_id.as_deref(),
                    all_local,
                    target.as_deref(),
                    config.as_ref(),
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
            SyncAction::Export {
                output,
                policy,
                config,
            } => {
                sync_cmd::sync_export(&output, policy.as_deref(), config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Ingest { action } => match action {
            IngestAction::Watch {
                dir,
                gateway,
                poll_secs,
                dry_run,
                meta_bundle,
                collector,
            } => {
                let collector = collector.or_else(|| std::env::var("FERRUM_COLLECTOR").ok());
                ingest_watch::watch_and_ingest(
                    dir,
                    &gateway,
                    poll_secs,
                    dry_run,
                    meta_bundle,
                    collector,
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Update { action } => match action {
            UpdateAction::Install {
                bundle,
                install_dir,
                sha256,
                jwks_dir,
            } => {
                let install_dir = if install_dir.starts_with("~/") {
                    std::env::var("HOME")
                        .map(|h| PathBuf::from(h).join(&install_dir.to_string_lossy()[2..]))
                        .unwrap_or(install_dir)
                } else {
                    install_dir
                };
                edge_update::install_bundle(
                    &bundle,
                    &install_dir,
                    sha256.as_deref(),
                    jwks_dir.as_deref(),
                )
                .map_err(CliExit::RuntimeFailed)?;
            }
            UpdateAction::Pack {
                gateway,
                output,
                version,
                jwks,
                active_jwks_kid,
            } => {
                let jwks_files: Vec<(String, PathBuf)> = jwks
                    .iter()
                    .filter_map(|spec| {
                        let (kid, path) = spec.split_once(':')?;
                        Some((kid.to_string(), PathBuf::from(path)))
                    })
                    .collect();
                edge_update::create_bundle(
                    &gateway,
                    &version,
                    &output,
                    &jwks_files,
                    active_jwks_kid.as_deref(),
                )
                .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Auth { action } => match action {
            AuthAction::AccountAdd {
                username,
                role,
                pin,
                config,
            } => {
                auth_cmd::account_add(&username, &role, &pin, config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
            AuthAction::AccountList { config } => {
                auth_cmd::account_list(config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
            AuthAction::Login {
                username,
                pin,
                ttl_hours,
                config,
            } => {
                auth_cmd::account_login(&username, &pin, ttl_hours, config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Pipeline { action } => match action {
            PipelineAction::Qc {
                object_id,
                fastq,
                gateway,
                allow_stub,
                config,
            } => {
                pipeline_cmd::pipeline_qc(
                    &object_id,
                    &fastq,
                    &gateway,
                    allow_stub,
                    config.as_ref(),
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
            PipelineAction::IndexBeacon {
                object_id,
                dataset,
                config,
            } => {
                pipeline_cmd::pipeline_index_beacon(
                    &object_id,
                    dataset.as_deref(),
                    config.as_ref(),
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
            PipelineAction::HtsgetStatus { object_id, config } => {
                pipeline_cmd::pipeline_htsget_status(&object_id, config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
            PipelineAction::ForwardWes {
                object_id,
                workflow,
                wes_url,
                config,
            } => {
                pipeline_cmd::pipeline_forward_wes(
                    &workflow,
                    &object_id,
                    wes_url.as_deref(),
                    config.as_ref(),
                )
                .await
                .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Reference { action } => match action {
            ReferenceAction::InstallFieldBundle {
                bundle_dir,
                gateway,
                config,
            } => {
                reference_cmd::install_field_bundle(&bundle_dir, &gateway, config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
        },
        Commands::Backup { action } => match action {
            BackupAction::Create {
                output,
                include_objects,
                config,
            } => {
                backup_cmd::backup_create(&output, include_objects, config.as_ref())
                    .map_err(CliExit::RuntimeFailed)?;
            }
            BackupAction::Restore {
                archive,
                force,
                config,
            } => {
                backup_cmd::backup_restore(&archive, force, config.as_ref())
                    .map_err(CliExit::RuntimeFailed)?;
            }
            BackupAction::Verify { config } => {
                backup_cmd::backup_verify(config.as_ref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
        },
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
        Commands::Outbreak { action } => match action {
            OutbreakAction::Package {
                policy,
                output,
                config,
            } => {
                outbreak_package(&policy, &output, config.as_deref())
                    .await
                    .map_err(CliExit::RuntimeFailed)?;
            }
        },
    }
    Ok(())
}

async fn outbreak_package(
    policy_name: &str,
    output: &PathBuf,
    config_path: Option<&std::path::Path>,
) -> Result<(), String> {
    use ferrum_core::{build_gisaid_package, GisaidEntry, OutbreakService};

    let cfg = config_path
        .and_then(|p| ferrum_core::FerrumConfig::load_from_path(p).ok())
        .or_else(|| ferrum_core::FerrumConfig::load().ok())
        .ok_or_else(|| "no config found".to_string())?;

    let policy = cfg
        .outbreak
        .policy_by_name(policy_name)
        .ok_or_else(|| format!("unknown outbreak policy '{policy_name}'"))?;

    let db = ferrum_core::DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    let pool = match db {
        ferrum_core::DatabasePool::Postgres(p) => ferrum_core::FerrumPool::Postgres(p),
        ferrum_core::DatabasePool::Sqlite(p) => ferrum_core::FerrumPool::Sqlite(p),
    };

    let svc = OutbreakService::new(pool, cfg.outbreak.clone());
    let rows = svc
        .pathogen_drs_objects(&policy.trigger_pathogen)
        .await
        .map_err(|e| e.to_string())?;

    let entries: Vec<GisaidEntry> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| GisaidEntry::from_package_row(&row, i, "NNNNATCGATCG"))
        .collect();

    if entries.is_empty() {
        return Err(format!(
            "no pathogen objects found for {}",
            policy.trigger_pathogen
        ));
    }

    let archive = build_gisaid_package(policy_name, &entries).map_err(|e| e.to_string())?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(output, archive).map_err(|e| e.to_string())?;
    println!("Wrote GISAID package to {}", output.display());
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

async fn demo_start(edge: bool, force_production: bool) -> Result<(), String> {
    let lang = i18n::current_lang();
    if edge {
        return start_edge_mode(lang).await;
    }

    let ready = wait_for_production_services(std::time::Duration::from_secs(30)).await;
    if ready {
        return Err(i18n::docker_not_implemented(lang).to_string());
    }
    if force_production {
        return Err(i18n::production_timeout(lang).to_string());
    }
    eprintln!("{}", i18n::production_fallback(lang));
    start_edge_mode(lang).await
}

async fn demo_seed(base_url: &str) -> Result<(), String> {
    let script = std::env::var("FERRUM_SEED_SCRIPT").ok().or_else(|| {
        for candidate in [
            "scripts/seed-edge-demo.sh",
            "scripts/seed-laptop-demo.sh",
            "../scripts/seed-edge-demo.sh",
            "../scripts/seed-laptop-demo.sh",
            "../../scripts/seed-edge-demo.sh",
            "../../scripts/seed-laptop-demo.sh",
        ] {
            if std::path::Path::new(candidate).exists() {
                return Some(candidate.to_string());
            }
        }
        None
    });
    let script = script
        .ok_or("seed script not found — run from Ferrum repo root or set FERRUM_SEED_SCRIPT")?;
    let status = tokio::process::Command::new("bash")
        .arg(&script)
        .env("BASE_URL", base_url)
        .status()
        .await
        .map_err(|e| format!("failed to run seed script: {e}"))?;
    if !status.success() {
        return Err(format!("seed script exited with {status}"));
    }
    Ok(())
}

async fn start_edge_mode(lang: i18n::Lang) -> Result<(), String> {
    std::env::set_var("FERRUM_OFFLINE", "1");
    std::env::set_var("FERRUM_DEMO", "1");
    // NON-PILOT: CLI `demo start --edge` spawns ferrum-gateway without the gateway
    // `demo start --edge` subcommand, so auth-open must be set here or ingest 403s.
    if std::env::var("FERRUM_AUTH__REQUIRE_AUTH").is_err() {
        std::env::set_var("FERRUM_AUTH__REQUIRE_AUTH", "false");
        println!("[ferrum] Demo auth is off (FERRUM_AUTH__REQUIRE_AUTH=false). This is NON-PILOT.");
    }
    println!("{}", i18n::edge_start(lang));
    if let Some(home) = ferrum_embed::default_ferrum_home() {
        println!("{}", i18n::edge_data_dir(lang, &home.display().to_string()));
    }
    println!("{}", i18n::production_config_hint(lang));

    let gateway =
        std::env::var("FERRUM_GATEWAY_BIN").unwrap_or_else(|_| "ferrum-gateway".to_string());
    let status = tokio::process::Command::new(gateway)
        .status()
        .await
        .map_err(|e| format!("failed to spawn gateway: {e}"))?;
    if !status.success() {
        return Err(format!("gateway exited with {status}"));
    }
    Ok(())
}

async fn wait_for_production_services(timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if postgres_reachable() && minio_reachable() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

fn minio_reachable() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9000".parse().unwrap(),
        std::time::Duration::from_secs(1),
    )
    .is_ok()
}

fn postgres_reachable() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:5432".parse().unwrap(),
        std::time::Duration::from_secs(2),
    )
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::{postgres_reachable, should_fail_validation};

    #[test]
    fn test_demo_timeout_fallback() {
        assert!(
            !postgres_reachable() || !super::minio_reachable(),
            "test assumes production stack is not fully up"
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        let ready = rt.block_on(super::wait_for_production_services(
            std::time::Duration::from_millis(100),
        ));
        assert!(!ready);
    }

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
