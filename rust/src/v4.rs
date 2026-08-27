//! RFC 9562 Section 5.4 — UUID version 4: fully random.

use crate::Uuid;

/// Creates a new UUID version 4 from a cryptographically strong random source.
///
/// All 122 free bits are random; the version nibble (`0x4`) and variant bits
/// (`10xxxxxx`) occupy their required positions at octet 6 and octet 8.
pub fn new_v4() -> Result<Uuid, getrandom::Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)?;

    let mut uuid = Uuid::from_bytes(bytes);
    uuid.set_version(4);
    uuid.set_variant();
    Ok(uuid)
}
