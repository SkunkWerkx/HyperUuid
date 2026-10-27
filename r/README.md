# hyperuuid (R)

RFC 9562 UUID v4 (random), v5 (name-based) and v7 (time-ordered) generation for R, with
inspection (version, variant, timestamp) and a version 8 SQL Server order for v7. A
hand-written C shim calls the Rust core through `.Call`; the core is the same one every other
HyperUuid binding uses, built here from source as a static library and linked into the
package. Every function is vectorised, and character, integer, logical and `POSIXct`
results propagate `NA` element-wise.

## Install

From CRAN:

```r
install.packages("hyperuuid")
```

CRAN ships the source package and compiles it per platform, so its binaries for Windows and
macOS need no Rust; a source install (Linux, or `type = "source"`) does.

From a source tarball, which needs Rust (`cargo` and `rustc` 1.85 or newer, from
[rustup](https://rustup.rs)):

```sh
R CMD INSTALL hyperuuid_<version>.tar.gz
```

The tarball is what `R CMD build r` makes from a checkout after `tools/vendor.sh` (see
[Local loop](#local-loop)). It carries every Rust dependency in `src/rust/vendor.tar.xz` and
builds offline. `configure` prints the `cargo` and `rustc` versions it found, and stops with
a pointer to rustup when either is missing or `rustc` is older than 1.85.

## Usage

```r
library(hyperuuid)

uuid_v4(3)
t <- as.POSIXct(1645557742, origin = "1970-01-01", tz = "UTC")
ids <- uuid_v7(3, time = t)       # one batch: strictly increasing
uuid_timestamp(ids)               # POSIXct in UTC
uuid_v5(uuid_namespaces$dns, "www.example.com")
#> [1] "2ed6657d-e927-568b-95e1-2665a8aea6a2"

uuid_version(c(ids[1], NA, "not a uuid"))
#> [1]  7 NA NA

sql <- uuid_to_sql_order(ids)     # the byte order SQL Server sorts by creation time
uuid_version(sql, layout = "sql") # read in place, no conversion back
#> [1] 7 7 7
identical(uuid_from_sql_order(sql), ids)
#> [1] TRUE
```

Ids out are lowercase and hyphenated. Ids in may be 32 hex digits or the 8-4-4-4-12 form, in
any case. A factor (as `read.csv` can give) is read as its levels, not its integer codes.

## API

Data (`x`, v5's `name`) propagates `NA`: `NA` or malformed in, `NA` out, element by element.
Arguments (`n`, `time`, `namespace`, `version`, `layout`) are strict and error. The raw
doors are strict everywhere.

| Function | Returns | `NA` and malformed input | Errors |
| --- | --- | --- | --- |
| `uuid_v4(n = 1)` | character, `n` ids | (no data input) | `n` not one non-negative whole number |
| `uuid_v4_raw(n = 1)` | raw matrix, 16 x `n` | (no data input) | as `uuid_v4()` |
| `uuid_v7(n = 1, time = Sys.time())` | character, `n` ids | (no data input) | `n` as `uuid_v4()`; `time` per [the time rule](#the-time-rule); `n` over `uuid_v7_max_batch` with a scalar `time` |
| `uuid_v7_raw(n = 1, time = Sys.time())` | raw matrix, 16 x `n` | (no data input) | as `uuid_v7()` |
| `uuid_v5(namespace, name)` | character, one id per name | `NA` name gives `NA` | `namespace` not one valid UUID (an `NA` namespace included); `name` not a character vector or a list of raw vectors |
| `uuid_to_raw(x)` | raw matrix, 16 x `length(x)` | error, naming the first bad element | `x` not character; any `NA` or malformed element |
| `uuid_from_raw(m)` | character, one id per column | (raw has no `NA`) | `m` not a 16-row raw matrix or a raw vector of 16k bytes |
| `uuid_version(x, layout = c("rfc", "sql"))` | integer | `NA` | bad `layout` |
| `uuid_variant(x)` | character: `"ncs"`, `"rfc9562"`, `"microsoft"`, `"future"` | `NA` | `x` not character |
| `uuid_is_rfc(x, version, layout = c("rfc", "sql"))` | logical | `NA` | `version` not one whole number from 0 to 15; bad `layout` |
| `uuid_timestamp(x, layout = c("rfc", "sql"))` | `POSIXct`, UTC | `NA`, and `NA` for any id that is not a v7 | bad `layout` |
| `uuid_v7_unix_millis(x, layout = c("rfc", "sql"))` | double, Unix milliseconds | `NA` | bad `layout` |
| `uuid_to_sql_order(x)` | character | `NA` | a well-formed id that is not a v7 |
| `uuid_from_sql_order(x)` | character | `NA` | a well-formed id that is not in SQL order |
| `hyperuuid_native_version()` | `package_version` of the loaded core | | |

Every function that takes `x` also errors on an `x` that is not a character vector (a factor
or a bare `NA` is accepted). `uuid_v7_unix_millis()` is the unchecked bit read: it returns
the 48 timestamp bits of any well-formed id without checking the version, so guard it with
`uuid_is_rfc(x, 7)`; `uuid_timestamp()` is the checked reading.

Constants: `uuid_nil` and `uuid_max` (RFC 9562 sections 5.9 and 5.10), `uuid_namespaces`
(`dns`, `url`, `oid`, `x500`, appendix A) and `uuid_v7_max_batch` (`67108864L`, 2^26).

In `layout = "sql"`, `uuid_version()` answers 7 for a converted v7 and 0 for anything else,
and `uuid_timestamp()`, `uuid_v7_unix_millis()` and `uuid_is_rfc()` read the SQL-ordered
bytes in place. The layout is the caller's to know, not something a value reveals; the
[root README](../README.md#sql-server-ordering) has the details.

### The time rule

`time` is a `POSIXct`, `POSIXlt` or `Date`, or a number of seconds since the Unix epoch. It
becomes whole Unix milliseconds as `floor(seconds * 1000)`; a `Date` is midnight UTC, not
local midnight. It has length 1 or `n`:

- Length 1 generates the whole request as one batch sharing the 26-bit monotonic counter, so
  the ids are strictly increasing; at most `uuid_v7_max_batch` per call.
- Length `n` generates one id per element, each from its own time.

Any other length errors with both lengths named. A missing or non-finite element errors
before the core is called, and a time before 1970 or past the 48-bit field errors too.

### The raw door

Raw results are strict, with no `NA`: `uuid_to_raw()` errors on the first `NA` or malformed
element, so filter with `is.na()` or `uuid_is_rfc()` first:

```r
ids <- c(uuid_namespaces$dns, NA, "not a uuid")
uuid_to_raw(ids[uuid_is_rfc(ids, 1) %in% TRUE])
```

`uuid_to_raw(character(0))` is a 16 x 0 raw matrix and `uuid_from_raw(raw(0))` is
`character(0)`, so empty data frames round-trip.

## Benchmarks

Apple M6 (Mac18,5), macOS, R 4.6.1 (Homebrew build), measured 2026-10-26, against the
[uuid](https://cran.r-project.org/package=uuid) package 1.2.2 with bench 1.1.4
(`R_LIBS=<lib> Rscript bench/bench.R` against an installed copy). These were measured on a
different machine and OS from the root README's linux-x64 tables and are not comparable to
them.

```text
2026-10-26, R version 4.6.1 (2026-06-24), Apple M6 (Mac18,5)
hyperuuid 1.0.0, uuid 1.2.2, bench 1.1.4

 expression                                    median   itr/sec mem_alloc
 hyperuuid::uuid_v4()                            1.31µs 734965  12.92KB  
 uuid::UUIDgenerate()                            9.31µs 106573  33.89KB  
 hyperuuid::uuid_v7()                            2.91µs 338648  26.09KB  
 uuid::UUIDgenerate(use.time = TRUE)             1.8µs  553148   0B      
 hyperuuid::uuid_v4(1000)                      791.1µs    1224  23.53KB  
 uuid::UUIDgenerate(n = 1000)                    6.98ms    143   7.86KB  
 hyperuuid::uuid_v7(1000)                      158.75µs   5834  23.53KB  
 uuid::UUIDgenerate(use.time = TRUE, n = 1000) 100.86µs   8059   7.86KB  
```

| Comparison (median) | hyperuuid | uuid | Result |
| --- | --- | --- | --- |
| v4, one id | 1.31 µs | 9.31 µs | hyperuuid 7.1x faster |
| v4, 1000 ids | 791.1 µs | 6.98 ms | hyperuuid 8.8x faster |
| time-based, one id | 2.91 µs | 1.8 µs | **hyperuuid 1.6x slower** |
| time-based, 1000 ids | 158.75 µs | 100.86 µs | **hyperuuid 1.6x slower** |

The time-based rows compare different work: `UUIDgenerate(use.time = TRUE)` makes version 1
ids (a clock sequence and a fixed node, no random bits), while `uuid_v7()` makes RFC 9562
version 7 ids with a monotonic counter and fresh random bits. uuid has no version 7. It is
still the call an R user reaching for time-based ids has today, and it is faster.
`uuid_v4(1000)` costs more than `uuid_v7(1000)` because each v4 is one random-source call,
while a v7 batch fetches its random bits once.

## Local loop

From `r/`, with Rust, GNU tar (`brew install gnu-tar` on macOS) and the R packages testthat,
jsonlite and devtools:

```sh
tools/vendor.sh            # stage the core and the corpus, vendor the crates, write inst/AUTHORS
tools/vendor.sh --verify   # the staged corpus, core and lock are present
Rscript -e 'devtools::test()'
cd .. && R CMD build r
_R_CHECK_CRAN_INCOMING_=false R CMD check --as-cran hyperuuid_<version>.tar.gz
```

`tools/vendor.sh --stage` copies the core and the corpus files and derives the lock only,
without vendoring. The core under `src/rust/core/`, the wrapper crate's
`src/rust/Cargo.lock` (the core's `rust/Cargo.lock` pruned to the wrapper's packages, so the
crates vendored are the versions the core is tested with), the vendor tarball and the corpus
copy under `tests/testthat/corpus/` are generated and gitignored; the tarball `R CMD build`
makes carries them all. Rerun `tools/vendor.sh` after any change in `rust/` or `corpus/`. A missing corpus file is a test failure, never a skip; the
replay skips only when jsonlite (a Suggests) is absent, as on CRAN's no-Suggests check.

Lint against an installed copy: lintr's object-usage linter loads the package it lints, and
without one every exported name is a false positive.

```sh
R CMD INSTALL --library=/tmp/rlib .
R_LIBS=/tmp/rlib Rscript -e 'print(c(lintr::lint_package(), lintr::lint_dir("bench"), lintr::lint_dir("tools")))'
cargo fmt --check --manifest-path src/rust/Cargo.toml
clang-format --dry-run -Werror src/*.c src/*.h
```

`lint_package()` reads `.lintr` and covers `R/` and `tests/`; the two `lint_dir()` calls
cover the directories outside the package. An empty list is a pass. The C shim is formatted
by `clang-format` with `.clang-format`; `core.h` keeps one prototype per line, whatever its
width, because `check_exports.py` reads them.

## Windows

Rtools ships no Rust, so a Windows source build needs rustup's `x86_64-pc-windows-gnu`
target (`rustup target add x86_64-pc-windows-gnu`), which `src/Makevars.win` picks through
`tools/rustarch.R`. On ARM64 it picks `aarch64-pc-windows-gnullvm`; R on Windows ARM64 is
experimental and that build is untested. CRAN's own Windows binaries are built by CRAN, so
installing them needs no Rust.

## License

MIT; see [LICENSE.md](LICENSE.md). The vendored Rust crates' authors are listed in
`inst/AUTHORS`.
