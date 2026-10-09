//! A Rust port of the RFC 9562 UUID generation from
//! [SequentialGuid](https://github.com/buvinghausen/SequentialGuid), covering the standard
//! UUID versions the RFC itself defines, plus [`v7::to_sql_order`]'s SQL Server byte-ordering
//! (ported from this project's own [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim))
//! — no ORM or serializer integrations.
//!
//! - [`v4::new_v4`] — random (RFC 9562 §5.4)
//! - [`v5::new_v5`] — deterministic, namespace + name based, SHA-1 (RFC 9562 §5.5)
//! - [`v6::new_v6`] — time-sortable, v1-field-compatible reordering (RFC 9562 §5.6)
//! - [`v7::new_v7`] — time-sortable, millisecond timestamp + monotonic counter (RFC 9562 §6.2)
//! - [`v7::to_sql_order`] / [`v7::to_rfc_order`] — the byte order SQL Server's
//!   `uniqueidentifier` needs to sort a version 7 UUID by creation order, and back
//! - [`Uuid::NIL`] / [`Uuid::MAX`] — the all-zero and all-one special values (RFC 9562 §5.9/§5.10)
//! - [`get_timestamp`] / [`v6::new_v6_at`] / [`v7::new_v7_at`] — [`Timestamp`]-based
//!   convenience wrappers matching the `uuid` crate's own `get_timestamp`/`Timestamp` shape,
//!   for callers who'd rather not pass a raw millisecond count around
//! - [`Uuid::version`] / [`Uuid::variant`] / [`Uuid::is_rfc`] — inspection, and
//!   [`Uuid::version_in`] / [`Uuid::is_rfc_in`] / [`get_timestamp_in`] for a value still in
//!   SQL Server order ([`Layout`])

#![deny(missing_docs)]
// The crate publishes the `no-std` and `no-std::no-alloc` categories, and this is what makes
// both compiler-enforced rather than claimed. Five things reached past `core` and none do
// now: the two error types implement `core::error::Error` (stable since 1.81), `getrandom`'s
// std-only error impl rides the feature rather than being pinned on, `v7::now_v7` joins the
// `wasm32` gate it already had (it's the one API needing an OS clock), `v7`'s monotonic
// counter trades std's `OnceLock` for a lock-free seed fold over `core::sync::atomic` (see
// `v7::counter` for why that keeps the ordering guarantee intact), and the two `*_batch`
// functions draw their entropy through the caller's own buffer instead of a `count`-sized
// scratch buffer, which is what retires the crate's last allocation. There is no
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
mod timestamp;
mod uuid;
pub mod v4;
pub mod v5;
pub mod v6;
pub mod v7;

#[cfg(feature = "php")]
mod php_ext;
#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;

pub use ffi::hyperuuid_version;
pub use timestamp::Timestamp;
pub use uuid::{Layout, ParseUuidError, Uuid, Variant};

/// Returns the Unix-epoch [`Timestamp`] embedded in `uuid`, or `None` if it isn't an RFC 9562
/// version 6 or 7 UUID — the same `Option`-returning shape as the `uuid` crate's own
/// `Uuid::get_timestamp`, so a caller doesn't need to already know (or separately check) the
/// version before asking. "RFC 9562" is part of the check: a value whose variant isn't
/// [`Variant::Rfc9562`] has no version, so a 6 or 7 in its version nibble carries no
/// timestamp either ([`Uuid::is_rfc`]). Delegates straight to
/// [`v6::unix_millis`]/[`v7::unix_millis`], with no bit-layout logic duplicated here.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn get_timestamp(uuid: &Uuid) -> Option<Timestamp> {
    get_timestamp_in(uuid, Layout::Rfc9562)
}

/// The version and Unix-epoch milliseconds [`get_timestamp_in`] answers with: `(6 | 7, millis)`
/// for an RFC 9562 version 6 or 7 UUID held in `layout`'s byte order, `None` for anything
/// else. One read for both, so the C ABI can hand back the pair in one call.
#[inline]
pub(crate) fn timestamp_in(uuid: &Uuid, layout: Layout) -> Option<(u8, u64)> {
    if uuid.is_rfc_in(7, layout) {
        Some((7, v7::unix_millis_in(uuid, layout)))
    } else if uuid.is_rfc_in(6, layout) {
        Some((6, v6::unix_millis_in(uuid, layout)))
    } else {
        None
    }
}

/// [`get_timestamp`] for a UUID held in `layout`'s byte order: in [`Layout::SqlServer`], the
/// timestamp of a SQL-ordered version 6 or 7, read straight from its permuted octets with no
/// conversion back to RFC order first, or `None` for anything else ([`Uuid::version_in`] has
/// how the two are told apart).
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn get_timestamp_in(uuid: &Uuid, layout: Layout) -> Option<Timestamp> {
    timestamp_in(uuid, layout).map(|(_, millis)| Timestamp::from_unix_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    #[test]
    fn version_export_packs_the_crate_version() {
        let packed = hyperuuid_version();
        let text = format!(
            "{}.{}.{}",
            packed >> 16,
            (packed >> 8) & 0xFF,
            packed & 0xFF
        );
        assert_eq!(text, env!("CARGO_PKG_VERSION"));
    }

    // The C ABI's own contract: an empty v5 name may cross as a null pointer, which is what
    // C#, Go and Ruby's Fiddle backend hand over for a zero-length name.
    #[test]
    fn v5_export_accepts_a_null_pointer_for_an_empty_name() {
        let mut out = [0u8; 16];
        let rc = ffi::uuid_new_v5(
            v5::namespace::DNS.as_bytes().as_ptr(),
            core::ptr::null(),
            0,
            out.as_mut_ptr(),
        );
        assert_eq!(rc, 0);
        assert_eq!(Uuid::from_bytes(out), v5::new_v5(v5::namespace::DNS, b""));
    }

    #[test]
    fn v4_has_version_and_variant_bits_set() {
        let id = v4::new_v4().unwrap();
        assert_eq!(id.version(), 4);
        assert!(id.is_rfc9562_variant());
    }

    #[test]
    fn v4_is_non_deterministic() {
        let a = v4::new_v4().unwrap();
        let b = v4::new_v4().unwrap();
        assert_ne!(a, b);
    }

    // RFC 9562 Appendix A.4 official test vector.
    #[test]
    fn v5_matches_rfc_test_vector() {
        let id = v5::new_v5(v5::namespace::DNS, b"www.example.com");
        assert_eq!(
            id,
            Uuid::from_str("2ed6657d-e927-568b-95e1-2665a8aea6a2").unwrap()
        );
    }

    // Python's `uuid` standard library documentation test vector.
    #[test]
    fn v5_matches_python_docs_vector() {
        let id = v5::new_v5(v5::namespace::DNS, b"python.org");
        assert_eq!(
            id,
            Uuid::from_str("886313e1-3b8a-5372-9b90-0c9aee199e5d").unwrap()
        );
    }

    #[test]
    fn v5_is_deterministic() {
        let a = v5::new_v5(v5::namespace::DNS, b"same-name");
        let b = v5::new_v5(v5::namespace::DNS, b"same-name");
        assert_eq!(a, b);
    }

    #[test]
    fn v5_different_names_differ() {
        let a = v5::new_v5(v5::namespace::DNS, b"name-a");
        let b = v5::new_v5(v5::namespace::DNS, b"name-b");
        assert_ne!(a, b);
    }

    #[test]
    fn v5_different_namespaces_differ() {
        let a = v5::new_v5(v5::namespace::DNS, b"test");
        let b = v5::new_v5(v5::namespace::URL, b"test");
        assert_ne!(a, b);
    }

    #[test]
    fn v5_has_version_and_variant_bits_set() {
        let id = v5::new_v5(v5::namespace::DNS, b"test");
        assert_eq!(id.version(), 5);
        assert!(id.is_rfc9562_variant());
    }

    // RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
    const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

    #[test]
    fn v7_embeds_the_timestamp() {
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(v7::unix_millis(&id), RFC_TEST_VECTOR_MS);
    }

    #[test]
    fn v7_has_version_and_variant_bits_set() {
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(id.version(), 7);
        assert!(id.is_rfc9562_variant());
    }

    #[test]
    fn v7_zero_timestamp_succeeds() {
        let id = v7::new_v7(0).unwrap();
        assert_eq!(v7::unix_millis(&id), 0);
    }

    #[test]
    fn v7_max_timestamp_succeeds() {
        let id = v7::new_v7(v7::MAX_UNIX_MILLIS).unwrap();
        assert_eq!(v7::unix_millis(&id), v7::MAX_UNIX_MILLIS);
    }

    #[test]
    fn v7_overflow_timestamp_errors() {
        let err = v7::new_v7(v7::MAX_UNIX_MILLIS + 1).unwrap_err();
        assert_eq!(err, v7::NewV7Error::TimestampOutOfRange);
    }

    #[test]
    fn v7_same_millisecond_batch_is_monotonically_ordered() {
        let ids: Vec<Uuid> = (0..100)
            .map(|_| v7::new_v7(RFC_TEST_VECTOR_MS).unwrap())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn a_timestamp_past_u64_millis_is_out_of_range_not_wrapped() {
        let far = Timestamp::from_unix(u64::MAX / 1000 + 1, 0);
        assert_eq!(far.to_unix_millis(), u64::MAX);
        assert_eq!(
            v6::new_v6_at(far).unwrap_err(),
            v6::NewV6Error::TimestampOutOfRange
        );
        assert_eq!(
            v7::new_v7_at(far).unwrap_err(),
            v7::NewV7Error::TimestampOutOfRange
        );
    }

    #[test]
    fn v7_increasing_timestamps_sort_in_creation_order() {
        const BASE_MS: u64 = 1_000_000;
        let ids: Vec<Uuid> = (0..10).map(|i| v7::new_v7(BASE_MS + i).unwrap()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn uuid_display_and_from_str_round_trip() {
        let id = v5::new_v5(v5::namespace::DNS, b"round-trip");
        let text = id.to_string();
        assert_eq!(Uuid::from_str(&text).unwrap(), id);
    }

    #[test]
    fn v6_embeds_the_timestamp() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(v6::unix_millis(&id), RFC_TEST_VECTOR_MS);
    }

    #[test]
    fn v6_has_version_and_variant_bits_set() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(id.version(), 6);
        assert!(id.is_rfc9562_variant());
    }

    #[test]
    fn v6_zero_timestamp_succeeds() {
        let id = v6::new_v6(0).unwrap();
        assert_eq!(v6::unix_millis(&id), 0);
    }

    #[test]
    fn v6_is_non_deterministic_within_the_same_millisecond() {
        let a = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let b = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn v6_sets_the_node_id_multicast_bit() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(id.as_bytes()[10] & 0x01, 0x01);
    }

    #[test]
    fn v6_increasing_timestamps_sort_in_creation_order() {
        const BASE_MS: u64 = 1_000_000;
        let ids: Vec<Uuid> = (0..10).map(|i| v6::new_v6(BASE_MS + i).unwrap()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn nil_uuid_is_all_zero_bytes() {
        assert_eq!(Uuid::NIL.as_bytes(), &[0u8; 16]);
    }

    #[test]
    fn max_uuid_is_all_one_bytes() {
        assert_eq!(Uuid::MAX.as_bytes(), &[0xFFu8; 16]);
    }

    #[test]
    fn nil_and_max_round_trip_through_display_and_from_str() {
        assert_eq!(Uuid::from_str(&Uuid::NIL.to_string()).unwrap(), Uuid::NIL);
        assert_eq!(Uuid::from_str(&Uuid::MAX.to_string()).unwrap(), Uuid::MAX);
    }

    #[test]
    fn from_str_rejects_a_misplaced_hyphen_instead_of_panicking() {
        // 36 bytes with the four fixed hyphens in place, plus a fifth that shifts the last
        // group's digits; the old parser read one byte past the end of this string.
        for text in [
            "00000000-0000-0000-0000--00000000000",
            "-0000000-0000-0000-0000-000000000000",
            "00000000-0000-0000-0000-00000000000-",
        ] {
            assert_eq!(text.len(), 36);
            assert_eq!(Uuid::from_str(text), Err(ParseUuidError));
        }
    }

    #[test]
    fn v6_batch_matches_single_call_generation() {
        let mut out = vec![0u8; 5 * 16];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 5, &mut out).unwrap();
        for &bytes in out.as_chunks::<16>().0 {
            let id = Uuid::from_bytes(bytes);
            assert_eq!(id.version(), 6);
            assert!(id.is_rfc9562_variant());
            assert_eq!(v6::unix_millis(&id), RFC_TEST_VECTOR_MS);
        }
    }

    #[test]
    fn v6_batch_items_are_pairwise_distinct() {
        let mut out = vec![0u8; 100 * 16];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 100, &mut out).unwrap();
        let ids: std::collections::HashSet<[u8; 16]> =
            out.as_chunks::<16>().0.iter().copied().collect();
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn v6_batch_zero_count_is_a_no_op() {
        let mut out: [u8; 0] = [];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 0, &mut out).unwrap();
    }

    #[test]
    fn v6_batch_short_buffer_errors_and_leaves_it_untouched() {
        let mut out = [0xAAu8; 31];
        let err = v6::new_v6_batch(RFC_TEST_VECTOR_MS, 2, &mut out).unwrap_err();
        assert_eq!(err, v6::NewV6Error::BufferTooSmall);
        assert_eq!(out, [0xAAu8; 31]);
    }

    #[test]
    fn v6_batch_overflow_timestamp_errors() {
        let mut out = vec![0u8; 16];
        let err = v6::new_v6_batch(u64::MAX, 1, &mut out).unwrap_err();
        assert_eq!(err, v6::NewV6Error::TimestampOutOfRange);
    }

    #[test]
    fn v7_batch_matches_single_call_generation() {
        let mut out = vec![0u8; 5 * 16];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 5, &mut out).unwrap();
        for &bytes in out.as_chunks::<16>().0 {
            let id = Uuid::from_bytes(bytes);
            assert_eq!(id.version(), 7);
            assert!(id.is_rfc9562_variant());
            assert_eq!(v7::unix_millis(&id), RFC_TEST_VECTOR_MS);
        }
    }

    #[test]
    fn v7_batch_is_monotonically_ordered() {
        let mut out = vec![0u8; 1000 * 16];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 1000, &mut out).unwrap();
        let ids: Vec<Uuid> = out
            .as_chunks::<16>()
            .0
            .iter()
            .map(|&c| Uuid::from_bytes(c))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn v7_batch_continues_the_same_counter_sequence_as_individual_calls() {
        // A batch call shouldn't collide with (or reorder relative to) individual calls
        // interleaved around it on the same shared counter.
        let before = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let mut batch = vec![0u8; 10 * 16];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 10, &mut batch).unwrap();
        let after = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();

        let mut ids = vec![before];
        let (batch_ids, _) = batch.as_chunks::<16>();
        ids.extend(batch_ids.iter().map(|&c| Uuid::from_bytes(c)));
        ids.push(after);

        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    /// Both batch functions draw the whole batch's entropy into the *front* of `out`, then move
    /// each item's share rightwards into its final octets, walking the batch backwards so a
    /// write never lands on entropy that hasn't been consumed yet. Reverse that walk and the
    /// later items' tails get overwritten with timestamp/counter/version bytes, which are
    /// identical or near-identical across a batch — so distinct tails are what actually proves
    /// the placement correct.
    ///
    /// This checks the tails specifically rather than whole UUIDs because for v7 the monotonic
    /// counter keeps the values distinct and ordered even when the entropy is wrecked: every
    /// other v7 batch test here would still pass.
    #[test]
    fn v7_batch_trailing_entropy_is_distinct_per_item() {
        const COUNT: usize = 500;
        let mut out = vec![0u8; COUNT * 16];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, COUNT as u32, &mut out).unwrap();

        let tails: std::collections::HashSet<[u8; 6]> = out
            .as_chunks::<16>()
            .0
            .iter()
            .map(|item| item[10..16].try_into().unwrap())
            .collect();
        assert_eq!(tails.len(), COUNT, "rand_b tails repeated across the batch");
    }

    /// v6's counterpart to [`v7_batch_trailing_entropy_is_distinct_per_item`]. v6 has no
    /// counter, so `clock_seq`/`node` are the only thing separating same-millisecond items and
    /// a botched walk would show up as outright duplicate UUIDs too — but checking the node
    /// field directly says which invariant broke rather than just that something did.
    #[test]
    fn v6_batch_node_entropy_is_distinct_per_item() {
        const COUNT: usize = 500;
        let mut out = vec![0u8; COUNT * 16];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, COUNT as u32, &mut out).unwrap();

        let nodes: std::collections::HashSet<[u8; 6]> = out
            .as_chunks::<16>()
            .0
            .iter()
            .map(|item| item[10..16].try_into().unwrap())
            .collect();
        assert_eq!(nodes.len(), COUNT, "node IDs repeated across the batch");
    }

    #[test]
    fn v7_batch_zero_count_is_a_no_op() {
        let mut out: [u8; 0] = [];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 0, &mut out).unwrap();
    }

    #[test]
    fn v7_batch_short_buffer_errors_and_leaves_it_untouched() {
        let mut out = [0xAAu8; 31];
        let err = v7::new_v7_batch(RFC_TEST_VECTOR_MS, 2, &mut out).unwrap_err();
        assert_eq!(err, v7::NewV7Error::BufferTooSmall);
        assert_eq!(out, [0xAAu8; 31]);
    }

    #[test]
    fn v7_batch_overflow_timestamp_errors() {
        let mut out = vec![0u8; 16];
        let err = v7::new_v7_batch(v7::MAX_UNIX_MILLIS + 1, 1, &mut out).unwrap_err();
        assert_eq!(err, v7::NewV7Error::TimestampOutOfRange);
    }

    #[test]
    fn v7_sql_order_round_trips() {
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let sql = v7::to_sql_order(&id);
        assert_ne!(
            sql, id,
            "a real timestamp/counter should actually move bytes around"
        );
        assert_eq!(v7::to_rfc_order(&sql), id);
    }

    #[test]
    fn v7_sql_order_zero_and_max_round_trip() {
        for id in [
            v7::new_v7(0).unwrap(),
            v7::new_v7(v7::MAX_UNIX_MILLIS).unwrap(),
        ] {
            assert_eq!(v7::to_rfc_order(&v7::to_sql_order(&id)), id);
        }
    }

    #[test]
    fn v7_sql_order_preserves_version_and_variant_at_octets_7_and_8() {
        // Matches Svartalfheim's own documented invariant: version/variant sit at the same
        // byte-and-nibble offsets in both orderings, so a value's version is readable without
        // first knowing which order it's in.
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let sql = v7::to_sql_order(&id);
        assert_eq!(sql.as_bytes()[7] & 0xF0, 0x70);
        assert_eq!(sql.as_bytes()[8] & 0xC0, 0x80);
    }

    #[test]
    fn v7_sql_order_extracts_the_same_timestamp_after_converting_back() {
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let round_tripped = v7::to_rfc_order(&v7::to_sql_order(&id));
        assert_eq!(v7::unix_millis(&round_tripped), RFC_TEST_VECTOR_MS);
    }

    /// Replicates `System.Data.SqlTypes.SqlGuid.CompareTo` — and therefore T-SQL `ORDER BY`
    /// on a `uniqueidentifier` column — which compares a GUID's 16 bytes in this fixed
    /// significance order rather than left to right. This is the correctness oracle for
    /// [`v7::to_sql_order`]: no real SQL Server available in this crate's test suite, so this
    /// stands in for it, the same role Svartalfheim's own tests use the real `SqlGuid` for.
    fn sql_guid_cmp(a: &[u8; 16], b: &[u8; 16]) -> std::cmp::Ordering {
        const SIGNIFICANCE_ORDER: [usize; 16] =
            [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3];
        for &i in &SIGNIFICANCE_ORDER {
            match a[i].cmp(&b[i]) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        std::cmp::Ordering::Equal
    }

    #[test]
    fn v7_sql_order_sorts_by_creation_order_under_sqlguid_comparison() {
        // Increasing timestamps, one per millisecond...
        let mut ids: Vec<Uuid> = (0..200)
            .map(|i| v7::new_v7(1_000_000 + i).unwrap())
            .collect();
        // ...plus a same-millisecond run, so the counter (not just the timestamp) has to sort
        // correctly too.
        ids.extend((0..200).map(|_| v7::new_v7(5_000_000).unwrap()));

        let sql: Vec<[u8; 16]> = ids
            .iter()
            .map(|id| *v7::to_sql_order(id).as_bytes())
            .collect();
        let mut sorted = sql.clone();
        sorted.sort_by(sql_guid_cmp);
        assert_eq!(
            sql, sorted,
            "SqlGuid-order comparison of SQL-ordered bytes must match creation order"
        );
    }

    /// Proves [`v7::unix_millis`] isn't just reading back what our own [`v7::new_v7`] wrote —
    /// it's a plain RFC 9562 bit-layout read, so it recovers the real embedded timestamp from
    /// a version 7 UUID minted by a completely independent implementation too. `::uuid` here
    /// is the external `uuid` crate dev-dependency, disambiguated by the leading `::` from
    /// this crate's own private `uuid` module of the same name.
    #[test]
    fn v7_timestamp_extracts_from_the_external_uuid_crates_native_generator() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let external = ::uuid::Uuid::now_v7();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let ours = Uuid::from_bytes(*external.as_bytes());
        let got = v7::unix_millis(&ours);
        assert!(
            got >= before && got <= after,
            "got {got}, want within [{before}, {after}]"
        );
    }

    #[test]
    fn v6_sql_order_round_trips() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let sql = v6::to_sql_order(&id);
        assert_ne!(
            sql, id,
            "a real timestamp should actually move bytes around"
        );
        assert_eq!(v6::to_rfc_order(&sql), id);
    }

    #[test]
    fn v6_sql_order_zero_and_a_large_timestamp_round_trip() {
        // 100_000_000_000_000 ms (~year 5138) is comfortably under v6's 60-bit tick ceiling
        // (~year 5236) without needing that exact boundary constant here.
        for id in [
            v6::new_v6(0).unwrap(),
            v6::new_v6(100_000_000_000_000).unwrap(),
        ] {
            assert_eq!(v6::to_rfc_order(&v6::to_sql_order(&id)), id);
        }
    }

    #[test]
    fn v6_sql_order_preserves_version_and_variant() {
        // Different offsets than v7's sql order (octet 8's top nibble / octet 6's top two
        // bits here, not 7/8) — see v6::to_sql_order's doc comment for why that's fine.
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let sql = v6::to_sql_order(&id);
        assert_eq!(sql.as_bytes()[8] & 0xF0, 0x60);
        assert_eq!(sql.as_bytes()[6] & 0xC0, 0x80);
    }

    #[test]
    fn v6_sql_order_extracts_the_same_timestamp_after_converting_back() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let round_tripped = v6::to_rfc_order(&v6::to_sql_order(&id));
        assert_eq!(v6::unix_millis(&round_tripped), RFC_TEST_VECTOR_MS);
    }

    #[test]
    fn get_timestamp_returns_none_for_non_time_based_versions() {
        assert_eq!(get_timestamp(&v4::new_v4().unwrap()), None);
        assert_eq!(
            get_timestamp(&v5::new_v5(v5::namespace::DNS, b"test")),
            None
        );
    }

    #[test]
    fn get_timestamp_matches_v6_unix_millis() {
        let id = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(
            get_timestamp(&id),
            Some(Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS))
        );
    }

    #[test]
    fn get_timestamp_matches_v7_unix_millis() {
        let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        assert_eq!(
            get_timestamp(&id),
            Some(Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS))
        );
    }

    #[test]
    fn timestamp_unix_millis_round_trips_through_from_unix() {
        let ts = Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS);
        let (secs, subsec_nanos) = ts.to_unix();
        assert_eq!(Timestamp::from_unix(secs, subsec_nanos), ts);
        assert_eq!(ts.to_unix_millis(), RFC_TEST_VECTOR_MS);
    }

    #[test]
    fn timestamp_to_unix_millis_truncates_sub_millisecond_nanos() {
        // 1500 subsec_nanos is a real, valid sub-millisecond value that isn't itself a whole
        // millisecond — round-tripping through the millisecond-only creation/extraction API
        // truncates it, not rounds it.
        let ts = Timestamp::from_unix(1, 1_500);
        assert_eq!(ts.to_unix_millis(), 1000);
    }

    #[test]
    fn new_v6_at_matches_new_v6_from_the_same_timestamp() {
        let by_millis = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let by_timestamp = v6::new_v6_at(Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS)).unwrap();
        assert_eq!(v6::unix_millis(&by_millis), v6::unix_millis(&by_timestamp));
    }

    #[test]
    fn new_v7_at_matches_new_v7_from_the_same_timestamp() {
        let by_millis = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let by_timestamp = v7::new_v7_at(Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS)).unwrap();
        assert_eq!(v7::unix_millis(&by_millis), v7::unix_millis(&by_timestamp));
    }

    #[test]
    fn v6_sql_order_sorts_by_creation_order_under_sqlguid_comparison_for_distinct_timestamps() {
        // Unlike v7, v6 has no counter — two UUIDs at the *same* millisecond aren't
        // guaranteed to sort in creation order even in plain RFC order, so this only
        // exercises strictly increasing timestamps, where the timestamp alone determines
        // order with no tie to break.
        let ids: Vec<Uuid> = (0..300)
            .map(|i| v6::new_v6(1_000_000 + i).unwrap())
            .collect();
        let sql: Vec<[u8; 16]> = ids
            .iter()
            .map(|id| *v6::to_sql_order(id).as_bytes())
            .collect();
        let mut sorted = sql.clone();
        sorted.sort_by(sql_guid_cmp);
        assert_eq!(
            sql, sorted,
            "SqlGuid-order comparison of SQL-ordered bytes must match creation order"
        );
    }

    // The two layout files pin field placement through the deterministic half of each
    // generator, which the public API (rightly) never exposes; rust/tests/conformance.rs
    // replays the rest of the corpus through the public API.
    fn corpus(name: &str) -> Vec<serde_json::Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../corpus")
            .join(name);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&text).unwrap()
    }

    fn hex16(text: &str) -> [u8; 16] {
        core::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
    }

    fn hex6(text: &str) -> [u8; 6] {
        core::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
    }

    #[test]
    fn corpus_v7_layout() {
        for vector in corpus("v7_layout.json") {
            let mut bytes = [0u8; 16];
            bytes[10..].copy_from_slice(&hex6(vector["entropy"].as_str().unwrap()));
            let millis = vector["unix_millis"].as_u64().unwrap();
            v7::write_fields(
                &mut bytes,
                millis,
                vector["counter"].as_u64().unwrap() as u32,
            );
            assert_eq!(bytes, hex16(vector["expect"].as_str().unwrap()), "{vector}");
            assert_eq!(
                v7::unix_millis(&Uuid::from_bytes(bytes)),
                millis,
                "{vector}"
            );
        }
    }

    #[test]
    fn corpus_v6_layout() {
        for vector in corpus("v6_layout.json") {
            let mut bytes = [0u8; 16];
            let clock_seq = vector["clock_seq"].as_u64().unwrap() as u16;
            bytes[8..10].copy_from_slice(&clock_seq.to_be_bytes());
            bytes[10..].copy_from_slice(&hex6(vector["node"].as_str().unwrap()));
            let millis = vector["unix_millis"].as_u64().unwrap();
            v6::write_fields(&mut bytes, millis * 10_000 + 0x01B2_1DD2_1381_4000);
            assert_eq!(bytes, hex16(vector["expect"].as_str().unwrap()), "{vector}");
            assert_eq!(
                v6::unix_millis(&Uuid::from_bytes(bytes)),
                millis,
                "{vector}"
            );
        }
    }

    // The batch paths assemble their items with wider stores than the single-item writers
    // the layout corpus pins, so tie every batch item back to those writers: re-running the
    // writer over an item's own random tail and counter must reproduce the item exactly.
    #[test]
    fn batch_items_match_the_pinned_single_item_layout() {
        let mut out = vec![0u8; 64 * 16];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 64, &mut out).unwrap();
        for item in out.chunks_exact(16) {
            let item: [u8; 16] = item.try_into().unwrap();
            let counter = ((item[6] as u32 & 0x0F) << 22)
                | ((item[7] as u32) << 14)
                | ((item[8] as u32 & 0x3F) << 8)
                | item[9] as u32;
            let mut rebuilt = [0u8; 16];
            rebuilt[10..].copy_from_slice(&item[10..]);
            v7::write_fields(&mut rebuilt, RFC_TEST_VECTOR_MS, counter);
            assert_eq!(rebuilt, item);
        }

        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 64, &mut out).unwrap();
        for item in out.chunks_exact(16) {
            let item: [u8; 16] = item.try_into().unwrap();
            let mut rebuilt = [0u8; 16];
            rebuilt[8..].copy_from_slice(&item[8..]);
            v6::write_fields(
                &mut rebuilt,
                RFC_TEST_VECTOR_MS * 10_000 + 0x01B2_1DD2_1381_4000,
            );
            assert_eq!(rebuilt, item);
        }
    }

    #[test]
    fn inspection_of_the_special_values_and_every_variant_class() {
        assert_eq!(
            (Uuid::NIL.version(), Uuid::NIL.variant()),
            (0, Variant::Ncs)
        );
        assert_eq!(
            (Uuid::MAX.version(), Uuid::MAX.variant()),
            (15, Variant::Future)
        );
        let mut b = [0u8; 16];
        for (top, variant) in [
            (0x00, Variant::Ncs),
            (0x70, Variant::Ncs),
            (0x80, Variant::Rfc9562),
            (0xBF, Variant::Rfc9562),
            (0xC0, Variant::Microsoft),
            (0xDF, Variant::Microsoft),
            (0xE0, Variant::Future),
            (0xFF, Variant::Future),
        ] {
            b[8] = top;
            assert_eq!(Uuid::from_bytes(b).variant(), variant, "{top:#x}");
        }
        for version in 1..=8u8 {
            b[6] = version << 4;
            b[8] = 0x80;
            let id = Uuid::from_bytes(b);
            assert_eq!(id.version(), version);
            assert!(id.is_rfc(version));
            assert!(!id.is_rfc(version + 1));
            b[8] = 0xC0;
            assert!(!Uuid::from_bytes(b).is_rfc(version));
        }
    }

    // In SQL order a v6's octet 7 is random clock_seq and reads as a v7 nibble one time in 16;
    // the layout-aware read must never take it for one, nor a v7 for a v6.
    #[test]
    fn sql_ordered_v6_and_v7_are_never_confused() {
        for _ in 0..4096 {
            let six = v6::to_sql_order(&v6::new_v6(RFC_TEST_VECTOR_MS).unwrap());
            let seven = v7::to_sql_order(&v7::new_v7(RFC_TEST_VECTOR_MS).unwrap());
            assert_eq!(six.version_in(Layout::SqlServer), 6, "{six}");
            assert_eq!(seven.version_in(Layout::SqlServer), 7, "{seven}");
            assert!(!six.is_rfc_in(7, Layout::SqlServer));
            assert!(!seven.is_rfc_in(6, Layout::SqlServer));
            let ms =
                |id: &Uuid| get_timestamp_in(id, Layout::SqlServer).map(|t| t.to_unix_millis());
            assert_eq!(ms(&six), Some(RFC_TEST_VECTOR_MS));
            assert_eq!(ms(&seven), Some(RFC_TEST_VECTOR_MS));
        }
    }

    // The C ABI's codes, layout and variant alike, including the unknown-code answers.
    #[test]
    fn inspection_exports_speak_the_documented_codes() {
        let seven = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
        let sql = v7::to_sql_order(&seven);
        let p = seven.as_bytes().as_ptr();
        let q = sql.as_bytes().as_ptr();
        assert_eq!(ffi::uuid_version(p, 1), 7);
        assert_eq!(ffi::uuid_version(q, 2), 7);
        assert_eq!(ffi::uuid_version(p, 0), 0);
        assert_eq!(ffi::uuid_version(p, 3), 0);
        assert_eq!(ffi::uuid_variant(p), 2);
        assert_eq!(ffi::uuid_variant(Uuid::NIL.as_bytes().as_ptr()), 1);
        assert_eq!(ffi::uuid_variant(Uuid::MAX.as_bytes().as_ptr()), 4);
        assert_eq!(ffi::uuid_variant([0xC0; 16].as_ptr()), 3);
        assert_eq!(ffi::uuid_is_rfc(p, 7, 1), 1);
        assert_eq!(ffi::uuid_is_rfc(q, 7, 2), 1);
        assert_eq!(ffi::uuid_is_rfc(p, 6, 1), 0);
        assert_eq!(ffi::uuid_is_rfc(p, 7, 0), 0);
        assert_eq!(ffi::uuid_is_rfc(p, 7 + 256, 1), 0);
        assert_eq!(ffi::uuid_v7_unix_millis_in(q, 2), RFC_TEST_VECTOR_MS);
        assert_eq!(ffi::uuid_v7_unix_millis_in(p, 1), RFC_TEST_VECTOR_MS);
        assert_eq!(ffi::uuid_v7_unix_millis_in(p, 9), 0);
        let six = v6::new_v6(RFC_TEST_VECTOR_MS).unwrap();
        let six_sql = v6::to_sql_order(&six);
        assert_eq!(
            ffi::uuid_v6_unix_millis_in(six_sql.as_bytes().as_ptr(), 2),
            RFC_TEST_VECTOR_MS
        );
        assert_eq!(
            ffi::uuid_v6_unix_millis_in(six.as_bytes().as_ptr(), 1),
            RFC_TEST_VECTOR_MS
        );
        let mut out = [0u8; 16];
        assert_eq!(
            ffi::uuid_new_v7_batch(1, v7::MAX_BATCH + 1, out.as_mut_ptr()),
            4
        );
        let mut millis = 0u64;
        assert_eq!(ffi::uuid_get_timestamp(q, 2, &mut millis), 7);
        assert_eq!(millis, RFC_TEST_VECTOR_MS);
        millis = 0;
        assert_eq!(
            ffi::uuid_get_timestamp(six_sql.as_bytes().as_ptr(), 2, &mut millis),
            6
        );
        assert_eq!(millis, RFC_TEST_VECTOR_MS);
        millis = 42;
        assert_eq!(ffi::uuid_get_timestamp(p, 0, &mut millis), 0);
        assert_eq!(
            ffi::uuid_get_timestamp(Uuid::NIL.as_bytes().as_ptr(), 1, &mut millis),
            0
        );
        // A 7 in the nibble but the NCS variant: no RFC version, so no timestamp.
        let mut ncs = *seven.as_bytes();
        ncs[8] &= 0x3F;
        assert_eq!(ffi::uuid_get_timestamp(ncs.as_ptr(), 1, &mut millis), 0);
        assert_eq!(millis, 42, "untouched on 0");
        assert_eq!(get_timestamp(&Uuid::from_bytes(ncs)), None);
    }
}
