//! The SQL Server layout [`v7::to_sql_order`](crate::v7::to_sql_order) writes: an RFC 9562
//! version 8 UUID (§5.8, the version for custom layouts), shaped so `uniqueidentifier` sorts it
//! by creation.
//!
//! SQL Server doesn't compare a GUID's 16 wire bytes left to right. `SqlGuid` compares them
//! in the significance order `10,11,12,13,14,15, 8,9, 6,7, 4,5, 0,1,2,3`. The version nibble
//! and the variant sit where a mixed-endian GUID holds them, which is how SQL Server shows the
//! value as text: octet 7's top nibble reads 8, octet 8's top two bits read `10`. Around them,
//! a version 7's 122 free bits fill the rest most significant first, as one key:
//!
//! | key bits | wire octets | width | holds |
//! |---|---|---|---|
//! | 121-74 | 10-15 | 48 | the millisecond timestamp |
//! | 73-60 | 8-9, under the variant | 14 | the counter's top 14 bits |
//! | 59-48 | 6, then 7 under the version | 12 | the counter's low 12 bits |
//! | 47-0 | 4-5, then 0-3 | 48 | the trailing random bits |
//!
//! That is SequentialGuid's permutation with an 8 where it keeps the 7. A v7 and a v8 both
//! have exactly 122 free bits, so the conversion is an exact inverse for every version 7.

use crate::Uuid;

/// The version nibble every SQL-ordered value carries.
const VERSION: u8 = 8;

/// Why a conversion to or from SQL Server order was refused.
///
/// Non-exhaustive, so a failure mode added later is not a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlOrderError {
    /// To SQL order: the value isn't an RFC 9562 version 7. From SQL order: it doesn't carry
    /// the SQL layout's version 8 and variant.
    WrongVersion,
}

impl core::fmt::Display for SqlOrderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::WrongVersion => "the value is not a UUID of the version this conversion takes",
        })
    }
}

impl core::error::Error for SqlOrderError {}

/// Whether `sql` carries the SQL layout's version and variant.
#[inline]
pub(crate) const fn is_sql(sql: &[u8; 16]) -> bool {
    sql[7] >> 4 == VERSION && sql[8] & 0xC0 == 0x80
}

/// Writes the 122-bit `key` around the version and variant.
#[inline]
pub(crate) fn scatter(key: u128) -> Uuid {
    let high = (key >> 74) as u64;
    let after_variant = (key >> 60) as u16 & 0x3FFF;
    let around_version = (key >> 48) as u16 & 0x0FFF;
    let low = key as u64;
    let mut sql = [0u8; 16];
    sql[0..4].copy_from_slice(&(low as u32).to_be_bytes());
    sql[4..6].copy_from_slice(&((low >> 32) as u16).to_be_bytes());
    sql[6] = (around_version >> 4) as u8;
    sql[7] = (VERSION << 4) | (around_version & 0x0F) as u8;
    sql[8] = 0x80 | (after_variant >> 8) as u8;
    sql[9] = after_variant as u8;
    let high = high.to_be_bytes();
    sql[10..16].copy_from_slice(&[high[2], high[3], high[4], high[5], high[6], high[7]]);
    Uuid::from_bytes(sql)
}

/// The 122-bit key [`scatter`] wrote, read back from around the version and variant.
#[inline]
pub(crate) const fn key(sql: &[u8; 16]) -> u128 {
    let high = u64::from_be_bytes([0, 0, sql[10], sql[11], sql[12], sql[13], sql[14], sql[15]]);
    let after_variant = ((sql[8] & 0x3F) as u16) << 8 | sql[9] as u16;
    let around_version = (sql[6] as u16) << 4 | (sql[7] & 0x0F) as u16;
    let low = u64::from_be_bytes([0, 0, sql[4], sql[5], sql[0], sql[1], sql[2], sql[3]]);
    (high as u128) << 74
        | (after_variant as u128) << 60
        | (around_version as u128) << 48
        | low as u128
}

/// The version a SQL-ordered value was converted from: 7, or 0 for bytes without the
/// layout's version and variant.
#[inline]
pub(crate) const fn source_version(sql: &[u8; 16]) -> u8 {
    if is_sql(sql) { 7 } else { 0 }
}
