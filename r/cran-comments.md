## Submission

This is a new submission.

## Test environments

* macOS 27.2 on Apple M6 (aarch64-apple-darwin27.0.0), R 4.6.2 (Homebrew build), rustc and
  cargo 1.99.1.
* Debian forky/sid on x86-64 (the `rocker/r-devel` image), R Under development (unstable)
  (2026-10-25 r90763), with the distribution's rustc and cargo 1.97.1.

## R CMD check results

`R CMD check --as-cran hyperuuid_1.0.0.tar.gz`, with `_R_CHECK_CRAN_INCOMING_REMOTE_=TRUE`.

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
`src/rust/vendor.tar.xz` (14 crates, authors in `inst/AUTHORS`), offline, with `cargo
build -j 2`, `CARGO_HOME` under `src/`, and removes the Rust build directory after
linking. `configure` prints the `cargo` and `rustc` versions and stops with a pointer to
https://rustup.rs when either is missing or `rustc` is older than 1.89.

The Rust library is `no_std` with `panic = "abort"`. In the built `hyperuuid.so`,
`nm -g` finds no `abort` symbol (`_abort`: 0), and the check reports no calls to `abort`,
`exit` or `printf` from compiled code.

## Windows

Rtools ships no Rust, so the Windows build uses rustup's `x86_64-pc-windows-gnu` target
(`aarch64-pc-windows-gnullvm` on ARM64), chosen by `tools/rustarch.R` in
`src/Makevars.win`. Not tested locally; to be confirmed on win-builder.
