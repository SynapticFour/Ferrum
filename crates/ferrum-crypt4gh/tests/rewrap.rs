use ferrum_crypt4gh::{
    recipient_keys_from_pubkey, stream_decrypt, stream_encrypt, stream_reencrypt, C4ghKeys,
};
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

fn dec_key(sk: &[u8], pk: &[u8]) -> C4ghKeys {
    C4ghKeys {
        method: 0,
        privkey: sk.to_vec(),
        recipient_pubkey: pk.to_vec(),
    }
}

#[tokio::test]
async fn test_rewrap_preserves_other_recipients() {
    // Learned from Crypt4GH multi-recipient interoperability:
    // header rewrap operations must not break existing recipients.
    let root = temp_path("rewrap");
    fs::create_dir_all(&root).await.expect("mkdir");

    let alice = crypt4gh::keys::generate_private_key();
    let bob = crypt4gh::keys::generate_private_key();
    let carol = crypt4gh::keys::generate_private_key();
    let (alice_sk, alice_pk) = alice.split_at(32);
    let (bob_sk, bob_pk) = bob.split_at(32);
    let (carol_sk, carol_pk) = carol.split_at(32);

    let mut initial_recipients = HashSet::new();
    initial_recipients.insert(recipient_keys_from_pubkey(alice_pk));
    initial_recipients.insert(recipient_keys_from_pubkey(bob_pk));

    let plain = root.join("plain.bin");
    let enc = root.join("enc.c4gh");
    let rewrapped = root.join("rewrapped.c4gh");
    fs::write(&plain, b"ferrum crypt4gh rewrap test payload")
        .await
        .expect("write plain");

    let r = fs::File::open(&plain).await.expect("open plain");
    let w = fs::File::create(&enc).await.expect("create enc");
    stream_encrypt(&initial_recipients, r, w).await.expect("encrypt");

    let mut rewrap_recipients = initial_recipients.clone();
    rewrap_recipients.insert(recipient_keys_from_pubkey(carol_pk));

    let r = fs::File::open(&enc).await.expect("open enc");
    let w = fs::File::create(&rewrapped).await.expect("create rewrapped");
    stream_reencrypt(&[dec_key(alice_sk, alice_pk)], &rewrap_recipients, r, w, false)
        .await
        .expect("rewrap");

    for (name, sk, pk) in [
        ("alice", alice_sk, alice_pk),
        ("bob", bob_sk, bob_pk),
        ("carol", carol_sk, carol_pk),
    ] {
        let out = root.join(format!(
            "{}.out",
            name
        ));
        let r = fs::File::open(&rewrapped).await.expect("open rewrapped");
        let w = fs::File::create(&out).await.expect("create out");
        stream_decrypt(&[dec_key(sk, pk)], r, w, None)
            .await
            .expect("decrypt as recipient");
        let got = fs::read(&out).await.expect("read out");
        assert_eq!(got, b"ferrum crypt4gh rewrap test payload");
    }

    let _ = fs::remove_dir_all(root).await;
}

