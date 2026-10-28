//! The arithmetic of the version 7 stream, the one word every v7 reservation is a
//! compare-and-swap on: `(ms + 1) << 20 | last_step`, where `last_step` is the last step
//! reserved in `ms` and 0 means unseeded. A UUID's 26-bit counter is `step << 6 | low6`.
//!
//! Everything here is pure, so each transition is tested without the global. [`plan`] reads a
//! state and returns the reservation a call would make and the state it would install:
//!
//! - **Advance** (unseeded, or the stream is behind `m`): steps `seed … seed + k − 1` at `m`.
//! - **Clamp** (the stream is at most [`CLAMP_WINDOW_MS`] ahead of `m`): the next `k` steps
//!   after the stream's, at the stream's ms, splitting into the next ms when they don't fit.
//!   A split may take the stream at most [`DRIFT_LIMIT_MS`] past `m`, and never past
//!   [`STREAM_MAX_MILLIS`]; otherwise the call fails with
//!   [`NewV7Error::CounterExhausted`] and reserves nothing.
//! - **Out of stream** (further behind, or `m` past [`STREAM_MAX_MILLIS`]): a seeded batch at
//!   `m` that installs nothing, ordered against nothing.
//!
//! Every installed word is strictly larger than the one it replaces, and the ceiling check
//! keeps the encoding from wrapping.

use crate::v7::NewV7Error;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Steps per millisecond: the counter's top 20 bits.
pub(crate) const STEPS: u32 = 1 << 20;
/// The largest seed an advance draws, so at least 2^19 steps stay free in a fresh ms.
pub(crate) const SEED_MAX: u32 = (1 << 19) - 1;
/// The last millisecond the stream stores: `(ms + 1) << 20` must fit 64 bits.
pub(crate) const STREAM_MAX_MILLIS: u64 = (1 << 44) - 2;
/// How far the stream may be ahead of a call's ms for the call to clamp onto it (W).
pub(crate) const CLAMP_WINDOW_MS: u64 = 11_000;
/// How far a split may take the stream ahead of the call's ms (D).
pub(crate) const DRIFT_LIMIT_MS: u64 = 1_000;

/// Mask of the step field.
const STEP_MASK: u64 = (1 << 20) - 1;

/// The stream word for `last_step` reserved at `ms`. Requires `ms <= STREAM_MAX_MILLIS` and
/// `last_step < STEPS`; never 0.
#[inline]
pub(crate) fn encode(ms: u64, last_step: u32) -> u64 {
    // Wrapping only so a caller's broken precondition can't add a panic path; on a valid input
    // `ms + 1 < 2^44` and the shift stays inside 64 bits.
    (ms.wrapping_add(1) << 20) | (u64::from(last_step) & STEP_MASK)
}

/// The `(ms, last_step)` a stream word holds, or `None` when it is unseeded.
#[inline]
pub(crate) fn decode(state: u64) -> Option<(u64, u32)> {
    // Every word with a zero ms field, 0 among them, reads as unseeded; only 0 is ever stored.
    // The step field is 20 bits, so the cast is lossless.
    let ms = (state >> 20).checked_sub(1)?;
    Some((ms, (state & STEP_MASK) as u32))
}

/// The first step of an out-of-stream or advancing reservation of `k` steps, from one random
/// `r`, over `[0, min(SEED_MAX, STEPS − k)]`, so the `k` steps always fit in one ms: exactly
/// uniform while that span is 2^19 (any `k` up to 2^19), and near-uniform past that.
#[inline]
pub(crate) fn seed(k: u32, r: u32) -> u32 {
    let span = u64::from(SEED_MAX.min(STEPS.saturating_sub(k))) + 1;
    // r < 2^32 and span <= 2^19, so the product fits and the shift leaves less than span.
    ((u64::from(r) * span) >> 32) as u32
}

/// The steps one call reserves and the word it installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reservation {
    /// The ms of step 0 of the reservation.
    pub ms: u64,
    /// The first step at `ms`. Step j is `first_step + j` at `ms` while that is below
    /// [`STEPS`], and `first_step + j − STEPS` at `ms + 1` past it (a split's tail).
    /// `STEPS` itself means the head is empty and the whole reservation is at `ms + 1`.
    pub first_step: u32,
    /// The word to install by compare-and-swap, or `None` for an out-of-stream call.
    pub next: Option<u64>,
}

/// The reservation of `k` steps a call at `m` makes against `state`, with `r` the call's
/// seed draw. Requires `m <= MAX_UNIX_MILLIS` and `1 <= k <= STEPS`. Fails with
/// [`NewV7Error::CounterExhausted`] only when the reservation would split past the drift
/// limit or the stream's last ms, and then it reserves nothing.
#[inline]
pub(crate) fn plan(state: u64, m: u64, k: u32, r: u32) -> Result<Reservation, NewV7Error> {
    let outside = |m| Reservation {
        ms: m,
        first_step: seed(k, r),
        next: None,
    };
    if m > STREAM_MAX_MILLIS {
        return Ok(outside(m));
    }
    let (l, s) = match decode(state) {
        Some((l, s)) if l >= m => (l, s),
        // Advance: unseeded, or the stream is behind m.
        _ => {
            let first = seed(k, r);
            // seed + k <= STEPS and k >= 1, so the last step is below STEPS.
            return Ok(Reservation {
                ms: m,
                first_step: first,
                next: Some(encode(m, first + k - 1)),
            });
        }
    };
    if l - m > CLAMP_WINDOW_MS {
        return Ok(outside(m));
    }
    // s < STEPS from the 20-bit field, so neither of these wraps.
    let room = STEPS - 1 - s;
    let first = s + 1;
    if k <= room {
        return Ok(Reservation {
            ms: l,
            first_step: first,
            next: Some(encode(l, s + k)),
        });
    }
    // Split: the tail of t steps runs 0 … t − 1 at l + 1. decode caps l at STREAM_MAX_MILLIS,
    // so l + 1 can't overflow, and l >= m keeps the subtraction in range.
    let rolled = l + 1;
    if rolled - m > DRIFT_LIMIT_MS || rolled > STREAM_MAX_MILLIS {
        return Err(NewV7Error::CounterExhausted);
    }
    let t = k - room;
    Ok(Reservation {
        ms: l,
        first_step: first,
        next: Some(encode(rolled, t - 1)),
    })
}

/// Reserves `k` steps at `m` on `stream`: [`plan`] against the word it loads, then a
/// compare-and-swap of the planned word, re-planned against the newer word with the same `r`
/// whenever another call got there first. Requires `m <= MAX_UNIX_MILLIS` and
/// `1 <= k <= STEPS`. An out-of-stream reservation and an error both return without writing,
/// so a failure leaves the word as it was.
///
/// `Relaxed` is enough: the word publishes no other memory, and its modification order alone
/// orders the reservations, so a call that loads after another's swap plans past it.
#[inline]
pub(crate) fn reserve(
    stream: &AtomicU64,
    m: u64,
    k: u32,
    r: u32,
) -> Result<Reservation, NewV7Error> {
    // k == 0 would plan `next == state`, a swap that reserves nothing.
    debug_assert!((1..=STEPS).contains(&k));
    let mut state = stream.load(Relaxed);
    loop {
        let got = plan(state, m, k, r)?;
        let Some(next) = got.next else {
            return Ok(got);
        };
        match stream.compare_exchange_weak(state, next, Relaxed, Relaxed) {
            Ok(_) => return Ok(got),
            Err(now) => state = now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v7::MAX_UNIX_MILLIS;
    use std::sync::Mutex;

    /// Some time in 2026; the tests move the clock around it.
    const T: u64 = 1_790_000_000_000;

    /// A position as a comparable word: `encode` with room for `step == STEPS`, an empty head.
    fn word(ms: u64, step: u32) -> u64 {
        ((ms + 1) << 20) + u64::from(step)
    }

    #[test]
    fn seed_bounds() {
        assert_eq!(seed(1, u32::MAX), SEED_MAX);
        assert_eq!(seed(1, 0), 0);
        assert_eq!(seed(STEPS, u32::MAX), 0);
        for k in [1, 2, 1000, STEPS / 2 + 1, STEPS - 1, STEPS] {
            assert!(seed(k, u32::MAX) + k <= STEPS, "k = {k}");
        }
    }

    #[test]
    fn encode_decode_round_trip() {
        assert_eq!(decode(encode(0, 0)), Some((0, 0)));
        assert_eq!(
            decode(encode(STREAM_MAX_MILLIS, STEPS - 1)),
            Some((STREAM_MAX_MILLIS, STEPS - 1))
        );
        assert_eq!(decode(0), None);
        assert_ne!(encode(0, 0), 0);
        // Any word whose ms field is zero reads as unseeded, not just 0 itself.
        assert_eq!(decode(1), None);
        assert_eq!(decode(STEP_MASK), None);
        // `encode` keeps only the step field, so an out-of-range step can't spill into the ms.
        assert_eq!(decode(encode(T, STEPS | 5)), Some((T, 5)));
    }

    #[test]
    fn advance_from_unseeded_reserves_the_whole_batch() {
        // A 65-item batch is k = 2 steps.
        let r = 0x9E37_79B9;
        let s = seed(2, r);
        assert_eq!(
            plan(0, T, 2, r),
            Ok(Reservation {
                ms: T,
                first_step: s,
                next: Some(encode(T, s + 1)),
            })
        );
    }

    #[test]
    fn advance_from_an_older_ms() {
        let r = 0x8000_0000;
        let s = seed(1, r);
        assert_eq!(
            plan(encode(T - 5, 7), T, 1, r),
            Ok(Reservation {
                ms: T,
                first_step: s,
                next: Some(encode(T, s)),
            })
        );
    }

    #[test]
    fn clamp_fits_at_lags_0_g_and_w() {
        for lag in [0, 10_000, 11_000] {
            assert_eq!(
                plan(encode(T, 9), T - lag, 3, u32::MAX),
                Ok(Reservation {
                    ms: T,
                    first_step: 10,
                    next: Some(encode(T, 12)),
                }),
                "lag = {lag}"
            );
        }
    }

    #[test]
    fn lag_past_w_is_out_of_stream() {
        let (k, r) = (3, 0x1234_5678);
        let m = T - 11_001;
        assert_eq!(
            plan(encode(T, 9), m, k, r),
            Ok(Reservation {
                ms: m,
                first_step: seed(k, r),
                next: None,
            })
        );
    }

    #[test]
    fn past_stream_max_is_out_of_stream() {
        for m in [STREAM_MAX_MILLIS + 1, MAX_UNIX_MILLIS] {
            for state in [0, encode(T, 9), encode(STREAM_MAX_MILLIS, STEPS - 1)] {
                assert_eq!(
                    plan(state, m, STEPS, u32::MAX),
                    Ok(Reservation {
                        ms: m,
                        first_step: 0,
                        next: None,
                    }),
                    "m = {m}, state = {state:#x}"
                );
            }
        }
    }

    #[test]
    fn residue_counterexamples_classify_correctly() {
        // A stream far ahead of m: out of stream, never a clamp onto a 38-bit residue.
        let r = 0x0F0F_0F0F;
        assert_eq!(
            plan(encode((1 << 38) + 100, 0), 100, 1, r),
            Ok(Reservation {
                ms: 100,
                first_step: seed(1, r),
                next: None,
            })
        );
        // A stream far behind m: an advance, never a clamp.
        let m = (1 << 38) + 99;
        assert_eq!(
            plan(encode(100, 0), m, 1, r),
            Ok(Reservation {
                ms: m,
                first_step: seed(1, r),
                next: Some(encode(m, seed(1, r))),
            })
        );
    }

    #[test]
    fn split_from_step_2_pow_20_minus_2() {
        let got = plan(encode(T, STEPS - 2), T, STEPS, u32::MAX).unwrap();
        assert_eq!(
            got,
            Reservation {
                ms: T,
                first_step: STEPS - 1,
                next: Some(encode(T + 1, STEPS - 2)),
            }
        );
    }

    #[test]
    fn split_from_an_exhausted_ms() {
        let got = plan(encode(T, STEPS - 1), T, STEPS, u32::MAX).unwrap();
        assert_eq!(
            got,
            Reservation {
                ms: T,
                first_step: STEPS,
                next: Some(encode(T + 1, STEPS - 1)),
            }
        );
    }

    #[test]
    fn drift_bound() {
        let m = T;
        let got = plan(encode(m + DRIFT_LIMIT_MS - 1, STEPS - 1), m, 1, u32::MAX).unwrap();
        assert_eq!(got.ms + 1, m + DRIFT_LIMIT_MS);
        assert_eq!(got.first_step, STEPS);
        assert_eq!(got.next, Some(encode(m + DRIFT_LIMIT_MS, 0)));

        assert_eq!(
            plan(encode(m + DRIFT_LIMIT_MS, STEPS - 1), m, 1, u32::MAX),
            Err(NewV7Error::CounterExhausted)
        );
    }

    #[test]
    fn ceiling() {
        let l = STREAM_MAX_MILLIS;
        assert_eq!(
            plan(encode(l, STEPS - 1), l, 1, u32::MAX),
            Err(NewV7Error::CounterExhausted)
        );
        assert_eq!(
            plan(encode(l, 5), l, STEPS - 5, u32::MAX),
            Err(NewV7Error::CounterExhausted)
        );
        // One step fewer still fits at the ceiling ms.
        assert_eq!(
            plan(encode(l, 5), l, STEPS - 6, u32::MAX),
            Ok(Reservation {
                ms: l,
                first_step: 6,
                next: Some(encode(l, STEPS - 1)),
            })
        );
        // A split into the ceiling ms itself still succeeds.
        assert_eq!(
            plan(encode(l - 1, STEPS - 1), l - 1, 1, u32::MAX),
            Ok(Reservation {
                ms: l - 1,
                first_step: STEPS,
                next: Some(encode(l, 0)),
            })
        );
    }

    #[test]
    fn repeated_max_batch_at_one_ms_is_ordered_until_exhausted() {
        let (mut state, mut last) = (0, 0);
        let mut reservations = 0u64;
        loop {
            match plan(state, T, STEPS, 0xDEAD_BEEF) {
                Ok(got) => {
                    let first = word(got.ms, got.first_step);
                    let next = got.next.expect("a stream reservation");
                    assert!(first > last, "reservation {reservations} overlaps");
                    // The k steps run contiguously from first to next, through any split.
                    assert_eq!(next - first, u64::from(STEPS - 1));
                    (state, last) = (next, next);
                    reservations += 1;
                }
                Err(e) => {
                    assert_eq!(e, NewV7Error::CounterExhausted);
                    break;
                }
            }
        }
        assert_eq!(reservations, 1 + DRIFT_LIMIT_MS);
    }

    #[test]
    fn capacity_counts_the_seeding_call() {
        fn singles(mut state: u64, calls: u32) -> u64 {
            for _ in 0..calls {
                let got = plan(state, T, 1, u32::MAX).unwrap();
                assert_eq!(got.ms, T, "a single split early");
                state = got.next.unwrap();
            }
            state
        }
        let split = |state| plan(state, T, 1, u32::MAX).unwrap();

        // The seeding call reserves step SEED_MAX, and 2^19 more reach the last step.
        let full = singles(0, SEED_MAX + 2);
        assert_eq!(full, encode(T, STEPS - 1));
        assert_eq!(split(full).next, Some(encode(T + 1, 0)));

        let full = singles(encode(T, SEED_MAX), 1 << 19);
        assert_eq!(full, encode(T, STEPS - 1));
        assert_eq!(split(full).next, Some(encode(T + 1, 0)));
    }

    #[test]
    fn state_only_grows() {
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let mut rand = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let (mut state, mut clock) = (0u64, T);
        let (mut advances, mut clamps, mut outside, mut errors) = (0, 0, 0, 0);
        for _ in 0..100_000 {
            clock += rand() % 3;
            let m = match rand() % 8 {
                // Behind the stream by more than W: out of stream.
                0 => clock - CLAMP_WINDOW_MS - 1 - rand() % 50_000,
                // Ahead: an advance unless a split has pushed the stream past it.
                1 => clock + 1 + rand() % 3,
                // Within G of the clock.
                _ => clock - rand() % 10_000,
            };
            let k = match rand() % 4 {
                0 => STEPS - (rand() % 4) as u32,
                1 => 1 + (rand() % u64::from(STEPS)) as u32,
                _ => 1 + (rand() % 3) as u32,
            };
            let r = rand() as u32;
            let before = decode(state);
            match plan(state, m, k, r) {
                Ok(got) => match got.next {
                    Some(next) => {
                        assert!(next > state, "{state:#x} -> {next:#x}");
                        assert!(word(got.ms, got.first_step) > state);
                        assert_eq!(next - word(got.ms, got.first_step), u64::from(k - 1));
                        match before {
                            Some((l, _)) if l >= m => clamps += 1,
                            _ => advances += 1,
                        }
                        state = next;
                    }
                    None => {
                        assert_eq!(got.ms, m);
                        assert!(got.first_step + k <= STEPS);
                        outside += 1;
                    }
                },
                // `plan` is pure, so the loop's state is left as it was: nothing is installed.
                Err(e) => {
                    assert_eq!(e, NewV7Error::CounterExhausted);
                    errors += 1;
                }
            }
        }
        assert!(
            advances > 0 && clamps > 0 && outside > 0 && errors > 0,
            "bands: {advances} advances, {clamps} clamps, {outside} out of stream, {errors} errors"
        );
    }

    #[test]
    fn reserve_returns_errors_without_touching_the_atomic() {
        let ceiling = encode(STREAM_MAX_MILLIS, STEPS - 1);
        let stream = AtomicU64::new(ceiling);
        assert_eq!(
            reserve(&stream, STREAM_MAX_MILLIS, 1, u32::MAX),
            Err(NewV7Error::CounterExhausted)
        );
        assert_eq!(stream.load(Relaxed), ceiling);
    }

    #[test]
    fn out_of_window_lags_single_threaded() {
        // Lags in (G, W] clamp onto the stream and install the next steps.
        let stream = AtomicU64::new(encode(T, 9));
        let mut last = 9;
        for lag in [10_001, 10_500, CLAMP_WINDOW_MS] {
            let got = reserve(&stream, T - lag, 2, u32::MAX).unwrap();
            assert_eq!(got.ms, T, "lag = {lag}");
            assert_eq!(got.first_step, last + 1, "lag = {lag}");
            last += 2;
            assert_eq!(got.next, Some(encode(T, last)), "lag = {lag}");
            assert_eq!(stream.load(Relaxed), encode(T, last), "lag = {lag}");
        }
        // Lags past W are out of stream: stamped at m, and the atomic is left alone.
        for lag in [CLAMP_WINDOW_MS + 1, 50_000] {
            let (k, r) = (2, 0x1234_5678);
            let m = T - lag;
            assert_eq!(
                reserve(&stream, m, k, r),
                Ok(Reservation {
                    ms: m,
                    first_step: seed(k, r),
                    next: None,
                }),
                "lag = {lag}"
            );
            assert_eq!(stream.load(Relaxed), encode(T, last), "lag = {lag}");
        }
    }

    /// The sort key of step `j` of a reservation of `got`: `ms << 20 | step`, with the tail
    /// rule applied, so a split's tail lands at `ms + 1`. Only the threaded tests use it.
    #[cfg(not(target_os = "wasi"))]
    fn key(got: &Reservation, j: u32) -> u128 {
        let at = got.first_step + j;
        let (ms, step) = if at < STEPS {
            (got.ms, at)
        } else {
            (got.ms + 1, at - STEPS)
        };
        (u128::from(ms) << 20) | u128::from(step)
    }

    /// The largest last key of every reservation that has completed. A reservation that starts
    /// after another completes must sort after it, so its first key must exceed the floor
    /// `before()` read.
    struct MaxOracle(Mutex<u128>);

    impl MaxOracle {
        fn before(&self) -> u128 {
            *self.0.lock().unwrap()
        }
        fn after(&self, last: u128) {
            let mut max = self.0.lock().unwrap();
            *max = (*max).max(last);
        }
    }

    /// The broken oracle: it keeps the last completion, not the largest, so a completion that
    /// lands late lowers the floor.
    struct LatestOracle(Mutex<u128>);

    impl LatestOracle {
        fn before(&self) -> u128 {
            *self.0.lock().unwrap()
        }
        fn after(&self, last: u128) {
            *self.0.lock().unwrap() = last;
        }
    }

    #[test]
    fn max_completed_oracle() {
        let oracle = MaxOracle(Mutex::new(0));
        oracle.after(200);
        oracle.after(100);
        let floor = oracle.before();
        // 150 sorts before the completion at 200: a violation, and the oracle rejects it.
        assert!(150 <= floor, "floor = {floor}");
    }

    #[test]
    fn latest_returned_oracle_misses_the_injected_violation() {
        let oracle = LatestOracle(Mutex::new(0));
        oracle.after(200);
        oracle.after(100);
        let floor = oracle.before();
        // The same violation passes, which is why the threaded test keeps the max.
        assert!(150 > floor, "floor = {floor}");
    }

    #[cfg(not(target_os = "wasi"))]
    #[test]
    fn concurrent_reservations_are_ordered_by_reservation() {
        use std::sync::Barrier;
        use std::thread;

        const THREADS: usize = 8;
        const ROUNDS: u64 = 2_000;
        const PER_ROUND: usize = 10;
        const G: u64 = CLAMP_WINDOW_MS - DRIFT_LIMIT_MS;

        let stream = AtomicU64::new(0);
        let oracle = MaxOracle(Mutex::new(0));
        let start = Barrier::new(THREADS);
        let end = Barrier::new(THREADS);

        let (mut all, violations): (Vec<(u128, u128)>, Vec<String>) = thread::scope(|scope| {
            let workers: Vec<_> = (0..THREADS)
                .map(|t| {
                    let (stream, oracle, start, end) = (&stream, &oracle, &start, &end);
                    scope.spawn(move || {
                        let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ (t as u64 + 1);
                        let mut rand = move || {
                            x ^= x << 13;
                            x ^= x >> 7;
                            x ^= x << 17;
                            x
                        };
                        // A worker never panics: one that did would leave the others
                        // waiting at a barrier, so a violation is recorded and reported
                        // after the join.
                        let mut mine = Vec::with_capacity(ROUNDS as usize * PER_ROUND);
                        let mut violations = Vec::new();
                        let mut last_mine = 0u128;
                        for round in 0..ROUNDS {
                            // The round's clock is fixed: it moves only between the barriers.
                            let clock = T + round;
                            start.wait();
                            for _ in 0..PER_ROUND {
                                let k = [1, 2, 64][(rand() % 3) as usize];
                                let m = clock - rand() % (G + 1);
                                let r = rand() as u32;
                                // The floor is read before reserving, never after.
                                let floor = oracle.before();
                                let got = match reserve(stream, m, k, r) {
                                    Ok(got) => got,
                                    Err(e) => {
                                        violations.push(format!("m = {m}, k = {k}: {e:?}"));
                                        continue;
                                    }
                                };
                                let (first, last) = (key(&got, 0), key(&got, k - 1));
                                if got.next.is_none() {
                                    violations.push(format!("m = {m} fell out of stream"));
                                }
                                if first <= floor {
                                    violations.push(format!("{first:#x} <= floor {floor:#x}"));
                                }
                                if first <= last_mine {
                                    violations.push(format!(
                                        "thread {t}: {first:#x} <= its own {last_mine:#x}"
                                    ));
                                }
                                oracle.after(last);
                                last_mine = last;
                                mine.push((first, last));
                            }
                            end.wait();
                        }
                        (mine, violations)
                    })
                })
                .collect();
            let (mut all, mut violations) = (Vec::new(), Vec::new());
            for worker in workers {
                let (mine, theirs) = worker.join().unwrap();
                all.extend(mine);
                violations.extend(theirs);
            }
            (all, violations)
        });
        assert!(
            violations.is_empty(),
            "{} violations, first: {:?}",
            violations.len(),
            &violations[..violations.len().min(5)]
        );

        // Every key is distinct: no two reservations overlap.
        assert_eq!(all.len(), THREADS * ROUNDS as usize * PER_ROUND);
        all.sort_unstable();
        for pair in all.windows(2) {
            assert!(
                pair[1].0 > pair[0].1,
                "{:#x?} overlaps {:#x?}",
                pair[0],
                pair[1]
            );
        }
    }

    // Contention where the stream splits: started 100 steps short of the end of T, every
    // thread reserves 2^18 steps at T, so the swaps race across a split into the next ms every
    // fourth reservation, on up to the drift limit, where the rest fail with CounterExhausted.
    // (64 steps a call would consume less than one ms in all 8,000 attempts and never reach
    // the limit.) No barriers: each thread runs until it fails or its attempts run out, and
    // everything is asserted after the join.
    #[cfg(not(target_os = "wasi"))]
    #[test]
    fn contended_reservations_split_and_exhaust_without_overlap() {
        const THREADS: usize = 8;
        const ATTEMPTS: usize = 1_000;
        const K: u32 = 1 << 18;

        let stream = AtomicU64::new(encode(T, STEPS - 100));
        type Run = (Vec<(u128, u128, u64)>, Option<NewV7Error>);
        let runs: Vec<Run> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..THREADS)
                .map(|t| {
                    let stream = &stream;
                    scope.spawn(move || {
                        let r = 0x9E37_79B9u32.wrapping_mul(t as u32 + 1);
                        let mut mine = Vec::new();
                        for _ in 0..ATTEMPTS {
                            match reserve(stream, T, K, r) {
                                // An out-of-stream result has no word: u64::MAX fails the
                                // final-word check below.
                                Ok(got) => mine.push((
                                    key(&got, 0),
                                    key(&got, K - 1),
                                    got.next.unwrap_or(u64::MAX),
                                )),
                                Err(e) => return (mine, Some(e)),
                            }
                        }
                        (mine, None)
                    })
                })
                .collect();
            workers.into_iter().map(|w| w.join().unwrap()).collect()
        });

        let errors: Vec<_> = runs.iter().filter_map(|(_, e)| *e).collect();
        assert!(!errors.is_empty(), "no thread reached CounterExhausted");
        assert!(
            errors.iter().all(|&e| e == NewV7Error::CounterExhausted),
            "{errors:?}"
        );
        let mut all: Vec<_> = runs.into_iter().flat_map(|(mine, _)| mine).collect();
        all.sort_unstable();
        for pair in all.windows(2) {
            assert!(
                pair[1].0 > pair[0].1,
                "{:#x?} overlaps {:#x?}",
                pair[0],
                pair[1]
            );
        }
        // A split: the reservation's last key sits in the ms after its first.
        let split = all
            .iter()
            .any(|&(first, last, _)| last >> 20 == (first >> 20) + 1);
        assert!(split, "no reservation split into the next ms");
        let last = stream.load(Relaxed);
        assert_eq!(Some(last), all.iter().map(|&(_, _, next)| next).max());
        assert!(decode(last).unwrap().0 <= T + DRIFT_LIMIT_MS);
    }
}
