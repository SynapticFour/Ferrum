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

#[tokio::test]
async fn test_tampered_chunk_is_rejected() {
    // Learned from neicnordic/crypt4gh and AEAD best-practice:
    // tampering in any chunk must fail decryption (per-chunk MAC validation).
    let root = temp_path("mac");
    fs::create_dir_all(&root).await.expect("mkdir");

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

    let plain = root.join("plain.bin");
    let enc = root.join("enc.c4gh");
    let out = root.join("out.bin");

    // > 1 chunk plaintext
    let payload: Vec<u8> = (0..100_000).map(|i| (i % 251) as u8).collect();
    fs::write(&plain, payload).await.expect("write plain");

    let r = fs::File::open(&plain).await.expect("open plain");
    let w = fs::File::create(&enc).await.expect("create enc");
    stream_encrypt(&recipients, r, w).await.expect("encrypt");

    let mut tampered = fs::read(&enc).await.expect("read enc");
    let idx = 65_592usize.min(tampered.len().saturating_sub(1));
    tampered[idx] ^= 0x01;
    fs::write(&enc, tampered).await.expect("write tampered");

    let r = fs::File::open(&enc).await.expect("open tampered");
    let w = fs::File::create(&out).await.expect("create out");
    let dec = stream_decrypt(&[dec_key], r, w, None).await;
    assert!(dec.is_err(), "tampered ciphertext must fail decryption");

    let _ = fs::remove_dir_all(root).await;
}

