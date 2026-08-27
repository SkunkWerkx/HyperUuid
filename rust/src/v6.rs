//! RFC 9562 Section 5.6 — UUID version 6: a field-compatible reordering of version 1's
//! time-based layout for better sort/index locality, without version 7's monotonic counter.

use crate::Uuid;

/// Number of 100-nanosecond intervals between the UUID Gregorian epoch (1582-10-15) and the
/// Unix epoch (1970-01-01) — the same well-known constant every UUID v1/v6 implementation
/// uses to bridge the two timestamp bases.
const GREGORIAN_OFFSET_100NS: u64 = 0x01B2_1DD2_1381_4000;

/// Largest 60-bit Gregorian-epoch tick count the `time_high`/`time_mid`/`time_low` fields
/// can hold.
const MAX_60_BIT: u64 = 0x0FFF_FFFF_FFFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewV6Error {
    /// `unix_millis`, converted to 100-nanosecond Gregorian-epoch ticks, doesn't fit the
    /// 60-bit timestamp field.
    TimestampOutOfRange,
    Random(getrandom::Error),
}

impl core::fmt::Display for NewV6Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TimestampOutOfRange => {
                write!(f, "unix millisecond timestamp does not fit the 60-bit v6 timestamp field")
            }
            Self::Random(e) => write!(f, "random source failed: {e}"),
        }
    }
}

impl std::error::Error for NewV6Error {}

/// Creates a new UUID version 6 from an explicit Unix-epoch millisecond timestamp.
///
/// `clock_seq` and `node` (RFC 9562 §5.1's remaining v1-compatible fields) are both randomly
/// generated on every call — HyperUuid has no real MAC address to draw from, and RFC 9562
/// §6.9 recommends randomizing the node ID anyway to avoid leaking hardware identity. The
/// node ID's multicast bit is set to flag it as random rather than a real MAC, the same
/// convention v1 implementations use.
pub fn new_v6(unix_millis: u64) -> Result<Uuid, NewV6Error> {
    let ticks_since_epoch = unix_millis
        .checked_mul(10_000)
        .and_then(|v| v.checked_add(GREGORIAN_OFFSET_100NS))
        .filter(|&v| v <= MAX_60_BIT)
        .ok_or(NewV6Error::TimestampOutOfRange)?;

    let mut bytes = [0u8; 16];

    let time_high = (ticks_since_epoch >> 28) as u32;
    let time_mid = ((ticks_since_epoch >> 12) & 0xFFFF) as u16;
    // Occupies octets 6-7 alongside the version nibble, written below by set_version.
    let time_low = (ticks_since_epoch & 0x0FFF) as u16;

    bytes[0..4].copy_from_slice(&time_high.to_be_bytes());
    bytes[4..6].copy_from_slice(&time_mid.to_be_bytes());
    bytes[6..8].copy_from_slice(&time_low.to_be_bytes());

    let mut rand_bytes = [0u8; 8];
    getrandom::fill(&mut rand_bytes).map_err(NewV6Error::Random)?;
    // clock_seq (14 bits, octets 8-9 alongside the variant, written below by set_variant).
    bytes[8..10].copy_from_slice(&rand_bytes[0..2]);
    // node (48 bits, octets 10-15).
    bytes[10..16].copy_from_slice(&rand_bytes[2..8]);
    bytes[10] |= 0x01;

    let mut uuid = Uuid::from_bytes(bytes);
    uuid.set_version(6);
    uuid.set_variant();
    Ok(uuid)
}

/// Creates `count` time-sortable UUID version 6 values sharing one Unix-epoch millisecond
/// timestamp capture, writing 16 bytes each into consecutive slots of `out` (which must be
/// exactly `count * 16` bytes long). `clock_seq` and `node` are independently random per
/// item, same as [`new_v6`] — there's no shared counter state to batch here, just one
/// `getrandom` call for the whole batch's random bytes instead of `count` separate ones,
/// the only allocating path in this crate (the scratch buffer is sized by `count`). A
/// `count` of 0 is a no-op success. Same errors as [`new_v6`].
pub fn new_v6_batch(unix_millis: u64, count: u32, out: &mut [u8]) -> Result<(), NewV6Error> {
    let ticks_since_epoch = unix_millis
        .checked_mul(10_000)
        .and_then(|v| v.checked_add(GREGORIAN_OFFSET_100NS))
        .filter(|&v| v <= MAX_60_BIT)
        .ok_or(NewV6Error::TimestampOutOfRange)?;
    if count == 0 {
        return Ok(());
    }

    let time_high = (ticks_since_epoch >> 28) as u32;
    let time_mid = ((ticks_since_epoch >> 12) & 0xFFFF) as u16;
    // Top nibble is always 0 (12-bit value in a 16-bit field), so `item[6] |= 0x60` below is
    // a safe way to write the version nibble without first masking it off.
    let time_low = (ticks_since_epoch & 0x0FFF) as u16;

    let mut rand_bytes = vec![0u8; count as usize * 8];
    getrandom::fill(&mut rand_bytes).map_err(NewV6Error::Random)?;

    for i in 0..count as usize {
        let item = &mut out[i * 16..(i + 1) * 16];

        item[0..4].copy_from_slice(&time_high.to_be_bytes());
        item[4..6].copy_from_slice(&time_mid.to_be_bytes());
        item[6..8].copy_from_slice(&time_low.to_be_bytes());
        item[6] |= 0x60;

        let r = &rand_bytes[i * 8..(i + 1) * 8];
        item[8] = 0x80 | (r[0] & 0x3F);
        item[9] = r[1];
        item[10..16].copy_from_slice(&r[2..8]);
        item[10] |= 0x01;
    }

    Ok(())
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 6 UUID. Saturates to
/// 0 for a (legitimately RFC-valid) pre-1970 Gregorian timestamp, matching this module's
/// Unix-millisecond-only API surface.
pub fn unix_millis(uuid: &Uuid) -> u64 {
    let b = uuid.as_bytes();
    let time_high = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64;
    let time_mid = u16::from_be_bytes([b[4], b[5]]) as u64;
    let time_low = (u16::from_be_bytes([b[6], b[7]]) & 0x0FFF) as u64;
    let ticks_since_epoch = (time_high << 28) | (time_mid << 12) | time_low;
    ticks_since_epoch.saturating_sub(GREGORIAN_OFFSET_100NS) / 10_000
}
