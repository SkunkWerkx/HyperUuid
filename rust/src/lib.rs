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

mod ffi;
mod uuid;
pub mod v4;
pub mod v5;
pub mod v6;
pub mod v7;

pub use uuid::{ParseUuidError, Uuid};

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

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
        let ids: Vec<Uuid> = (0..100).map(|_| v7::new_v7(RFC_TEST_VECTOR_MS).unwrap()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
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
    fn v6_batch_matches_single_call_generation() {
        let mut out = vec![0u8; 5 * 16];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 5, &mut out).unwrap();
        for chunk in out.chunks_exact(16) {
            let bytes: [u8; 16] = chunk.try_into().unwrap();
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
            out.chunks_exact(16).map(|c| c.try_into().unwrap()).collect();
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn v6_batch_zero_count_is_a_no_op() {
        let mut out: [u8; 0] = [];
        v6::new_v6_batch(RFC_TEST_VECTOR_MS, 0, &mut out).unwrap();
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
        for chunk in out.chunks_exact(16) {
            let bytes: [u8; 16] = chunk.try_into().unwrap();
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
            .chunks_exact(16)
            .map(|c| Uuid::from_bytes(c.try_into().unwrap()))
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
        ids.extend(batch.chunks_exact(16).map(|c| Uuid::from_bytes(c.try_into().unwrap())));
        ids.push(after);

        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn v7_batch_zero_count_is_a_no_op() {
        let mut out: [u8; 0] = [];
        v7::new_v7_batch(RFC_TEST_VECTOR_MS, 0, &mut out).unwrap();
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
        assert_ne!(sql, id, "a real timestamp/counter should actually move bytes around");
        assert_eq!(v7::to_rfc_order(&sql), id);
    }

    #[test]
    fn v7_sql_order_zero_and_max_round_trip() {
        for id in [v7::new_v7(0).unwrap(), v7::new_v7(v7::MAX_UNIX_MILLIS).unwrap()] {
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
        let mut ids: Vec<Uuid> = (0..200).map(|i| v7::new_v7(1_000_000 + i).unwrap()).collect();
        // ...plus a same-millisecond run, so the counter (not just the timestamp) has to sort
        // correctly too.
        ids.extend((0..200).map(|_| v7::new_v7(5_000_000).unwrap()));

        let sql: Vec<[u8; 16]> = ids.iter().map(|id| *v7::to_sql_order(id).as_bytes()).collect();
        let mut sorted = sql.clone();
        sorted.sort_by(sql_guid_cmp);
        assert_eq!(sql, sorted, "SqlGuid-order comparison of SQL-ordered bytes must match creation order");
    }

    /// Proves [`v7::unix_millis`] isn't just reading back what our own [`v7::new_v7`] wrote —
    /// it's a plain RFC 9562 bit-layout read, so it recovers the real embedded timestamp from
    /// a version 7 UUID minted by a completely independent implementation too. `::uuid` here
    /// is the external `uuid` crate dev-dependency, disambiguated by the leading `::` from
    /// this crate's own private `uuid` module of the same name.
    #[test]
    fn v7_timestamp_extracts_from_the_external_uuid_crates_native_generator() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let external = ::uuid::Uuid::now_v7();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        let ours = Uuid::from_bytes(*external.as_bytes());
        let got = v7::unix_millis(&ours);
        assert!(got >= before && got <= after, "got {got}, want within [{before}, {after}]");
    }
}
