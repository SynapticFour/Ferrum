//! WES reference mismatch warning tests.

use ferrum_core::FerrumPool;
use ferrum_reference::{check_reference_mismatch, ReferenceRegistry};

async fn pool_with_african_drs() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO drs_objects (id, name, description, size, created_time, updated_time)
         VALUES ('africa-sample-1', 'H3Africa cohort sample', 'East Africa genomic data', 100, datetime('now'), datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn test_reference_mismatch_warning() {
    let pool = pool_with_african_drs().await;
    let registry = ReferenceRegistry::new(FerrumPool::Sqlite(pool));
    let params = serde_json::json!({
        "input_drs_uri": "drs://africa-sample-1"
    });
    let warning = check_reference_mismatch(&registry, Some("GRCh38"), &params)
        .await
        .unwrap()
        .expect("expected REFERENCE_MISMATCH warning");
    assert_eq!(warning.code, "REFERENCE_MISMATCH");
    assert_eq!(warning.reference_used, "GRCh38");
    assert!(warning
        .suggested_alternatives
        .contains(&"H3Africa_v1".to_string()));
}

#[tokio::test]
async fn test_reference_mismatch_absent_for_african_reference() {
    let pool = pool_with_african_drs().await;
    let registry = ReferenceRegistry::new(FerrumPool::Sqlite(pool));
    let params = serde_json::json!({
        "input_drs_uri": "drs://africa-sample-1"
    });
    let warning = check_reference_mismatch(&registry, Some("H3Africa_v1"), &params)
        .await
        .unwrap();
    assert!(warning.is_none());
}
