// SPDX-License-Identifier: BUSL-1.1
//! Concurrent residency appends must keep chain_valid true.
//!
//! Seeded from evidence/2026-10/p7-claims-behavior: 20 trials of 8 concurrent
//! appends each ended with chain_valid false and 0 append errors.

use ferrum_core::{FerrumPool, ResidencyAuditLog};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn concurrent_appends_keep_chain_valid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(8));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let audit = Arc::new(ResidencyAuditLog::new(FerrumPool::Sqlite(pool)));
    let trials = 20u32;
    let writers = 8u32;
    for trial in 0..trials {
        let mut tasks = Vec::new();
        for i in 0..writers {
            let audit = Arc::clone(&audit);
            tasks.push(tokio::spawn(async move {
                audit
                    .append(
                        "data_uploaded",
                        Some(&format!("t{trial}-c{i}")),
                        Some("writer"),
                        None,
                        false,
                        Some(i64::from(i)),
                    )
                    .await
            }));
        }
        for task in tasks {
            task.await.expect("join").expect("append");
        }
        let verified = audit.verify().await.unwrap();
        assert!(
            verified.chain_valid,
            "trial {trial} broke the chain (entries {})",
            verified.entry_count
        );
    }
    let final_chain = audit.verify().await.unwrap();
    assert!(final_chain.chain_valid);
    assert_eq!(final_chain.entry_count, i64::from(trials * writers));
}
