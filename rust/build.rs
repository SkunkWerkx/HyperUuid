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
    php_thread_safety();
    if std::env::var_os("CARGO_FEATURE_CDYLIB").is_none() {
        return;
    }
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => println!("cargo:rustc-link-arg=-Wl,-install_name,@rpath/libhyperuuid.dylib"),
        Ok("linux" | "android") => println!("cargo:rustc-link-arg=-Wl,-soname,libhyperuuid.so"),
        _ => {}
    }
}

// The `php` feature's `php_zts`: set when the PHP ext-php-rs builds against is thread-safe,
// asked the way ext-php-rs asks it (`PHP`, else `php` on PATH, then `php -i`), since that is the
// PHP whose headers the extension compiles with. Two things read it. On Windows, the extern
// blocks in src/php_ext/ name the engine's import library, php8 for an NTS PHP and php8ts for a
// TS one (a TS PHP ships no php8.lib, and is php8ts.dll). And MINIT refuses a `dl()` load on a
// thread-safe PHP, which would register the classes again from a request thread into the
// process-wide state ext-php-rs keeps for them (src/php_ext/mod.rs).
#[cfg(feature = "php")]
fn php_thread_safety() {
    println!("cargo::rustc-check-cfg=cfg(php_zts)");
    ext_php_rs_build::emit_rerun_if_env_changed();
    let php = ext_php_rs_build::find_php().expect("the php feature builds against a PHP");
    let info = ext_php_rs_build::PHPInfo::get(&php).expect("`php -i` runs");
    let zts = info.thread_safety().expect("`php -i` names it");
    if zts {
        println!("cargo:rustc-cfg=php_zts");
    }
}

#[cfg(not(feature = "php"))]
fn php_thread_safety() {
    println!("cargo::rustc-check-cfg=cfg(php_zts)");
}
