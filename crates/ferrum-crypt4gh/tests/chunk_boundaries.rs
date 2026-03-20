use ferrum_crypt4gh::{recipient_keys_from_pubkey, stream_decrypt, stream_encrypt, C4ghKeys};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

fn temp_path(name: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("ferrum-c4gh-{name}-{ts}"))
}

fn make_payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

async fn roundtrip(payload: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let root = temp_path("chunk");
    fs::create_dir_all(&root).await?;

    let skpk = crypt4gh::keys::generate_private_key();
    let (sk, pk) = skpk.split_at(32);
    let pub_bytes = pk.to_vec();
    let mut recipients = HashSet::new();
    recipients.insert(recipient_keys_from_pubkey(&pub_bytes));

    let dec_key = C4ghKeys {
        method: 0,
        privkey: sk.to_vec(),
        recipient_pubkey: pub_bytes,
    };
    assert!(
        !dec_key.privkey.is_empty(),
        "test setup error: decryptor privkey must not be empty"
    );

    let plain = root.join("plain.bin");
    let enc = root.join("enc.c4gh");
    let out = root.join("out.bin");

    fs::write(&plain, payload).await?;

    let r = fs::File::open(&plain).await?;
    let w = fs::File::create(&enc).await?;
    stream_encrypt(&recipients, r, w).await?;

    let r = fs::File::open(&enc).await?;
    let w = fs::File::create(&out).await?;
    stream_decrypt(&[dec_key], r, w, None).await?;

    let decoded = fs::read(out).await?;
    let _ = fs::remove_dir_all(root).await;
    Ok(decoded)
}

#[tokio::test]
async fn test_encrypt_zero_bytes_produces_non_empty_ciphertext() {
    // If this hangs, the streaming encrypt path is broken.
    let root = temp_path("chunk-only-encrypt");
    fs::create_dir_all(&root).await.expect("mkdir");

    let skpk = crypt4gh::keys::generate_private_key();
    let (_sk, pk) = skpk.split_at(32);
    let pub_bytes = pk.to_vec();
    let mut recipients = HashSet::new();
    recipients.insert(recipient_keys_from_pubkey(&pub_bytes));

    let plain = root.join("plain.bin");
    let enc = root.join("enc.c4gh");
    fs::write(&plain, Vec::<u8>::new()).await.expect("write plain");

    let r = fs::File::open(&plain).await.expect("open plain");
    let w = fs::File::create(&enc).await.expect("create enc");
    stream_encrypt(&recipients, r, w).await.expect("encrypt");

    let md = fs::metadata(&enc).await.expect("metadata");
    assert!(
        md.len() >= 16,
        "ciphertext should contain at least header bytes; len={}",
        md.len()
    );
}

#[tokio::test]
async fn test_encrypt_decrypt_exact_chunk_boundary() {
    // Learned from neicnordic/crypt4gh + htslib interop: 65536-byte boundaries are critical.
    let payload = make_payload(65536);
    let got = roundtrip(payload.clone()).await.expect("roundtrip");
    assert_eq!(got, payload);
}

#[tokio::test]
async fn test_encrypt_decrypt_one_byte_over_chunk_boundary() {
    // Learned from neicnordic/crypt4gh + htslib interop: 65536-byte boundaries are critical.
    let payload = make_payload(65537);
    let got = roundtrip(payload.clone()).await.expect("roundtrip");
    assert_eq!(got, payload);
}

#[tokio::test]
async fn test_encrypt_decrypt_zero_bytes() {
    let payload = make_payload(0);
    let got = roundtrip(payload.clone()).await.expect("roundtrip");
    assert_eq!(got, payload);
}

#[tokio::test]
async fn test_encrypt_decrypt_large_bam_like_payload() {
    // Learned from Crypt4GH performance papers: test realistic payload sizes, not only tiny fixtures.
    let payload = make_payload(10 * 1024 * 1024);
    let got = roundtrip(payload.clone()).await.expect("roundtrip");
    assert_eq!(got, payload);
}

