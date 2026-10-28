//! A Rust port of the RFC 9562 UUID generation from
//! [SequentialGuid](https://github.com/buvinghausen/SequentialGuid), covering the standard
//! UUID versions the RFC itself defines, plus [`v7::to_sql_order`]'s SQL Server byte-ordering
//! (SequentialGuid's permutation, written as an RFC 9562 version 8; see [`sql`]) — no ORM or
//! serializer integrations.
//!
//! - [`v4::new_v4`] — random (RFC 9562 §5.4)
//! - [`v5::new_v5`] — deterministic, namespace + name based, SHA-1 (RFC 9562 §5.5)
//! - [`v7::new_v7`] — time-sortable, millisecond timestamp + re-seeded counter (RFC 9562 §6.2)
//! - [`v7::to_sql_order`] / [`v7::to_rfc_order`] — the byte order SQL Server's
//!   `uniqueidentifier` needs to sort a version 7 UUID by creation order, written as an RFC
//!   9562 version 8, and back
//! - [`Uuid::NIL`] / [`Uuid::MAX`] — the all-zero and all-one special values (RFC 9562 §5.9/§5.10)
//! - [`get_timestamp`] / [`v7::new_v7_at`] — [`Timestamp`]-based
//!   convenience wrappers matching the `uuid` crate's own `get_timestamp`/`Timestamp` shape,
//!   for callers who'd rather not pass a raw millisecond count around
//! - [`Uuid::version`] / [`Uuid::variant`] / [`Uuid::is_rfc`] — inspection, and
//!   [`Uuid::version_in`] / [`Uuid::is_rfc_in`] / [`get_timestamp_in`] for a value still in
//!   SQL Server order ([`Layout`])

#![deny(missing_docs)]
// The crate publishes the `no-std` and `no-std::no-alloc` categories, and this is what makes
// both compiler-enforced rather than claimed. Five things reached past `core` and none do
// now: the error types implement `core::error::Error` (stable since 1.81), `getrandom`'s
// std-only error impl rides the feature rather than being pinned on, `v7::now_v7` joins the
// `wasm32` gate it already had (it's the one API needing an OS clock), `v7`'s stream is one
// `core::sync::atomic` word seeded by its first compare-and-swap rather than through std's
// `OnceLock` (the never-reset counter kept for targets without 64-bit atomics folds its seed
// in lock-free instead; `v7::fallback` has why that keeps its ordering intact), and the two
// `*_batch` functions draw their entropy through the caller's own buffer instead of a
// `count`-sized scratch buffer, which is what retires the crate's last allocation. There is no
// `extern crate alloc` here on purpose: nothing needs it, and a consumer with no heap at all
// is a consumer this crate can now serve.
//
// Gated on the default-on `std` feature rather than unconditional, because the extension
// modules (python, ruby, php) need std through their own dependencies and the test harness
// needs it to run at all. Every artifact this crate ships itself is built without it: the
// static libraries (`cargo staticlib`) and the shared library every binding dlopens
// (`cargo cdylib`), which bring the panic handler below in std's place. A no_std rlib
// consumer takes the crate with `default-features = false` and, where the target has no
// operating system, supplies a panic handler plus a `getrandom` custom backend, the way
// such a consumer must anyway. Verified against a real target rather than argued:
//
//     RUSTFLAGS='--cfg getrandom_backend="custom"' \
//         cargo check --no-default-features --target thumbv7em-none-eabi
#![cfg_attr(not(feature = "std"), no_std)]

// The panic handler for the no_std artifacts this crate links itself: the static libraries
// behind the `staticlib` feature and the shared library behind `cdylib` (Cargo.toml has
// what each is and why neither carries std). Built with `panic = "abort"`, so a panic ends
// the program instead of unwinding into the host: on wasm32 it is the `unreachable` trap,
// which the host sees as a RuntimeError rather than as a corrupted return value; anywhere
// else it is the C library's `abort`, which every process the library is loaded or linked
// into already has. Never compiled for a bare-metal rlib consumer, who brings a handler of
// their own, nor with `std`, which has one.
#[cfg(all(feature = "staticlib", not(feature = "std")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    {
        unsafe extern "C" {
            safe fn abort() -> !;
        }
        abort()
    }
}

// The one symbol a no_std shared library can need that a static library does not. `core`
// and `compiler_builtins` ship precompiled with unwind tables, and an entry linked in from
// them can name `rust_eh_personality` (in HyperCast, that of the 128-bit division
// intrinsic). In a static library the reference is left for the final link to resolve; a
// shared library is the final link, so without a definition every dlopen fails with
// "undefined symbol: rust_eh_personality". Nothing in this crate pulls such an entry in
// today, so the linker drops the definition; it is here so that a change which does pull
// one in still produces a library that loads. With `panic = "abort"` nothing ever unwinds,
// so nothing ever calls it — the definition only has to exist.
//
// Assembly rather than a `#[no_mangle]` function, for the visibility: a `#[no_mangle]`
// item is exported from a cdylib whatever its Rust visibility, which would put a symbol
// beside the C ABI exports that any other library in the process could bind to. Defined
// hidden instead, it satisfies the library's own reference and is seen by nothing outside
// it. Never compiled for the static libraries: two Hyper* archives that each defined it
// could not be linked into one program, the duplicate-symbol failure that kept std out of
// them in the first place. Windows needs no definition (its unwind tables name the C
// runtime's handler), and wasm32 has no unwind tables to name one.
#[cfg(all(feature = "cdylib", not(feature = "std"), target_vendor = "apple"))]
core::arch::global_asm!(
    ".globl _rust_eh_personality",
    ".private_extern _rust_eh_personality",
    "_rust_eh_personality:",
    "ret",
);
#[cfg(all(
    feature = "cdylib",
    not(feature = "std"),
    not(target_vendor = "apple"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
))]
core::arch::global_asm!(
    ".globl rust_eh_personality",
    ".hidden rust_eh_personality",
    ".type rust_eh_personality, %function",
    "rust_eh_personality:",
    "ret",
);

// The C runtime. Under std it comes in through std's own link directives; a no_std shared
// library has to name it, or nothing does. On Linux the library then links with no NEEDED
// entry at all, its imports unversioned and resolved only because the host process happens
// to have libc loaded; on Windows nothing names the runtime its `memcpy` and DLL entry point
// come from. The choice mirrors the one std makes by way of the libc crate — libc on Unix,
// and on Windows the DLL import library ordinarily or the static runtime when crt-static
// asks for it — so the library depends on exactly the C runtime the std build did and
// nothing more.
#[cfg(all(feature = "cdylib", not(feature = "std"), unix))]
#[link(name = "c")]
unsafe extern "C" {}
#[cfg(all(feature = "cdylib", not(feature = "std"), target_env = "msvc"))]
#[cfg_attr(target_feature = "crt-static", link(name = "libcmt"))]
#[cfg_attr(not(target_feature = "crt-static"), link(name = "msvcrt"))]
unsafe extern "C" {}

mod entropy;
mod ffi;
pub mod sql;
// The v7 stream is one 64-bit word swapped as a whole; a target without 64-bit atomics keeps
// v7's never-reset counter instead (`v7::fallback`).
#[cfg(target_has_atomic = "64")]
mod stream;
mod timestamp;
mod uuid;
pub mod v4;
pub mod v5;
pub mod v7;

#[cfg(feature = "php")]
mod php_ext;
#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;

pub use ffi::hyperuuid_version;
pub use sql::SqlOrderError;
pub use timestamp::Timestamp;
pub use uuid::{Layout, ParseUuidError, Uuid, Variant};

/// Returns the Unix-epoch [`Timestamp`] embedded in `uuid`, or `None` if it isn't an RFC 9562
/// version 7 UUID — the same `Option`-returning shape as the `uuid` crate's own
/// `Uuid::get_timestamp`, so a caller doesn't need to already know (or separately check) the
/// version before asking. "RFC 9562" is part of the check: a value whose variant isn't
/// [`Variant::Rfc9562`] has no version, so a 7 in its version nibble carries no timestamp
/// either ([`Uuid::is_rfc`]). Version 7 is the only version this crate generates with a
/// timestamp, so it is the only one it reads one from. Delegates straight to
/// [`v7::unix_millis`], with no bit-layout logic duplicated here.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn get_timestamp(uuid: &Uuid) -> Option<Timestamp> {
    get_timestamp_in(uuid, Layout::Rfc9562)
}

/// The version and Unix-epoch milliseconds [`get_timestamp_in`] answers with: `(7, millis)`
/// for an RFC 9562 version 7 UUID held in `layout`'s byte order, `None` for anything else. One read for both, so the C ABI can hand back the pair in one call.
#[inline]
pub(crate) fn timestamp_in(uuid: &Uuid, layout: Layout) -> Option<(u8, u64)> {
    if uuid.is_rfc_in(7, layout) {
        Some((7, v7::unix_millis_in(uuid, layout)))
    } else {
        None
    }
}

/// [`get_timestamp`] for a UUID held in `layout`'s byte order: in [`Layout::SqlServer`], the
/// timestamp of a SQL-ordered version 7, read straight from its permuted octets with no
/// conversion back to RFC order first, or `None` for anything else ([`Uuid::version_in`]).
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn get_timestamp_in(uuid: &Uuid, layout: Layout) -> Option<Timestamp> {
    timestamp_in(uuid, layout).map(|(_, millis)| Timestamp::from_unix_millis(millis))
}

// The unit tests are in their own file, `tests.rs`: CodeQL's path filters are per file, and a
// test that formats a UUID into a panic message is not a finding worth carrying against the
// module that ships. See `.github/codeql/codeql-config.yml`.
#[cfg(test)]
mod tests;
