//! Part-planning for S3 multipart (no AWS I/O) — catches regressions in range splitting.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ferrum_storage::split_file_part_ranges;

fn bench_plan_large_object(c: &mut Criterion) {
    c.bench_function("split_file_part_ranges_5gib_64mib", |b| {
        let size: u64 = 5u64 * 1024 * 1024 * 1024;
        let part: u64 = 64 * 1024 * 1024;
        b.iter(|| black_box(split_file_part_ranges(size, part)));
    });
}

criterion_group!(benches, bench_plan_large_object);
criterion_main!(benches);
