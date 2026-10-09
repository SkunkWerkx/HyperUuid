use core::fmt;
use core::str::FromStr;

/// A 128-bit UUID, stored in RFC 9562 network (big-endian) byte order.
///
/// Unlike .NET's `Guid`, this has no internal mixed-endian field layout to work
/// around — the 16 bytes here are exactly the wire/text representation defined
/// by RFC 9562 Section 4.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid([u8; 16]);

impl Uuid {
    /// The RFC 9562 §5.9 Nil UUID — all 128 bits zero.
    pub const NIL: Uuid = Uuid([0u8; 16]);

    /// The RFC 9562 §5.10 Max UUID — all 128 bits one.
    pub const MAX: Uuid = Uuid([0xFFu8; 16]);

    /// Builds a UUID directly from its 16 raw bytes in RFC 9562 (big-endian) order.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Borrows the UUID's 16 raw bytes in RFC 9562 (big-endian) order.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Consumes the UUID, returning its 16 raw bytes in RFC 9562 (big-endian) order.
    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    /// The RFC 9562 version nibble (bits 48-51, the high nibble of octet 6), 0 through 15.
    /// Nil reads as 0 and Max as 15. Says nothing about the variant: a value whose variant
    /// isn't [`Variant::Rfc9562`] has no RFC version, whatever this nibble holds, so check
    /// [`is_rfc`](Self::is_rfc) instead when the answer has to mean "an RFC 9562 UUID of
    /// version N".
    pub const fn version(&self) -> u8 {
        self.0[6] >> 4
    }

    /// The version of a UUID held in `layout`'s byte order.
    ///
    /// [`Layout::Rfc9562`] is [`version`](Self::version). [`Layout::SqlServer`] is defined only
    /// for the two versions that have a SQL Server order, and answers 6, 7, or 0 for anything
    /// that isn't a SQL-ordered version 6 or 7 RFC 9562 UUID. The version nibble lands at a
    /// different octet for each (octet 7 for v7, octet 8 for v6), and the other version's
    /// random bits can mimic it there, so this checks the variant too, where each version
    /// puts it: a v7's variant shares octet 8 with the v6 nibble's slot and can never read 6
    /// there, and a v6's version byte sits where a v7's variant would be and never reads as
    /// one. Either way the answer can't confuse the two.
    ///
    /// What it can't know is whether the bytes are in SQL Server order at all: that is the
    /// caller's to track. Bytes in RFC order can happen to form a valid SQL-ordered v7 (a
    /// random v4 does one time in 16), and then this answers 7.
    pub const fn version_in(&self, layout: Layout) -> u8 {
        let b = &self.0;
        match layout {
            Layout::Rfc9562 => self.version(),
            Layout::SqlServer if b[7] >> 4 == 7 && b[8] & 0xC0 == 0x80 => 7,
            Layout::SqlServer if b[8] >> 4 == 6 && b[6] & 0xC0 == 0x80 => 6,
            Layout::SqlServer => 0,
        }
    }

    /// The variant field (RFC 9562 §4.1, the top bits of octet 8). Nil reads as
    /// [`Variant::Ncs`] and Max as [`Variant::Future`], which is how the RFC classifies them.
    pub const fn variant(&self) -> Variant {
        match self.0[8] >> 5 {
            0..=3 => Variant::Ncs,
            4 | 5 => Variant::Rfc9562,
            6 => Variant::Microsoft,
            _ => Variant::Future,
        }
    }

    /// Whether the variant bits (top two bits of octet 8) match RFC 9562 (`10`).
    pub const fn is_rfc9562_variant(&self) -> bool {
        (self.0[8] & 0xC0) == 0x80
    }

    /// Whether this is an RFC 9562 UUID of version `version`: the RFC variant and that
    /// version nibble, in one call. The guard to run before trusting a value's
    /// version-specific fields, such as a version 7's timestamp.
    pub const fn is_rfc(&self, version: u8) -> bool {
        self.is_rfc9562_variant() && self.version() == version
    }

    /// [`is_rfc`](Self::is_rfc) for a UUID held in `layout`'s byte order. In
    /// [`Layout::SqlServer`] only versions 6 and 7 can be true, the two that have a SQL Server
    /// order; see [`version_in`](Self::version_in).
    pub const fn is_rfc_in(&self, version: u8, layout: Layout) -> bool {
        match layout {
            Layout::Rfc9562 => self.is_rfc(version),
            Layout::SqlServer => version != 0 && self.version_in(layout) == version,
        }
    }

    pub(crate) fn set_version(&mut self, version: u8) {
        self.0[6] = (self.0[6] & 0x0F) | (version << 4);
    }

    pub(crate) fn set_variant(&mut self) {
        self.0[8] = (self.0[8] & 0x3F) | 0x80;
    }
}

/// The variant field of a UUID (RFC 9562 §4.1), which says how the rest of its bits are laid
/// out. Only [`Rfc9562`](Self::Rfc9562) has versions; the RFC defines no others, so the set is
/// complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// `0xxx`: reserved, Network Computing System backward compatibility. Includes Nil.
    Ncs,
    /// `10xx`: the variant RFC 9562 (and RFC 4122 before it) specifies.
    Rfc9562,
    /// `110x`: reserved, Microsoft Corporation backward compatibility.
    Microsoft,
    /// `111x`: reserved for future definition. Includes Max.
    Future,
}

/// The byte order a UUID's 16 bytes are held in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    /// RFC 9562 network order, what every function in this crate takes unless it says
    /// otherwise.
    Rfc9562,
    /// The order [`v6::to_sql_order`](crate::v6::to_sql_order) and
    /// [`v7::to_sql_order`](crate::v7::to_sql_order) write, which SQL Server's
    /// `uniqueidentifier` sorts by creation order. Defined for versions 6 and 7 only.
    SqlServer,
}

impl From<[u8; 16]> for Uuid {
    fn from(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            b[6],
            b[7],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15]
        )
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The input string wasn't a valid 8-4-4-4-12 hyphenated hex UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseUuidError;

impl fmt::Display for ParseUuidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid UUID string; expected 8-4-4-4-12 hyphenated hex")
    }
}

impl FromStr for Uuid {
    type Err = ParseUuidError;

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.as_bytes();
        if s.len() != 36 || s[8] != b'-' || s[13] != b'-' || s[18] != b'-' || s[23] != b'-' {
            return Err(ParseUuidError);
        }

        fn hex_val(c: u8) -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        }

        // Where each byte's two hex digits start. The hyphens sit at fixed offsets, so every
        // pair does too, and a hyphen anywhere else is simply a non-hex digit. Skipping
        // hyphens wherever they fell instead let one in the last group shift the pairing,
        // so the final pair started at offset 35 and read past the end of the string.
        const PAIRS: [usize; 16] = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

        let mut bytes = [0u8; 16];
        for (byte, &at) in bytes.iter_mut().zip(PAIRS.iter()) {
            let hi = hex_val(s[at]).ok_or(ParseUuidError)?;
            let lo = hex_val(s[at + 1]).ok_or(ParseUuidError)?;
            *byte = (hi << 4) | lo;
        }

        Ok(Self(bytes))
    }
}
