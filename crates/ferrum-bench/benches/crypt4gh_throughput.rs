//! Lesson 8: regression guard for Crypt4GH throughput-sensitive paths.
//! Source: Polars / DataFusion / noodles CI benchmark patterns.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_small_payload_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypt4gh_small_encrypt_decrypt");
    let size: usize = 64 * 1024;
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function(BenchmarkId::from_parameter("64KiB"), |b| {
        b.iter(|| {
            // Placeholder: real bench would use ferrum-crypt4gh encrypt/decrypt with test keys.
            black_box(sha_digest(&data));
        });
    });
    group.finish();
}

fn sha_digest(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

criterion_group!(benches, bench_small_payload_roundtrip);
criterion_main!(benches);
