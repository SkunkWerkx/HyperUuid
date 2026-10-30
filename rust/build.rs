// The shared library's own name, for a C program that links against it rather than a binding
// that opens it by path. Without these, macOS records the build machine's absolute path as the
// dylib's install name (`otool -D`), so a program linked against it looks there at run time,
// and Linux and Android record no soname at all, so the linker writes whatever path it was
// given into the program's NEEDED entry. `@rpath/` and the unversioned soname let the program
// find the library wherever its rpath or the loader's search path puts it; the soname carries
// no version because `hyperuuid_version()` is the one way to read it.
//
// Only for `cargo cdylib` (the `cdylib` feature): the extension modules are loaded by their
// hosts from a path and keep the defaults. `rustc-link-arg` rather than
// `rustc-cdylib-link-arg`, which Cargo ignores, with a warning, for a package whose manifest
// lists no cdylib (this one's lists only the rlib; Cargo.toml has why). It reaches every
// target this package links, and a build with the `cdylib` feature links nothing else.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_CDYLIB").is_none() {
        return;
    }
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => println!("cargo:rustc-link-arg=-Wl,-install_name,@rpath/libhyperuuid.dylib"),
        Ok("linux" | "android") => println!("cargo:rustc-link-arg=-Wl,-soname,libhyperuuid.so"),
        _ => {}
    }
}
