//! HyperUuid inside a real browser: generation, the clock a browser caller hands in, parsing,
//! and the C ABI's version export, compiled for wasm32-unknown-unknown with getrandom drawing
//! from `crypto.getRandomValues`. Run with `wasm-pack test --headless --chrome` (or
//! `--firefox`); on any other target this file compiles to nothing.

#![cfg(target_arch = "wasm32")]

use hyperuuid::{Uuid, hyperuuid_version, v4, v5, v6, v7};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

/// `v7::now_v7` reads the OS clock and is not compiled for wasm32; a browser caller passes
/// `Date.now()` in, which is exactly what this does.
fn now_millis() -> u64 {
    js_sys::Date::now() as u64
}

#[wasm_bindgen_test]
fn the_core_reports_the_crate_version() {
    let manifest = include_str!("../../Cargo.toml");
    let want = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("a version line in rust/Cargo.toml");
    let v = hyperuuid_version();
    assert_eq!(
        format!("{}.{}.{}", v >> 16, (v >> 8) & 0xFF, v & 0xFF),
        want
    );
}

#[wasm_bindgen_test]
fn v4_draws_from_the_browser_random_source() {
    let a = v4::new_v4().unwrap();
    let b = v4::new_v4().unwrap();
    assert_eq!(a.version(), 4);
    assert!(a.is_rfc9562_variant());
    assert_ne!(a, b);
}

#[wasm_bindgen_test]
fn v5_matches_the_rfc_vector() {
    let id = v5::new_v5(v5::namespace::DNS, b"www.example.com");
    assert_eq!(id.to_string(), "2ed6657d-e927-568b-95e1-2665a8aea6a2");
}

#[wasm_bindgen_test]
fn v6_and_v7_embed_the_time_the_browser_hands_in() {
    let ms = now_millis();
    let id6 = v6::new_v6(ms).unwrap();
    let id7 = v7::new_v7(ms).unwrap();
    assert_eq!((id6.version(), id7.version()), (6, 7));
    assert_eq!(v6::unix_millis(&id6), ms);
    assert_eq!(v7::unix_millis(&id7), ms);
    assert_eq!(v7::to_rfc_order(&v7::to_sql_order(&id7)), id7);
}

#[wasm_bindgen_test]
fn a_v7_batch_is_ordered_and_unique() {
    let mut out = [0u8; 16 * 1000];
    v7::new_v7_batch(now_millis(), 1000, &mut out).unwrap();
    let ids = out.as_chunks::<16>().0;
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "v7 batch not strictly increasing"
    );
    v6::new_v6_batch(now_millis(), 1000, &mut out).unwrap();
}

#[wasm_bindgen_test]
fn text_round_trips() {
    let id = v4::new_v4().unwrap();
    assert_eq!(id.to_string().parse::<Uuid>(), Ok(id));
    assert!(
        "00000000-0000-0000-0000--00000000000"
            .parse::<Uuid>()
            .is_err()
    );
}
