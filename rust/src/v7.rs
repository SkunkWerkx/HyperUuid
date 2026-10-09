//! RFC 9562 Section 6.2 Method 1 — UUID version 7: time-ordered, monotonically increasing.

use crate::{Layout, Timestamp, Uuid};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Largest Unix-epoch millisecond timestamp that fits the 48-bit `unix_ts_ms` field
/// (valid until the year 10889).
pub const MAX_UNIX_MILLIS: u64 = 0x0000_FFFF_FFFF_FFFF;

/// 26-bit counter mask (67,108,864 values) spanning `rand_a` and the top of `rand_b`.
const COUNTER_MASK: u32 = 0x03FF_FFFF;

/// The most UUIDs one [`new_v7_batch`] call mints: the size of the 26-bit counter space
/// (67,108,864). A batch this size or smaller crosses the counter's wrap at most once, which
/// the batch absorbs by moving the timestamp forward a millisecond (see [`new_v7_batch`]); a
/// larger one would have to reuse counter values within a single millisecond, so it is
/// refused with [`NewV7Error::BatchTooLarge`].
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
        }
    }
}

impl core::error::Error for NewV7Error {}

// Process-global monotonic counter (RFC 9562 §6.2 Method 1 — Fixed Bit-Length Dedicated
// Counter), seeded randomly on first use and advanced with fetch_add. This guarantees sort
// order for UUIDs minted within the same millisecond regardless of caller concurrency.
static COUNTER: AtomicU32 = AtomicU32::new(0);

// Whether the one-shot random seed has been claimed yet. A separate flag rather than testing
// COUNTER against a sentinel, because 0 is a legitimate seed draw and the counter wraps back
// through 0 on its own — neither would distinguish "unseeded" from "seeded".
static SEED_CLAIMED: AtomicBool = AtomicBool::new(false);

/// Returns the shared counter, folding in the one-shot random seed on the first call.
///
/// `OnceLock` is the obvious tool and is what this used, but it's std-only and this crate is
/// `#![no_std]` without its default `std` feature. What replaces it has to preserve the one
/// property the counter exists for: the values `fetch_add` hands out must never go backwards,
/// including while a seeding race is in flight — a regression there is a silent ordering bug,
/// not a build failure.
///
/// It holds because the winner of the `SEED_CLAIMED` race *adds* the seed instead of storing
/// it. Addition commutes with the concurrent `fetch_add(1)` of a thread that read the flag
/// before it flipped, so every increment already handed out survives and the running total
/// only ever grows; a `store` would not be safe here, since it could roll the counter back
/// over an increment another thread had already minted a UUID from. Exactly one thread ever
/// draws a seed (`compare_exchange`), and a thread that loses simply proceeds — no spinning,
/// nothing to block on, which is also what makes this usable on a bare-metal target.
///
/// The one visible difference from the blocking `OnceLock` version: a caller racing the very
/// first seeding can draw a counter value from below the seed. That's harmless — the seed is
/// wrap headroom, not a uniqueness or ordering input.
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

/// Creates a new UUID version 7 from an explicit Unix-epoch millisecond timestamp.
///
/// The timestamp is supplied by the caller rather than read from the clock, so this
/// function has no platform-specific time dependency and works identically compiled
/// natively or to `wasm32`.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v7(unix_millis: u64) -> Result<Uuid, NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }

    // Claim a unique, strictly increasing counter slot.
    let counter_val = counter().fetch_add(1, Ordering::Relaxed).wrapping_add(1) & COUNTER_MASK;

    let mut bytes = [0u8; 16];
    crate::entropy::fill_at(&mut bytes[10..], unix_millis).map_err(NewV7Error::Random)?;
    write_fields(&mut bytes, unix_millis, counter_val);
    Ok(Uuid::from_bytes(bytes))
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
/// Reserves one contiguous block of `count` counter slots up front (one atomic op instead of
/// `count`) and draws every UUID's random tail in one go, into the caller's own buffer with no
/// scratch space at all — not on the heap and not on the stack. That is what lets this crate
/// build with no allocator rather than merely without `std`. The draw is one `getrandom` call,
/// except that on Linux and Android a batch of 32 or more is drawn as one 32-byte `getrandom`
/// call keying a ChaCha20 keystream, the construction the kernel uses itself, about 2.5 times
/// faster there.
///
/// A `count` of 0 is a no-op success. Same errors as [`new_v7`], plus
/// [`NewV7Error::BufferTooSmall`] when `out` is shorter than `count * 16` bytes. The entropy
/// is drawn before any item is assembled, so on [`NewV7Error::Random`] no UUID has been
/// written at all — but the front of `out` may hold partial entropy from the failed draw, so
/// treat the buffer as clobbered rather than untouched.
///
/// Every batch is in strictly increasing order, however it lands on the counter. The counter
/// is one process-wide sequence that wraps every 2^26 values, so a batch can straddle the
/// wrap; the items from the wrap on are stamped `unix_millis + 1` rather than letting them
/// sort before the items ahead of them (RFC 9562 §6.2 Method 1 lets the timestamp run ahead
/// on counter overflow), so an embedded timestamp is never more than a millisecond ahead of
/// the one supplied. A `count` past [`MAX_BATCH`] would have to cross the wrap twice and is
/// refused with [`NewV7Error::BatchTooLarge`], and a batch that would roll forward from
/// [`MAX_UNIX_MILLIS`] with [`NewV7Error::TimestampOutOfRange`]. Individual [`new_v7`] calls
/// don't roll forward: they share no state but the counter, so two calls in the same
/// millisecond either side of the wrap sort in reverse, as they always have. Nor does
/// anything here notice a clock that goes backwards; the timestamp is the caller's, trusted
/// as given.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v7_batch(unix_millis: u64, count: u32, out: &mut [u8]) -> Result<(), NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }
    if count == 0 {
        return Ok(());
    }
    if count > MAX_BATCH {
        return Err(NewV7Error::BatchTooLarge);
    }

    // Narrowed once, up front, so the entropy fill and the per-item writes below both stay
    // inside exactly the region this call owns. That matters more than it used to: the fill
    // now writes through `out` itself, and a caller's oversized buffer must keep its tail
    // untouched. A buffer shorter than that region is an error rather than an out-of-bounds
    // panic, and `checked_mul` because `u32::MAX * 16` overflows a 32-bit `usize`.
    let out = (count as usize)
        .checked_mul(16)
        .and_then(|len| out.get_mut(..len))
        .ok_or(NewV7Error::BufferTooSmall)?;

    // Reserves [base+1, base+count] in this one call, continuing the same global sequence a
    // series of individual fetch_add(1) calls would have produced (matching new_v7's own
    // base.wrapping_add(1) convention).
    let base = counter().fetch_add(count, Ordering::Relaxed);
    fill_batch(unix_millis, count, out, base)
}

/// The rest of [`new_v7_batch`], once its counter block `[base+1, base+count]` is reserved:
/// separate so a test can hand it a block that straddles the wrap without first minting
/// 67 million UUIDs to get the shared counter there. `out` is exactly `count * 16` bytes and
/// `count` is 1 to [`MAX_BATCH`].
///
/// `inline(always)`, not a hint: the indexing below is proven in bounds only by
/// [`new_v7_batch`]'s narrowing of `out` to `count * 16`, so it has to be optimized as one
/// function for `no-panic` to see that proof (without it the check fails the link).
#[inline(always)]
fn fill_batch(unix_millis: u64, count: u32, out: &mut [u8], base: u32) -> Result<(), NewV7Error> {
    // Items before `wrap_at` count up to the top of the 26-bit space; the item at `wrap_at`
    // comes back round to 0 and would sort before them, so it and everything after it move a
    // millisecond on. A block that starts at 0 has nothing before it to wrap from, and gives
    // MAX_BATCH, which no count reaches past. (`first` is at most COUNTER_MASK, so no overflow.)
    let first = base.wrapping_add(1) & COUNTER_MASK;
    let wrap_at = (MAX_BATCH - first) as usize;
    if wrap_at < count as usize && unix_millis == MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }

    // One entropy draw for the whole batch (entropy.rs), with no scratch buffer of any kind:
    // not a heap one (this crate has no allocator to get it from) and not a fixed stack one
    // either (a frame big enough to be worth the syscalls it saves is a poor thing to charge a
    // microcontroller for, where an overflow corrupts silently). The entropy is drawn into the
    // *front* of the caller's own `out`, packed 6 bytes per item, and each item's share is
    // moved out to its final octets as that item is written.
    //
    // That works in place because the packed entropy always sits to the left of where it is
    // going: item i's 6 bytes are at 6i but belong at 16i+10. So the 16 bytes written for item
    // i can only ever land on entropy belonging to items at index >= i — anything from 16i
    // onwards is item 2i's share or later. Walking the batch backwards therefore only
    // overwrites entropy that has already been consumed, and item i's own share is moved
    // before its own 16 bytes are written. Hence `.rev()`, which is load-bearing, not taste.
    crate::entropy::fill_at(
        &mut out[..count as usize * RAND_BYTES_PER_ITEM],
        unix_millis,
    )
    .map_err(NewV7Error::Random)?;

    // unix_ts_ms in the top 48 bits of the u64 that becomes octets 0-7 of every item, and
    // the same a millisecond on for the items past the wrap.
    let ts_shifted = unix_millis << 16;
    let ts_rolled = (unix_millis + 1) << 16;

    for i in (0..count as usize).rev() {
        let src = i * RAND_BYTES_PER_ITEM;
        // rand_b's trailing 48 bits (octets 10-15), straight from the packed entropy above.
        out.copy_within(src..src + RAND_BYTES_PER_ITEM, i * 16 + 10);

        let counter_val = base.wrapping_add(1 + i as u32) & COUNTER_MASK;
        let item = &mut out[i * 16..(i + 1) * 16];

        // Octets 0-9 as two big-endian stores rather than ten single-byte ones:
        // unix_ts_ms (octets 0-5, identical for every item in the batch), then the version
        // nibble (0111) and rand_a, the upper 12 bits of the 26-bit counter (octets 6-7)...
        let ts = if i < wrap_at { ts_shifted } else { ts_rolled };
        let head = ts | 0x7000 | u64::from(counter_val >> 14);
        item[..8].copy_from_slice(&head.to_be_bytes());
        // ...then the variant (10) and the lower 14 bits of the counter (octets 8-9).
        let tail = 0x8000 | (counter_val & 0x3FFF) as u16;
        item[8..10].copy_from_slice(&tail.to_be_bytes());
    }

    Ok(())
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
/// value's permuted octets directly rather than converting it back first. Meaningful only for
/// a genuine version 7 UUID in that layout, the same as [`unix_millis`];
/// [`Uuid::is_rfc_in`] is the check.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn unix_millis_in(uuid: &Uuid, layout: Layout) -> u64 {
    let b = uuid.as_bytes();
    match layout {
        Layout::Rfc9562 => unix_millis(uuid),
        // to_sql_order moves octets 0-5, the timestamp, to octets 10-15 unchanged.
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

/// Converts an RFC 9562-ordered version 7 UUID's bytes to the byte order SQL Server's
/// `uniqueidentifier` needs on the wire to sort by creation order.
///
/// `System.Data.SqlTypes.SqlGuid` (and therefore T-SQL `ORDER BY` on a `uniqueidentifier`
/// column) doesn't compare a GUID's 16 bytes left to right — it compares them in this fixed
/// significance order, most-significant first: octets `10,11,12,13,14,15, 8,9, 6,7, 4,5,
/// 0,1,2,3`. This function moves this UUID's 48-bit timestamp and 26-bit counter — the two
/// fields that actually determine creation order — into those most-significant octets, and
/// moves the 48 bits of trailing entropy, which carries no ordering information, into the
/// least-significant ones as one untouched 6-byte block (its bits are relocated, not
/// individually reshuffled). The version nibble stays at octet 7's top nibble and the variant
/// bits at octet 8's top two, matching where they already sit once run through .NET's own
/// `Guid.ToByteArray()` layout — the reason a value's version is readable without first
/// knowing which of the two orders it's in.
///
/// Re-derived directly against these RFC 9562 byte offsets — see this project's own
/// [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) and
/// [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) for the C# prior art this
/// was checked against, which works in terms of .NET's internal mixed-endian `Guid` layout
/// instead; the two are algebraically equivalent.
///
/// Meaningful only for a genuine version 7 UUID — same convention as [`unix_millis`], the
/// caller is responsible for checking that first if it matters.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn to_sql_order(uuid: &Uuid) -> Uuid {
    let rfc = uuid.as_bytes();

    let counter = ((rfc[6] as u32 & 0x0F) << 22)
        | ((rfc[7] as u32) << 14)
        | ((rfc[8] as u32 & 0x3F) << 8)
        | (rfc[9] as u32);
    let top14 = (counter >> 12) & 0x3FFF;
    let bottom12 = counter & 0xFFF;
    let version = rfc[6] & 0xF0;
    let variant = rfc[8] & 0xC0;

    let mut sql = [0u8; 16];
    sql[0] = rfc[12];
    sql[1] = rfc[13];
    sql[2] = rfc[14];
    sql[3] = rfc[15];
    sql[4] = rfc[10];
    sql[5] = rfc[11];
    sql[6] = ((bottom12 >> 4) & 0xFF) as u8;
    sql[7] = version | (bottom12 & 0x0F) as u8;
    sql[8] = variant | ((top14 >> 8) & 0x3F) as u8;
    sql[9] = (top14 & 0xFF) as u8;
    sql[10] = rfc[0];
    sql[11] = rfc[1];
    sql[12] = rfc[2];
    sql[13] = rfc[3];
    sql[14] = rfc[4];
    sql[15] = rfc[5];

    Uuid::from_bytes(sql)
}

/// Inverse of [`to_sql_order`] — converts a SQL-Server-ordered version 7 UUID's bytes back to
/// RFC 9562 order.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn to_rfc_order(uuid: &Uuid) -> Uuid {
    let sql = uuid.as_bytes();

    let top14 = ((sql[8] as u32 & 0x3F) << 8) | (sql[9] as u32);
    let bottom12 = ((sql[6] as u32) << 4) | (sql[7] as u32 & 0x0F);
    let counter = (top14 << 12) | bottom12;
    let version = sql[7] & 0xF0;
    let variant = sql[8] & 0xC0;

    let mut rfc = [0u8; 16];
    rfc[0] = sql[10];
    rfc[1] = sql[11];
    rfc[2] = sql[12];
    rfc[3] = sql[13];
    rfc[4] = sql[14];
    rfc[5] = sql[15];
    rfc[6] = version | ((counter >> 22) & 0x0F) as u8;
    rfc[7] = ((counter >> 14) & 0xFF) as u8;
    rfc[8] = variant | ((counter >> 8) & 0x3F) as u8;
    rfc[9] = (counter & 0xFF) as u8;
    rfc[10] = sql[4];
    rfc[11] = sql[5];
    rfc[12] = sql[0];
    rfc[13] = sql[1];
    rfc[14] = sql[2];
    rfc[15] = sql[3];

    Uuid::from_bytes(rfc)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_750_000_000_123;

    fn items(out: &[u8]) -> Vec<Uuid> {
        out.chunks_exact(16)
            .map(|c| Uuid::from_bytes(c.try_into().unwrap()))
            .collect()
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
        fill_batch(MS, 5, &mut out, COUNTER_MASK - 2).unwrap();
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
        fill_batch(MS, 3, &mut out, u32::MAX).unwrap(); // base + 1 wraps to counter 0
        assert!(items(&out).iter().all(|id| unix_millis(id) == MS));
        fill_batch(MS, 3, &mut out, COUNTER_MASK - 3).unwrap(); // last item is 0x3FFFFFF
        let ids = items(&out);
        assert!(ids.iter().all(|id| unix_millis(id) == MS));
        assert_eq!(counter_of(&ids[2]), COUNTER_MASK);
    }

    #[test]
    fn rolling_forward_past_the_last_millisecond_is_out_of_range() {
        let mut out = [0u8; 2 * 16];
        assert_eq!(
            fill_batch(MAX_UNIX_MILLIS, 2, &mut out, COUNTER_MASK - 1),
            Err(NewV7Error::TimestampOutOfRange)
        );
        // The same batch without a wrap fits.
        fill_batch(MAX_UNIX_MILLIS, 2, &mut out, 10).unwrap();
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
        assert!(
            out.chunks_exact(16)
                .collect::<Vec<_>>()
                .windows(2)
                .all(|w| w[0] < w[1])
        );
        let last = Uuid::from_bytes(out[out.len() - 16..].try_into().unwrap());
        assert!(unix_millis(&last) <= MS + 1);
    }
}
