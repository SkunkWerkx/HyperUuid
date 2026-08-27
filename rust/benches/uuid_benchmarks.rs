//! Criterion micro-benchmarks for the single-item and batch generation paths. Run with
//! `cargo bench` from `rust/`; HTML reports land in `target/criterion/report/index.html`.
//!
//! This measures *time*, not allocations — see `tests/allocation_free.rs` for the
//! allocation-count assertions that back this crate's "allocation-free" claim empirically
//! rather than just in a doc comment.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use hyperuuid::{v4, v5, v6, v7};
use std::hint::black_box;

// RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

fn bench_single_item(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_item");
    group.bench_function("v4", |b| b.iter(|| v4::new_v4().unwrap()));
    group.bench_function("v5", |b| {
        b.iter(|| v5::new_v5(v5::namespace::DNS, black_box(b"www.example.com")))
    });
    group.bench_function("v6", |b| b.iter(|| v6::new_v6(black_box(RFC_TEST_VECTOR_MS)).unwrap()));
    group.bench_function("v7", |b| b.iter(|| v7::new_v7(black_box(RFC_TEST_VECTOR_MS)).unwrap()));
    group.finish();
}

// Head-to-head against the `uuid` crate (the de facto standard Rust UUID crate) — same
// process, same machine, same run, so the comparison is apples-to-apples rather than pulled
// from someone else's benchmark environment. `uuid` has no batch-generation API, so this
// comparison is single-item only; see `bench_batch` for what batch generation buys on its own.
fn bench_vs_uuid_crate(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_uuid_crate");
    group.bench_function("hyperuuid_v4", |b| b.iter(|| v4::new_v4().unwrap()));
    group.bench_function("uuid_crate_v4", |b| b.iter(uuid::Uuid::new_v4));
    group.bench_function("hyperuuid_v5", |b| {
        b.iter(|| v5::new_v5(v5::namespace::DNS, black_box(b"www.example.com")))
    });
    group.bench_function("uuid_crate_v5", |b| {
        b.iter(|| uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, black_box(b"www.example.com")))
    });
    group.bench_function("hyperuuid_v6", |b| {
        b.iter(|| v6::new_v6(black_box(RFC_TEST_VECTOR_MS)).unwrap())
    });
    group.bench_function("uuid_crate_v6", |b| {
        b.iter(|| uuid::Uuid::now_v6(black_box(&[0, 0, 0, 0, 0, 0])))
    });
    group.bench_function("hyperuuid_v7", |b| {
        b.iter(|| v7::new_v7(black_box(RFC_TEST_VECTOR_MS)).unwrap())
    });
    group.bench_function("uuid_crate_v7", |b| b.iter(uuid::Uuid::now_v7));
    group.finish();
}

fn bench_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_1000");
    group.bench_function("v6_batch", |b| {
        b.iter_batched(
            || vec![0u8; 1000 * 16],
            |mut out| v6::new_v6_batch(black_box(RFC_TEST_VECTOR_MS), 1000, &mut out).unwrap(),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("v6_individual_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(v6::new_v6(RFC_TEST_VECTOR_MS).unwrap());
            }
        })
    });
    group.bench_function("v7_batch", |b| {
        b.iter_batched(
            || vec![0u8; 1000 * 16],
            |mut out| v7::new_v7_batch(black_box(RFC_TEST_VECTOR_MS), 1000, &mut out).unwrap(),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("v7_individual_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(v7::new_v7(RFC_TEST_VECTOR_MS).unwrap());
            }
        })
    });
    group.finish();
}

// Head-to-head against the `uuid` crate's own timestamp-extraction API. Each UUID being
// measured is generated once, outside the timed closure, so only the extraction call itself
// is timed, not generation. The two crates' extraction APIs return different shapes — this
// crate's `unix_millis` returns a plain `u64` millisecond count, `uuid`'s `get_timestamp()`
// returns an `Option<Timestamp>` (100ns ticks since the Gregorian epoch, `Option`-wrapped
// since it's only defined for time-based versions) — but timing "the cost of getting the
// embedded time back out of an existing UUID" is still a fair like-for-like comparison
// regardless of what shape that time comes back in.
fn bench_timestamp_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("timestamp_extraction");

    let hyperuuid_v6 = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
    group.bench_function("hyperuuid_v6", |b| b.iter(|| v6::unix_millis(black_box(&hyperuuid_v6))));

    let uuid_crate_v6 = uuid::Uuid::now_v6(&[0, 0, 0, 0, 0, 0]);
    group.bench_function("uuid_crate_v6", |b| b.iter(|| black_box(&uuid_crate_v6).get_timestamp()));

    let hyperuuid_v7 = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    group.bench_function("hyperuuid_v7", |b| b.iter(|| v7::unix_millis(black_box(&hyperuuid_v7))));

    let uuid_crate_v7 = uuid::Uuid::now_v7();
    group.bench_function("uuid_crate_v7", |b| b.iter(|| black_box(&uuid_crate_v7).get_timestamp()));

    group.finish();
}

criterion_group!(benches, bench_single_item, bench_vs_uuid_crate, bench_timestamp_extraction, bench_batch);
criterion_main!(benches);
