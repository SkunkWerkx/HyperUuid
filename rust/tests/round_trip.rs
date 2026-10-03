//! Text in and out of `Uuid`: every value formats to the canonical form and parses back to
//! itself, and every string near a valid one — one character replaced, removed or added —
//! parses exactly when a plain reference reading of RFC 9562's 8-4-4-4-12 form says it
//! should, to the bytes that reading gives.
//!
//! The reference parser below is deliberately naive (fixed hyphen offsets, then
//! `u8::from_str_radix` on each pair) so that it shares nothing with the crate's own. The
//! panic-freedom of `from_str` is proven at link time by the `no-panic` feature; what that
//! cannot see is a parse that succeeds with the wrong bytes, or fails on a valid string —
//! 0.5.0's stray-hyphen bug was the first kind's neighbour, and these would have caught it.

use hyperuuid::{Uuid, v4, v5, v6, v7};

const HYPHENS: [usize; 4] = [8, 13, 18, 23];

fn reference_parse(s: &str) -> Option<[u8; 16]> {
    let b = s.as_bytes();
    if b.len() != 36 || HYPHENS.iter().any(|&i| b[i] != b'-') {
        return None;
    }
    let digits: Vec<u8> = b
        .iter()
        .enumerate()
        .filter(|(i, _)| !HYPHENS.contains(i))
        .map(|(_, &c)| c)
        .collect();
    if !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, pair) in digits.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

fn assert_agrees(s: &str) {
    let got = s.parse::<Uuid>().ok().map(Uuid::into_bytes);
    assert_eq!(
        got,
        reference_parse(s),
        "parse disagrees with the reference on {s:?}"
    );
}

fn samples() -> Vec<Uuid> {
    let mut ids = vec![Uuid::NIL, Uuid::MAX];
    // Every byte value in every position, so each hex digit pair is exercised everywhere.
    for pos in 0..16 {
        for value in [
            0x00, 0x09, 0x0a, 0x0f, 0x10, 0x7f, 0x80, 0x9a, 0xa9, 0xf0, 0xff,
        ] {
            let mut bytes = [0u8; 16];
            bytes[pos] = value;
            ids.push(Uuid::from_bytes(bytes));
        }
    }
    for _ in 0..1000 {
        ids.push(v4::new_v4().unwrap());
    }
    ids.push(v5::new_v5(v5::namespace::DNS, b"www.example.com"));
    ids.push(v6::new_v6(1645557742000).unwrap());
    ids.push(v7::new_v7(1645557742000).unwrap());
    ids
}

#[test]
fn every_value_formats_canonically_and_parses_back_to_itself() {
    for id in samples() {
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert!(
            text.bytes()
                .enumerate()
                .all(|(i, c)| if HYPHENS.contains(&i) {
                    c == b'-'
                } else {
                    matches!(c, b'0'..=b'9' | b'a'..=b'f')
                }),
            "not canonical lowercase 8-4-4-4-12: {text}"
        );
        assert_eq!(format!("{id:?}"), text, "Debug and Display differ");
        assert_eq!(
            text.parse::<Uuid>(),
            Ok(id),
            "lowercase round trip of {text}"
        );
        assert_eq!(
            text.to_uppercase().parse::<Uuid>(),
            Ok(id),
            "uppercase round trip of {text}"
        );
        assert_eq!(reference_parse(&text), Some(id.into_bytes()));
    }
}

#[test]
fn every_one_character_edit_parses_exactly_when_the_reference_says_so() {
    // Hex digits of both cases, the separator, and characters a sloppy parser might wave
    // through: a non-hex letter, whitespace, braces, a NUL, and multi-byte UTF-8 (which
    // changes the byte length without changing the character count).
    let alphabet = [
        '0', '7', '9', 'a', 'A', 'f', 'F', '-', 'g', 'G', ' ', '{', '}', '\0', 'é', '€',
    ];
    let bases = [
        "00000000-0000-0000-0000-000000000000",
        "ffffffff-ffff-ffff-ffff-ffffffffffff",
        "01234567-89ab-cdef-0123-456789ABCDEF",
    ];
    let mut checked = 0usize;
    for base in bases {
        let chars: Vec<char> = base.chars().collect();
        for i in 0..=chars.len() {
            if i < chars.len() {
                let mut removed = chars.clone();
                removed.remove(i);
                assert_agrees(&removed.iter().collect::<String>());
                checked += 1;
            }
            for &c in &alphabet {
                if i < chars.len() {
                    let mut replaced = chars.clone();
                    replaced[i] = c;
                    assert_agrees(&replaced.iter().collect::<String>());
                    checked += 1;
                }
                let mut inserted = chars.clone();
                inserted.insert(i, c);
                assert_agrees(&inserted.iter().collect::<String>());
                checked += 1;
            }
        }
    }
    assert!(checked > 3000, "only {checked} edits checked");
}

#[test]
fn the_shapes_people_actually_paste_are_rejected() {
    for s in [
        "",
        "0123456789abcdef0123456789abcdef",
        "{01234567-89ab-cdef-0123-456789abcdef}",
        "urn:uuid:01234567-89ab-cdef-0123-456789abcdef",
        " 01234567-89ab-cdef-0123-456789abcdef",
        "01234567-89ab-cdef-0123-456789abcdef ",
        "01234567-89abc-def-0123-456789abcdef",
        "00000000-0000-0000-0000--00000000000",
        "0123456789ab-cdef-0123-456789abcdef-",
    ] {
        assert!(s.parse::<Uuid>().is_err(), "{s:?} parsed");
        assert_agrees(s);
    }
}
