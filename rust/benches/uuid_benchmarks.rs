//! Criterion micro-benchmarks for the single-item and batch generation paths. Run with
//! `cargo bench` from `rust/`; Criterion keeps each run's figures under `target/criterion/`.
//!
//! The v7 stream is process-global and Criterion runs every selected bench in one process, so
//! the v7 cases are timed one per process, each selected by its anchored full ID:
//! `cargo bench --bench uuid_benchmarks -- '^single_item/v7$'`, and the same for
//! `single_item/v7_current_timestamp`, `single_item/v7_backfill`, `vs_uuid_crate/hyperuuid_v7`,
//! `batch_1000/v7_batch`, `batch_1000/v7_individual_x1000`, `contended_8t/v7` and
//! `contended_8t/v7_batch`. A plain `cargo bench` runs them all in one process, where
//! `v7_current_timestamp` pulls the stream to the present and every synthetic-timeline case
//! after it falls more than 11 s behind and is timed on the out-of-stream path instead.
//!
//! This measures *time*, not allocations — see `tests/allocation_free.rs` for the
//! allocation-count assertions that back this crate's "allocation-free" claim empirically
//! rather than just in a doc comment.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperuuid::{v4, v5, v7};
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

// RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

// The synthetic timeline the stream-path v7 cases run on (2023-11-14T22:13:20Z). It moves
// forward one millisecond every 1,000 calls, so the stream advances, and re-seeds, every 1,000
// calls and the bench never falls out of stream behind its own earlier calls, however long
// Criterion holds it. A fixed timestamp would instead clamp every call onto one millisecond,
// split forward about a thousand times a second and reach `CounterExhausted` within seconds.
const BENCH_BASE_MS: u64 = 1_700_000_000_000;
const CALLS_PER_MS: u64 = 1_000;
static BENCH_MS: AtomicU64 = AtomicU64::new(BENCH_BASE_MS);
static BENCH_CALLS: AtomicU64 = AtomicU64::new(0);

/// The synthetic timeline's millisecond for the next call, advancing it every 1,000 calls.
#[inline]
fn bench_ms() -> u64 {
    let m = BENCH_MS.load(Relaxed);
    if BENCH_CALLS.fetch_add(1, Relaxed) % CALLS_PER_MS == CALLS_PER_MS - 1 {
        BENCH_MS.fetch_add(1, Relaxed);
    }
    m
}

const THREADS: u64 = 8;

/// Splits `iters` calls of `call` across eight threads and returns the wall time from the
/// moment all eight are ready until the last finishes, so a figure is the wall time per call
/// with eight threads contending for the stream. Each thread's `setup` (a batch's buffer) and
/// the thread spawns run before the clock starts.
fn contended<S>(
    iters: u64,
    setup: impl Fn() -> S + Sync,
    call: impl Fn(&mut S) + Sync,
) -> Duration {
    let per_thread = iters.div_ceil(THREADS);
    let ready = Barrier::new(THREADS as usize + 1);
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                let mut state = setup();
                ready.wait();
                for _ in 0..per_thread {
                    call(&mut state);
                }
            });
        }
        ready.wait();
        let start = Instant::now();
        // Leaving the scope joins every thread, so the clock stops after the last one.
        start
    })
    .elapsed()
}

fn bench_single_item(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_item");
    group.bench_function("v4", |b| b.iter(|| v4::new_v4().unwrap()));
    group.bench_function("v5", |b| {
        b.iter(|| v5::new_v5(v5::namespace::DNS, black_box(b"www.example.com")))
    });
    group.bench_function("v7", |b| {
        b.iter(|| v7::new_v7(black_box(bench_ms())).unwrap())
    });
    // The same call handed the current time, as a `now` door hands it, and the only case on the
    // real clock. On Linux the entropy pool takes a timestamp within a second of its key's as
    // proof the key is fresh, and reads the clock itself when handed any other, which is what
    // the synthetic-timeline rows pay for. The clock is read once per Criterion sample, a few
    // tens of milliseconds of iterations, outside the timed loop; a sample's calls clamp onto
    // the stream's millisecond and split forward when it fills, well inside the drift limit.
    group.bench_function("v7_current_timestamp", |b| {
        let now = now_ms();
        b.iter(|| v7::new_v7(black_box(now)).unwrap())
    });
    // The out-of-stream ("backfill") path: a timestamp 60 s behind the stream, which is minted
    // at itself and leaves the stream alone. The seeding call runs outside the timed loop: a
    // fresh process starts unseeded, and without it the first timed call would seed the stream
    // at the old timestamp and every later one would be in stream.
    group.bench_function("v7_backfill", |b| {
        v7::new_v7(BENCH_BASE_MS).unwrap();
        b.iter(|| v7::new_v7(black_box(BENCH_BASE_MS - 60_000)).unwrap())
    });
    group.finish();
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_millis() as u64
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
    group.bench_function("hyperuuid_v7", |b| {
        b.iter(|| v7::new_v7(black_box(bench_ms())).unwrap())
    });
    group.bench_function("uuid_crate_v7", |b| b.iter(uuid::Uuid::now_v7));
    group.finish();
}

fn bench_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_1000");
    group.bench_function("v7_batch", |b| {
        b.iter_batched(
            || vec![0u8; 1000 * 16],
            |mut out| v7::new_v7_batch(black_box(bench_ms()), 1000, &mut out).unwrap(),
            BatchSize::SmallInput,
        )
    });
    group.bench_function("v7_individual_x1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(v7::new_v7(bench_ms()).unwrap());
            }
        })
    });
    group.finish();
}

// Eight threads on one stream, on the synthetic timeline: singles, and batches of 1000 into a
// buffer each thread allocates once, outside the timed region.
fn bench_contended(c: &mut Criterion) {
    let mut group = c.benchmark_group("contended_8t");
    group.bench_function("v7", |b| {
        b.iter_custom(|iters| {
            contended(
                iters,
                || (),
                |_| {
                    black_box(v7::new_v7(bench_ms()).unwrap());
                },
            )
        })
    });
    group.bench_function("v7_batch", |b| {
        b.iter_custom(|iters| {
            contended(
                iters,
                || vec![0u8; 1000 * 16],
                |out| {
                    v7::new_v7_batch(bench_ms(), 1000, out).unwrap();
                    black_box(&out);
                },
            )
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

    // The input is minted inside the closure, which runs only when this case is selected, so a
    // process timing a v7 generation case never has its stream touched by it.
    group.bench_function("hyperuuid_v7", |b| {
        let hyperuuid_v7 = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        b.iter(|| v7::unix_millis(black_box(&hyperuuid_v7)))
    });

    let uuid_crate_v7 = uuid::Uuid::now_v7();
    group.bench_function("uuid_crate_v7", |b| {
        b.iter(|| black_box(&uuid_crate_v7).get_timestamp())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_item,
    bench_vs_uuid_crate,
    bench_timestamp_extraction,
    bench_batch,
    bench_contended
);
criterion_main!(benches);
