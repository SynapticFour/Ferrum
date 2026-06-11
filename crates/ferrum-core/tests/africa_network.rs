//! Solar/battery mode and residency audit chain tests.

use ferrum_core::{
    allows_background_work, max_concurrent_requests, resolve_power_mode, BackgroundWorkGate,
    FerrumPool, FerrumPowerMode, PowerConfig, PowerLevel, PowerSource, ResidencyAuditLog,
    StubPowerMonitor,
};
use std::sync::Arc;

#[test]
fn test_solar_mode_low_power() {
    let cfg = PowerConfig {
        enabled: true,
        low_power_threshold: 50,
        emergency_threshold: 10,
    };
    let monitor = StubPowerMonitor {
        source: PowerSource::Battery,
        level: PowerLevel::Low,
        battery_percent: Some(30),
    };
    let mode = resolve_power_mode(&cfg, &monitor);
    assert_eq!(mode, FerrumPowerMode::LowPower);
    assert_eq!(max_concurrent_requests(mode), 4);
    assert!(!allows_background_work(mode));
}

#[test]
fn test_background_work_gate_tracks_low_power() {
    let gate = BackgroundWorkGate::new(FerrumPowerMode::HighPerformance);
    assert!(gate.allows_background_work());
    gate.set_mode(FerrumPowerMode::LowPower);
    assert!(!gate.allows_background_work());
    gate.set_mode(FerrumPowerMode::HighPerformance);
    assert!(gate.allows_background_work());
}

#[test]
fn test_emergency_mode_from_battery() {
    let cfg = PowerConfig {
        enabled: true,
        low_power_threshold: 50,
        emergency_threshold: 10,
    };
    let monitor = StubPowerMonitor {
        source: PowerSource::Battery,
        level: PowerLevel::Critical,
        battery_percent: Some(5),
    };
    let mode = resolve_power_mode(&cfg, &monitor);
    assert_eq!(mode, FerrumPowerMode::Emergency);
    assert_eq!(max_concurrent_requests(mode), 0);
}

#[tokio::test]
async fn test_low_power_semaphore_enforces_four_concurrent() {
    let sem = Arc::new(tokio::sync::Semaphore::new(max_concurrent_requests(
        FerrumPowerMode::LowPower,
    )));
    let mut guards = Vec::new();
    for _ in 0..4 {
        guards.push(sem.clone().acquire_owned().await.expect("low-power permit"));
    }
    assert!(sem.try_acquire().is_err());
}

#[tokio::test]
async fn test_emergency_shutdown_checkpoint() {
    let path = ferrum_core::checkpoint_path();
    if let Some(p) = path.as_ref() {
        let _ = std::fs::remove_file(p);
    }
    ferrum_core::write_emergency_checkpoint(Some(42))
        .await
        .expect("checkpoint");
    let p = ferrum_core::checkpoint_path().expect("path");
    assert!(p.exists());
    let text = std::fs::read_to_string(p).unwrap();
    assert!(text.contains("FERRUM_EMERGENCY_SHUTDOWN"));
}

#[tokio::test]
async fn test_audit_chain_integrity() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let audit = ResidencyAuditLog::new(FerrumPool::Sqlite(pool.clone()));
    for i in 0..100 {
        audit
            .append(
                "data_accessed",
                Some(&format!("obj-{i}")),
                Some("alice"),
                None,
                false,
                None,
            )
            .await
            .unwrap();
    }
    let verify = audit.verify().await.unwrap();
    assert!(verify.chain_valid);
    assert_eq!(verify.entry_count, 100);

    let mut entries = audit.query_range(None, None).await.unwrap().entries;
    entries[10].entry_hash = "deadbeef".into();
    assert!(!ferrum_core::verify_chain(&entries));
}

#[tokio::test]
async fn test_audit_append_only() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let audit = ResidencyAuditLog::new(FerrumPool::Sqlite(pool.clone()));
    audit
        .append("beacon_query", None, Some("bob"), None, false, None)
        .await
        .unwrap();
    let result = sqlx::query("DELETE FROM residency_audit")
        .execute(&pool)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_audit_chain_integrity_postgres() {
    let Ok(url) = std::env::var("FERRUM_TEST_POSTGRES_URL") else {
        eprintln!("skipping postgres residency test (set FERRUM_TEST_POSTGRES_URL)");
        return;
    };
    let pool = sqlx::PgPool::connect(&url).await.expect("postgres pool");
    sqlx::migrate!("../ferrum-core/migrations")
        .run(&pool)
        .await
        .expect("migrate postgres");
    let audit = ResidencyAuditLog::new(FerrumPool::Postgres(pool));
    audit
        .append(
            "data_uploaded",
            Some("pg-obj-1"),
            Some("alice"),
            None,
            false,
            Some(1024),
        )
        .await
        .unwrap();
    let verify = audit.verify().await.unwrap();
    assert!(verify.chain_valid);
    assert_eq!(verify.entry_count, 1);
}
