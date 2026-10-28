//! The crate's unit tests, in a file of their own rather than inline in `lib.rs`.
//!
//! Nothing here is built into an artifact — the module is `#[cfg(test)]` — and CodeQL filters
//! paths per file, so with the tests inline in `lib.rs` every UUID a failure message formats
//! (`assert!(..., "{id}")`) was reported as `rust/cleartext-logging` against the file that also
//! ships. `.github/codeql/codeql-config.yml` ignores this file by name instead, which keeps the
//! diagnostics and takes the tests out of the scan in one move.

use super::*;
use core::str::FromStr;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

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
//
// Also the newest timestamp any test here hands the process-wide v7 stream through the public
// API, so those calls stamp it exactly: one at a later in-stream time would make them clamp.
// A test that asserts an order across calls mints from a stream of its own ([`v7_in`]).
const RFC_TEST_VECTOR_MS: u64 = 1_645_557_742_000;

/// One version 7 from `stream` at `unix_millis`, with real entropy: the public [`v7::new_v7`]
/// on a stream no other test shares.
fn v7_in(stream: &AtomicU64, unix_millis: u64) -> Uuid {
    v7::new_v7_in(stream, unix_millis, entropy::fill_at).unwrap()
}

// RFC 9562 Appendix A.6's version 7. A freshly minted v7 can't stand in for "a value in RFC
// order" where a conversion from SQL order must refuse it: its random octet 7 carries the
// SQL layout's 8 one time in 16. This one's octet 7 is 0xC3.
const RFC_V7_VECTOR: Uuid = Uuid::from_bytes([
    0x01, 0x7F, 0x22, 0xE2, 0x79, 0xB0, 0x7C, 0xC3, 0x98, 0xC4, 0xDC, 0x0C, 0x0C, 0x07, 0x39, 0x8F,
]);

// RFC 9562 Appendix A.5's version 6, minted by another generator: a version this crate
// neither generates nor reads a timestamp from.
const RFC_V6_VECTOR: Uuid = Uuid::from_bytes([
    0x1E, 0xC9, 0x41, 0x4C, 0x23, 0x2A, 0x6B, 0x00, 0xB3, 0xC8, 0x9F, 0x6B, 0xDE, 0xCE, 0xD8, 0x46,
]);

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
    let stream = AtomicU64::new(0);
    let ids: Vec<Uuid> = (0..100)
        .map(|_| v7_in(&stream, RFC_TEST_VECTOR_MS))
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn a_timestamp_past_u64_millis_is_out_of_range_not_wrapped() {
    let far = Timestamp::from_unix(u64::MAX / 1000 + 1, 0).unwrap();
    assert_eq!(far.to_unix_millis(), u64::MAX);
    assert_eq!(
        v7::new_v7_at(far).unwrap_err(),
        v7::NewV7Error::TimestampOutOfRange
    );
}

#[test]
fn v7_increasing_timestamps_sort_in_creation_order() {
    const BASE_MS: u64 = 1_000_000;
    let stream = AtomicU64::new(0);
    let ids: Vec<Uuid> = (0..10).map(|i| v7_in(&stream, BASE_MS + i)).collect();
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
    // group's digits; a parser that trusts the hyphen positions reads one byte past its end.
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
fn v7_batch_and_individual_calls_on_one_stream_sort_in_call_order() {
    // A batch shouldn't collide with (or reorder relative to) individual calls interleaved
    // around it on the same stream.
    let stream = AtomicU64::new(0);
    let before = v7_in(&stream, RFC_TEST_VECTOR_MS);
    let mut batch = vec![0u8; 10 * 16];
    v7::new_v7_batch_in(
        &stream,
        RFC_TEST_VECTOR_MS,
        10,
        &mut batch,
        entropy::fill_at,
    )
    .unwrap();
    let after = v7_in(&stream, RFC_TEST_VECTOR_MS);

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

#[test]
fn counter_exhausted_display_text() {
    assert_eq!(
        v7::NewV7Error::CounterExhausted.to_string(),
        "version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond"
    );
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
    let sql = v7::to_sql_order(&id).unwrap();
    assert_ne!(
        sql, id,
        "a real timestamp/counter should actually move bytes around"
    );
    assert_eq!(v7::to_rfc_order(&sql), Ok(id));
}

// The layout holds all 122 of a v7's free bits, so every timestamp v7 can carry converts
// and comes back exactly, up to the 48-bit ceiling.
#[test]
fn v7_sql_order_covers_every_timestamp() {
    for millis in [
        0,
        RFC_TEST_VECTOR_MS,
        (1 << 47) - 1,
        1 << 47,
        v7::MAX_UNIX_MILLIS,
    ] {
        let id = v7::new_v7(millis).unwrap();
        assert_eq!(v7::to_rfc_order(&v7::to_sql_order(&id).unwrap()), Ok(id));
    }
}

// RFC 9562 §6.11 sends a byte order other than network order to version 8, so the SQL
// layout carries an 8 and the RFC variant where a mixed-endian GUID holds them: octet 7's
// top nibble and octet 8's top bits. Read as a .NET Guid or SQL Server's text, that is a
// version 8 RFC 9562 UUID.
#[test]
fn sql_order_writes_a_version_8_in_guid_order() {
    let guid_view = |sql: &Uuid| {
        let b = sql.as_bytes();
        let mut rfc = *b;
        rfc[0..4].copy_from_slice(&[b[3], b[2], b[1], b[0]]);
        rfc[4..6].copy_from_slice(&[b[5], b[4]]);
        rfc[6..8].copy_from_slice(&[b[7], b[6]]);
        Uuid::from_bytes(rfc)
    };
    for _ in 0..1024 {
        let sql = v7::to_sql_order(&v7::new_v7(RFC_TEST_VECTOR_MS).unwrap()).unwrap();
        assert!(guid_view(&sql).is_rfc(8), "{sql}");
        assert_eq!(get_timestamp(&guid_view(&sql)), None);
    }
}

#[test]
fn sql_order_refuses_a_value_of_another_version_in_both_directions() {
    let seven = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    let four = v4::new_v4().unwrap();
    let wrong = Err(SqlOrderError::WrongVersion);
    for id in [RFC_V6_VECTOR, four, Uuid::NIL, Uuid::MAX] {
        assert_eq!(v7::to_sql_order(&id), wrong, "{id}");
    }
    let seven_sql = v7::to_sql_order(&seven).unwrap();
    // 0.8.0 wrote this layout with a 7 where the 8 now is; such a value is not in SQL order
    // until its nibble is changed.
    let mut with_a_seven = *seven_sql.as_bytes();
    with_a_seven[7] = 0x70 | (with_a_seven[7] & 0x0F);
    assert_eq!(v7::to_rfc_order(&Uuid::from_bytes(with_a_seven)), wrong);
    with_a_seven[7] |= 0x80;
    with_a_seven[7] &= 0x8F;
    assert_eq!(v7::to_rfc_order(&Uuid::from_bytes(with_a_seven)), Ok(seven));
    // An RFC-ordered value is not in SQL order, and nor is one whose variant was cleared.
    assert_eq!(v7::to_rfc_order(&RFC_V7_VECTOR), wrong);
    let mut cleared = *seven_sql.as_bytes();
    cleared[8] &= 0x3F;
    assert_eq!(v7::to_rfc_order(&Uuid::from_bytes(cleared)), wrong);
}

#[test]
fn v7_sql_order_extracts_the_same_timestamp_after_converting_back() {
    let id = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    let round_tripped = v7::to_rfc_order(&v7::to_sql_order(&id).unwrap()).unwrap();
    assert_eq!(v7::unix_millis(&round_tripped), RFC_TEST_VECTOR_MS);
}

/// Replicates `System.Data.SqlTypes.SqlGuid.CompareTo` — and therefore T-SQL `ORDER BY`
/// on a `uniqueidentifier` column — which compares a GUID's 16 bytes in this fixed
/// significance order rather than left to right. This is the correctness oracle for
/// [`v7::to_sql_order`]: no real SQL Server available in this crate's test suite, so this
/// stands in for it, the same role the C# binding's tests use the real `SqlGuid` for.
fn sql_guid_cmp(a: &[u8; 16], b: &[u8; 16]) -> std::cmp::Ordering {
    const SIGNIFICANCE_ORDER: [usize; 16] = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3];
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
    let stream = AtomicU64::new(0);
    let mut ids: Vec<Uuid> = (0..200).map(|i| v7_in(&stream, 1_000_000 + i)).collect();
    // ...plus a same-millisecond run, so the counter (not just the timestamp) has to sort
    // correctly too.
    ids.extend((0..200).map(|_| v7_in(&stream, 5_000_000)));

    let sql: Vec<[u8; 16]> = ids
        .iter()
        .map(|id| *v7::to_sql_order(id).unwrap().as_bytes())
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
fn get_timestamp_returns_none_for_every_version_but_7() {
    assert_eq!(get_timestamp(&RFC_V6_VECTOR), None);
    assert_eq!(get_timestamp(&v4::new_v4().unwrap()), None);
    assert_eq!(
        get_timestamp(&v5::new_v5(v5::namespace::DNS, b"test")),
        None
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
    assert_eq!(Timestamp::from_unix(secs, subsec_nanos), Some(ts));
    assert_eq!(ts.to_unix_millis(), RFC_TEST_VECTOR_MS);
}

#[test]
fn timestamp_to_unix_millis_truncates_sub_millisecond_nanos() {
    // 1500 subsec_nanos is a real, valid sub-millisecond value that isn't itself a whole
    // millisecond — round-tripping through the millisecond-only creation/extraction API
    // truncates it, not rounds it.
    let ts = Timestamp::from_unix(1, 1_500).unwrap();
    assert_eq!(ts.to_unix_millis(), 1000);
}

#[test]
fn new_v7_at_matches_new_v7_from_the_same_timestamp() {
    let by_millis = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    let by_timestamp = v7::new_v7_at(Timestamp::from_unix_millis(RFC_TEST_VECTOR_MS)).unwrap();
    assert_eq!(v7::unix_millis(&by_millis), v7::unix_millis(&by_timestamp));
}

// The two layout files pin field placement through the deterministic half of each
// generator, which the public API (rightly) never exposes; rust/tests/conformance.rs
// replays the rest of the corpus through the public API.
fn corpus(name: &str) -> Vec<serde_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
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

// The conversion exports' codes, and the bytes left as they were on every refusal.
#[test]
fn sql_order_exports_speak_the_documented_codes() {
    let seven = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    let mut bytes = *seven.as_bytes();
    assert_eq!(ffi::uuid_v7_to_sql_order(bytes.as_mut_ptr()), 0);
    assert_eq!(bytes, *v7::to_sql_order(&seven).unwrap().as_bytes());
    assert_eq!(ffi::uuid_v7_to_rfc_order(bytes.as_mut_ptr()), 0);
    assert_eq!(bytes, *seven.as_bytes());

    let refused = |convert: extern "C" fn(*mut u8) -> i32, id: Uuid, code: i32| {
        let mut bytes = *id.as_bytes();
        assert_eq!(convert(bytes.as_mut_ptr()), code, "{id}");
        assert_eq!(bytes, *id.as_bytes(), "untouched on {code}");
    };
    refused(ffi::uuid_v7_to_sql_order, RFC_V6_VECTOR, 5);
    refused(ffi::uuid_v7_to_rfc_order, RFC_V7_VECTOR, 5);
}

// The C ABI's codes, layout and variant alike, including the unknown-code answers.
#[test]
fn inspection_exports_speak_the_documented_codes() {
    let seven = v7::new_v7(RFC_TEST_VECTOR_MS).unwrap();
    let sql = v7::to_sql_order(&seven).unwrap();
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
    let mut out = [0u8; 16];
    assert_eq!(
        ffi::uuid_new_v7_batch(1, v7::MAX_BATCH + 1, out.as_mut_ptr()),
        4
    );
    let mut millis = 0u64;
    assert_eq!(ffi::uuid_get_timestamp(q, 2, &mut millis), 7);
    assert_eq!(millis, RFC_TEST_VECTOR_MS);
    millis = 42;
    assert_eq!(
        ffi::uuid_get_timestamp(RFC_V6_VECTOR.as_bytes().as_ptr(), 1, &mut millis),
        0
    );
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

#[test]
fn timestamp_rejects_nanoseconds_outside_a_second() {
    assert!(Timestamp::from_unix(0, 1_000_000_000).is_none());
    assert!(Timestamp::from_unix(0, u32::MAX).is_none());
    assert_eq!(
        Timestamp::from_unix(u64::MAX, 999_999_999)
            .unwrap()
            .to_unix(),
        (u64::MAX, 999_999_999)
    );
    assert_eq!(
        Timestamp::from_unix(1, 0).unwrap(),
        Timestamp::from_unix_millis(1000)
    );
}

#[test]
fn parse_errors_compose_with_standard_error_propagation() {
    fn parse(text: &str) -> Result<Uuid, Box<dyn std::error::Error>> {
        Ok(text.parse()?)
    }
    let error = parse("not-a-uuid").unwrap_err();
    assert!(error.downcast_ref::<ParseUuidError>().is_some());
}

// The v7 stream through the injectable paths, `new_v7_in` and `new_v7_batch_in`: a local
// stream word and a fake entropy source, so each test sets the state it needs and reads back
// exactly what the call installed.
mod v7_stream {
    use super::*;
    use crate::stream::{STEPS, STREAM_MAX_MILLIS, encode};
    use v7::NewV7Error;

    const M: u64 = RFC_TEST_VECTOR_MS;

    fn ones(buf: &mut [u8], _: u64) -> Result<(), getrandom::Error> {
        buf.fill(0xFF);
        Ok(())
    }

    fn zeros(buf: &mut [u8], _: u64) -> Result<(), getrandom::Error> {
        buf.fill(0);
        Ok(())
    }

    fn broken(_: &mut [u8], _: u64) -> Result<(), getrandom::Error> {
        Err(getrandom::Error::UNSUPPORTED)
    }

    fn never(_: &mut [u8], _: u64) -> Result<(), getrandom::Error> {
        panic!("a call that fails validation drew entropy")
    }

    fn item(out: &[u8], i: usize) -> Uuid {
        Uuid::from_bytes(out[i * 16..(i + 1) * 16].try_into().unwrap())
    }

    // An advance reserves every step of the batch, not just its first: the batch's last item
    // is step seed + 1, and the single after it must reserve seed + 2. All-ones entropy for the
    // batch and all-zero for the single, so a single landing on the batch's last step (low6
    // and tail 0 against the batch's 0xFF tail) sorts below it instead of tying by luck.
    #[test]
    fn v7_single_after_a_batch_sorts_after_its_last_item() {
        let stream = AtomicU64::new(0);
        let mut out = vec![0u8; 65 * 16];
        v7::new_v7_batch_in(&stream, M, 65, &mut out, ones).unwrap();
        let last = item(&out, 64);
        let single = v7::new_v7_in(&stream, M, zeros).unwrap();
        assert!(single > last, "{single} <= {last}");
    }

    #[test]
    fn v7_entropy_failure_reserves_nothing() {
        let state = encode(M, 9);
        let stream = AtomicU64::new(state);
        let failed = Err(NewV7Error::Random(getrandom::Error::UNSUPPORTED));
        assert_eq!(v7::new_v7_in(&stream, M, broken), failed);
        let mut out = vec![0u8; 65 * 16];
        assert_eq!(
            v7::new_v7_batch_in(&stream, M, 65, &mut out, broken),
            failed.map(|_| ())
        );
        assert_eq!(stream.load(Relaxed), state);
    }

    // The buffer isn't asserted: the batch's entropy is drawn into it before the reservation.
    #[test]
    fn v7_exhaustion_reserves_nothing() {
        let state = encode(STREAM_MAX_MILLIS, STEPS - 1);
        let stream = AtomicU64::new(state);
        assert_eq!(
            v7::new_v7_in(&stream, STREAM_MAX_MILLIS, ones),
            Err(NewV7Error::CounterExhausted)
        );
        assert_eq!(stream.load(Relaxed), state);
        let mut out = vec![0u8; 65 * 16];
        assert_eq!(
            v7::new_v7_batch_in(&stream, STREAM_MAX_MILLIS, 65, &mut out, ones),
            Err(NewV7Error::CounterExhausted)
        );
        assert_eq!(stream.load(Relaxed), state);
    }

    // A distinct byte at every offset of the draw, so a field read from the wrong octet shows.
    fn counting(buf: &mut [u8], _: u64) -> Result<(), getrandom::Error> {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(1);
        }
        Ok(())
    }

    // The 26-bit counter of a v7: rand_a's 12 bits over the 14 after the variant.
    fn counter26(id: &Uuid) -> u32 {
        let b = id.as_bytes();
        (u32::from(b[6] & 0x0F) << 22)
            | (u32::from(b[7]) << 14)
            | (u32::from(b[8] & 0x3F) << 8)
            | u32::from(b[9])
    }

    // The exact counter a reservation produces, the per-millisecond sort key. The corpus pins
    // only where `write_fields` puts a counter; this pins which counter each path composes,
    // `step << 6 | low6` for a single and `step << 6 | i % 64` for a batch's item i, and the
    // empty-head placement of a reservation that starts past the stream's last step.
    #[test]
    fn v7_reservations_compose_the_exact_counter() {
        // A single clamped onto step 9 takes step 10, its low 6 bits from octet 9, the fifth
        // byte of the 11-byte draw into octets 5-15, and its tail from the last six.
        let stream = AtomicU64::new(encode(M, 9));
        let single = v7::new_v7_in(&stream, M, counting).unwrap();
        assert_eq!(v7::unix_millis(&single), M);
        assert_eq!(counter26(&single), (10 << 6) | 5);
        assert_eq!(single.as_bytes()[10..], [6, 7, 8, 9, 10, 11]);

        // A 130-item batch from the same state: steps 10 to 12, 64 items to a step.
        let stream = AtomicU64::new(encode(M, 9));
        let mut out = vec![0u8; 130 * 16];
        v7::new_v7_batch_in(&stream, M, 130, &mut out, counting).unwrap();
        for i in 0..130u32 {
            let id = item(&out, i as usize);
            assert_eq!(v7::unix_millis(&id), M, "item {i}");
            assert_eq!(counter26(&id), ((10 + i / 64) << 6) | (i % 64), "item {i}");
        }

        // A single on a stream at its millisecond's last step has an empty head: it lands at
        // step 0 of the next millisecond.
        let stream = AtomicU64::new(encode(M, STEPS - 1));
        let single = v7::new_v7_in(&stream, M, counting).unwrap();
        assert_eq!(v7::unix_millis(&single), M + 1);
        assert_eq!(counter26(&single), 5);

        // A 65-item batch there: the first 64 at step 0 of M + 1, item 64 at step 1.
        let stream = AtomicU64::new(encode(M, STEPS - 1));
        let mut out = vec![0u8; 65 * 16];
        v7::new_v7_batch_in(&stream, M, 65, &mut out, counting).unwrap();
        for i in 0..65u32 {
            let id = item(&out, i as usize);
            assert_eq!(v7::unix_millis(&id), M + 1, "item {i}");
            assert_eq!(counter26(&id), ((i / 64) << 6) | (i % 64), "item {i}");
        }
    }

    // A count of 0 is a success that draws nothing and reserves nothing.
    #[test]
    fn v7_empty_batch_draws_and_reserves_nothing() {
        let state = encode(M, 9);
        let stream = AtomicU64::new(state);
        assert_eq!(v7::new_v7_batch_in(&stream, M, 0, &mut [], never), Ok(()));
        assert_eq!(stream.load(Relaxed), state);
    }

    #[test]
    fn v7_validation_failures_leave_the_buffer_untouched() {
        let state = encode(M, 9);
        let stream = AtomicU64::new(state);
        let mut out = [0xABu8; 31];
        let cases = [
            (M, 2, NewV7Error::BufferTooSmall),
            (M, v7::MAX_BATCH + 1, NewV7Error::BatchTooLarge),
            (v7::MAX_UNIX_MILLIS + 1, 1, NewV7Error::TimestampOutOfRange),
        ];
        for (millis, count, want) in cases {
            assert_eq!(
                v7::new_v7_batch_in(&stream, millis, count, &mut out, never),
                Err(want)
            );
            assert_eq!(out, [0xABu8; 31], "{want:?}");
        }
        assert_eq!(
            v7::new_v7_in(&stream, v7::MAX_UNIX_MILLIS + 1, never),
            Err(NewV7Error::TimestampOutOfRange)
        );
        assert_eq!(stream.load(Relaxed), state);
    }
}
