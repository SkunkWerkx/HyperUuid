# CRAN Submission Checklist — HyperUuid

_As of 2026-10-30. Ticked items were verified on the built tarball that day; what proved each is in italics._

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

- [x] Local `devtools::check()`, run as `--as-cran` on R-devel
  _`R CMD check --as-cran` on the tarball: R 4.6.2 on macOS, 1 NOTE (new submission); R-devel r90763 on x86-64 Debian (`rocker/r-devel`, the distribution's rustc 1.97.1), 2 NOTEs (new submission; that image's own hardening `CFLAGS`). The first R-devel run also flagged `command -v` in `configure` as a bashism (the image's checkbashisms 2.19.4); fixed in `configure` and `configure.win`._
- [ ] `devtools::check_win_devel()` and `devtools::check_mac_release()`
  _Not run: both upload the package and win-builder emails the maintainer. Windows is the one platform no local check covers._
- [ ] rhub v2 through GitHub Actions (the old single-call rhub method is retired)
- [x] `urlchecker::url_check()` and `devtools::spell_check()`
  _All URLs correct. The spell check flags only technical terms (UUIDs, POSIXct, vendored, rustup, Werkx), no typos._
- [ ] One run of `checktor`, which catches things that pass R CMD check but fail review, such as `http://` links or a tarball over the size limit
- [x] `cran-comments.md` lists the test environments, the check results, that this is a new submission, and why any remaining NOTE can't be removed

### DESCRIPTION

- [x] `Title` is in Title Case, has no full stop at the end, and has no "A package for…" or "…for R"
- [x] `Description` doesn't start with the package name or "This package"; software names are in single quotes ('Rust', 'UUID'); references are written as `<doi:...>` or `<https:...>`
  _'Rust' and 'SQL Server' are quoted (UUID is a format, not software). The method's reference is cited as reviewers ask: RFC 9562 (Davis, Peabody and Leach 2024) <doi:10.17487/RFC9562>._
- [x] `Authors@R` lists everyone with roles, and includes at least `aut`, `cre` and `cph`
- [x] `License: MIT + file LICENSE`, where the LICENSE file holds only the `YEAR` and `COPYRIGHT HOLDER` lines
- [x] Version has no `.9000` dev suffix
  _CRAN asks only that. The package carries the repository's one coordinated version, set by `prepare-release.yml`, not `0.0.1`._
- [x] Name confirmed as free with `available::available("hyperuuid")`
  _The package is `hyperuuid`, lowercase; not in CRAN's package list._

### Documentation and examples

- [x] Every exported function's Rd has a `\value` section, including those that return `invisible(NULL)`
- [x] `\dontrun{}` is used only where an example really can't run
- [x] Examples and tests write only to `tempdir()`
- [x] Non-result output goes through `message()` or `warning()`, not `print()` or `cat()`

## Rust-specific requirements

These come from CRAN's "Using Rust in CRAN packages" page.

- [x] **SystemRequirements:** declare exactly `SystemRequirements: Cargo (Rust's package manager), rustc`. CRAN uses this exact string to track Rust packages.
- [x] **Toolchain detection:** `configure` and `configure.win` look for `cargo` and `rustc` on the PATH and in `~/.cargo/bin`. If they're missing, tell the user how to install them, and never install them yourself.
- [x] **Report the compiler version:** print the `rustc` version into the install log, the same way R reports its C, C++ and Fortran compilers. R CMD check now looks for this line.
- [x] **Two cores at most:** pass `cargo build -j 2`. Cargo's default is every logical CPU, which breaks CRAN policy.
- [x] **Offline build:** run `cargo vendor`, compress the result as `vendor.tar.xz` (xz is required), and build with `--offline`. Nothing may be downloaded during install.
- [x] **No writes to the user's home directory:** point `CARGO_HOME` at a directory inside the build tree.
- [x] **Credit the crates:**
    - Generate `inst/AUTHORS` from `cargo metadata`.
    - Add this to `Authors@R`: `person("Authors of the dependency Rust crates", role = "cph", comment = "see inst/AUTHORS")`.
    - Add a `LICENSE.note` covering the vendored crates' licenses.
      _Covered instead by `inst/AUTHORS`, which `r/tools/vendor.sh` writes with each crate's license (all MIT OR Apache-2.0; `r-efi` also offers LGPL-2.1+). Add the file only if a reviewer asks._
- [x] **Low minimum Rust version:** CRAN's Linux machines use the Rust packaged by the distribution, which lags behind. Set `rust-version` in `Cargo.toml` and test against it.
  _1.89, which `configure` enforces and CI proves for the core; Debian testing packages 1.97.1 and Fedora 1.99.1._

## Rust traps that aren't in the official list

- [x] **Tarball size:** CRAN's limit is about 5 MB, and vendored `windows-sys` and similar crates can push you over it. Prune the dependency tree, or trim the vendored crates and fix up their `.cargo-checksum.json` files.
  _847 KB._
- [x] **Build leftovers:** exclude `src/rust/target/` and any loose `vendor/` directory with `.Rbuildignore`, or remove them in `make clean`, so they stay out of the tarball.
- [x] **Installed size:** turn on `strip`, and possibly `lto`, in the release profile. This avoids the "installed size is X Mb" NOTE.
  _No such NOTE on either check._
- [x] **Panics and output:** a Rust panic must never abort the R session. extendr and savvy both catch panics at the FFI boundary, so don't bypass that. Never use `println!` or `std::process::exit`; send output through R.
  _The core is `no_std` with `panic = "abort"` and no extendr or savvy; R reaches it through a C shim, and the compiled-code check is OK on both._
- [x] **Symbol NOTE:** R CMD check may flag the Rust standard library for calling `abort` or `exit` in compiled code. If that NOTE appears, explain in `cran-comments.md` that it comes from the standard library's panic machinery and can't be reached from R.
  _It does not appear: `nm -g` finds no `abort` in the built library._

## Tooling shortcut

Not applicable: the package uses no extendr. If the bindings use extendr, `rextendr::use_cran_defaults()` and `rextendr::vendor_pkgs()` set up most of the Rust section for you. Between them they handle vendoring, `-j 2`, `--offline` and printing the rustc version.

## Before HyperCast and HyperTabular go to CRAN

_From a review of both repositories' master on 2026-10-31. Each finding marked confirmed was reproduced with a probe program; the inputs and outputs are kept so they can become corpus vectors. HyperUuid's own submission does not wait on any of this: it is the rehearsal, and these are the gates for the two packages that have to land right._

### Fix in the cores first (wrong data, or a whole file refused)

- [ ] **HyperTabular: a stray `"` in an unquoted cell.** Confirmed. The scanner's quote parity runs across the whole input, so one cell with an odd number of quotes (`12" pipe`) switches it into quoted mode. One such row aborts the read ("input ends inside a quoted cell"). Two such rows merge every record between them into one cell, with no error: `12" pipe,1⏎3,4⏎6" pipe` is one cell and the record `3,4` is gone. This contradicts `docs/delimited.md` ("a cell that does not start with `"` is delivered raw, quotes and all"), and `corpus/delimited.json` only has cells with an even number of quotes. Fix: a quote is a quote character only when it opens a cell or sits inside an open quoted cell. Add odd-count vectors.
- [ ] **HyperTabular: ragged rows.** Confirmed. A short row, a long row or a single trailing comma (`1,2,` under two headers) is a structural error that ends the read. R users expect `fill = TRUE`. Add a declared policy: short rows padded with `Empty` verdicts, long rows a per-row fault or truncated.
- [ ] **HyperTabular: no skip, header row or range.** Confirmed: a file that opens with title lines fails, and `SheetOptions` has only `header`, `skip_empty_rows` and `batch_rows`. Add leading rows to skip and a header row index for delimited text and sheets, and a cell range (`A5:F200`) for sheets, as readxl has.
- [ ] **HyperCast: group sizes are not validated.** Confirmed and documented (`verdict.rs`, `NumFormat::GROUPING`): under `INVARIANT`, `1,2` reads as 12, `1,00` as 100, `123,4567` as 1234567, so European data read with the defaults comes back silently wrong. Add a strict-grouping flag (groups of three, and the Indian two-and-three pattern) and make it the R binding's default. `DETECT` already refuses `1,234` as ambiguous.
- [ ] **HyperTabular: bytes that are not UTF-8.** Confirmed: `\xe9t\xe9` is handed out raw. R errors later on a string marked UTF-8 that is not. Add a declared encoding (single-byte code-page tables, which fit `no_std`, and UTF-16LE), or validate every text cell in the R binding.

### Gaps that will cost adoption

- [ ] **Fault detail.** Confirmed: `12.185` under `DETECT` and `12x185` both come back `Malformed` at a span, so a `problems()` table cannot say why. Add a detail code beside the reason (ambiguous separator, zone required, undeclared order, trailing input, which field is out of range), through new exports so the 1.0 ABI layout holds.
- [ ] **Missing-value markers.** `NA`, `N/A`, `NULL`, `-` and `#N/A` are `Malformed`, not `Empty`. Add a declared token list per column or per dialect, the `na =` every R reader takes.
- [ ] **Date and time shapes, all confirmed refused:** compact `20260107`; two-digit years (`01/07/26`, with a declared pivot); month names (`Jan 7, 2026`, Oracle's default `07-JAN-26`); Postgres' `timestamptz` text (`2026-01-02 15:04:05+00`: the space RFC 3339 allows, and an offset without minutes); a trailing no-break space (`42\u00A0`).
- [ ] **Formulas with no cached value** read as `Empty`, the same as a blank cell. Workbooks written by openpyxl and report generators often have none, so a whole calculated column reads as blank with no hint. Give it its own signal.
- [ ] Smaller: `"` is the only quote character, there is no escape or comment character, and the integer doors refuse `1.5e3`.

### Decide in R before writing either binding

- [ ] **`NA_integer_` is `INT_MIN`.** An i32 of `-2147483648` collides with R's missing value: make it `OutOfRange` in R, or return i32 as double.
- [ ] **64-bit integers:** `bit64::integer64` (a dependency) or double (inexact above 2^53). u32 above 2^31 needs double too.
- [ ] **Decimal:** R has no decimal type. Choose double (lossy, what R users expect), character, or a vctrs class, and document it.
- [ ] **Timestamps:** `POSIXct` is a double of seconds, about a microsecond at today's dates, so nanoseconds truncate. Say so, as the Python and PHP bindings do.
- [ ] **Time of day:** base R has no class for it: `hms` (a dependency) or `difftime`.
- [ ] **Shape:** a `col_types`-style plan and readr's `problems()` attribute for faults. Nothing is sniffed, so offer a helper that suggests a plan for the user to read and pass back, never one applied silently.
- [ ] **Text columns dominate the cost,** as in Ruby and Python: every cell becomes a `CHARSXP` in R's global string cache. vroom avoids that with ALTREP. The comparison will be vroom, `data.table::fread` (multi-threaded) and readxl: sell the verdicts, not raw speed.

### CRAN packaging for the two

- [ ] **HyperTabular's R package depends on HyperCast's,** as its crate does, so the dependency tree is honest on CRAN too. That means HyperCast is accepted first (one extra review cycle), and every HyperCast update is checked against HyperTabular, which blocks the update if it breaks. The Rust lockstep already works this way.
- [ ] **Tarball size:** HyperCast vendors nothing and HyperTabular only `hypercast`, so the crates are small. HyperTabular's corpus is 3 MB against CRAN's 5 MB guideline: ship a small subset and skip the cases whose files are missing.
- [ ] **Panics:** both cores are proven panic-free, which is worth saying in `cran-comments.md`. The R glue code still catches unwinds at the boundary.
- [ ] **Carry over every reviewer comment HyperUuid gets** to both packages before either is submitted.

## Sources

- [CRAN: Using Rust in CRAN packages (mirror)](https://brieger.esalq.usp.br/CRAN/web/packages/using_rust.html)
- [extendr: CRAN compliant extendr packages](https://extendr.github.io/rextendr/dev/articles/cran-compliance.html)
- [r-rust FAQ](https://github.com/r-rust/faq)
- [Datanovia: Check an R Package Before Submitting to CRAN](https://www.datanovia.com/blog/check-r-package-before-cran)
- [checktor](https://blog.thecoatlessprofessor.com/projects/checktor/)
