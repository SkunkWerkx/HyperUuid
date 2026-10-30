//! include/hyperuuid.h, the C header, is generated from src/ffi.rs by cbindgen (configured in
//! cbindgen.toml), and the Swift artifact bundle carries a byte-identical copy as its module's
//! header. This regenerates it and fails when either committed file differs, so a change to an
//! export or its doc comment cannot leave a header behind. `HYPERUUID_BLESS=1` writes both.
//!
//! Two places cannot check it, and every other leg does. wasm: cbindgen runs `cargo
//! metadata`, a subprocess wasmtime cannot start, so the test is compiled out. A copy of rust/
//! without the repository around it (the Forge's musl build takes only rust/ and corpus/ into
//! its container): the test says so and passes. On Windows, git may check the headers out with
//! CRLF line endings, so a committed CRLF reads as LF before the comparison.
#![cfg(not(target_family = "wasm"))]

use std::path::Path;

const HEADERS: [&str; 2] = [
    "../include/hyperuuid.h",
    "../swift/HyperUuidCore.artifactbundle/include/hyperuuid.h",
];

#[test]
fn committed_headers_are_what_cbindgen_generates() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bless = std::env::var_os("HYPERUUID_BLESS").is_some();
    if !bless && !crate_dir.join("../include").is_dir() {
        eprintln!("skipped: no include/ beside rust/, so this is not a whole checkout");
        return;
    }
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).unwrap();
    let mut generated = Vec::new();
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .unwrap()
        .write(&mut generated);
    for header in HEADERS.map(|path| crate_dir.join(path)) {
        if bless {
            std::fs::write(&header, &generated).unwrap();
            continue;
        }
        let committed = std::fs::read_to_string(&header).unwrap_or_default();
        assert!(
            committed.replace("\r\n", "\n").as_bytes() == generated,
            "{} is not what cbindgen generates from src/ffi.rs; \
             run `HYPERUUID_BLESS=1 cargo test --test header` in rust/",
            header.display()
        );
    }
}
