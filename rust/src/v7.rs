//! RFC 9562 Section 6.2 Method 1 — UUID version 7: time-ordered, monotonically increasing.

use crate::Uuid;
use core::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

/// Largest Unix-epoch millisecond timestamp that fits the 48-bit `unix_ts_ms` field
/// (valid until the year 10889).
pub const MAX_UNIX_MILLIS: u64 = 0x0000_FFFF_FFFF_FFFF;

/// 26-bit counter mask (67,108,864 values) spanning `rand_a` and the top of `rand_b`.
const COUNTER_MASK: u32 = 0x03FF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewV7Error {
    /// `unix_millis` was negative-equivalent-out-of-range or exceeded [`MAX_UNIX_MILLIS`].
    TimestampOutOfRange,
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

impl std::error::Error for NewV7Error {}

// Process-global monotonic counter (RFC 9562 §6.2 Method 1 — Fixed Bit-Length Dedicated
// Counter), seeded randomly on first use and advanced with fetch_add. This guarantees sort
// order for UUIDs minted within the same millisecond regardless of caller concurrency.
static COUNTER: OnceLock<AtomicU32> = OnceLock::new();

fn counter() -> &'static AtomicU32 {
    COUNTER.get_or_init(|| {
        // Seed in [0, 512) to leave ample headroom before the 26-bit wrap, mirroring the
        // C# SequentialGuid implementation this is ported from.
        let seed = getrandom::u32().unwrap_or(0) & 0x1FF;
        AtomicU32::new(seed)
    })
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

/// Creates `count` time-sortable UUID version 7 values sharing one Unix-epoch millisecond
/// timestamp capture, writing 16 bytes each into consecutive slots of `out` (which must be
/// exactly `count * 16` bytes long).
///
/// Reserves one contiguous block of `count` counter slots up front (one atomic op instead of
/// `count`) and fills every UUID's random tail from a single `getrandom` call — unlike
/// calling [`new_v7`] `count` times, this is the only allocating path in this crate, since
/// the scratch buffer for that call is sized by `count`. A `count` of 0 is a no-op success.
/// Same errors as [`new_v7`]; a very large `count` can still wrap the 26-bit counter
/// mid-batch, the same wrap-boundary caveat individual calls already carry.
pub fn new_v7_batch(unix_millis: u64, count: u32, out: &mut [u8]) -> Result<(), NewV7Error> {
    if unix_millis > MAX_UNIX_MILLIS {
        return Err(NewV7Error::TimestampOutOfRange);
    }
    if count == 0 {
        return Ok(());
    }

    // Reserves [base+1, base+count] in this one call, continuing the same global sequence a
    // series of individual fetch_add(1) calls would have produced (matching new_v7's own
    // base.wrapping_add(1) convention below).
    let base = counter().fetch_add(count, Ordering::Relaxed);

    let mut rand_bytes = vec![0u8; count as usize * 6];
    getrandom::fill(&mut rand_bytes).map_err(NewV7Error::Random)?;

    for i in 0..count as usize {
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

        item[10..16].copy_from_slice(&rand_bytes[i * 6..i * 6 + 6]);
    }

    Ok(())
}

/// Creates a new UUID version 7 using the current system time.
///
/// Not available on `wasm32` targets, which have no OS clock — call [`new_v7`] there
/// with a timestamp supplied by the host instead.
#[cfg(not(target_arch = "wasm32"))]
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
