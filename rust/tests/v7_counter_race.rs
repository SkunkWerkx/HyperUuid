//! Exercises the property `v7`'s stream exists for — UUIDs minted at one millisecond never go
//! backwards on a thread, and no position in the stream is ever handed out twice — under real
//! multi-threaded contention, including the window where the stream is first seeded: the very
//! first concurrent calls, all racing to install the first state over the unseeded 0.
//!
//! This lives in its own integration test binary rather than in `src/lib.rs`'s test module on
//! purpose. The stream is a process-global static, and `cargo test` runs a whole test module in
//! one process — by the time any unit test ran, seeding would already be long finished and that
//! first window would be unreachable. An integration test file gets its own process, so the
//! threads below genuinely are the first callers. (The unit tests drive the same reservation
//! through a stream of their own, `new_v7_in`, and never touch this one.)
//!
//! What this is and isn't: the invariants below hold by construction in `stream::reserve`
//! (every reservation is one compare-and-swap that installs a strictly larger word, and the
//! first call's advance from 0 is a swap like any other), so correct code passes
//! deterministically. It's a regression guard on that reasoning, not a scheduler-dependent
//! reproduction — a reservation split into a load and a separate store, the tempting wrong
//! version, does fail here, but only on the runs where two threads interleave between them.

// wasm32-wasip1 has no threads to contend with.
#![cfg(not(target_os = "wasi"))]

use std::collections::HashSet;
use std::sync::{Arc, Barrier};

use hyperuuid::v7;

const THREADS: usize = 8;
const PER_THREAD: usize = 5_000;

// RFC 9562 Appendix A.6's vector. One fixed millisecond for every call, so the stream's
// position is the *only* thing separating these UUIDs — exactly the case it's there to cover.
// 40,000 singles stay far below the 2^19 steps a seeded millisecond holds at the least, so none
// of them splits into the next millisecond.
const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

/// Reads back the position in the stream a version 7 UUID was minted at: its 48-bit
/// `unix_ts_ms`, and the step its 26-bit counter carries above 6 random low bits. The counter
/// is 12 bits of `rand_a` (octets 6-7, below the version nibble) then 14 more at the top of
/// `rand_b` (octets 8-9, below the variant bits), the same field extraction `v7::to_sql_order`
/// does. The step, not the whole counter, because two calls handed one step would still differ
/// in their random low bits.
fn position_of(uuid: &hyperuuid::Uuid) -> (u64, u32) {
    let b = uuid.as_bytes();
    let counter = ((b[6] as u32 & 0x0F) << 22)
        | ((b[7] as u32) << 14)
        | ((b[8] as u32 & 0x3F) << 8)
        | (b[9] as u32);
    (v7::unix_millis(uuid), counter >> 6)
}

#[test]
fn stream_never_reissues_or_goes_backwards_under_contention() {
    let barrier = Arc::new(Barrier::new(THREADS));

    let batches: Vec<Vec<(u64, u32)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    // Release every thread into its first new_v7 together, so the seeding call
                    // is contested rather than won uncontested by whoever spawned first.
                    barrier.wait();
                    (0..PER_THREAD)
                        .map(|_| position_of(&v7::new_v7(RFC_TEST_VECTOR_MS).unwrap()))
                        .collect::<Vec<(u64, u32)>>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // Each thread's own view of the stream only ever moves forward: a call that starts after
    // another returns reserves a later step, on any thread, so a thread's own calls are in order.
    for (thread, positions) in batches.iter().enumerate() {
        for pair in positions.windows(2) {
            assert!(
                pair[1] > pair[0],
                "thread {thread} saw the stream go from {:?} to {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    // And no position was ever issued twice across all of them, which is what makes
    // same-millisecond version 7 UUIDs distinct and orderable in the first place.
    let all: Vec<(u64, u32)> = batches.into_iter().flatten().collect();
    let distinct: HashSet<(u64, u32)> = all.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        all.len(),
        "{} of {} stream positions were reissued",
        all.len() - distinct.len(),
        all.len()
    );
}
