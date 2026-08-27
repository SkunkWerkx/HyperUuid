//! Empirically verifies this crate's own "allocation-free" claim rather than just asserting
//! it in a doc comment: a counting `#[global_allocator]` wraps the system allocator for this
//! one test binary only (integration tests compile as separate binaries, so this never
//! affects the library itself or its other consumers) and asserts zero allocations across
//! 1000 calls to each single-item generator.
//!
//! The batch functions are the one deliberate exception — documented in v6.rs/v7.rs as the
//! only allocating path in this crate — so this file also asserts they *do* allocate,
//! turning that doc claim into an executable fact too.
//!
//! Deliberately one `#[test]` function, not five: `ALLOC_COUNT` is one process-wide counter,
//! and `cargo test` spawns a real OS thread per test function by default — thread *creation*
//! itself can allocate (stack/TLS bookkeeping), and that's indistinguishable from this
//! binary's own allocations to a global counter. Confirmed empirically: splitting these into
//! separate `#[test]` fns produced 1-allocation-off failures that moved around between runs
//! and even a shared `Mutex` didn't fix it, since serializing the test *bodies* does nothing
//! about the other five threads' concurrent spawn overhead. One test function means one
//! thread for this whole file — no flag (`--test-threads=1`) required, so it can't silently
//! regress if CI's invocation ever changes.

use hyperuuid::{v4, v5, v6, v7};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
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
    let before = ALLOC_COUNT.load(Ordering::SeqCst);
    let result = std::hint::black_box(f());
    let after = ALLOC_COUNT.load(Ordering::SeqCst);
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

    let mut v6_out = vec![0u8; 100 * 16];
    let (v6_batch_allocs, _) = allocs_during(|| v6::new_v6_batch(RFC_TEST_VECTOR_MS, 100, &mut v6_out).unwrap());
    assert!(v6_batch_allocs > 0, "expected v6::new_v6_batch's scratch buffer to allocate, it didn't");

    let mut v7_out = vec![0u8; 100 * 16];
    let (v7_batch_allocs, _) = allocs_during(|| v7::new_v7_batch(RFC_TEST_VECTOR_MS, 100, &mut v7_out).unwrap());
    assert!(v7_batch_allocs > 0, "expected v7::new_v7_batch's scratch buffer to allocate, it didn't");
}
