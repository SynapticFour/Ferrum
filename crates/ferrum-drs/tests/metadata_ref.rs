//! ferrum-meta ↔ DRS binding tests.

use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::types::CreateObjectRequest;
use ferrum_meta_connect::{submission_alias, validate_submission, MetaProfile};
use serde_json::json;

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
async fn metadata_ref_stored_on_object() {
    let pool = sqlite_pool().await;
    let repo = DrsRepo::new(pool, "localhost".into());

    let bundle = json!({
        "ferrum_meta_version": "0.1.0",
        "studies": [{"alias": "s1", "title": "t", "description": "d", "type": "PATHOGEN_SURVEILLANCE", "data_use_conditions": ["DUO:0000007"]}],
        "individuals": [{"alias": "i1", "consent_type": "RESEARCH"}],
        "samples": [{"alias": "sa1", "individual_alias": "i1", "collection_date": "2026-01-01", "collection_site": "site"}],
        "experiments": [{"alias": "e1", "sample_alias": "sa1", "pathogen_organism": "Plasmodium_falciparum"}],
        "files": [{"alias": "f1"}],
        "datasets": [{"alias": "dataset_path001", "title": "t", "file_aliases": ["f1"]}]
    });
    let report = validate_submission(&bundle, Some(MetaProfile::Pathogen));
    assert!(report.valid, "{report}");

    let alias = submission_alias(&bundle).expect("alias");
    repo.upsert_metadata_submission(&alias, "pathogen", &bundle.to_string())
        .await
        .expect("store submission");

    let req = CreateObjectRequest {
        name: Some("seq".into()),
        description: None,
        mime_type: Some("application/x-fastq".into()),
        size: 64,
        checksums: vec![],
        aliases: None,
        storage_backend: "local".into(),
        storage_key: "drs/seq".into(),
        is_encrypted: Some(false),
        workspace_id: None,
        ont_metrics: None,
        gisaid_metadata: None,
        metadata_ref: Some(alias.clone()),
    };
    let id = repo.create_object(&req).await.expect("create");
    let obj = repo.get_object(&id, false).await.unwrap().expect("object");
    assert_eq!(obj.metadata_ref.as_deref(), Some(alias.as_str()));
}
