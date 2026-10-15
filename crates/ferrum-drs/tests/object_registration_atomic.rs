// SPDX-License-Identifier: BUSL-1.1
//! Process-death checks for object registration and local files.
//!
//! Seeded from evidence/2026-10/p7-claims-behavior `stage_worker`: register
//! called `create_object_with_id` in a loop, and ingest called `put_file` then
//! `create_object_with_id`. Those runs left object rows without a storage
//! reference and final files without a row. `kill` here is sudden process
//! death, not a power cut. This test does not run startup cleanup. Startup
//! removes `.ferrum-partial-*` files older than one hour and leaves newer ones.

use ferrum_core::FerrumPool;
use ferrum_drs::repo::DrsRepo;
use ferrum_drs::types::CreateObjectRequest;
use sqlx::SqlitePool;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn test_exe() -> PathBuf {
    std::env::current_exe().expect("current test executable")
}

fn wait_ready(child: &mut std::process::Child, ready: &Path) {
    let start = Instant::now();
    loop {
        if ready.is_file() {
            return;
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not become ready");
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("child exited before ready: {status}");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn spawn_stage(stage: &str, workdir: &Path) -> std::process::Child {
    Command::new(test_exe())
        .arg("--exact")
        .arg(stage)
        .env("FERRUM_FAULT_STAGE", stage)
        .env("FERRUM_FAULT_WORKDIR", workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn fault child")
}

fn object_req(id: &str, size: i64) -> CreateObjectRequest {
    CreateObjectRequest {
        name: Some(id.to_string()),
        description: None,
        mime_type: None,
        size,
        checksums: vec![],
        aliases: None,
        storage_backend: "local".into(),
        storage_key: format!("drs/{id}"),
        is_encrypted: Some(false),
        workspace_id: None,
        ont_metrics: None,
        gisaid_metadata: None,
        metadata_ref: None,
    }
}

async fn sqlite_repo(dir: &Path) -> (SqlitePool, DrsRepo) {
    let db = dir.join("ferrum.db");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("../ferrum-embed/migrations")
        .run(&pool)
        .await
        .unwrap();
    let repo = DrsRepo::new(FerrumPool::Sqlite(pool.clone()), "localhost".into());
    (pool, repo)
}

#[tokio::test]
async fn fault_child_register() {
    if std::env::var("FERRUM_FAULT_STAGE").ok().as_deref() != Some("fault_child_register") {
        return;
    }
    let dir = PathBuf::from(std::env::var("FERRUM_FAULT_WORKDIR").expect("workdir"));
    let (_pool, repo) = sqlite_repo(&dir).await;
    fs::write(dir.join("ready"), b"1").unwrap();
    let start = Instant::now();
    let mut n = 0u32;
    while start.elapsed() < Duration::from_secs(2) {
        let id = format!("reg-{n:05}");
        repo.create_object_with_id(&object_req(&id, 0), Some(id))
            .await
            .unwrap();
        n += 1;
    }
}

#[tokio::test]
async fn fault_child_ingest() {
    if std::env::var("FERRUM_FAULT_STAGE").ok().as_deref() != Some("fault_child_ingest") {
        return;
    }
    let dir = PathBuf::from(std::env::var("FERRUM_FAULT_WORKDIR").expect("workdir"));
    let objects = dir.join("objects");
    let (_pool, repo) = sqlite_repo(&dir).await;
    let src = dir.join("payload.bin");
    {
        let mut file = File::create(&src).unwrap();
        file.write_all(&vec![0xA5; 64 * 1024]).unwrap();
        file.sync_all().unwrap();
    }
    fs::write(dir.join("ready"), b"1").unwrap();
    let start = Instant::now();
    let mut n = 0u32;
    while start.elapsed() < Duration::from_secs(2) {
        let id = format!("obj-{n:05}");
        repo.create_local_object_from_file(&objects, &src, &object_req(&id, 64 * 1024), Some(id))
            .await
            .unwrap();
        n += 1;
    }
}

async fn open_db(dir: &Path) -> SqlitePool {
    let db = dir.join("ferrum.db");
    let mut last = None;
    for _ in 0..8 {
        match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&db))
            .await
        {
            Ok(pool) => return pool,
            Err(err) => {
                last = Some(err);
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    panic!("could not open database after kill: {last:?}");
}

async fn assert_no_row_without_storage_ref(dir: &Path) {
    let pool = open_db(dir).await;
    let missing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM drs_objects o \
         LEFT JOIN storage_references s ON s.object_id = o.id \
         WHERE s.object_id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(missing, 0, "object row without a storage reference");
    pool.close().await;
}

fn final_files(root: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.contains(".ferrum-partial-") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(root).unwrap();
            let key = rel.to_string_lossy().replace('\\', "/");
            let len = entry.metadata().unwrap().len();
            assert!(len > 0, "final file {key} is empty");
            out.push((key, len));
        }
    }
    out
}

async fn assert_final_files_have_rows(dir: &Path) {
    let pool = open_db(dir).await;
    for (key, len) in final_files(&dir.join("objects")) {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM storage_references WHERE storage_key = $1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            n, 1,
            "final file {key} ({len} bytes) has no storage reference"
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn killed_register_leaves_no_object_without_storage_ref() {
    if std::env::var("FERRUM_FAULT_STAGE").is_ok() {
        return;
    }
    for _ in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut child = spawn_stage("fault_child_register", dir.path());
        wait_ready(&mut child, &dir.path().join("ready"));
        thread::sleep(Duration::from_millis(40));
        let _ = child.kill();
        let _ = child.wait();
        assert_no_row_without_storage_ref(dir.path()).await;
    }
}

#[tokio::test]
async fn killed_local_ingest_leaves_no_orphan_row_or_final_file() {
    if std::env::var("FERRUM_FAULT_STAGE").is_ok() {
        return;
    }
    for _ in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut child = spawn_stage("fault_child_ingest", dir.path());
        wait_ready(&mut child, &dir.path().join("ready"));
        thread::sleep(Duration::from_millis(40));
        let _ = child.kill();
        let _ = child.wait();
        assert_no_row_without_storage_ref(dir.path()).await;
        assert_final_files_have_rows(dir.path()).await;
    }
}
