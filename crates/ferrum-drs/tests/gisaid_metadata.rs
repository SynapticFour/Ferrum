//! GISAID metadata ingest storage tests.

use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::types::CreateObjectRequest;

async fn sqlite_pool() -> FerrumPool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    FerrumPool::Sqlite(pool)
}

#[tokio::test]
async fn test_gisaid_metadata_stored() {
    let pool = sqlite_pool().await;
    let repo = DrsRepo::new(pool, "localhost".into());
    let gisaid = serde_json::json!({
        "collection_date": "2025-11-01",
        "location": "Liberia/Margibi",
        "host": "Human",
        "submitting_lab": "NPHIL",
        "submitting_lab_address": "Monrovia, Liberia",
        "originating_lab": "NPHIL National Reference Laboratory"
    });
    let req = CreateObjectRequest {
        name: Some("sample-seq".into()),
        description: None,
        mime_type: Some("application/fasta".into()),
        size: 128,
        checksums: vec![],
        aliases: None,
        storage_backend: "local".into(),
        storage_key: "drs/test-seq".into(),
        is_encrypted: Some(false),
        workspace_id: None,
        ont_metrics: None,
        gisaid_metadata: Some(gisaid.clone()),
        metadata_ref: None,
    };
    let id = repo.create_object(&req).await.expect("create object");
    let obj = repo.get_object(&id, false).await.unwrap().expect("object");
    assert_eq!(obj.gisaid_metadata, Some(gisaid));
}
