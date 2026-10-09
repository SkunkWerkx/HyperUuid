//! C-ABI exports — built as a `cdylib` (`cargo cdylib`), this same source produces a
//! native `libhyperuuid.so`/`.dylib`/`.dll` loaded through ordinary P/Invoke/FFM/ctypes-style
//! FFI. This is the one contract every host binding calls through: a caller shares this
//! library's address space directly (no separate guest/host memory boundary to bridge), so
//! every export just takes plain pointers into the caller's own stack- or heap-allocated
//! buffers — no allocator exports, no protocol beyond "here's a 16-byte buffer, fill it in".
//!
//! Return codes: `0` success, `1` random source failure, `2` timestamp out of range, `3` a
//! batch too large to address (`count * 16` overflows `usize`, which only a 32-bit target can
//! reach, and where no buffer that size can exist), `4` a version 7 batch larger than the
//! 26-bit counter space.
//!
//! Enum codes reserve 0 as "unspecified", so a code a binding failed to set is detectable:
//! layout `1` RFC 9562, `2` SQL Server; variant `1` NCS, `2` RFC 9562, `3` Microsoft,
//! `4` Future. An export handed a layout code it doesn't know answers as it would for a value
//! the layout doesn't apply to (0 / false) rather than guessing.

use crate::{Layout, Uuid, Variant, v4, v5, v6, v7};
use core::slice;

/// This library's version, packed `major << 16 | minor << 8 | patch` from the crate's own
/// manifest — so a host can prove the library it loaded is the one its binding was built
/// against before minting the first UUID, and can name the mismatch when it isn't. Takes
/// nothing, touches nothing: the cheapest possible "did the native library resolve" probe,
/// and the same shape as HyperCast's `hypercast_version`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn hyperuuid_version() -> u32 {
    const fn field(text: &str) -> u32 {
        let bytes = text.as_bytes();
        let mut value = 0u32;
        let mut i = 0;
        while i < bytes.len() {
            value = value * 10 + (bytes[i] - b'0') as u32;
            i += 1;
        }
        value
    }
    const VERSION: u32 = (field(env!("CARGO_PKG_VERSION_MAJOR")) << 16)
        | (field(env!("CARGO_PKG_VERSION_MINOR")) << 8)
        | field(env!("CARGO_PKG_VERSION_PATCH"));
    VERSION
}

/// The layout behind an ABI layout code, or `None` for an unknown one.
#[inline]
const fn layout(code: u32) -> Option<Layout> {
    match code {
        1 => Some(Layout::Rfc9562),
        2 => Some(Layout::SqlServer),
        _ => None,
    }
}

/// Reads the 16 bytes at `uuid_ptr`.
#[inline]
fn read(uuid_ptr: *const u8) -> Uuid {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live bytes, per the module contract.
    Uuid::from_bytes(unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) })
}

/// The version of the UUID at `uuid_ptr` (16 bytes) held in `layout`'s byte order (see
/// [`Uuid::version_in`]): the version nibble, 0-15, in RFC 9562 order; 6, 7, or 0 for "not a
/// SQL-ordered v6/v7" in SQL Server order; 0 for an unknown layout code.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_version(uuid_ptr: *const u8, layout_code: u32) -> u32 {
    match layout(layout_code) {
        Some(layout) => read(uuid_ptr).version_in(layout) as u32,
        None => 0,
    }
}

/// The variant of the UUID at `uuid_ptr` (16 bytes, RFC 9562 order) as a variant code, 1-4.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_variant(uuid_ptr: *const u8) -> u32 {
    match read(uuid_ptr).variant() {
        Variant::Ncs => 1,
        Variant::Rfc9562 => 2,
        Variant::Microsoft => 3,
        Variant::Future => 4,
    }
}

/// 1 if the UUID at `uuid_ptr` (16 bytes, held in `layout`'s byte order) is an RFC 9562 UUID
/// of version `version`, else 0 (see [`Uuid::is_rfc_in`]) — the one-call guard. 0 for an
/// unknown layout code or a `version` past 15.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_is_rfc(uuid_ptr: *const u8, version: u32, layout_code: u32) -> u32 {
    match (layout(layout_code), u8::try_from(version)) {
        (Some(layout), Ok(version)) => read(uuid_ptr).is_rfc_in(version, layout) as u32,
        _ => 0,
    }
}

/// Writes a random UUID version 4 (RFC 9562 §5.4) to `out_ptr` (16 bytes).
/// Returns 0 on success, 1 if the random source failed.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v4(out_ptr: *mut u8) -> i32 {
    match v4::new_v4() {
        Ok(uuid) => {
            // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
            unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
            0
        }
        Err(_) => 1,
    }
}

/// Writes a deterministic UUID version 5 (RFC 9562 §5.5) to `out_ptr` (16 bytes), derived
/// from a 16-byte namespace UUID at `ns_ptr` and a `name_len`-byte name at `name_ptr`. A
/// `name_len` of 0 never dereferences `name_ptr`, so an empty name may cross as null.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v5(
    ns_ptr: *const u8,
    name_ptr: *const u8,
    name_len: u32,
    out_ptr: *mut u8,
) -> i32 {
    // SAFETY: caller guarantees `ns_ptr` points to 16 live bytes and `name_ptr`/`name_len`
    // describe a live byte range, per the module contract.
    let namespace_bytes: [u8; 16] = unsafe { core::ptr::read(ns_ptr.cast::<[u8; 16]>()) };
    // name_len == 0 never touches name_ptr — several bindings pass null for an empty name
    // (C#'s fixed over an empty span, Go's nil slice, Fiddle's nil), and
    // `slice::from_raw_parts` requires non-null even for a 0-length slice.
    let name: &[u8] = if name_len == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(name_ptr, name_len as usize) }
    };

    let uuid = v5::new_v5(namespace_bytes.into(), name);
    // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
    unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
    0
}

/// Writes a time-sortable UUID version 6 (RFC 9562 §5.6) to `out_ptr` (16 bytes), embedding
/// `unix_millis` (milliseconds since the Unix epoch, supplied by the host — the guest has
/// no clock of its own). `clock_seq` and `node` are randomly generated on every call.
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v6(unix_millis: u64, out_ptr: *mut u8) -> i32 {
    match v6::new_v6(unix_millis) {
        Ok(uuid) => {
            // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
            unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
            0
        }
        Err(v6::NewV6Error::Random(_)) => 1,
        Err(v6::NewV6Error::TimestampOutOfRange) => 2,
        // Only a batch has a buffer to be short of; mapped anyway rather than panicking.
        Err(v6::NewV6Error::BufferTooSmall) => 3,
    }
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 6 UUID at `uuid_ptr`
/// (16 bytes). Pure bit-shifting over the caller's bytes — meaningful only for a genuine
/// version 6 UUID; the caller is responsible for checking the version first if that matters.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v6_unix_millis(uuid_ptr: *const u8) -> u64 {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live bytes, per the module contract.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    v6::unix_millis(&Uuid::from_bytes(bytes))
}

/// The version-agnostic timestamp read ([`crate::get_timestamp_in`]) in one call: for an RFC
/// 9562 version 6 or 7 UUID at `uuid_ptr` (16 bytes, held in `layout`'s byte order), writes
/// its Unix-epoch milliseconds to `millis_out` and returns the version, 6 or 7. For anything
/// else — another version, another variant, an unknown layout code — returns 0 and leaves
/// `millis_out` untouched.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_get_timestamp(
    uuid_ptr: *const u8,
    layout_code: u32,
    millis_out: *mut u64,
) -> u32 {
    match layout(layout_code).and_then(|layout| crate::timestamp_in(&read(uuid_ptr), layout)) {
        Some((version, millis)) => {
            // SAFETY: caller guarantees `millis_out` points to a live, writable u64.
            unsafe { millis_out.write_unaligned(millis) };
            version as u32
        }
        None => 0,
    }
}

/// [`uuid_v6_unix_millis`] for a version 6 UUID held in `layout`'s byte order, reading a
/// SQL-ordered value's permuted octets directly. Meaningful only for a genuine version 6
/// UUID in that layout ([`uuid_is_rfc`] is the check); 0 for an unknown layout code.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v6_unix_millis_in(uuid_ptr: *const u8, layout_code: u32) -> u64 {
    match layout(layout_code) {
        Some(layout) => v6::unix_millis_in(&read(uuid_ptr), layout),
        None => 0,
    }
}

/// Writes `count` time-sortable UUID version 6 values to `out_ptr` (`count * 16` bytes),
/// sharing one `unix_millis` timestamp capture. `clock_seq` and `node` are randomly
/// generated per item. A `count` of 0 is a no-op success.
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range,
/// 3 if `count * 16` overflows `usize` (32-bit targets only).
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v6_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32 {
    // count == 0 never touches out_ptr — some callers reasonably pass null/dangling for an
    // empty batch, and `slice::from_raw_parts_mut` requires non-null even for a 0-length slice.
    // A length that overflows `usize` crosses as an empty slice, which the core rejects as
    // too small for `count` — code 3, never a wrapped length.
    let out: &mut [u8] = match (count as usize).checked_mul(16) {
        Some(len) if len > 0 => {
            // SAFETY: caller guarantees `out_ptr` points to a live `count * 16`-byte allocation.
            unsafe { slice::from_raw_parts_mut(out_ptr, len) }
        }
        _ => &mut [],
    };
    match v6::new_v6_batch(unix_millis, count, out) {
        Ok(()) => 0,
        Err(v6::NewV6Error::Random(_)) => 1,
        Err(v6::NewV6Error::TimestampOutOfRange) => 2,
        Err(v6::NewV6Error::BufferTooSmall) => 3,
    }
}

/// Rewrites the 16 bytes at `uuid_ptr` in place from RFC 9562 order to the byte order SQL
/// Server's `uniqueidentifier` needs on the wire to sort a version 6 UUID by creation order.
/// See [`v6::to_sql_order`] for the byte-level rationale. Meaningful only for a genuine
/// version 6 UUID.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v6_to_sql_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    let sql = v6::to_sql_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(sql.as_bytes().as_ptr(), uuid_ptr, 16) };
}

/// Inverse of [`uuid_v6_to_sql_order`] — rewrites the 16 bytes at `uuid_ptr` in place from
/// SQL Server order back to RFC 9562 order.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v6_to_rfc_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    let rfc = v6::to_rfc_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(rfc.as_bytes().as_ptr(), uuid_ptr, 16) };
}

/// Writes a time-sortable UUID version 7 (RFC 9562 §6.2) to `out_ptr` (16 bytes), embedding
/// `unix_millis` (milliseconds since the Unix epoch, supplied by the host — the guest has
/// no clock of its own).
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v7(unix_millis: u64, out_ptr: *mut u8) -> i32 {
    match v7::new_v7(unix_millis) {
        Ok(uuid) => {
            // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
            unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
            0
        }
        Err(v7::NewV7Error::Random(_)) => 1,
        Err(v7::NewV7Error::TimestampOutOfRange) => 2,
        // Only a batch has a buffer to be short of; mapped anyway rather than panicking.
        Err(v7::NewV7Error::BufferTooSmall) => 3,
        Err(v7::NewV7Error::BatchTooLarge) => 4,
    }
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 7 UUID at `uuid_ptr`
/// (16 bytes). Pure bit-shifting over the caller's bytes — meaningful only for a genuine
/// version 7 UUID; the caller is responsible for checking the version first if that matters.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v7_unix_millis(uuid_ptr: *const u8) -> u64 {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live bytes, per the module contract.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    v7::unix_millis(&Uuid::from_bytes(bytes))
}

/// [`uuid_v7_unix_millis`] for a version 7 UUID held in `layout`'s byte order, reading a
/// SQL-ordered value's permuted octets directly. Meaningful only for a genuine version 7
/// UUID in that layout ([`uuid_is_rfc`] is the check); 0 for an unknown layout code.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v7_unix_millis_in(uuid_ptr: *const u8, layout_code: u32) -> u64 {
    match layout(layout_code) {
        Some(layout) => v7::unix_millis_in(&read(uuid_ptr), layout),
        None => 0,
    }
}

/// Writes `count` time-sortable UUID version 7 values to `out_ptr` (`count * 16` bytes),
/// sharing one `unix_millis` timestamp capture and one contiguous block of the monotonic
/// counter. A `count` of 0 is a no-op success. Items past the counter's wrap are stamped
/// `unix_millis + 1`, so the batch is always in order (see [`v7::new_v7_batch`]).
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range,
/// 4 if `count` exceeds [`v7::MAX_BATCH`] (2^26).
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_new_v7_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32 {
    // count == 0 never touches out_ptr — some callers reasonably pass null/dangling for an
    // empty batch, and `slice::from_raw_parts_mut` requires non-null even for a 0-length slice.
    // A length that overflows `usize` crosses as an empty slice, which the core rejects as
    // too small for `count` — code 3, never a wrapped length.
    let out: &mut [u8] = match (count as usize).checked_mul(16) {
        Some(len) if len > 0 => {
            // SAFETY: caller guarantees `out_ptr` points to a live `count * 16`-byte allocation.
            unsafe { slice::from_raw_parts_mut(out_ptr, len) }
        }
        _ => &mut [],
    };
    match v7::new_v7_batch(unix_millis, count, out) {
        Ok(()) => 0,
        Err(v7::NewV7Error::Random(_)) => 1,
        Err(v7::NewV7Error::TimestampOutOfRange) => 2,
        Err(v7::NewV7Error::BufferTooSmall) => 3,
        Err(v7::NewV7Error::BatchTooLarge) => 4,
    }
}

/// Rewrites the 16 bytes at `uuid_ptr` in place from RFC 9562 order to the byte order SQL
/// Server's `uniqueidentifier` needs on the wire to sort a version 7 UUID by creation order.
/// See [`v7::to_sql_order`] for the byte-level rationale. Meaningful only for a genuine
/// version 7 UUID.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v7_to_sql_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    let sql = v7::to_sql_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(sql.as_bytes().as_ptr(), uuid_ptr, 16) };
}

/// Inverse of [`uuid_v7_to_sql_order`] — rewrites the 16 bytes at `uuid_ptr` in place from
/// SQL Server order back to RFC 9562 order.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn uuid_v7_to_rfc_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { core::ptr::read(uuid_ptr.cast::<[u8; 16]>()) };
    let rfc = v7::to_rfc_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(rfc.as_bytes().as_ptr(), uuid_ptr, 16) };
}
