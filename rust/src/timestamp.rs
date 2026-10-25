//! [`Timestamp`] — Unix-epoch seconds plus sub-second nanoseconds, the same two-field shape
//! the upstream [`uuid`](https://docs.rs/uuid) crate's own `Timestamp` exposes via `to_unix`.
//! [`crate::get_timestamp`] returns one for any version 7 [`crate::Uuid`], and
//! [`crate::v7::new_v7_at`] accepts one back — a caller already
//! familiar with the `uuid` crate's `get_timestamp`/`Timestamp::to_unix` gets the same
//! vocabulary here instead of passing a raw millisecond count around.

/// Unix-epoch seconds and sub-second nanoseconds, mirroring the `uuid` crate's own
/// `Timestamp::to_unix` shape. The nanosecond component is always below one billion;
/// [`Timestamp::from_unix`] returns `None` for an invalid component.
///
/// This crate's own creation and extraction functions all work in whole milliseconds — the
/// precision RFC 9562 v7 actually stores — so converting to and from that unit is exact;
/// constructing a [`Timestamp`] from a sub-millisecond-precision `(seconds, subsec_nanos)`
/// pair and converting it back to milliseconds truncates, the same way `uuid`'s own
/// `Timestamp::to_unix_millis` documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    seconds: u64,
    subsec_nanos: u32,
}

impl Timestamp {
    /// Builds a [`Timestamp`] from Unix-epoch seconds and sub-second nanoseconds.
    /// Returns `None` unless `subsec_nanos` is below one billion; every accepted pair
    /// round-trips through [`Timestamp::to_unix`] and has one canonical representation.
    ///
    /// ```
    /// use hyperuuid::Timestamp;
    /// let at = Timestamp::from_unix(1, 1_500).unwrap();
    /// assert_eq!(at.to_unix(), (1, 1_500));
    /// assert_eq!(at.to_unix_millis(), 1_000); // sub-millisecond digits are truncated
    /// assert!(Timestamp::from_unix(1, 1_000_000_000).is_none());
    /// ```
    pub const fn from_unix(seconds: u64, subsec_nanos: u32) -> Option<Self> {
        if subsec_nanos >= 1_000_000_000 {
            return None;
        }
        Some(Self {
            seconds,
            subsec_nanos,
        })
    }

    /// Returns the Unix-epoch seconds and sub-second nanoseconds this timestamp represents.
    pub const fn to_unix(&self) -> (u64, u32) {
        (self.seconds, self.subsec_nanos)
    }

    /// Builds a [`Timestamp`] from a millisecond count since the Unix epoch — the unit every
    /// creation/extraction function in this crate actually works in.
    pub const fn from_unix_millis(millis: u64) -> Self {
        Self {
            seconds: millis / 1000,
            subsec_nanos: ((millis % 1000) as u32) * 1_000_000,
        }
    }

    /// Collapses this timestamp down to the millisecond count [`crate::v7::new_v7_at`]
    /// actually passes along to [`crate::v7::new_v7`] — truncates any sub-millisecond precision this crate doesn't
    /// track, matching [`Timestamp::from_unix_millis`]'s own rounding-down direction.
    ///
    /// Saturates at `u64::MAX` for a timestamp past what a millisecond count can hold, which
    /// the generator rejects as out of range. Plain arithmetic panicked there in a debug
    /// build and, in a release one, wrapped into a valid-looking but wrong timestamp.
    pub const fn to_unix_millis(&self) -> u64 {
        self.seconds
            .saturating_mul(1000)
            .saturating_add((self.subsec_nanos / 1_000_000) as u64)
    }
}
