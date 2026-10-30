# CRAN Submission Checklist — HyperUuid

_As of 2026-10-29_

To get HyperUuid onto CRAN, you need a clean `R CMD check --as-cran` on R-devel, and you need to meet CRAN's Rust rules: vendored crates, an offline build, two cores at most, the rustc version reported, and credit for every crate author. HyperUuid is the dress rehearsal before HyperCast and HyperTabular. Most first-time rejections of Rust-backed packages come from the Rust rules, not the general ones.

## How the process works

1. **Run the checks yourself.** You need zero errors and zero warnings. Aim for zero NOTEs too, apart from the unavoidable "New submission" NOTE. CRAN policy asks that `R CMD check --as-cran` be run on the exact tarball you upload, using current R-devel.
2. **Submit the tarball.** Use the web form or `devtools::submit_cran()`. CRAN emails a confirmation link, and nothing happens until you click it.
3. **Automated pretests.** These run on Debian and Windows. A failure there gets an automatic rejection within hours.
4. **Human review.** Every new package gets one. The reviewer reads DESCRIPTION, the Rd files and the examples, and enforces the style rules below.
5. **Fix and resubmit.** If you get feedback, add a `## Resubmission` section at the top of `cran-comments.md` that says what changed, then resubmit.
6. **After acceptance.** CRAN keeps checking the package on all its platforms. If a new R or toolchain release breaks it, you get a deadline to fix it, or the package is archived.

## Before you submit (general)

This is the list a CRAN reviewer works through by hand.

### Checks

- [ ] Local `devtools::check()`, run as `--as-cran` on R-devel
- [ ] `devtools::check_win_devel()` and `devtools::check_mac_release()`
- [ ] rhub v2 through GitHub Actions (the old single-call rhub method is retired)
- [ ] `urlchecker::url_check()` and `devtools::spell_check()`
- [ ] One run of `checktor`, which catches things that pass R CMD check but fail review, such as `http://` links or a tarball over the size limit
- [ ] `cran-comments.md` lists the test environments, the check results, that this is a new submission, and why any remaining NOTE can't be removed

### DESCRIPTION

- [ ] `Title` is in Title Case, has no full stop at the end, and has no "A package for…" or "…for R"
- [ ] `Description` doesn't start with the package name or "This package"; software names are in single quotes ('Rust', 'UUID'); references are written as `<doi:...>` or `<https:...>`
- [ ] `Authors@R` lists everyone with roles, and includes at least `aut`, `cre` and `cph`
- [ ] `License: MIT + file LICENSE`, where the LICENSE file holds only the `YEAR` and `COPYRIGHT HOLDER` lines
- [ ] Version is `0.0.1` with no `.9000` dev suffix
- [ ] Name confirmed as free with `available::available("HyperUuid")`

### Documentation and examples

- [ ] Every exported function's Rd has a `\value` section, including those that return `invisible(NULL)`
- [ ] `\dontrun{}` is used only where an example really can't run
- [ ] Examples and tests write only to `tempdir()`
- [ ] Non-result output goes through `message()` or `warning()`, not `print()` or `cat()`

## Rust-specific requirements

These come from CRAN's "Using Rust in CRAN packages" page.

- [ ] **SystemRequirements:** declare exactly `SystemRequirements: Cargo (Rust's package manager), rustc`. CRAN uses this exact string to track Rust packages.
- [ ] **Toolchain detection:** `configure` and `configure.win` look for `cargo` and `rustc` on the PATH and in `~/.cargo/bin`. If they're missing, tell the user how to install them, and never install them yourself.
- [ ] **Report the compiler version:** print the `rustc` version into the install log, the same way R reports its C, C++ and Fortran compilers. R CMD check now looks for this line.
- [ ] **Two cores at most:** pass `cargo build -j 2`. Cargo's default is every logical CPU, which breaks CRAN policy.
- [ ] **Offline build:** run `cargo vendor`, compress the result as `vendor.tar.xz` (xz is required), and build with `--offline`. Nothing may be downloaded during install.
- [ ] **No writes to the user's home directory:** point `CARGO_HOME` at a directory inside the build tree.
- [ ] **Credit the crates:**
    - Generate `inst/AUTHORS` from `cargo metadata`.
    - Add this to `Authors@R`: `person("Authors of the dependency Rust crates", role = "cph", comment = "see inst/AUTHORS")`.
    - Add a `LICENSE.note` covering the vendored crates' licenses.
- [ ] **Low minimum Rust version:** CRAN's Linux machines use the Rust packaged by the distribution, which lags behind. Set `rust-version` in `Cargo.toml` and test against it.

## Rust traps that aren't in the official list

- [ ] **Tarball size:** CRAN's limit is about 5 MB, and vendored `windows-sys` and similar crates can push you over it. Prune the dependency tree, or trim the vendored crates and fix up their `.cargo-checksum.json` files.
- [ ] **Build leftovers:** exclude `src/rust/target/` and any loose `vendor/` directory with `.Rbuildignore`, or remove them in `make clean`, so they stay out of the tarball.
- [ ] **Installed size:** turn on `strip`, and possibly `lto`, in the release profile. This avoids the "installed size is X Mb" NOTE.
- [ ] **Panics and output:** a Rust panic must never abort the R session. extendr and savvy both catch panics at the FFI boundary, so don't bypass that. Never use `println!` or `std::process::exit`; send output through R.
- [ ] **Symbol NOTE:** R CMD check may flag the Rust standard library for calling `abort` or `exit` in compiled code. If that NOTE appears, explain in `cran-comments.md` that it comes from the standard library's panic machinery and can't be reached from R.

## Tooling shortcut

If the bindings use extendr, `rextendr::use_cran_defaults()` and `rextendr::vendor_pkgs()` set up most of the Rust section for you. Between them they handle vendoring, `-j 2`, `--offline` and printing the rustc version.

## Sources

- [CRAN: Using Rust in CRAN packages (mirror)](https://brieger.esalq.usp.br/CRAN/web/packages/using_rust.html)
- [extendr: CRAN compliant extendr packages](https://extendr.github.io/rextendr/dev/articles/cran-compliance.html)
- [r-rust FAQ](https://github.com/r-rust/faq)
- [Datanovia: Check an R Package Before Submitting to CRAN](https://www.datanovia.com/blog/check-r-package-before-cran)
- [checktor](https://blog.thecoatlessprofessor.com/projects/checktor/)
