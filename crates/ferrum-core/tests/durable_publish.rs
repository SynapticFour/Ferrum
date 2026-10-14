// SPDX-License-Identifier: BUSL-1.1
//! Process-death checks for the emergency shutdown note and field backups.
//!
//! Seeded from evidence/2026-10/p7-claims-behavior: `std::fs::write` left 0-byte
//! CHECKPOINT files, and `create_field_backup` left gzip members that ended
//! early. `kill` here is sudden process death, not a power cut. Startup does
//! not read the note.

use ferrum_core::{create_field_backup, write_emergency_checkpoint, FerrumConfig};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::Read;
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
        if start.elapsed() > Duration::from_secs(15) {
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
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fault child")
}

#[tokio::test]
async fn fault_child_checkpoint() {
    if std::env::var("FERRUM_FAULT_STAGE").ok().as_deref() != Some("fault_child_checkpoint") {
        return;
    }
    let dir = std::env::var("FERRUM_FAULT_WORKDIR").expect("workdir");
    std::env::set_var("HOME", &dir);
    fs::write(Path::new(&dir).join("ready"), b"1").unwrap();
    let start = Instant::now();
    let mut n = 0i64;
    while start.elapsed() < Duration::from_secs(2) {
        write_emergency_checkpoint(Some(n)).await.unwrap();
        n += 1;
    }
}

#[tokio::test]
async fn fault_child_backup() {
    if std::env::var("FERRUM_FAULT_STAGE").ok().as_deref() != Some("fault_child_backup") {
        return;
    }
    let dir = PathBuf::from(std::env::var("FERRUM_FAULT_WORKDIR").expect("workdir"));
    let db = dir.join("ferrum.db");
    let objects = dir.join("objects");
    fs::create_dir_all(objects.join("drs")).unwrap();
    let blob = objects.join("drs").join("blob.bin");
    {
        let mut file = File::create(&blob).unwrap();
        let mut state: u64 = 0x1234_5678_9abc_def0;
        let mut chunk = vec![0u8; 64 * 1024];
        let mut left = 4 * 1024 * 1024;
        while left > 0 {
            for byte in &mut chunk {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 33) as u8;
            }
            let n = left.min(chunk.len());
            std::io::Write::write_all(&mut file, &chunk[..n]).unwrap();
            left -= n;
        }
        file.sync_all().unwrap();
    }
    fs::write(&db, b"sqlite-stand-in").unwrap();
    let cfg_path = dir.join("config.toml");
    let toml = format!(
        "bind = \"127.0.0.1:9\"\n[database]\ndriver = \"sqlite\"\nsqlite_path = \"{}\"\nrun_migrations = false\n[storage]\nbackend = \"local\"\nbase_path = \"{}\"\n",
        db.display(),
        objects.display()
    );
    fs::write(&cfg_path, toml).unwrap();
    let cfg = FerrumConfig::load_from_sources(&[cfg_path], "127.0.0.1:8080", false).unwrap();
    let archive = dir.join("backup.tar.gz");
    fs::write(dir.join("ready"), b"1").unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        create_field_backup(&cfg, &archive, true).unwrap();
    }
}

#[test]
fn killed_checkpoint_writer_leaves_json_or_no_final_file() {
    if std::env::var("FERRUM_FAULT_STAGE").is_ok() {
        return;
    }
    let mut interrupted = 0u32;
    for _ in 0..8 {
        let dir = tempfile::tempdir().unwrap();
        let mut child = spawn_stage("fault_child_checkpoint", dir.path());
        wait_ready(&mut child, &dir.path().join("ready"));
        thread::sleep(Duration::from_millis(40));
        let _ = child.kill();
        let status = child.wait().unwrap();
        if !status.success() {
            interrupted += 1;
        }
        let note = dir.path().join(".ferrum").join("CHECKPOINT");
        if note.is_file() {
            let bytes = fs::read(&note).unwrap();
            assert!(!bytes.is_empty(), "CHECKPOINT must not be 0 bytes");
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("CHECKPOINT must be JSON");
            assert!(value.get("message").is_some());
            assert!(value.get("last_transaction_id").is_some());
        }
        let ferrum_dir = dir.path().join(".ferrum");
        if ferrum_dir.is_dir() {
            for entry in fs::read_dir(&ferrum_dir).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.contains(".ferrum-partial-") {
                    continue;
                }
                assert_eq!(name, "CHECKPOINT", "unexpected file {name}");
            }
        }
    }
    assert!(interrupted > 0, "expected at least one killed writer");
}

#[test]
fn killed_backup_writer_leaves_valid_gzip_or_no_final_archive() {
    if std::env::var("FERRUM_FAULT_STAGE").is_ok() {
        return;
    }
    let mut interrupted = 0u32;
    for _ in 0..6 {
        let dir = tempfile::tempdir().unwrap();
        let mut child = spawn_stage("fault_child_backup", dir.path());
        wait_ready(&mut child, &dir.path().join("ready"));
        thread::sleep(Duration::from_millis(30));
        let _ = child.kill();
        let status = child.wait().unwrap();
        if !status.success() {
            interrupted += 1;
        }
        let archive = dir.path().join("backup.tar.gz");
        if archive.is_file() {
            let bytes = fs::read(&archive).unwrap();
            assert!(!bytes.is_empty(), "archive must not be 0 bytes");
            let mut dec = GzDecoder::new(bytes.as_slice());
            let mut decoded = Vec::new();
            dec.read_to_end(&mut decoded)
                .expect("final archive must be a complete gzip stream");
            assert!(!decoded.is_empty());
        }
    }
    assert!(
        interrupted > 0,
        "expected at least one killed backup writer"
    );
}
