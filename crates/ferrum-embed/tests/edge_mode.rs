//! Integration tests for embedded / edge mode.

use ferrum_core::{FerrumConfig, FerrumPool, IngestConfig};
use ferrum_drs::repo::DrsRepo;
use ferrum_embed::{
    Database, EmbedMode, MemoryCapGuard, MemoryCapLevel, MemoryCapState, SqliteStorage,
};
use ferrum_storage::{LocalStorage, ObjectStorage};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn edge_config(dir: &TempDir) -> FerrumConfig {
    let db_path = dir.path().join("ferrum.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let objects_path = dir.path().join("objects");
    std::fs::create_dir_all(&objects_path).unwrap();
    let mut cfg = FerrumConfig::load().expect("default config");
    cfg.africa = Some(ferrum_core::AfricaProfile {
        offline_first: true,
        max_memory_mb: None,
        sqlite_path: Some(db_path.clone()),
        objects_path: Some(objects_path),
        ..Default::default()
    });
    cfg.apply_embedded_defaults();
    cfg.database.sqlite_path = db_path.to_string_lossy().into_owned();
    cfg.database.driver = "sqlite".to_string();
    cfg.storage.backend = "local".to_string();
    cfg.storage.base_path = Some(dir.path().join("objects").to_string_lossy().into_owned());
    cfg
}

fn drs_state_for_edge(pool: FerrumPool, objects_dir: &std::path::Path) -> ferrum_drs::AppState {
    let local = LocalStorage::new(objects_dir).expect("local storage");
    ferrum_drs::AppState {
        repo: Arc::new(DrsRepo::new(pool.clone(), "localhost".to_string())),
        storage: Some(Arc::new(local) as Arc<dyn ObjectStorage>),
        s3_presigner: None,
        provenance_store: None,
        crypt4gh_key_dir: None,
        crypt4gh_master_key_id: "node".to_string(),
        crypt4gh_decrypt_stream: false,
        ingest: IngestConfig::default(),
        object_storage_backend: "local".to_string(),
        outbreak: None,
        bandwidth: Some(Arc::new(ferrum_storage::BandwidthMonitor::new(
            Default::default(),
        ))),
        transfer_queue: Some(Arc::new(ferrum_storage::TransferQueue::new(300))),
        residency_audit: Some(Arc::new(ferrum_core::ResidencyAuditLog::new(pool))),
        background_gate: Some(Arc::new(ferrum_core::BackgroundWorkGate::default())),
        ads_introspect: None,
        solum_consent: None,
        ingest_require_auth: false,
        metadata_store_enabled: false,
        pipeline: ferrum_core::PipelineConfig::default(),
    }
}

async fn spawn_edge_gateway(
    cfg: &FerrumConfig,
    pool: FerrumPool,
    objects_dir: &std::path::Path,
) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let drs_state = drs_state_for_edge(pool.clone(), objects_dir);
    let app = ferrum_gateway::app_edge_embed(
        Some(cfg),
        Some(drs_state),
        Some(pool),
        Arc::new(ferrum_gateway::shutdown::ShutdownCoordinator::new()),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    addr
}

fn beacon_variant_query_envelope(request_parameters: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "meta": { "apiVersion": "v2.0.0" },
        "query": { "requestParameters": request_parameters }
    })
}

async fn seed_beacon_fixture_sqlite(pool: &sqlx::SqlitePool) {
    sqlx::query(
        "INSERT INTO beacon_datasets (id, name, description, assembly_id)
         VALUES ('fixture-public', 'Laptop Beacon Fixture', 'SQLite edge mode', 'GRCh38')",
    )
    .execute(pool)
    .await
    .expect("insert beacon dataset");

    sqlx::query(
        "INSERT INTO beacon_variants (dataset_id, chromosome, start, \"end\", reference, alternate, variant_type)
         VALUES ('fixture-public', 'chr1', 1000, 1000, 'A', 'T', 'SNV')",
    )
    .execute(pool)
    .await
    .expect("insert beacon variant");
}

#[tokio::test]
async fn test_sqlite_full_lifecycle() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("ferrum.db");
    let objects_dir = dir.path().join("objects");
    std::fs::create_dir_all(&objects_dir).unwrap();

    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("sqlite connect");
    storage.migrate().await.expect("migrate");
    let pool = storage.pool().clone();
    let cfg = edge_config(&dir);
    ferrum_core::set_health_data_path(objects_dir.clone());
    let addr = spawn_edge_gateway(&cfg, pool.clone(), &objects_dir).await;

    let payload = b"GA4GH edge mode round-trip payload";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(payload.to_vec())
            .file_name("laptop-test.bin")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let ingest_body: serde_json::Value = client
        .post(format!("http://{addr}/ga4gh/drs/v1/ingest/file"))
        .multipart(form)
        .send()
        .await
        .expect("ingest")
        .json()
        .await
        .expect("ingest json");
    let object_id = ingest_body["id"]
        .as_str()
        .expect("object id in ingest response")
        .to_string();

    let get_resp = client
        .get(format!("http://{addr}/ga4gh/drs/v1/objects/{object_id}"))
        .send()
        .await
        .expect("get object");
    assert!(get_resp.status().is_success());
    let obj: serde_json::Value = get_resp.json().await.expect("object json");
    assert_eq!(obj["size"].as_i64(), Some(payload.len() as i64));

    let stream_resp = client
        .get(format!(
            "http://{addr}/ga4gh/drs/v1/objects/{object_id}/stream"
        ))
        .send()
        .await
        .expect("stream");
    assert!(stream_resp.status().is_success());
    let downloaded = stream_resp.bytes().await.expect("stream bytes");
    assert_eq!(downloaded.as_ref(), payload);

    // Beacon v2 HTTP round-trip on the same SQLite-backed gateway.
    let FerrumPool::Sqlite(sqlite_pool) = &pool else {
        panic!("expected sqlite pool");
    };
    seed_beacon_fixture_sqlite(sqlite_pool).await;

    let positive_params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 1000,
        "referenceBases": "A",
        "alternateBases": "T"
    });
    let beacon_resp = client
        .post(format!("http://{addr}/ga4gh/beacon/v2/query"))
        .json(&beacon_variant_query_envelope(positive_params))
        .send()
        .await
        .expect("beacon query");
    assert!(beacon_resp.status().is_success());
    let beacon_json: serde_json::Value = beacon_resp.json().await.expect("beacon json");
    assert_eq!(
        beacon_json
            .pointer("/response/exists")
            .and_then(|x| x.as_bool()),
        Some(true),
        "beacon variant should exist: {beacon_json}"
    );

    let negative_params = serde_json::json!({
        "assemblyId": "GRCh38",
        "referenceName": "1",
        "start": 999999999,
        "referenceBases": "C",
        "alternateBases": "G"
    });
    let miss_resp = client
        .post(format!("http://{addr}/ga4gh/beacon/v2/query"))
        .json(&beacon_variant_query_envelope(negative_params))
        .send()
        .await
        .expect("beacon negative query");
    assert!(miss_resp.status().is_success());
    let miss_json: serde_json::Value = miss_resp.json().await.expect("beacon miss json");
    assert_eq!(
        miss_json
            .pointer("/response/exists")
            .and_then(|x| x.as_bool()),
        Some(false)
    );
}

#[tokio::test]
async fn test_drs_repo_sqlite_crud() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("ferrum.db");
    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("sqlite connect");
    storage.migrate().await.expect("migrate");
    let pool = storage.pool().clone();
    let repo = DrsRepo::new(pool, "localhost".to_string());

    let id = repo
        .create_object(&ferrum_drs::types::CreateObjectRequest {
            name: Some("repo-test".into()),
            description: None,
            mime_type: Some("application/octet-stream".into()),
            size: 4,
            checksums: vec![],
            aliases: None,
            storage_backend: "local".into(),
            storage_key: "repo-test-key".into(),
            is_encrypted: Some(false),
            workspace_id: None,
            ont_metrics: None,
            gisaid_metadata: None,
            metadata_ref: None,
        })
        .await
        .expect("create");

    let obj = repo
        .get_object(&id, false)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(obj.name.as_deref(), Some("repo-test"));
    assert!(repo.delete_object(&id).await.expect("delete"));
}

#[tokio::test]
async fn test_offline_startup() {
    std::env::set_var("FERRUM_OFFLINE", "1");
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("ferrum.db");
    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("connect");
    storage.migrate().await.expect("migrate");
    let cfg = edge_config(&dir);
    ferrum_embed::probe_auth_endpoints(&cfg, true).await;
    std::env::remove_var("FERRUM_OFFLINE");
}

#[tokio::test]
async fn test_memory_cap_warning() {
    let state = MemoryCapState::new(512);
    assert_eq!(state.update_from_rss_mb(410), MemoryCapLevel::Approaching);
    assert!(!state.is_over_limit());

    let dir = TempDir::new().unwrap();
    let mut cfg = edge_config(&dir);
    cfg.africa.as_mut().unwrap().max_memory_mb = Some(512);

    let db_path = dir.path().join("ferrum.db");
    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("connect");
    storage.migrate().await.expect("migrate");
    let pool = storage.pool().clone();
    let objects_dir = dir.path().join("objects");
    let _memory_guard = MemoryCapGuard::spawn_monitor(MemoryCapState::new(512));
    let addr = spawn_edge_gateway(&cfg, pool, &objects_dir).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let client = client.clone();
        let url = format!("http://{addr}/health");
        handles.push(tokio::spawn(async move { client.get(url).send().await }));
    }

    for handle in handles {
        let resp = handle
            .await
            .expect("join health task")
            .expect("health request");
        assert!(
            resp.status().is_success(),
            "gateway must stay healthy under load"
        );
    }

    if let Some(rss_mb) = state.check_and_update() {
        let warn_threshold = 512_u64.saturating_mul(80) / 100;
        if rss_mb >= warn_threshold {
            assert_eq!(
                state.update_from_rss_mb(rss_mb),
                if rss_mb >= 512 {
                    MemoryCapLevel::Exceeded
                } else {
                    MemoryCapLevel::Approaching
                }
            );
        }
    }
}

#[test]
fn resolve_embed_mode_sqlite_by_default() {
    let mut cfg = FerrumConfig::load().expect("config");
    cfg.database.url = None;
    cfg.database.driver = "sqlite".to_string();
    assert_eq!(EmbedMode::resolve(&cfg), EmbedMode::Sqlite);
}

#[tokio::test]
async fn test_laptop_mode_health_endpoint() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("ferrum.db");
    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("connect");
    storage.migrate().await.expect("migrate");
    let pool = storage.pool().clone();
    let cfg = edge_config(&dir);
    let objects_dir = dir.path().join("objects");
    let addr = spawn_edge_gateway(&cfg, pool, &objects_dir).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("health");
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_laptop_mode_drs_list_objects() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("ferrum.db");
    let objects_dir = dir.path().join("objects");
    std::fs::create_dir_all(&objects_dir).unwrap();
    let storage = SqliteStorage::connect_path(&db_path)
        .await
        .expect("connect");
    storage.migrate().await.expect("migrate");
    let pool = storage.pool().clone();
    let cfg = edge_config(&dir);
    let addr = spawn_edge_gateway(&cfg, pool, &objects_dir).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/ga4gh/drs/v1/objects"))
        .send()
        .await
        .expect("list");
    assert!(resp.status().is_success());
}
