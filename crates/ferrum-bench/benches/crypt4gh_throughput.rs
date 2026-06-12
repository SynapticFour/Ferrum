//! Crypt4GH throughput regression guard (uses real ferrum-crypt4gh encrypt path).

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use crypt4gh::keys::generate_private_key;
use ferrum_crypt4gh::encrypt_bytes_for_pubkey;
use std::hint::black_box;

fn bench_crypt4gh_encrypt_64kb(c: &mut Criterion) {
    let skpk = generate_private_key();
    let (_, pk) = skpk.split_at(32);
    let size: usize = 64 * 1024;
    let data: Vec<u8> = vec![0u8; size];
    let mut group = c.benchmark_group("crypt4gh_throughput");
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function("encrypt_64kb", |b| {
        b.iter(|| {
            black_box(
                encrypt_bytes_for_pubkey(black_box(pk), black_box(&data)).unwrap(),
            );
        });
    });
    group.finish();
}

criterion_group!(benches, bench_crypt4gh_encrypt_64kb);
criterion_main!(benches);
