use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crypt4gh::reencrypt;
use ferrum_crypt4gh::{recipient_keys_from_pubkey, reencrypt_bytes, C4ghKeys};
use std::collections::HashSet;
use std::io::Cursor;

/// Create valid Crypt4GH ciphertext for a header-only rewrap benchmark.
fn setup_ciphertext_and_keys() -> (Vec<C4ghKeys>, HashSet<C4ghKeys>, Vec<u8>, Bytes) {
    // Master keypair: this is the existing recipient for the stored object.
    let master_skpk = crypt4gh::keys::generate_private_key();
    let (master_sk, master_pk) = master_skpk.split_at(32);
    let master_pub_bytes = master_pk.to_vec();

    let master_key = C4ghKeys {
        method: 0,
        privkey: master_sk.to_vec(),
        recipient_pubkey: master_pub_bytes.clone(),
    };
    let master_keys = vec![master_key];

    // Encryption recipients for generating ciphertext (stored object is encrypted to master pubkey).
    let stored_recipients = HashSet::from([recipient_keys_from_pubkey(&master_pub_bytes)]);

    // Client keypair: this is the new recipient for header rewrap.
    let client_skpk = crypt4gh::keys::generate_private_key();
    let (client_sk, client_pk) = client_skpk.split_at(32);
    let client_pub_bytes = client_pk.to_vec();
    let _client_dec_key = C4ghKeys {
        method: 0,
        privkey: client_sk.to_vec(),
        recipient_pubkey: client_pub_bytes.clone(),
    };

    let client_recipient_keys = HashSet::from([recipient_keys_from_pubkey(&client_pub_bytes)]);

    // Generate ciphertext once (benchmark focuses on rewrap, not encryption cost).
    let plaintext = vec![0xABu8; 128 * 1024]; // 128 KiB keeps setup fast enough for local runs.
    let mut reader = Cursor::new(plaintext);
    let mut ciphertext = Vec::<u8>::new();
    crypt4gh::encrypt(&stored_recipients, &mut reader, &mut ciphertext, 0, None)
        .expect("encrypt ciphertext for bench");

    let ciphertext_bytes = Bytes::from(ciphertext.clone());
    (
        master_keys,
        client_recipient_keys,
        ciphertext,
        ciphertext_bytes,
    )
}

fn reencrypt_header_vec_baseline(
    keys: &[C4ghKeys],
    recipients: &HashSet<C4ghKeys>,
    input_vec: Vec<u8>,
) -> Vec<u8> {
    let mut reader = Cursor::new(input_vec);
    let mut writer = Vec::<u8>::new();
    reencrypt(keys, recipients, &mut reader, &mut writer, true).expect("reencrypt header");
    writer
}

fn bench_header_rewrap(c: &mut Criterion) {
    let (master_keys, client_recipient_keys, ciphertext_vec, ciphertext_bytes) =
        setup_ciphertext_and_keys();

    c.bench_function("header_rewrap_vec_baseline", |b| {
        b.iter(|| {
            let input = black_box(ciphertext_vec.clone());
            let out = reencrypt_header_vec_baseline(&master_keys, &client_recipient_keys, input);
            black_box(out)
        })
    });

    c.bench_function("header_rewrap_bytes", |b| {
        b.iter(|| {
            let input = black_box(ciphertext_bytes.clone());
            let out = reencrypt_bytes(&master_keys, &client_recipient_keys, input, true)
                .expect("reencrypt header bytes");
            black_box(out)
        })
    });
}

criterion_group!(benches, bench_header_rewrap);
criterion_main!(benches);
