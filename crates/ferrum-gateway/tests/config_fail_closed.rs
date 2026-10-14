// SPDX-License-Identifier: BUSL-1.1
//! A discovered config.toml that does not parse must stop the process.
//!
//! Seeded from evidence/2026-10/p3-deployment-proof: `bind = [` in the working
//! directory was discarded and the process fell back to 0.0.0.0:8080.
//! `FERRUM_CONFIG` pointing at an invalid file already panicked (exit 101).

use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn broken_config_toml_in_workdir_exits_and_names_the_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "bind = [\n").unwrap();
    let stderr_path = dir.path().join("stderr.txt");
    let stderr_file = fs::File::create(&stderr_path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ferrum-gateway"))
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env_remove("FERRUM_CONFIG")
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .expect("spawn gateway");

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            let mut text = String::new();
            let _ = fs::File::open(&stderr_path).and_then(|mut f| f.read_to_string(&mut text));
            panic!("gateway still running after a broken config.toml; stderr: {text}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let mut text = String::new();
    fs::File::open(&stderr_path)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert!(
        !status.success(),
        "broken config.toml must not start the process: {text}"
    );
    assert!(
        text.contains("refusing to start"),
        "stderr should say the process is refusing to start, got: {text}"
    );
    assert!(
        text.contains("configuration error"),
        "stderr should include the parse error, got: {text}"
    );
}
