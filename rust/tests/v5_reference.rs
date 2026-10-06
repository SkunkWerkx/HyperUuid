//! `new_v5` pads SHA-1 by hand rather than through `Digest`, and padding is where a hand-rolled
//! hash goes wrong: a name that ends a block exactly, or leaves fewer than the 8 bytes the
//! length needs, takes a different path from one that doesn't. Every name length from empty to
//! well past four blocks is checked here against the `uuid` crate, which shares no code with
//! this one past the compression function, so every one of those paths is covered.

use hyperuuid::v5;

#[test]
fn every_name_length_matches_the_uuid_crate() {
    let namespaces = [
        (v5::namespace::DNS, uuid::Uuid::NAMESPACE_DNS),
        (v5::namespace::URL, uuid::Uuid::NAMESPACE_URL),
        (v5::namespace::OID, uuid::Uuid::NAMESPACE_OID),
        (v5::namespace::X500, uuid::Uuid::NAMESPACE_X500),
    ];
    for len in 0..=300usize {
        let name: Vec<u8> = (0..len).map(|i| (i * 31 + 7) as u8).collect();
        for (ours, theirs) in namespaces {
            assert_eq!(
                v5::new_v5(ours, &name).into_bytes(),
                *uuid::Uuid::new_v5(&theirs, &name).as_bytes(),
                "name length {len}"
            );
        }
    }
}
