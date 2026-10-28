#![cfg(feature = "uuid1")]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hyperuuid::{Uuid, v4, v5, v7};

type R = Result<(), Box<dyn std::error::Error>>;

#[test]
fn round_trips_both_ways() {
    let id = v4::new_v4().expect("v4");
    assert_eq!(Uuid::from(uuid::Uuid::from(id)), id);

    let theirs = uuid::Uuid::new_v4();
    assert_eq!(uuid::Uuid::from(Uuid::from(theirs)), theirs);
}

#[test]
fn nil_and_max_cross_both_ways() {
    for bytes in [[0u8; 16], [0xFFu8; 16]] {
        let ours = Uuid::from_bytes(bytes);
        let theirs = uuid::Uuid::from_bytes(bytes);

        assert_eq!(uuid::Uuid::from(ours), theirs);
        assert_eq!(Uuid::from(theirs), ours);
        assert!(ours == theirs);
        assert!(theirs == ours);
    }
}

#[test]
fn borrow_conversion_keeps_the_original() {
    let id = v4::new_v4().expect("v4");
    let u: uuid::Uuid = (&id).into();
    assert!(u == id);
    assert!(id == u);
    assert_eq!(id.into_bytes(), *u.as_bytes());
}

#[test]
fn display_matches_for_every_layout() -> R {
    let v7 = v7::now_v7()?;
    let v5 = v5::new_v5(Uuid::NIL, b"hyperuuid");
    let sql = v7::to_sql_order(&v7)?;

    for id in [v7, v5, sql] {
        assert_eq!(id.to_string(), uuid::Uuid::from(id).to_string());
    }
    Ok(())
}

#[test]
fn v7_order_survives_conversion() -> R {
    let a = v7::now_v7()?;
    let b = v7::now_v7()?;
    assert!(a < b);
    assert!(uuid::Uuid::from(a) < uuid::Uuid::from(b));
    Ok(())
}

#[test]
fn inequality_is_detected() {
    let a = [7u8; 16];
    let mut b = a;
    b[15] ^= 1;
    let (ours_a, theirs_a) = (Uuid::from_bytes(a), uuid::Uuid::from_bytes(a));
    let (ours_b, theirs_b) = (Uuid::from_bytes(b), uuid::Uuid::from_bytes(b));

    assert!(ours_a != theirs_b);
    assert!(theirs_a != ours_b);
    assert!(ours_b != theirs_a);
    assert!(theirs_b != ours_a);
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

// Empirical: `uuid` hashes its 16 bytes with one `write`, while the derived `Hash` over
// `[u8; 16]` writes a length prefix first, so the two must not be mixed as map keys. If this
// starts failing, the README's warning can be relaxed.
#[test]
fn hashes_differ_today() {
    let ours = v4::new_v4().expect("v4");
    let theirs = uuid::Uuid::from(ours);
    assert_ne!(hash_of(&ours), hash_of(&theirs));
}
