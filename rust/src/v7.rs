//! RFC 9562 Section 6.2 Method 1 — UUID version 7: time-ordered, monotonically increasing.

use crate::{Timestamp, Uuid};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Largest Unix-epoch millisecond timestamp that fits the 48-bit `unix_ts_ms` field
/// (valid until the year 10889).
pub const MAX_UNIX_MILLIS: u64 = 0x0000_FFFF_FFFF_FFFF;

/// 26-bit counter mask (67,108,864 values) spanning `rand_a` and the top of `rand_b`.
const COUNTER_MASK: u32 = 0x03FF_FFFF;

/// Random octets each version 7 UUID needs: `rand_b`'s trailing 48 bits (octets 10-15).
const RAND_BYTES_PER_ITEM: usize = 6;


/// An error returned when minting a version 7 UUID fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewV7Error {
    /// `unix_millis` was negative-equivalent-out-of-range or exceeded [`MAX_UNIX_MILLIS`].
    TimestampOutOfRange,
    /// The system's random source failed while generating `rand_a`/`rand_b`.
    Random(getrandom::Error),
}

impl core::fmt::Display for NewV7Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TimestampOutOfRange => {
                write!(f, "unix millisecond timestamp must fit within 48 bits")
            }
            Self::Random(e) => write!(f, "random source failed: {e}"),
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
pub fn new_v7(unix_millis: u64) -> Result<Uuid, NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }

    // Claim a unique, strictly increasing counter slot.
    let counter_val = counter().fetch_add(1, Ordering::Relaxed).wrapping_add(1) & COUNTER_MASK;

    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes[10..]).map_err(NewV7Error::Random)?;

    // unix_ts_ms: 48-bit big-endian millisecond timestamp (octets 0-5).
    bytes[0] = (unix_millis >> 40) as u8;
    bytes[1] = (unix_millis >> 32) as u8;
    bytes[2] = (unix_millis >> 24) as u8;
    bytes[3] = (unix_millis >> 16) as u8;
    bytes[4] = (unix_millis >> 8) as u8;
    bytes[5] = unix_millis as u8;

    // rand_a: upper 12 bits of the 26-bit counter (octets 6-7).
    bytes[6] = (counter_val >> 22) as u8;
    bytes[7] = ((counter_val >> 14) & 0xFF) as u8;

    // rand_b extension: lower 14 bits of the counter (octets 8-9).
    bytes[8] = ((counter_val >> 8) & 0x3F) as u8;
    bytes[9] = (counter_val & 0xFF) as u8;

    let mut uuid = Uuid::from_bytes(bytes);
    uuid.set_version(7);
    uuid.set_variant();
    Ok(uuid)
}

/// Creates a new UUID version 7 from a [`Timestamp`] instead of a raw millisecond count —
/// pulls the Unix-epoch milliseconds off `timestamp` and mints it through [`new_v7`], so it's
/// the exact same UUID [`new_v7(timestamp.to_unix_millis())`](new_v7) would produce.
pub fn new_v7_at(timestamp: Timestamp) -> Result<Uuid, NewV7Error> {
    new_v7(timestamp.to_unix_millis())
}

/// Creates `count` time-sortable UUID version 7 values sharing one Unix-epoch millisecond
/// timestamp capture, writing 16 bytes each into consecutive slots of `out` (which must be at
/// least `count * 16` bytes long; anything past that is left untouched).
///
/// Reserves one contiguous block of `count` counter slots up front (one atomic op instead of
/// `count`) and draws every UUID's random tail from a single `getrandom` call, into the
/// caller's own buffer with no scratch space at all — not on the heap and not on the stack.
/// That is what lets this crate build with no allocator rather than merely without `std`.
///
/// A `count` of 0 is a no-op success. Same errors as [`new_v7`]. The entropy is drawn before
/// any item is assembled, so on [`NewV7Error::Random`] no UUID has been written at all — but
/// the front of `out` may hold partial entropy from the failed draw, so treat the buffer as
/// clobbered rather than untouched. A very large `count` can still wrap the 26-bit counter
/// mid-batch, the same wrap-boundary caveat individual calls already carry.
pub fn new_v7_batch(unix_millis: u64, count: u32, out: &mut [u8]) -> Result<(), NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }
    if count == 0 {
        return Ok(());
    }

    // Narrowed once, up front, so the entropy fill and the per-item writes below both stay
    // inside exactly the region this call owns. That matters more than it used to: the fill
    // now writes through `out` itself, and a caller's oversized buffer must keep its tail
    // untouched.
    let out = &mut out[..count as usize * 16];

    // Reserves [base+1, base+count] in this one call, continuing the same global sequence a
    // series of individual fetch_add(1) calls would have produced (matching new_v7's own
    // base.wrapping_add(1) convention below).
    let base = counter().fetch_add(count, Ordering::Relaxed);

    // One `getrandom` call for the whole batch, with no scratch buffer of any kind: not a heap
    // one (this crate has no allocator to get it from) and not a fixed stack one either (a
    // frame big enough to be worth the syscalls it saves is a poor thing to charge a
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
    getrandom::fill(&mut out[..count as usize * RAND_BYTES_PER_ITEM])
        .map_err(NewV7Error::Random)?;

    for i in (0..count as usize).rev() {
        let src = i * RAND_BYTES_PER_ITEM;
        // rand_b's trailing 48 bits (octets 10-15), straight from the packed entropy above.
        out.copy_within(src..src + RAND_BYTES_PER_ITEM, i * 16 + 10);

        let counter_val = base.wrapping_add(1 + i as u32) & COUNTER_MASK;
        let item = &mut out[i * 16..(i + 1) * 16];

        // unix_ts_ms: 48-bit big-endian millisecond timestamp (octets 0-5), identical for
        // every item in the batch.
        item[0] = (unix_millis >> 40) as u8;
        item[1] = (unix_millis >> 32) as u8;
        item[2] = (unix_millis >> 24) as u8;
        item[3] = (unix_millis >> 16) as u8;
        item[4] = (unix_millis >> 8) as u8;
        item[5] = unix_millis as u8;

        // version nibble (0111) + rand_a: upper 12 bits of the 26-bit counter (octets 6-7).
        item[6] = 0x70 | (counter_val >> 22) as u8;
        item[7] = ((counter_val >> 14) & 0xFF) as u8;

        // variant (10) + rand_b extension: lower 14 bits of the counter (octets 8-9).
        item[8] = 0x80 | ((counter_val >> 8) & 0x3F) as u8;
        item[9] = (counter_val & 0xFF) as u8;
    }

    Ok(())
}

/// Creates a new UUID version 7 using the current system time.
///
/// Not available on `wasm32` targets, which have no OS clock, nor without this crate's
/// default `std` feature, which is the same situation one step further out — no OS at all to
/// read a clock from. Call [`new_v7`] there with a timestamp supplied by the host instead.
#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
pub fn now_v7() -> Result<Uuid, NewV7Error> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_millis() as u64;
    new_v7(millis)
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 7 UUID.
pub fn unix_millis(uuid: &Uuid) -> u64 {
    let b = uuid.as_bytes();
    ((b[0] as u64) << 40)
        | ((b[1] as u64) << 32)
        | ((b[2] as u64) << 24)
        | ((b[3] as u64) << 16)
        | ((b[4] as u64) << 8)
        | (b[5] as u64)
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
