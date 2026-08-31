//! Exercises the one property `v7`'s monotonic counter exists for — values handed out never go
//! backwards, and no value is ever issued twice — under real multi-threaded contention,
//! including the window where it's hardest to hold: the very first concurrent calls, while the
//! one-shot random seed is still being folded in.
//!
//! This lives in its own integration test binary rather than in `src/lib.rs`'s test module on
//! purpose. The counter and its seed flag are process-global statics, and `cargo test` runs a
//! whole test module in one process — by the time any unit test ran, seeding would already be
//! long finished and that first window would be unreachable. An integration test file gets its
//! own process, so the threads below genuinely are the first callers.
//!
//! What this is and isn't: the invariants below hold by construction in `v7::counter` (the
//! seed is *added*, so it commutes with concurrent increments rather than clobbering them), so
//! correct code passes deterministically. It's a regression guard on that reasoning, not a
//! scheduler-dependent reproduction — seeding by `store`ing the seed instead, the tempting
//! wrong version, does fail here, but only on the fraction of runs where the counter climbs
//! past the seed before the store lands.

use std::collections::HashSet;
use std::sync::{Arc, Barrier};

use hyperuuid::v7;

const THREADS: usize = 8;
const PER_THREAD: usize = 5_000;

// RFC 9562 Appendix A.6's vector. One fixed millisecond for every call, so the counter is the
// *only* thing separating these UUIDs — exactly the case it's there to cover.
const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

/// Reads back the 26-bit counter a version 7 UUID was minted with — 12 bits of `rand_a`
/// (octets 6-7, below the version nibble) then 14 more at the top of `rand_b` (octets 8-9,
/// below the variant bits). Same field extraction `v7::to_sql_order` does.
fn counter_of(uuid: &hyperuuid::Uuid) -> u32 {
    let b = uuid.as_bytes();
    ((b[6] as u32 & 0x0F) << 22) | ((b[7] as u32) << 14) | ((b[8] as u32 & 0x3F) << 8) | (b[9] as u32)
}

#[test]
fn counter_never_reissues_or_goes_backwards_under_contention() {
    let barrier = Arc::new(Barrier::new(THREADS));

    let batches: Vec<Vec<u32>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    // Release every thread into its first new_v7 together, so the seeding call
                    // is contested rather than won uncontested by whoever spawned first.
                    barrier.wait();
                    (0..PER_THREAD)
                        .map(|_| counter_of(&v7::new_v7(RFC_TEST_VECTOR_MS).unwrap()))
                        .collect::<Vec<u32>>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // Each thread's own view of the counter only ever moves forward. THREADS * PER_THREAD stays
    // far below the 26-bit wrap, so there's no legitimate way for one of these to decrease.
    for (thread, counters) in batches.iter().enumerate() {
        for pair in counters.windows(2) {
            assert!(
                pair[1] > pair[0],
                "thread {thread} saw the counter go from {} to {}",
                pair[0],
                pair[1]
            );
        }
    }

    // And no counter value was ever issued twice across all of them, which is what makes
    // same-millisecond version 7 UUIDs distinct and orderable in the first place.
    let all: Vec<u32> = batches.into_iter().flatten().collect();
    let distinct: HashSet<u32> = all.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        all.len(),
        "{} of {} counter values were reissued",
        all.len() - distinct.len(),
        all.len()
    );
}
