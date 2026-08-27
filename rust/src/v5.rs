//! RFC 9562 Section 5.5 — UUID version 5: deterministic, namespace + name based (SHA-1).

use crate::Uuid;
use sha1::{Digest, Sha1};

/// Well-known namespace UUIDs defined in RFC 9562 Section 6.6.
pub mod namespace {
    use crate::Uuid;

    /// Name string is a fully-qualified domain name.
    pub const DNS: Uuid = Uuid::from_bytes([
        0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// Name string is a URL.
    pub const URL: Uuid = Uuid::from_bytes([
        0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// Name string is an ISO OID.
    pub const OID: Uuid = Uuid::from_bytes([
        0x6b, 0xa7, 0xb8, 0x12, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// Name string is an X.500 DN (in DER or a text output format).
    pub const X500: Uuid = Uuid::from_bytes([
        0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);
}

/// Creates a deterministic UUID version 5 from a namespace UUID and raw name bytes.
///
/// The same `(namespace, name)` pair always produces the same UUID.
pub fn new_v5(namespace: Uuid, name: &[u8]) -> Uuid {
    let mut hasher = Sha1::new();
    hasher.update(namespace.as_bytes());
    hasher.update(name);
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);

    let mut uuid = Uuid::from_bytes(bytes);
    uuid.set_version(5);
    uuid.set_variant();
    uuid
}
