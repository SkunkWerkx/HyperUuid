//! C-ABI exports — this crate's `crate-type` is `cdylib`, so this same source produces a
//! native `libhyperuuid.so`/`.dylib`/`.dll` loaded through ordinary P/Invoke/FFM/ctypes-style
//! FFI. This is the one contract every host binding calls through: a caller shares this
//! library's address space directly (no separate guest/host memory boundary to bridge), so
//! every export just takes plain pointers into the caller's own stack- or heap-allocated
//! buffers — no allocator exports, no protocol beyond "here's a 16-byte buffer, fill it in".
//!
//! Return codes: `0` success, `1` random source failure, `2` timestamp out of range.

use crate::{v4, v5, v7};
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
