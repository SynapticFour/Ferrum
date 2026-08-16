// SPDX-License-Identifier: BUSL-1.1
//! Install minimal reference FASTA stubs for offline Edge field nodes.

use ferrum_core::{DatabasePool, FerrumConfig, FerrumPool};
use ferrum_reference::{LoadReferenceRequest, ReferenceRegistry};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct FieldBundleManifest {
    references: Vec<FieldBundleEntry>,
}

#[derive(serde::Deserialize)]
struct FieldBundleEntry {
    id: String,
    file: String,
}

pub async fn install_field_bundle(
    bundle_dir: &Path,
    gateway: &str,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let manifest_path = bundle_dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest: FieldBundleManifest =
        serde_json::from_str(&raw).map_err(|e| format!("manifest json: {e}"))?;

    let mut cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found".to_string())?;
    cfg.database.run_migrations = false;
    let db = DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    let pool = match db {
        DatabasePool::Postgres(p) => FerrumPool::Postgres(p),
        DatabasePool::Sqlite(p) => FerrumPool::Sqlite(p),
    };
    let registry = ReferenceRegistry::new(pool.clone());

    let client = reqwest::Client::new();
    let ingest_base = format!("{}/api/v1/ingest/upload", gateway.trim_end_matches('/'));

    for entry in manifest.references {
        let fasta_path = bundle_dir.join(&entry.file);
        if !fasta_path.is_file() {
            return Err(format!("missing FASTA {}", fasta_path.display()));
        }
        let bytes = std::fs::read(&fasta_path).map_err(|e| e.to_string())?;
        let part = reqwest::multipart::Part::bytes(bytes).file_name(entry.file.clone());
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("name", entry.file.clone())
            .text("client_request_id", format!("field-ref-{}", entry.id));
        let resp = client
            .post(&ingest_base)
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!(
                "ingest {} failed: {}",
                entry.id,
                resp.text().await.unwrap_or_default()
            ));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let drs_id = json
            .pointer("/result/object_ids/0")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| format!("unexpected ingest response for {}", entry.id))?;

        registry
            .load_fasta(
                &entry.id,
                &LoadReferenceRequest {
                    fasta_drs_id: drs_id.to_string(),
                    index_drs_id: None,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        println!("Linked reference {} → DRS {drs_id}", entry.id);
    }
    println!(
        "Field reference bundle installed from {}",
        bundle_dir.display()
    );
    Ok(())
}
