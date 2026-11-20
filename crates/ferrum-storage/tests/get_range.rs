// SPDX-License-Identifier: Apache-2.0
//! `ObjectStorage::get_range`: native local ranges and the default (skip) implementation agree.

use async_trait::async_trait;
use ferrum_core::error::Result;
use ferrum_storage::{LocalStorage, ObjectStorage};
use tokio::io::{AsyncRead, AsyncReadExt};

fn data() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

async fn read_all(mut r: Box<dyn AsyncRead + Send + Unpin>) -> Vec<u8> {
    let mut v = Vec::new();
    r.read_to_end(&mut v).await.expect("read");
    v
}

/// Uses only `get`, so `get_range` falls back to the trait default.
struct DefaultOnly(LocalStorage);

#[async_trait]
impl ObjectStorage for DefaultOnly {
    async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        self.0.put_bytes(key, data).await
    }
    async fn get(&self, key: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        self.0.get(key).await
    }
    async fn delete(&self, key: &str) -> Result<()> {
        self.0.delete(key).await
    }
    async fn exists(&self, key: &str) -> Result<bool> {
        self.0.exists(key).await
    }
    async fn size(&self, key: &str) -> Result<u64> {
        self.0.size(key).await
    }
}

fn tmpdir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ferrum-get-range-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[tokio::test]
async fn local_and_default_ranges_match_the_bytes() {
    let dir = tmpdir();
    let local = LocalStorage::new(&dir).expect("local");
    let bytes = data();
    local.put_bytes("obj.bin", &bytes).await.expect("put");
    let fallback = DefaultOnly(LocalStorage::new(&dir).expect("local"));

    let n = bytes.len() as u64;
    for (a, b) in [
        (0, 0),
        (0, 99),
        (65_535, 65_536),
        (123_456, n - 1),
        (n - 1, n - 1),
    ] {
        let want = &bytes[a as usize..=b as usize];
        let got = read_all(local.get_range("obj.bin", a, b).await.expect("local range")).await;
        assert_eq!(got, want, "local {a}-{b}");
        let got = read_all(
            fallback
                .get_range("obj.bin", a, b)
                .await
                .expect("default range"),
        )
        .await;
        assert_eq!(got, want, "default {a}-{b}");
    }
    // An end past the object is clamped to the object, like HTTP.
    let got = read_all(local.get_range("obj.bin", n - 10, n + 100).await.unwrap()).await;
    assert_eq!(got, &bytes[(n - 10) as usize..]);

    assert!(local.get_range("obj.bin", 10, 5).await.is_err());
    assert!(local.get_range("obj.bin", n, n + 5).await.is_err());
    assert!(fallback.get_range("obj.bin", n + 1, n + 5).await.is_err());
    assert!(local.get_range("missing.bin", 0, 1).await.is_err());
    let _ = std::fs::remove_dir_all(dir);
}
