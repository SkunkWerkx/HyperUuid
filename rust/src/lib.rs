//! A Rust port of the RFC 9562 UUID generation from
//! [SequentialGuid](https://github.com/buvinghausen/SequentialGuid), covering only the
//! standard UUID versions the RFC itself defines — no SQL Server byte-ordering, ORM, or
//! serializer integrations.
//!
//! - [`v4::new_v4`] — random (RFC 9562 §5.4)
//! - [`v5::new_v5`] — deterministic, namespace + name based, SHA-1 (RFC 9562 §5.5)
//! - [`v7::new_v7`] — time-sortable, millisecond timestamp + monotonic counter (RFC 9562 §6.2)

mod ffi;
mod uuid;
pub mod v4;
pub mod v5;
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
}
