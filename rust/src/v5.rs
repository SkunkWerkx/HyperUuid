//! RFC 9562 Section 5.5 — UUID version 5: deterministic, namespace + name based (SHA-1).

use crate::Uuid;
use sha1::block_api::compress;

/// SHA-1's initial hash value (FIPS 180-4 §5.3.1).
const SHA1_H0: [u32; 5] = [
    0x6745_2301,
    0xEFCD_AB89,
    0x98BA_DCFE,
    0x1032_5476,
    0xC3D2_E1F0,
];

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
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn new_v5(namespace: Uuid, name: &[u8]) -> Uuid {
    // SHA-1 over `namespace || name`, padded by hand and fed to the bare compression function
    // rather than through `Digest`, whose streaming buffer cost about a fifth of the call.
    let mut state = SHA1_H0;
    let mut block = [0u8; 64];
    block[..16].copy_from_slice(namespace.as_bytes());

    // The first block is the namespace and up to 48 bytes of name. If the name fills it, every
    // further whole block is compressed straight out of `name`, and what is left over is moved
    // into a fresh block to be padded.
    let (head, mut rest) = name.split_at(name.len().min(48));
    block[16..16 + head.len()].copy_from_slice(head);
    let mut filled = 16 + head.len();
    if filled == 64 {
        compress(&mut state, &[block]);
        let mut chunks = rest.chunks_exact(64);
        for chunk in &mut chunks {
            if let Ok(chunk) = <&[u8; 64]>::try_from(chunk) {
                compress(&mut state, core::slice::from_ref(chunk));
            }
        }
        rest = chunks.remainder();
        block = [0u8; 64];
        block[..rest.len()].copy_from_slice(rest);
        filled = rest.len();
    }

    // Padding: a single 1 bit, zeroes, then the message length in bits as a big-endian u64 in
    // the last 8 bytes, which takes a block of its own when fewer than 8 bytes are left.
    block[filled] = 0x80;
    if filled >= 56 {
        compress(&mut state, &[block]);
        block = [0u8; 64];
    }
    let bit_len = (16 + name.len() as u64).wrapping_mul(8);
    block[56..].copy_from_slice(&bit_len.to_be_bytes());
    compress(&mut state, &[block]);

    // The UUID is the digest's first 16 bytes: the first four state words, big-endian.
    let mut bytes = [0u8; 16];
    for (out, word) in bytes.chunks_exact_mut(4).zip(state) {
        out.copy_from_slice(&word.to_be_bytes());
    }

    let mut uuid = Uuid::from_bytes(bytes);
    uuid.set_version(5);
    uuid.set_variant();
    uuid
}
