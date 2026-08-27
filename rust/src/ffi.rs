//! C-ABI exports — this crate's `crate-type` is `cdylib`, so this same source produces a
//! native `libhyperuuid.so`/`.dylib`/`.dll` loaded through ordinary P/Invoke/FFM/ctypes-style
//! FFI. This is the one contract every host binding calls through: a caller shares this
//! library's address space directly (no separate guest/host memory boundary to bridge), so
//! every export just takes plain pointers into the caller's own stack- or heap-allocated
//! buffers — no allocator exports, no protocol beyond "here's a 16-byte buffer, fill it in".
//!
//! Return codes: `0` success, `1` random source failure, `2` timestamp out of range.

use crate::{v4, v5, v6, v7, Uuid};
use core::slice;

/// Writes a random UUID version 4 (RFC 9562 §5.4) to `out_ptr` (16 bytes).
/// Returns 0 on success, 1 if the random source failed.
#[unsafe(no_mangle)]
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
/// from a 16-byte namespace UUID at `ns_ptr` and a `name_len`-byte name at `name_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_new_v5(
    ns_ptr: *const u8,
    name_ptr: *const u8,
    name_len: u32,
    out_ptr: *mut u8,
) -> i32 {
    // SAFETY: caller guarantees `ns_ptr` points to 16 live bytes and `name_ptr`/`name_len`
    // describe a live byte range, per the module contract.
    let namespace_bytes: [u8; 16] = unsafe { slice::from_raw_parts(ns_ptr, 16) }
        .try_into()
        .unwrap();
    let name = unsafe { slice::from_raw_parts(name_ptr, name_len as usize) };

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
pub extern "C" fn uuid_new_v6(unix_millis: u64, out_ptr: *mut u8) -> i32 {
    match v6::new_v6(unix_millis) {
        Ok(uuid) => {
            // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
            unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
            0
        }
        Err(v6::NewV6Error::Random(_)) => 1,
        Err(v6::NewV6Error::TimestampOutOfRange) => 2,
    }
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 6 UUID at `uuid_ptr`
/// (16 bytes). Pure bit-shifting over the caller's bytes — meaningful only for a genuine
/// version 6 UUID; the caller is responsible for checking the version first if that matters.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_v6_unix_millis(uuid_ptr: *const u8) -> u64 {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live bytes, per the module contract.
    let bytes: [u8; 16] = unsafe { slice::from_raw_parts(uuid_ptr, 16) }
        .try_into()
        .unwrap();
    v6::unix_millis(&Uuid::from_bytes(bytes))
}

/// Writes `count` time-sortable UUID version 6 values to `out_ptr` (`count * 16` bytes),
/// sharing one `unix_millis` timestamp capture. `clock_seq` and `node` are randomly
/// generated per item. A `count` of 0 is a no-op success.
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_new_v6_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32 {
    // count == 0 never touches out_ptr — some callers reasonably pass null/dangling for an
    // empty batch, and `slice::from_raw_parts_mut` requires non-null even for a 0-length slice.
    let out: &mut [u8] = if count == 0 {
        &mut []
    } else {
        // SAFETY: caller guarantees `out_ptr` points to a live `count * 16`-byte allocation.
        unsafe { slice::from_raw_parts_mut(out_ptr, count as usize * 16) }
    };
    match v6::new_v6_batch(unix_millis, count, out) {
        Ok(()) => 0,
        Err(v6::NewV6Error::Random(_)) => 1,
        Err(v6::NewV6Error::TimestampOutOfRange) => 2,
    }
}

/// Writes a time-sortable UUID version 7 (RFC 9562 §6.2) to `out_ptr` (16 bytes), embedding
/// `unix_millis` (milliseconds since the Unix epoch, supplied by the host — the guest has
/// no clock of its own).
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_new_v7(unix_millis: u64, out_ptr: *mut u8) -> i32 {
    match v7::new_v7(unix_millis) {
        Ok(uuid) => {
            // SAFETY: caller guarantees `out_ptr` points to a live 16-byte allocation.
            unsafe { core::ptr::copy_nonoverlapping(uuid.as_bytes().as_ptr(), out_ptr, 16) };
            0
        }
        Err(v7::NewV7Error::Random(_)) => 1,
        Err(v7::NewV7Error::TimestampOutOfRange) => 2,
    }
}

/// Extracts the Unix-epoch millisecond timestamp embedded in a version 7 UUID at `uuid_ptr`
/// (16 bytes). Pure bit-shifting over the caller's bytes — meaningful only for a genuine
/// version 7 UUID; the caller is responsible for checking the version first if that matters.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_v7_unix_millis(uuid_ptr: *const u8) -> u64 {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live bytes, per the module contract.
    let bytes: [u8; 16] = unsafe { slice::from_raw_parts(uuid_ptr, 16) }
        .try_into()
        .unwrap();
    v7::unix_millis(&Uuid::from_bytes(bytes))
}

/// Writes `count` time-sortable UUID version 7 values to `out_ptr` (`count * 16` bytes),
/// sharing one `unix_millis` timestamp capture and one contiguous block of the monotonic
/// counter. A `count` of 0 is a no-op success.
/// Returns 0 on success, 1 if the random source failed, 2 if `unix_millis` is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_new_v7_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32 {
    // count == 0 never touches out_ptr — some callers reasonably pass null/dangling for an
    // empty batch, and `slice::from_raw_parts_mut` requires non-null even for a 0-length slice.
    let out: &mut [u8] = if count == 0 {
        &mut []
    } else {
        // SAFETY: caller guarantees `out_ptr` points to a live `count * 16`-byte allocation.
        unsafe { slice::from_raw_parts_mut(out_ptr, count as usize * 16) }
    };
    match v7::new_v7_batch(unix_millis, count, out) {
        Ok(()) => 0,
        Err(v7::NewV7Error::Random(_)) => 1,
        Err(v7::NewV7Error::TimestampOutOfRange) => 2,
    }
}

/// Rewrites the 16 bytes at `uuid_ptr` in place from RFC 9562 order to the byte order SQL
/// Server's `uniqueidentifier` needs on the wire to sort a version 7 UUID by creation order.
/// See [`v7::to_sql_order`] for the byte-level rationale. Meaningful only for a genuine
/// version 7 UUID.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_v7_to_sql_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { slice::from_raw_parts(uuid_ptr, 16) }.try_into().unwrap();
    let sql = v7::to_sql_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(sql.as_bytes().as_ptr(), uuid_ptr, 16) };
}

/// Inverse of [`uuid_v7_to_sql_order`] — rewrites the 16 bytes at `uuid_ptr` in place from
/// SQL Server order back to RFC 9562 order.
#[unsafe(no_mangle)]
pub extern "C" fn uuid_v7_to_rfc_order(uuid_ptr: *mut u8) {
    // SAFETY: caller guarantees `uuid_ptr` points to 16 live, writable bytes.
    let bytes: [u8; 16] = unsafe { slice::from_raw_parts(uuid_ptr, 16) }.try_into().unwrap();
    let rfc = v7::to_rfc_order(&Uuid::from_bytes(bytes));
    unsafe { core::ptr::copy_nonoverlapping(rfc.as_bytes().as_ptr(), uuid_ptr, 16) };
}
