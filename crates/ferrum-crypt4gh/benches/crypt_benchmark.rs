//! Crypt4GH encrypt throughput (64 KiB chunks — typical genomic segment size).
//! Run on Pi 5: `cargo bench -p ferrum-crypt4gh --bench crypt_benchmark`
//! With ARM flags from `.cargo/config.toml`, expect >500 MB/s when NEON is active.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use crypt4gh::keys::generate_private_key;
use ferrum_crypt4gh::encrypt_bytes_for_pubkey;
use std::hint::black_box;

fn bench_crypt4gh_encrypt(c: &mut Criterion) {
    let skpk = generate_private_key();
    let (_, pk) = skpk.split_at(32);
    let sizes = [64 * 1024, 1024 * 1024];
    let mut group = c.benchmark_group("crypt4gh_encrypt");

    for size in sizes {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                black_box(
                    encrypt_bytes_for_pubkey(black_box(pk), black_box(data.as_slice())).unwrap(),
                );
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_crypt4gh_encrypt);
criterion_main!(benches);
