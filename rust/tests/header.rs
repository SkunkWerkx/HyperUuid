//! include/hyperuuid.h, the C header, is generated from src/ffi.rs by cbindgen (configured in
//! cbindgen.toml), and the Swift artifact bundle carries a byte-identical copy as its module's
//! header. This regenerates it and fails when either committed file differs, so a change to an
//! export or its doc comment cannot leave a header behind. `HYPERUUID_BLESS=1` writes both.

use std::path::Path;

const HEADERS: [&str; 2] = [
    "../include/hyperuuid.h",
    "../swift/HyperUuidCore.artifactbundle/include/hyperuuid.h",
];

#[test]
fn committed_headers_are_what_cbindgen_generates() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).unwrap();
    let mut generated = Vec::new();
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .unwrap()
        .write(&mut generated);
    let bless = std::env::var_os("HYPERUUID_BLESS").is_some();
    for header in HEADERS.map(|path| crate_dir.join(path)) {
        if bless {
            std::fs::write(&header, &generated).unwrap();
            continue;
        }
        let committed = std::fs::read(&header).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is not what cbindgen generates from src/ffi.rs; \
             run `HYPERUUID_BLESS=1 cargo test --test header` in rust/",
            header.display()
        );
    }
}
