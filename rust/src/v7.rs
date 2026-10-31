//! RFC 9562 Section 6.2 Method 1 — UUID version 7: time-ordered; strictly increasing
//! within one process's stream (see [`new_v7`]).

#[cfg(target_has_atomic = "64")]
use crate::stream::{self, STEPS};
use crate::{Layout, SqlOrderError, Timestamp, Uuid, sql};
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::AtomicU64;

/// Largest Unix-epoch millisecond timestamp that fits the 48-bit `unix_ts_ms` field
/// (valid until the year 10889).
pub const MAX_UNIX_MILLIS: u64 = 0x0000_FFFF_FFFF_FFFF;

/// 26-bit counter mask (67,108,864 values) spanning `rand_a` and the top of `rand_b`.
const COUNTER_MASK: u32 = 0x03FF_FFFF;

/// The most UUIDs one [`new_v7_batch`] call mints: the size of the 26-bit counter space
/// (67,108,864), every one of the 2^20 steps a millisecond holds, 64 UUIDs to a step. A larger
/// batch would have to reuse counter values within a single millisecond, so it is refused with
/// [`NewV7Error::BatchTooLarge`]. On a target without 64-bit atomics, where the counter is one
/// never-reset sequence, a batch this size or smaller crosses its wrap at most once (see
/// [`new_v7_batch`]).
pub const MAX_BATCH: u32 = COUNTER_MASK + 1;

/// Random octets each version 7 UUID needs: `rand_b`'s trailing 48 bits (octets 10-15).
const RAND_BYTES_PER_ITEM: usize = 6;

/// An error returned when minting a version 7 UUID fails.
///
/// Non-exhaustive, so a failure mode added later is not a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewV7Error {
    /// `unix_millis` was negative-equivalent-out-of-range or exceeded [`MAX_UNIX_MILLIS`].
    TimestampOutOfRange,
    /// The system's random source failed while generating `rand_a`/`rand_b`.
    Random(getrandom::Error),
    /// The batch output buffer is shorter than `count * 16` bytes.
    BufferTooSmall,
    /// The batch `count` exceeds [`MAX_BATCH`].
    BatchTooLarge,
    /// The process-wide v7 stream cannot advance without running more than 1000 ms ahead of
    /// the supplied timestamp or past its last millisecond.
    CounterExhausted,
}

impl core::fmt::Display for NewV7Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TimestampOutOfRange => {
                write!(f, "unix millisecond timestamp must fit within 48 bits")
            }
            Self::Random(e) => write!(f, "random source failed: {e}"),
            Self::BufferTooSmall => write!(f, "output buffer is shorter than count * 16 bytes"),
            Self::BatchTooLarge => write!(
                f,
                "batch count exceeds the 26-bit counter space ({MAX_BATCH} per call)"
            ),
            Self::CounterExhausted => write!(
                f,
                "version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond"
            ),
        }
    }
}

impl core::error::Error for NewV7Error {}

// The process-wide version 7 stream (RFC 9562 §6.2 Method 1, a Fixed Bit-Length Dedicated
// Counter, re-initialised when the timestamp advances): one word, `(ms + 1) << 20 | last_step`,
// that every v7 reservation is a compare-and-swap on. A UUID's 26-bit counter is the step it
// reserved above 6 random low bits, so UUIDs from the stream sort in reservation order on any
// thread. 0 is the unseeded state, so first use needs no flag and no lock; `stream.rs` has the
// transitions and why each holds. Only `new_v7` and `new_v7_batch` name it: tests mint through
// `new_v7_in` and `new_v7_batch_in` on a stream of their own.
#[cfg(target_has_atomic = "64")]
static STREAM: AtomicU64 = AtomicU64::new(0);

/// Creates a new UUID version 7 from an explicit Unix-epoch millisecond timestamp.
///
/// The timestamp is supplied by the caller rather than read from the clock, so this
/// function has no platform-specific time dependency and works identically compiled
/// natively or to `wasm32`.
///
/// Each call reserves the next step of one process-wide stream, which is re-seeded at random
/// whenever a timestamp moves it forward, so counters don't count UUIDs across milliseconds.
/// On a target with 64-bit atomics, for timestamps up to 2^44 − 2 (the year 2527):
///
/// - A call whose timestamp is at most 11 s behind the stream's millisecond is clamped onto
///   it, and a millisecond whose 2^20 steps run out splits into the next. So a call within 10 s
///   of the newest timestamp the stream has reserved at sorts after every stream UUID whose
///   call completed before it began, on any thread; overlapping calls are ordered by their
///   reservation. The embedded timestamp then runs at most 11 s ahead of the supplied one.
/// - A timestamp further behind, or past 2^44 − 2, is minted at itself, out of stream: it
///   carries its own timestamp exactly, leaves the stream alone and is ordered against
///   nothing. One call far in the future therefore leaves real-time calls out of stream until
///   the clock catches up.
/// - A split that would take the stream more than 1000 ms ahead of the supplied timestamp, or
///   past 2^44 − 2, fails with [`NewV7Error::CounterExhausted`] and reserves nothing.
///
/// Within one millisecond the steps between two stream UUIDs are exactly the steps reserved
/// between them; across a re-seed they show nothing. Without 64-bit atomics (`thumbv7em-none-eabi`
/// and the like) none of this applies: the counter is one never-reset sequence (see
/// [`new_v7_batch`] for its wrap).
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v7(unix_millis: u64) -> Result<Uuid, NewV7Error> {
    #[cfg(target_has_atomic = "64")]
    let mint = |millis| new_v7_in(&STREAM, millis, crate::entropy::fill_at);
    #[cfg(not(target_has_atomic = "64"))]
    let mint = fallback::new_v7;
    mint(unix_millis)
}

/// [`new_v7`] on `stream`, with entropy drawn through `fill`: the public function is this on
/// the process-wide stream and the real entropy source, and a test hands it its own of each.
///
/// Entropy first, then the reservation, then the write: a failed draw returns before anything
/// is reserved, and a failed reservation installs nothing, so every failure leaves `stream` as
/// it was.
#[cfg(target_has_atomic = "64")]
#[inline(always)]
pub(crate) fn new_v7_in(
    stream: &AtomicU64,
    unix_millis: u64,
    fill: impl FnOnce(&mut [u8], u64) -> Result<(), getrandom::Error>,
) -> Result<Uuid, NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }

    // Octets 5-15 in one 11-byte draw: the seed draw from octets 5-8 and the 6 random low
    // counter bits from octet 9, both of which `write_fields` then overwrites, and rand_b's
    // random tail in octets 10-15. Not all 16: on macOS a draw of 12 bytes or more costs about
    // 100 ns more than one of 11 (entropy.rs has the measurement).
    let mut bytes = [0u8; 16];
    fill(&mut bytes[5..], unix_millis).map_err(NewV7Error::Random)?;
    let r = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
    let low6 = u32::from(bytes[9] & 0x3F);

    let got = stream::reserve(stream, unix_millis, 1, r)?;
    let (millis, step) = place(got.ms, got.first_step);
    write_fields(&mut bytes, millis, (step << 6) | low6);
    Ok(Uuid::from_bytes(bytes))
}

/// The ms and step of step `at` of a reservation whose first ms is `ms`: at `ms` while `at` is
/// below [`STEPS`], and past it in the next ms, a split's tail.
#[cfg(target_has_atomic = "64")]
#[inline(always)]
fn place(ms: u64, at: u32) -> (u64, u32) {
    if at < STEPS {
        (ms, at)
    } else {
        (ms + 1, at - STEPS)
    }
}

/// Writes octets 0-9 of a version 7 UUID — timestamp, version, counter and variant — around
/// the random tail already in octets 10-15. The one deterministic piece of [`new_v7`], kept
/// separate so the corpus (`corpus/v7_layout.json`) can pin every field's placement without
/// going through the random source.
#[inline]
pub(crate) fn write_fields(bytes: &mut [u8; 16], unix_millis: u64, counter: u32) {
    let counter = counter & COUNTER_MASK;

    // unix_ts_ms: 48-bit big-endian millisecond timestamp (octets 0-5).
    bytes[0] = (unix_millis >> 40) as u8;
    bytes[1] = (unix_millis >> 32) as u8;
    bytes[2] = (unix_millis >> 24) as u8;
    bytes[3] = (unix_millis >> 16) as u8;
    bytes[4] = (unix_millis >> 8) as u8;
    bytes[5] = unix_millis as u8;

    // The version nibble (0111) over rand_a, the upper 12 bits of the 26-bit counter
    // (octets 6-7).
    bytes[6] = 0x70 | (counter >> 22) as u8;
    bytes[7] = ((counter >> 14) & 0xFF) as u8;

    // The variant (10) over the rand_b extension, the lower 14 bits of the counter
    // (octets 8-9).
    bytes[8] = 0x80 | ((counter >> 8) & 0x3F) as u8;
    bytes[9] = (counter & 0xFF) as u8;
}

/// Creates a new UUID version 7 from a [`Timestamp`] instead of a raw millisecond count —
/// pulls the Unix-epoch milliseconds off `timestamp` and mints it through [`new_v7`], so it's
/// the exact same UUID [`new_v7(timestamp.to_unix_millis())`](new_v7) would produce.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v7_at(timestamp: Timestamp) -> Result<Uuid, NewV7Error> {
    new_v7(timestamp.to_unix_millis())
}

/// Creates `count` time-sortable UUID version 7 values sharing one Unix-epoch millisecond
/// timestamp capture, writing 16 bytes each into consecutive slots of `out` (which must be at
/// least `count * 16` bytes long; anything past that is left untouched).
///
/// Reserves ⌈count / 64⌉ consecutive steps of the stream up front, in one compare-and-swap
/// instead of `count`, and gives each step's 64 counter values to its items in order, so the
/// batch is contiguous and strictly increasing. Every UUID's random tail is drawn in one go,
/// into the caller's own buffer with no scratch space at all — not on the heap and not on the
/// stack. That is what lets this crate build with no allocator rather than merely without
/// `std`. The draw is one `getrandom` call, except that on Linux and Android a batch of 32 or
/// more is drawn as one 32-byte `getrandom` call keying a ChaCha20 keystream, the construction
/// the kernel uses itself, about 2.5 times faster there.
///
/// A `count` of 0 is a no-op success. Same errors as [`new_v7`], plus
/// [`NewV7Error::BufferTooSmall`] when `out` is shorter than `count * 16` bytes and
/// [`NewV7Error::BatchTooLarge`] when `count` exceeds [`MAX_BATCH`]; on those two, and on an
/// out-of-range timestamp, nothing is written. The entropy is drawn before the reservation and
/// before any item is assembled, so on [`NewV7Error::Random`] or
/// [`NewV7Error::CounterExhausted`] no UUID has been written at all — but the front of `out`
/// may hold entropy from the draw, so treat the buffer as clobbered rather than untouched.
///
/// A batch orders against the stream as one call (see [`new_v7`]). One that doesn't fit in
/// what is left of the stream's millisecond splits into the next, its tail stamped a
/// millisecond later, and the stream moves on with it, so the next call sorts after that tail.
/// An out-of-stream batch draws a seed that leaves room for all of it, so it never splits and
/// every item carries `unix_millis` itself.
///
/// Without 64-bit atomics the counter is one process-wide sequence that wraps every 2^26
/// values, so a batch can straddle the wrap; the items from the wrap on are stamped
/// `unix_millis + 1` rather than letting them sort before the items ahead of them (RFC 9562
/// §6.2 Method 1 lets the timestamp run ahead on counter overflow), and a batch that would roll
/// forward from [`MAX_UNIX_MILLIS`] fails with [`NewV7Error::TimestampOutOfRange`]. That
/// roll-forward orders one batch, not the sequence: two individual calls in the same
/// millisecond either side of the wrap sort in reverse, and the next batch or call in the same
/// real millisecond sorts before the previous batch's tail, stamped a millisecond later.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v7_batch(unix_millis: u64, count: u32, out: &mut [u8]) -> Result<(), NewV7Error> {
    #[cfg(target_has_atomic = "64")]
    let mint = |millis, count, out: &mut [u8]| {
        new_v7_batch_in(&STREAM, millis, count, out, crate::entropy::fill_at)
    };
    #[cfg(not(target_has_atomic = "64"))]
    let mint = fallback::new_v7_batch;
    mint(unix_millis, count, out)
}

/// [`new_v7_batch`] on `stream`, with entropy drawn through `fill`, the way [`new_v7_in`] is
/// [`new_v7`]: entropy, then the reservation, then the writes, so a failure leaves `stream` as
/// it was.
#[cfg(target_has_atomic = "64")]
#[inline(always)]
pub(crate) fn new_v7_batch_in(
    stream: &AtomicU64,
    unix_millis: u64,
    count: u32,
    out: &mut [u8],
    fill: impl FnOnce(&mut [u8], u64) -> Result<(), getrandom::Error>,
) -> Result<(), NewV7Error> {
    let Some(out) = batch_region(unix_millis, count, out)? else {
        return Ok(());
    };

    // Every item's tail, packed at the front of `out` the way `fill_batch` takes it, and the
    // seed draw in the 4 bytes after them, read before the walk overwrites them. `out` is
    // `count * 16` bytes, and `count * 6 + 4` is never more for a `count` of 1 or more.
    let packed = count as usize * RAND_BYTES_PER_ITEM;
    fill(&mut out[..packed + 4], unix_millis).map_err(NewV7Error::Random)?;
    let r = u32::from_le_bytes([
        out[packed],
        out[packed + 1],
        out[packed + 2],
        out[packed + 3],
    ]);

    // One step per 64 items: 1 to 2^20 steps, for a `count` of 1 to MAX_BATCH.
    let got = stream::reserve(stream, unix_millis, count.div_ceil(64), r)?;
    fill_batch(count, out, |i| {
        let (millis, step) = place(got.ms, got.first_step + i / 64);
        (millis, (step << 6) | (i % 64))
    });
    Ok(())
}

/// Checks a batch's arguments, in the order they have always been checked, and narrows `out`
/// to exactly the `count * 16` bytes the call owns: `None` for a `count` of 0, which owns none.
/// None of these failures writes anything.
#[inline(always)]
fn batch_region(
    unix_millis: u64,
    count: u32,
    out: &mut [u8],
) -> Result<Option<&mut [u8]>, NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }
    if count == 0 {
        return Ok(None);
    }
    if count > MAX_BATCH {
        return Err(NewV7Error::BatchTooLarge);
    }

    // Narrowed once, up front, so the entropy fill and the per-item writes both stay inside
    // exactly the region this call owns. That matters because the fill writes through `out`
    // itself, and a caller's oversized buffer must keep its tail untouched. A buffer shorter
    // than that region is an error rather than an out-of-bounds panic, and `checked_mul`
    // because `u32::MAX * 16` overflows a 32-bit `usize`.
    (count as usize)
        .checked_mul(16)
        .and_then(|len| out.get_mut(..len))
        .map(Some)
        .ok_or(NewV7Error::BufferTooSmall)
}

/// Assembles a batch in place: item `i` gets the ms and 26-bit counter `item(i)` returns, around
/// the random tail already packed for it at the front of `out`. The one walk both counters
/// share, the stream's and the fallback's, each passing its own rule. `out` is exactly
/// `count * 16` bytes and `count` is 1 to [`MAX_BATCH`].
///
/// There is no scratch buffer of any kind: not a heap one (this crate has no allocator to get
/// it from) and not a fixed stack one either (a frame big enough to be worth the syscalls it
/// saves is a poor thing to charge a microcontroller for, where an overflow corrupts silently).
/// So the caller draws the entropy into the *front* of the caller's own `out`, packed 6 bytes
/// per item, and each item's share is moved out to its final octets as that item is written.
///
/// That works in place because the packed entropy always sits to the left of where it is
/// going: item i's 6 bytes are at 6i but belong at 16i+10. So the 16 bytes written for item
/// i can only ever land on entropy belonging to items at index >= i — anything from 16i
/// onwards is item 2i's share or later. Walking the batch backwards therefore only
/// overwrites entropy that has already been consumed, and item i's own share is moved
/// before its own 16 bytes are written. Hence `.rev()`, which is load-bearing, not taste.
///
/// Two walks write the same bytes, one per kind of target, because the cheapest way to write
/// a big-endian field differs: [`fill_batch_stores`] everywhere but WebAssembly, and
/// [`fill_batch_words`] on it. Both are compiled for tests, which hold them to each other.
/// `--cfg hyperuuid_words_walk` selects the WebAssembly walk on any target, for the no-panic
/// proof: WebAssembly builds abort on panic, and `no-panic` sees only an unwinding one, so
/// that walk is proved on the host, the way the rest of the WebAssembly build is.
///
/// `inline(always)`, not a hint, here and on both walks: the indexing in them is proven in
/// bounds only by [`batch_region`]'s narrowing of `out` to `count * 16`, so it has to be
/// optimized as one function for `no-panic` to see that proof (without it the check fails the
/// link).
#[inline(always)]
fn fill_batch(count: u32, out: &mut [u8], item: impl Fn(u32) -> (u64, u32)) {
    #[cfg(not(any(target_family = "wasm", hyperuuid_words_walk)))]
    fill_batch_stores(count, out, item);
    #[cfg(any(target_family = "wasm", hyperuuid_words_walk))]
    fill_batch_words(count, out, item);
}

/// [`fill_batch`] off WebAssembly: each item's share moved out with one 6-byte copy, then
/// octets 0-9 as a big-endian u64 and a big-endian u16. A real CPU swaps bytes in one
/// instruction (or none, folded into the store), and this was measured faster natively than
/// [`fill_batch_words`]: 5.2 against 5.7 µs for 1000 items on Linux x86-64, 3.45 against 3.9
/// on Windows (i9-11900H, 2026-10-30).
#[cfg(any(test, not(any(target_family = "wasm", hyperuuid_words_walk))))]
#[inline(always)]
pub(crate) fn fill_batch_stores(count: u32, out: &mut [u8], item: impl Fn(u32) -> (u64, u32)) {
    for i in (0..count as usize).rev() {
        let src = i * RAND_BYTES_PER_ITEM;
        // rand_b's trailing 48 bits (octets 10-15), straight from the packed entropy.
        out.copy_within(src..src + RAND_BYTES_PER_ITEM, i * 16 + 10);

        let (millis, counter_val) = item(i as u32);
        let counter_val = counter_val & COUNTER_MASK;
        let slot = &mut out[i * 16..(i + 1) * 16];

        // Octets 0-9 as two big-endian stores rather than ten single-byte ones:
        // unix_ts_ms in the top 48 bits (octets 0-5), then the version nibble (0111) and
        // rand_a, the upper 12 bits of the 26-bit counter (octets 6-7)...
        let head = (millis << 16) | 0x7000 | u64::from(counter_val >> 14);
        slot[..8].copy_from_slice(&head.to_be_bytes());
        // ...then the variant (10) and the lower 14 bits of the counter (octets 8-9).
        let tail = 0x8000 | (counter_val & 0x3FFF) as u16;
        slot[8..10].copy_from_slice(&tail.to_be_bytes());
    }
}

/// [`fill_batch`] on WebAssembly, which has no byte-swap instruction: every big-endian store
/// there is a chain of shifts and masks, and a small copy is a `memory.copy`. So each item is
/// one 8-byte load and two 8-byte stores, the words built in little-endian order from the same
/// byte expressions as [`write_fields`], with no byte swap per item at all. Measured through
/// the core translated to Go by wasm2go: 7.7 against 5.4 µs for 1000 items on Linux x86-64,
/// 7.5 against 5.6 on Windows (i9-11900H, 2026-10-30; `walk-ab.sh` in the spike in
/// `docs/superpowers/spikes/2026-10-30-go-cgo-free/`).
///
/// The load takes item i's 6 bytes and the 2 after them, which are item i+1's (already
/// consumed) or, for the last item, the seed draw's (the region always has them: `count * 6 +
/// 4` bytes of entropy), and the 2 are shifted out. It is read before the stores, which for
/// item 0 overlap it. The timestamp's six octets change at most once in a batch (a split's
/// tail), so they are rebuilt only when the ms does.
#[cfg(any(test, target_family = "wasm", hyperuuid_words_walk))]
#[inline(always)]
pub(crate) fn fill_batch_words(count: u32, out: &mut [u8], item: impl Fn(u32) -> (u64, u32)) {
    // unix_ts_ms as octets 0-5 of a little-endian word, and the ms it was built from. No
    // item's ms is u64::MAX (it is at most one past MAX_UNIX_MILLIS), so the first item
    // builds it.
    let (mut stamped, mut stamp) = (u64::MAX, 0);
    for i in (0..count as usize).rev() {
        let src = i * RAND_BYTES_PER_ITEM;
        let mut packed = [0u8; 8];
        packed.copy_from_slice(&out[src..src + 8]);

        let (millis, counter_val) = item(i as u32);
        let counter_val = counter_val & COUNTER_MASK;
        if millis != stamped {
            stamped = millis;
            stamp = u64::from_le_bytes((millis << 16).to_be_bytes());
        }

        // Octets 0-7: unix_ts_ms, then the version nibble (0111) over rand_a, the upper 12
        // bits of the 26-bit counter.
        let head = stamp
            | u64::from(0x70 | (counter_val >> 22)) << 48
            | u64::from((counter_val >> 14) & 0xFF) << 56;
        // Octets 8-15: the variant (10) over the lower 14 bits of the counter, then rand_b's
        // trailing 48 bits, straight from the packed entropy.
        let tail = u64::from(0x80 | ((counter_val >> 8) & 0x3F))
            | u64::from(counter_val & 0xFF) << 8
            | u64::from_le_bytes(packed) << 16;

        let slot = &mut out[i * 16..(i + 1) * 16];
        slot[..8].copy_from_slice(&head.to_le_bytes());
        slot[8..].copy_from_slice(&tail.to_le_bytes());
    }
}

/// The version 7 counter on targets without 64-bit atomics, where the stream's word can't be
/// swapped as one: one process-wide 26-bit sequence, seeded once, never reset, wrapping every
/// 2^26 values. Compiled for tests too, so its wrap rule stays tested on a host that has 64-bit
/// atomics and so never uses it.
#[cfg(any(test, not(target_has_atomic = "64")))]
mod fallback {
    use super::*;
    use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    // Process-global monotonic counter (RFC 9562 §6.2 Method 1 — Fixed Bit-Length Dedicated
    // Counter), seeded randomly on first use and advanced with fetch_add. This guarantees sort
    // order for UUIDs minted within the same millisecond regardless of caller concurrency, up
    // to the wrap.
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    // Whether the one-shot random seed has been claimed yet. A separate flag rather than
    // testing COUNTER against a sentinel, because 0 is a legitimate seed draw and the counter
    // wraps back through 0 on its own — neither would distinguish "unseeded" from "seeded".
    static SEED_CLAIMED: AtomicBool = AtomicBool::new(false);

    /// Returns the shared counter, folding in the one-shot random seed on the first call.
    ///
    /// `OnceLock` is the obvious tool and is what this used, but it's std-only and this crate
    /// is `#![no_std]` without its default `std` feature. What replaces it has to preserve the
    /// one property the counter exists for: the values `fetch_add` hands out must never go
    /// backwards, including while a seeding race is in flight — a regression there is a silent
    /// ordering bug, not a build failure.
    ///
    /// It holds because the winner of the `SEED_CLAIMED` race *adds* the seed instead of
    /// storing it. Addition commutes with the concurrent `fetch_add(1)` of a thread that read
    /// the flag before it flipped, so every increment already handed out survives and the
    /// running total only ever grows; a `store` would not be safe here, since it could roll the
    /// counter back over an increment another thread had already minted a UUID from. Exactly
    /// one thread ever draws a seed (`compare_exchange`), and a thread that loses simply
    /// proceeds — no spinning, nothing to block on, which is also what makes this usable on a
    /// bare-metal target.
    ///
    /// The one visible difference from the blocking `OnceLock` version: a caller racing the
    /// very first seeding can draw a counter value from below the seed. That's harmless — the
    /// seed is wrap headroom, not a uniqueness or ordering input.
    fn counter() -> &'static AtomicU32 {
        if !SEED_CLAIMED.load(Ordering::Relaxed)
            && SEED_CLAIMED
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            // Seed in [0, 512) to leave ample headroom before the 26-bit wrap, mirroring the
            // C# SequentialGuid implementation this is ported from.
            let seed = getrandom::u32().unwrap_or(0) & 0x1FF;
            COUNTER.fetch_add(seed, Ordering::Relaxed);
        }
        &COUNTER
    }

    /// [`super::new_v7`] on the fallback counter.
    pub(super) fn new_v7(unix_millis: u64) -> Result<Uuid, NewV7Error> {
        if unix_millis > MAX_UNIX_MILLIS {
            return Err(NewV7Error::TimestampOutOfRange);
        }

        // Claim a unique, strictly increasing counter slot.
        let counter_val = counter().fetch_add(1, Ordering::Relaxed).wrapping_add(1) & COUNTER_MASK;
        single(unix_millis, counter_val)
    }

    /// The rest of [`new_v7`] once its counter value is claimed: separate so a test can mint
    /// at a chosen counter.
    #[inline(always)]
    pub(super) fn single(unix_millis: u64, counter_val: u32) -> Result<Uuid, NewV7Error> {
        let mut bytes = [0u8; 16];
        crate::entropy::fill_at(&mut bytes[10..], unix_millis).map_err(NewV7Error::Random)?;
        write_fields(&mut bytes, unix_millis, counter_val);
        Ok(Uuid::from_bytes(bytes))
    }

    /// [`super::new_v7_batch`] on the fallback counter.
    pub(super) fn new_v7_batch(
        unix_millis: u64,
        count: u32,
        out: &mut [u8],
    ) -> Result<(), NewV7Error> {
        let Some(out) = batch_region(unix_millis, count, out)? else {
            return Ok(());
        };

        // Reserves [base+1, base+count] in this one call, continuing the same global sequence
        // a series of individual fetch_add(1) calls would have produced (matching new_v7's own
        // base.wrapping_add(1) convention).
        let base = counter().fetch_add(count, Ordering::Relaxed);
        fill(unix_millis, count, out, base)
    }

    /// The rest of [`new_v7_batch`], once its counter block `[base+1, base+count]` is
    /// reserved: separate so a test can hand it a block that straddles the wrap without first
    /// minting 67 million UUIDs to get the shared counter there. `out` is exactly `count * 16`
    /// bytes and `count` is 1 to [`MAX_BATCH`].
    #[inline(always)]
    pub(super) fn fill(
        unix_millis: u64,
        count: u32,
        out: &mut [u8],
        base: u32,
    ) -> Result<(), NewV7Error> {
        // Items before `wrap_at` count up to the top of the 26-bit space; the item at
        // `wrap_at` comes back round to 0 and would sort before them, so it and everything
        // after it move a millisecond on. A block that starts at 0 has nothing before it to
        // wrap from, and gives MAX_BATCH, which no count reaches past. (`first` is at most
        // COUNTER_MASK, so no overflow.)
        let first = base.wrapping_add(1) & COUNTER_MASK;
        let wrap_at = MAX_BATCH - first;
        if wrap_at < count && unix_millis == MAX_UNIX_MILLIS {
            return Err(NewV7Error::TimestampOutOfRange);
        }

        // One entropy draw for the whole batch (entropy.rs), packed at the front of `out` the
        // way `fill_batch` takes it.
        crate::entropy::fill_at(
            &mut out[..count as usize * RAND_BYTES_PER_ITEM],
            unix_millis,
        )
        .map_err(NewV7Error::Random)?;

        fill_batch(count, out, |i| {
            let millis = if i < wrap_at {
                unix_millis
            } else {
                unix_millis + 1
            };
            (millis, base.wrapping_add(1 + i) & COUNTER_MASK)
        });
        Ok(())
    }
}

/// Creates a new UUID version 7 using the current system time.
///
/// Not available on `wasm32` targets, which have no OS clock, nor without this crate's
/// default `std` feature, which is the same situation one step further out — no OS at all to
/// read a clock from. Call [`new_v7`] there with a timestamp supplied by the host instead.
#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn now_v7() -> Result<Uuid, NewV7Error> {
    // A clock set before 1970, or one so far ahead its milliseconds overflow a u64, is out of
    // range rather than a panic — the same answer new_v7 gives a timestamp past the field.
    new_v7(system_millis().ok_or(NewV7Error::TimestampOutOfRange)?)
}

/// The wall clock in Unix-epoch milliseconds, or `None` if it can't be read or is out of range.
///
/// std's `SystemTime::now` unwraps `clock_gettime` on Unix, a panic path `#[no_panic]` can't
/// see past, so the call is made directly and a failure comes back as `None` instead.
#[cfg(all(feature = "std", unix, not(target_arch = "wasm32")))]
fn system_millis() -> Option<u64> {
    // SAFETY: `timespec` is plain integers, for which all zeroes is a valid value, and
    // `clock_gettime` writes only through the pointer it is given.
    let mut ts: libc::timespec = unsafe { core::mem::zeroed() };
    if unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) } != 0 {
        return None;
    }
    let secs = u64::try_from(ts.tv_sec).ok()?;
    let nanos = u64::try_from(ts.tv_nsec).ok()?;
    secs.checked_mul(1000)?.checked_add(nanos / 1_000_000)
}

/// The wall clock in Unix-epoch milliseconds, or `None` if it is out of range.
///
/// std's Windows clock read can't fail, so std is used as is; CI's `check-no-panic` job links
/// this on Windows too, which is what holds that up.
#[cfg(all(feature = "std", not(unix), not(target_arch = "wasm32")))]
fn system_millis() -> Option<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis()).ok()
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 7 UUID.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn unix_millis(uuid: &Uuid) -> u64 {
    let b = uuid.as_bytes();
    ((b[0] as u64) << 40)
        | ((b[1] as u64) << 32)
        | ((b[2] as u64) << 24)
        | ((b[3] as u64) << 16)
        | ((b[4] as u64) << 8)
        | (b[5] as u64)
}

/// [`unix_millis`] for a version 7 UUID held in `layout`'s byte order, reading a SQL-ordered
/// value's octets directly rather than converting it back first. Meaningful only for a genuine
/// version 7 UUID in that layout, the same as [`unix_millis`]; [`Uuid::is_rfc_in`] is the
/// check.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn unix_millis_in(uuid: &Uuid, layout: Layout) -> u64 {
    let b = uuid.as_bytes();
    match layout {
        Layout::Rfc9562 => unix_millis(uuid),
        // to_sql_order puts the timestamp first in the key, octets 10-15 unchanged.
        Layout::SqlServer => {
            ((b[10] as u64) << 40)
                | ((b[11] as u64) << 32)
                | ((b[12] as u64) << 24)
                | ((b[13] as u64) << 16)
                | ((b[14] as u64) << 8)
                | (b[15] as u64)
        }
    }
}

/// Converts an RFC 9562 version 7 UUID to SQL Server order: the version 8 layout [`sql`]
/// describes, whose `uniqueidentifier` sort is creation order.
///
/// The key is the 48-bit timestamp, then the 26-bit counter, then the 48 trailing random bits
/// as one block. So the timestamp lands in octets 10-15, the counter's top 14 bits in 8-9 and
/// its low 12 in 6-7, both split around the version and variant, and the random block in 4-5
/// and 0-3, relocated whole rather than reshuffled. Within a millisecond, values sort by
/// counter, which [`new_v7_batch`] and [`new_v7`] fill in reservation order.
///
/// The value is no longer a version 7: RFC 9562 defines a v7's layout, and §6.11 sends a
/// different byte order to version 8. So SQL Server's text form, and every `Guid` built from
/// these bytes, shows version 8. [`Uuid::version_in`] with [`Layout::SqlServer`] still
/// answers 7, and [`to_rfc_order`] gives back the exact original: the layout holds all 122 of
/// a v7's free bits, so every version 7 converts.
///
/// Refuses a value that isn't an RFC 9562 version 7 ([`SqlOrderError::WrongVersion`]).
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn to_sql_order(uuid: &Uuid) -> Result<Uuid, SqlOrderError> {
    if !uuid.is_rfc(7) {
        return Err(SqlOrderError::WrongVersion);
    }
    let millis = unix_millis(uuid);
    let rfc = uuid.as_bytes();
    let counter = ((rfc[6] as u32 & 0x0F) << 22)
        | ((rfc[7] as u32) << 14)
        | ((rfc[8] as u32 & 0x3F) << 8)
        | (rfc[9] as u32);
    let random = u64::from_be_bytes([0, 0, rfc[10], rfc[11], rfc[12], rfc[13], rfc[14], rfc[15]]);
    Ok(sql::scatter(
        (millis as u128) << 74 | (counter as u128) << 48 | random as u128,
    ))
}

/// Inverse of [`to_sql_order`]: converts a SQL-ordered value back to the RFC 9562 version 7
/// it came from. Refuses anything without the SQL layout's version 8 and variant
/// ([`SqlOrderError::WrongVersion`]), including 0.8.0's form of this layout, which kept a 7
/// where the 8 now is.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn to_rfc_order(uuid: &Uuid) -> Result<Uuid, SqlOrderError> {
    let sql = uuid.as_bytes();
    if sql::source_version(sql) != 7 {
        return Err(SqlOrderError::WrongVersion);
    }
    let key = sql::key(sql);
    let millis = (key >> 74) as u64;
    let counter = (key >> 48) as u32 & COUNTER_MASK;
    let random = (key as u64).to_be_bytes();

    let mut rfc = [0u8; 16];
    let millis = millis.to_be_bytes();
    rfc[0..6].copy_from_slice(&[
        millis[2], millis[3], millis[4], millis[5], millis[6], millis[7],
    ]);
    rfc[6] = 0x70 | ((counter >> 22) & 0x0F) as u8;
    rfc[7] = ((counter >> 14) & 0xFF) as u8;
    rfc[8] = 0x80 | ((counter >> 8) & 0x3F) as u8;
    rfc[9] = (counter & 0xFF) as u8;
    rfc[10..16].copy_from_slice(&[
        random[2], random[3], random[4], random[5], random[6], random[7],
    ]);
    Ok(Uuid::from_bytes(rfc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{self, STEPS};

    const MS: u64 = 1_750_000_000_123;

    fn items(out: &[u8]) -> Vec<Uuid> {
        let (chunks, _) = out.as_chunks::<16>();
        chunks.iter().map(|&c| Uuid::from_bytes(c)).collect()
    }

    fn counter_of(id: &Uuid) -> u32 {
        let b = id.as_bytes();
        ((b[6] as u32 & 0x0F) << 22)
            | ((b[7] as u32) << 14)
            | ((b[8] as u32 & 0x3F) << 8)
            | b[9] as u32
    }

    // A block straddling the wrap: counters 0x3FFFFFE and 0x3FFFFFF at MS, then 0, 1, 2 at
    // MS + 1, and the batch stays in order end to end.
    #[test]
    fn a_batch_across_the_counter_wrap_rolls_forward_a_millisecond() {
        let mut out = [0u8; 5 * 16];
        fallback::fill(MS, 5, &mut out, COUNTER_MASK - 2).unwrap();
        let ids = items(&out);
        let stamps: Vec<u64> = ids.iter().map(unix_millis).collect();
        let counters: Vec<u32> = ids.iter().map(counter_of).collect();
        assert_eq!(stamps, [MS, MS, MS + 1, MS + 1, MS + 1]);
        assert_eq!(counters, [COUNTER_MASK - 1, COUNTER_MASK, 0, 1, 2]);
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        assert!(ids.iter().all(|id| id.is_rfc(7)));
    }

    #[test]
    fn a_batch_that_starts_at_zero_or_ends_at_the_top_does_not_roll() {
        let mut out = [0u8; 3 * 16];
        fallback::fill(MS, 3, &mut out, u32::MAX).unwrap(); // base + 1 wraps to counter 0
        assert!(items(&out).iter().all(|id| unix_millis(id) == MS));
        fallback::fill(MS, 3, &mut out, COUNTER_MASK - 3).unwrap(); // last item is 0x3FFFFFF
        let ids = items(&out);
        assert!(ids.iter().all(|id| unix_millis(id) == MS));
        assert_eq!(counter_of(&ids[2]), COUNTER_MASK);
    }

    #[test]
    fn rolling_forward_past_the_last_millisecond_is_out_of_range() {
        let mut out = [0u8; 2 * 16];
        assert_eq!(
            fallback::fill(MAX_UNIX_MILLIS, 2, &mut out, COUNTER_MASK - 1),
            Err(NewV7Error::TimestampOutOfRange)
        );
        // The same batch without a wrap fits.
        fallback::fill(MAX_UNIX_MILLIS, 2, &mut out, 10).unwrap();
    }

    // The old caveat, pinned on the fallback's rule: a batch's rolled tail is stamped MS + 1,
    // and the next single takes the counter just past the block (3) at MS, so it sorts
    // before that tail.
    #[test]
    fn fallback_wrap_rule_tail_runs_ahead_of_the_next_call() {
        let mut out = [0u8; 5 * 16];
        let base = COUNTER_MASK - 2;
        fallback::fill(MS, 5, &mut out, base).unwrap();
        let tail = items(&out)[4];
        let next = base.wrapping_add(5 + 1) & COUNTER_MASK;
        assert_eq!(next, 3);
        let single = fallback::single(MS, next).unwrap();
        assert!(single < tail, "{single} >= {tail}");
    }

    // The same shape through the stream: a batch whose reservation splits into MS + 1, then a
    // single at MS, which clamps onto the stream the batch left at MS + 1 and sorts after it.
    // A MAX_BATCH, so that the same batch on the fallback's counter would cross the wrap.
    // Deliberately not `#[ignore]`d despite the 1 GiB allocation: only a MAX_BATCH block makes
    // the "route through the fallback" negative control go red, and every CI leg that runs
    // the core suite has the memory.
    #[test]
    fn stream_tail_never_runs_ahead_of_the_next_call() {
        let stream = AtomicU64::new(stream::encode(MS, STEPS - 2));
        let mut out = vec![0u8; MAX_BATCH as usize * 16];
        // Fake entropy: one draw this size is more than some hosts' random source takes at once
        // (wasmtime's caps a call at 64 MiB), and the order under test is the counter's.
        let fill = |buf: &mut [u8], _| {
            buf.fill(0x5A);
            Ok(())
        };
        new_v7_batch_in(&stream, MS, MAX_BATCH, &mut out, fill).unwrap();
        let tail = items(&out[out.len() - 16..])[0];
        assert_eq!(unix_millis(&tail), MS + 1);
        let single = new_v7_in(&stream, MS, fill).unwrap();
        assert!(single > tail, "{single} <= {tail}");
    }

    // The fallback end to end, on its own process-wide counter: a batch continues the same
    // sequence individual calls draw from, so all of them sort in call order.
    #[test]
    fn fallback_batch_continues_the_counter_sequence_of_individual_calls() {
        let before = fallback::new_v7(MS).unwrap();
        let mut batch = [0u8; 10 * 16];
        fallback::new_v7_batch(MS, 10, &mut batch).unwrap();
        let after = fallback::new_v7(MS).unwrap();
        let mut ids = vec![before];
        ids.extend(items(&batch));
        ids.push(after);
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
    }

    #[test]
    fn a_batch_past_the_counter_space_is_refused_before_the_buffer_is_checked() {
        let mut out = [0u8; 16];
        assert_eq!(
            new_v7_batch(MS, MAX_BATCH + 1, &mut out),
            Err(NewV7Error::BatchTooLarge)
        );
        assert_eq!(
            new_v7_batch(MS, u32::MAX, &mut out),
            Err(NewV7Error::BatchTooLarge)
        );
        assert_eq!(
            new_v7_batch(MS, MAX_BATCH, &mut out),
            Err(NewV7Error::BufferTooSmall)
        );
    }

    // The full counter space in one call: 1 GiB of output, so opt-in
    // (`cargo test --release -- --ignored`).
    #[test]
    #[ignore = "allocates 1 GiB"]
    fn a_batch_of_the_whole_counter_space_is_strictly_increasing() {
        let mut out = vec![0u8; MAX_BATCH as usize * 16];
        new_v7_batch(MS, MAX_BATCH, &mut out).unwrap();
        let (chunks, _) = out.as_chunks::<16>();
        assert!(chunks.windows(2).all(|w| w[0] < w[1]));
        let last = Uuid::from_bytes(out[out.len() - 16..].try_into().unwrap());
        assert!(unix_millis(&last) <= MS + 1);
    }
}
