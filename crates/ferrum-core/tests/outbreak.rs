use ferrum_core::{
    build_gisaid_package, ActivateRequest, DeactivateRequest, GisaidEntry, OutbreakConfig,
    OutbreakPolicy, OutbreakService,
};

async fn sqlite_service(policies: Vec<OutbreakPolicy>) -> OutbreakService {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    OutbreakService::new(
        ferrum_core::FerrumPool::Sqlite(pool),
        OutbreakConfig {
            enabled: true,
            policies,
        },
    )
}

fn mpox_policy() -> OutbreakPolicy {
    OutbreakPolicy {
        name: "mpox_who_emergency".into(),
        trigger_pathogen: "Monkeypox_virus".into(),
        emergency_recipients: vec!["who.int".into(), "africacdc.org".into()],
        access_level: "beacon_only".into(),
        gisaid_auto_package: true,
    }
}

#[tokio::test]
async fn test_outbreak_activate_deactivate() {
    let svc = sqlite_service(vec![mpox_policy()]).await;
    svc.activate(&ActivateRequest {
        policy: "mpox_who_emergency".into(),
        activated_by: "ops@lab.org".into(),
    })
    .await
    .expect("activate");

    assert!(
        svc.emergency_beacon_access("who.int", "Monkeypox_virus")
            .await
            .expect("access")
    );

    svc.deactivate(
        &DeactivateRequest {
            policy: "mpox_who_emergency".into(),
            reason: "contained".into(),
        },
        "ops@lab.org",
    )
    .await
    .expect("deactivate");

    assert!(
        !svc.emergency_beacon_access("who.int", "Monkeypox_virus")
            .await
            .expect("access")
    );
}

#[tokio::test]
async fn test_outbreak_audit_immutable() {
    let svc = sqlite_service(vec![mpox_policy()]).await;
    svc.activate(&ActivateRequest {
        policy: "mpox_who_emergency".into(),
        activated_by: "ops@lab.org".into(),
    })
    .await
    .expect("activate");
    svc.audit_beacon_query(
        "mpox_who_emergency",
        "who-user",
        "who.int",
        "Monkeypox_virus",
        "organism filter",
    )
    .await
    .expect("audit");

    let count = svc.audit_count().await.expect("count");
    assert!(count >= 2);
}

#[tokio::test]
async fn test_outbreak_audit_immutable_no_delete() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .expect("migrate");
    sqlx::query(
        "INSERT INTO outbreak_audit (policy_name, action, actor) VALUES ('p', 'activate', 'a')",
    )
    .execute(&pool)
    .await
    .expect("insert");
    let err = sqlx::query("DELETE FROM outbreak_audit")
        .execute(&pool)
        .await
        .expect_err("delete blocked by trigger");
    assert!(err.to_string().contains("append-only") || err.to_string().contains("ABORT"));
}

#[test]
fn test_gisaid_package_generation() {
    let entries = vec![
        GisaidEntry {
            virus_name: "hCoV-19/Test/001/2024".into(),
            organism: "Monkeypox_virus".into(),
            collection_date: "2024-01-15".into(),
            location: "Africa/NG".into(),
            sequence: "ATCGATCG".into(),
        },
        GisaidEntry {
            virus_name: "hCoV-19/Test/002/2024".into(),
            organism: "Monkeypox_virus".into(),
            collection_date: "2024-02-01".into(),
            location: "Africa/KE".into(),
            sequence: "GGCCTTAA".into(),
        },
        GisaidEntry {
            virus_name: "hCoV-19/Test/003/2024".into(),
            organism: "Monkeypox_virus".into(),
            collection_date: "2024-02-10".into(),
            location: "Africa/ZA".into(),
            sequence: "NNNNNNNN".into(),
        },
    ];
    let archive = build_gisaid_package("mpox_who_emergency", &entries).expect("package");
    assert!(!archive.is_empty());
    let decoded = flate2::read::GzDecoder::new(&archive[..]);
    let mut archive = tar::Archive::new(decoded);
    let mut found_fasta = false;
    for entry in archive.entries().expect("entries") {
        let mut entry = entry.expect("entry");
        if entry.path().expect("path").to_string_lossy() == "sequences.fasta" {
            found_fasta = true;
            let mut buf = String::new();
            use std::io::Read;
            entry.read_to_string(&mut buf).expect("read");
            assert!(buf.contains("hCoV-19/Test/001/2024"));
        }
    }
    assert!(found_fasta);
}
