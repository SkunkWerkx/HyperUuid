//! Empirically verifies this crate's own "allocation-free" claim rather than just asserting
//! it in a doc comment: a counting `#[global_allocator]` wraps the system allocator for this
//! one test binary only (integration tests compile as separate binaries, so this never
//! affects the library itself or its other consumers) and asserts zero allocations across
//! 1000 calls to each single-item generator.
//!
//! The batch functions used to be the one deliberate exception, and this file used to assert
//! they *did* allocate. They don't any more: each one's single `getrandom` call now fills the
//! caller's own output buffer and the deterministic octets get written over the top, so there
//! is no `count`-sized scratch buffer left to allocate. That's what lets the crate compile
//! without `alloc` at all, not just without `std` — so the assertions below cover the whole
//! public API with no exception, and this test is what would catch a scratch buffer creeping
//! back in on a target where `cargo check` alone would never notice.
//!
//! The counter is per-thread, not process-wide, and this file is deliberately one `#[test]`
//! function. Both matter. `cargo test` runs each test body on its own spawned thread while
//! the harness's main thread keeps going: right after `spawn` returns it inserts the join
//! handle into a `HashMap` and pushes a timeout entry onto a `VecDeque`, and the first of
//! each allocates — concurrently with the body already running. A process-wide counter saw
//! exactly that on a slow-to-schedule CI runner (linux-arm64, 1-allocation-off on the very
//! first measured call, green on retry), and had earlier seen the same shape from sibling
//! test threads' spawn overhead when this was five `#[test]` fns. Counting in a thread-local
//! makes the harness invisible by construction; one test function keeps the whole claim on
//! one thread with no `--test-threads=1` flag for CI's invocation to drift away from.
//!
//! The thread-local is `const`-initialised and holds a type with no `Drop`, so reaching it
//! from inside the allocator registers no destructor and allocates nothing itself.

use hyperuuid::{v4, v5, v6, v7};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    static ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

fn allocs_during<T>(f: impl FnOnce() -> T) -> (usize, T) {
    let before = ALLOC_COUNT.with(Cell::get);
    let result = std::hint::black_box(f());
    let after = ALLOC_COUNT.with(Cell::get);
    (after - before, result)
}

#[test]
fn allocation_free() {
    // getrandom's own one-time lazy init (e.g. opening a file descriptor on some platforms)
    // is a per-process cost, not a per-call one, and shouldn't count against the
    // steady-state claim below.
    v4::new_v4().unwrap();
    v5::new_v5(v5::namespace::DNS, b"warmup");
    v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
    v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();

    for _ in 0..1000 {
        let (allocs, _) = allocs_during(|| v4::new_v4().unwrap());
        assert_eq!(allocs, 0, "v4::new_v4 allocated {allocs} time(s) in one call");
    }

    for _ in 0..1000 {
        let (allocs, _) = allocs_during(|| v5::new_v5(v5::namespace::DNS, b"www.example.com"));
        assert_eq!(allocs, 0, "v5::new_v5 allocated {allocs} time(s) in one call");
    }

    for _ in 0..1000 {
        let (allocs, _) = allocs_during(|| v6::new_v6(RFC_TEST_VECTOR_MS).unwrap());
        assert_eq!(allocs, 0, "v6::new_v6 allocated {allocs} time(s) in one call");
    }

    for _ in 0..1000 {
        let (allocs, _) = allocs_during(|| v7::new_v7(RFC_TEST_VECTOR_MS).unwrap());
        assert_eq!(allocs, 0, "v7::new_v7 allocated {allocs} time(s) in one call");
    }

    // The caller's `out` is allocated outside the measured region on purpose — it's the
    // caller's buffer by contract, and the claim under test is that the batch call itself adds
    // nothing on top of it.
    let mut v6_out = vec![0u8; 100 * 16];
    let (allocs, _) = allocs_during(|| v6::new_v6_batch(RFC_TEST_VECTOR_MS, 100, &mut v6_out).unwrap());
    assert_eq!(allocs, 0, "v6::new_v6_batch allocated {allocs} time(s) in one call");

    let mut v7_out = vec![0u8; 100 * 16];
    let (allocs, _) = allocs_during(|| v7::new_v7_batch(RFC_TEST_VECTOR_MS, 100, &mut v7_out).unwrap());
    assert_eq!(allocs, 0, "v7::new_v7_batch allocated {allocs} time(s) in one call");
}
