## Submission

This is a new submission.

## Test environments

* macOS 27.2 on Apple M6 (aarch64-apple-darwin27.0.0), R 4.6.2 (Homebrew build), rustc and
  cargo 1.99.1.
* Debian forky/sid on x86-64 (the `rocker/r-devel` image), R Under development (unstable)
  (2026-10-25 r90763), with the distribution's rustc and cargo 1.97.1.
* The release's CI (GitHub Actions, rustc and cargo 1.99.1), `R CMD check --no-manual
  --as-cran` failing on any NOTE, on the source the submitted tarball was built from: Ubuntu
  26.04 x86-64 with R 4.6.2 and R 4.5.3, Ubuntu 26.04 aarch64 with R 4.6.2, macOS 27 aarch64
  with R 4.6.2, and Windows Server 2022 x86-64 with R 4.6.2 (ucrt) and rustup's
  `x86_64-pc-windows-gnu` target. All five: `Status: OK`.

## R CMD check results

`R CMD check --as-cran hyperuuid_1.1.0.tar.gz`, with `_R_CHECK_CRAN_INCOMING_REMOTE_=TRUE`.

macOS, R 4.6.2, with the PDF manual (TinyTeX), `checkbashisms`, `qpdf` and `pandoc` installed:

    Status: 1 NOTE

    * checking CRAN incoming feasibility ... NOTE
    Maintainer: 'Brian Buvinghausen <buvinghausen@gmail.com>'

    New submission

R-devel on Debian, `--no-manual`:

    Status: 2 NOTEs

    * checking CRAN incoming feasibility ... NOTE
    New submission

    * checking compilation flags used ... NOTE
    Compilation used the following non-portable flag(s):
      '-Wdate-time' '-Werror=format-security' '-Wformat'

The first NOTE is the new-submission notice. The second names flags from that image's R
build configuration (Debian's hardening defaults in `R CMD config CFLAGS`), not from this
package's `Makevars`, which sets no compiler flags. On both, the 1,228 testthat expectations
pass, including the replay of the five bundled corpus files.
`NEWS.md`'s heading carries the version number, which R's NEWS parser requires.

## Compiled code

The package builds a Rust static library from the vendored crates in
`src/rust/vendor.tar.xz` (15 crates), offline, with `cargo build -j 2`, `CARGO_HOME` under
`src/`, and removes the Rust build directory after linking. `configure` prints the `cargo`
and `rustc` versions and stops with a pointer to https://rustup.rs when either is missing or
`rustc` is older than 1.89.

The wrapper crate in `src/rust/` depends on the `hyperuuid` crate from crates.io at exactly
this package's version, as any Rust consumer of it would, and on nothing else; the 14
crates below it are its dependencies, credited in `inst/AUTHORS`. The vendored `hyperuuid`
is that release's source as `cargo package` writes it, from the tagged commit the crates.io
release is uploaded from.

The core is `#![no_std]`, allocation-free and panic-free, and leaves what those would need
to the binding and to R:

* No std. The crate is built with `default-features = false`, which its CI proves by
  compiling it for a bare-metal target (`thumbv7em-none-eabi`): no I/O, threads,
  formatting or heap from Rust's standard library, and the archive R links carries no copy
  of it. The one thing it takes from the operating system is its random source
  (`getrandom(2)` on Linux, `CCRandomGenerateBytes` on macOS, `ProcessPrng` on Windows);
  the clock is R's (`uuid_v7()` passes `Sys.time()` down as milliseconds).
* No allocation. No function in the core allocates, batch generation included; the crate
  declares no `alloc` at all, and its suite asserts zero allocations across the public API
  with a counting allocator (`tests/allocation_free.rs`). Every buffer is R's: the C shim
  writes into vectors R allocated and sized.
* No panics. No input makes the core panic; every failure is a return code over the C
  ABI, which the shim turns into an R error or an `NA` as each function documents. The
  crate proves it at link time (under its `no-panic` feature every generation, parsing and
  conversion function and every C export carries `#[no_panic]`, linked in a release build
  in CI), and the archive is built with `panic = "abort"` behind a panic handler nothing
  reaches: in the built `hyperuuid.so`, `nm -g` finds no `abort` symbol (`_abort`: 0), and
  the check reports no calls to `abort`, `exit` or `printf` from compiled code.

## Windows

Rtools ships no Rust, so the Windows build uses rustup's `x86_64-pc-windows-gnu` target
(`aarch64-pc-windows-gnullvm` on ARM64), chosen by `tools/rustarch.R` in
`src/Makevars.win`. Checked on Windows Server 2022 x86-64 in the release's CI (above).
