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
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    /// The RFC 9562 version nibble (bits 48-51, the high nibble of octet 6).
    pub const fn version(&self) -> u8 {
        self.0[6] >> 4
    }

    /// Whether the variant bits (top two bits of octet 8) match RFC 9562 (`10`).
    pub const fn is_rfc9562_variant(&self) -> bool {
        (self.0[8] & 0xC0) == 0x80
    }

    pub(crate) fn set_version(&mut self, version: u8) {
        self.0[6] = (self.0[6] & 0x0F) | (version << 4);
    }

    pub(crate) fn set_variant(&mut self) {
        self.0[8] = (self.0[8] & 0x3F) | 0x80;
    }
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
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseUuidError;

impl fmt::Display for ParseUuidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid UUID string; expected 8-4-4-4-12 hyphenated hex")
    }
}

impl FromStr for Uuid {
    type Err = ParseUuidError;

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

        let mut bytes = [0u8; 16];
        let mut out = 0usize;
        let mut i = 0usize;
        while i < s.len() {
            if s[i] == b'-' {
                i += 1;
                continue;
            }
            let hi = hex_val(s[i]).ok_or(ParseUuidError)?;
            let lo = hex_val(s[i + 1]).ok_or(ParseUuidError)?;
            bytes[out] = (hi << 4) | lo;
            out += 1;
            i += 2;
        }

        Ok(Self(bytes))
    }
}
