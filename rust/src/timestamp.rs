//! [`Timestamp`] — Unix-epoch seconds plus sub-second nanoseconds, the same two-field shape
//! the upstream [`uuid`](https://docs.rs/uuid) crate's own `Timestamp` exposes via `to_unix`.
//! [`crate::get_timestamp`] returns one for any version 6 or 7 [`crate::Uuid`], and
//! [`crate::v6::new_v6_at`]/[`crate::v7::new_v7_at`] accept one back — a caller already
//! familiar with the `uuid` crate's `get_timestamp`/`Timestamp::to_unix` gets the same
//! vocabulary here instead of passing a raw millisecond count around.

/// Unix-epoch seconds and sub-second nanoseconds, mirroring the `uuid` crate's own
/// `Timestamp::to_unix` shape.
///
/// This crate's own creation and extraction functions all work in whole milliseconds — the
/// precision RFC 9562 v6/v7 actually store — so converting to and from that unit is exact;
/// constructing a [`Timestamp`] from a sub-millisecond-precision `(seconds, subsec_nanos)`
/// pair and converting it back to milliseconds truncates, the same way `uuid`'s own
/// `Timestamp::to_unix_millis` documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    seconds: u64,
    subsec_nanos: u32,
}

impl Timestamp {
    /// Builds a [`Timestamp`] from Unix-epoch seconds and sub-second nanoseconds — the same
    /// two values [`Timestamp::to_unix`] returns, so the pair round-trips.
    pub const fn from_unix(seconds: u64, subsec_nanos: u32) -> Self {
        Self { seconds, subsec_nanos }
    }

    /// Returns the Unix-epoch seconds and sub-second nanoseconds this timestamp represents.
    pub const fn to_unix(&self) -> (u64, u32) {
        (self.seconds, self.subsec_nanos)
    }

    /// Builds a [`Timestamp`] from a millisecond count since the Unix epoch — the unit every
    /// creation/extraction function in this crate actually works in.
    pub const fn from_unix_millis(millis: u64) -> Self {
        Self { seconds: millis / 1000, subsec_nanos: ((millis % 1000) as u32) * 1_000_000 }
    }

    /// Collapses this timestamp down to the millisecond count [`crate::v6::new_v6_at`]/
    /// [`crate::v7::new_v7_at`] actually pass along to [`crate::v6::new_v6`]/
    /// [`crate::v7::new_v7`] — truncates any sub-millisecond precision this crate doesn't
    /// track, matching [`Timestamp::from_unix_millis`]'s own rounding-down direction.
    pub const fn to_unix_millis(&self) -> u64 {
        self.seconds * 1000 + (self.subsec_nanos / 1_000_000) as u64
    }
}
