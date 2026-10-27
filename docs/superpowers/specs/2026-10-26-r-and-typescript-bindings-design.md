# v1.1 design: R and TypeScript bindings

Brainstormed 2026-10-26 (architectural path), revised the same day after two review rounds
(the changes are marked **[review N]** inline and listed at the end). This is the design
spec; `docs/superpowers/plans/` holds the implementation plan derived from it.

## Context

The owner's actuarial team works in R, so HyperUuid needs an R package they can install from
CRAN, with the v7 and SQL Server layout behaviour vectorised the way R code expects. A
TypeScript package that runs on both Node and Bun is the second v1.1 binding. Both are proven
in this repo first and then cascaded to HyperCast and HyperTabular, so every recipe and every
Forge change below is generic (driven by inputs and `.github/hyper.json`), never
HyperUuid-shaped.

Nothing in the core changes. Both bindings consume the existing thirteen C exports in
`rust/src/ffi.rs` (or the crate API behind them) with the same persisted-bytes contract; the
corpus is replayed unchanged.

## Decisions taken in the brainstorm

| Decision | Choice | Why |
| --- | --- | --- |
| R channel | CRAN | What an actuarial IT team allows without an exception |
| R vehicle | Hand-written C shim (`.Call`) over the Rust **staticlib** | Thirteen exports stay the only contract; no `r` feature or extendr tree in the vendor tarball; the hex loop sits in C beside the batch call |
| R return type | Character vectors (lowercase, hyphenated), `n` argument; `*_raw` variants return a 16×n raw matrix | What `uuid::UUIDgenerate`, DBI and data frames expect |
| R API names | `uuid_` prefix, snake_case, `n` first (`uuid_v7(n = 1, time = Sys.time())`) | R idiom; a bare `new_v7()` is too generic once attached |
| TS vehicle | napi-rs v3 native addon, no wasm fallback this release | Matches the "no bridge" pitch and the benchmark culture; Bun and Node both load Node-API addons |
| npm naming | `hyperuuid` (main) + `@skunkwerkx/hyperuuid-<platform>` optionalDependencies | One org serves all three repos; unscoped namespace stays clean |
| Platforms | TS: the repo's eight RIDs. R: source package, CRAN builds its own binaries | Parity with the platform table |

Assumptions stated to the owner and not contested: Node floor 22 (tested 22, 24, 26 and
latest Bun); R tested on 4.6 (release) and 4.5 (oldrel) with a looser declared floor; the tag
run packs and attests the R source tarball like every other package and the owner submits it
to CRAN by hand from the preserved bytes; the first npm publish of each package is by token
because npm Trusted Publishing is configured on an existing package; both bindings get a
benchmark harness and any loss against the platform's own call is printed.

## TypeScript binding (`typescript/`)

Versions checked 2026-10-26: `@napi-rs/cli` 3.10.x, crates `napi` 3.15 / `napi-derive` 3.6 /
`napi-build` 2.6 (Rust ≥ 1.88; the repo's toolchain is 1.99.1). Node-API level 5 is the
lowest that gives `Date` values, and it is satisfied by every Node ≥ 22. Node 22 is Maintenance
LTS until 2027-04-30, 24 until 2028-04-30, 26 is current and becomes LTS 2026-10-27. Bun is 1.4.4.

**Crate placement: a separate crate, not a core feature.** `typescript/Cargo.toml` is a
`publish = false` cdylib depending on `hyperuuid = { path = "../rust" }`, with `build.rs`
calling `napi_build::setup()`. `napi build` drives cargo itself and needs a manifest whose
`[lib]` is a cdylib; the core's `[lib]` is rlib-only on purpose, and a feature there could not
carry napi's build script. `rust/browser-test` is the precedent for this shape (own
`Cargo.lock`, own `target/`). `typescript/.cargo/config.toml` carries only the two musl
`-crt-static` entries; cargo config discovery is cwd-upward so `rust/.cargo/config.toml` is
not read from here. Release profile: lto, codegen-units 1, strip symbols, `panic = "unwind"`
(the default, stated explicitly in the manifest with the reason).

**Panics [review 4].** The core is proven no-panic, but that proof does not cover the binding
code in `src/lib.rs`, and napi-rs does not convert a panic on its own: an uncaught panic in a
synchronous `#[napi]` function can terminate the process. Every `#[napi]` function therefore
carries `#[napi(catch_unwind)]`, which wraps the call in `std::panic::catch_unwind` and
surfaces a panic as a JS `Error` (`GenericFailure`). This is why the profile must stay
`panic = "unwind"`: under `abort` nothing is catchable. Proof at implementation time: a
scratch crate (not shipped) with one `#[napi(catch_unwind)] fn probe() { panic!() }` and one
without the attribute, loaded from Node; the first must throw a catchable `Error`, the second
must take the process down (the negative control). The result is recorded in the PR report.
`check_exports.py`'s napi site regex matches `#[napi(catch_unwind…)]` as well as bare
`#[napi]`, and a test in `.github/scripts/tests` asserts that every napi site line carries
`catch_unwind`, so a new function cannot drop it silently.

**Files.** `package.json` (napi block, `files` allowlist, eight exact-version
`optionalDependencies` injected at pack time, never committed), `package-lock.json`, `Cargo.toml`, `Cargo.lock`, `build.rs`,
`.cargo/config.toml`, `src/lib.rs`, generated and committed
`binding.js` / `binding.d.ts` (`napi build --js binding.js --dts binding.d.ts`), hand-written
`index.js` / `index.d.ts` (re-exports via shorthand keys so cjs-module-lexer exposes ESM named
exports, plus `isAvailable()`, `Namespaces`, the error types), `tests/*.test.ts`,
`bench/bench.ts`, `tsconfig.json`, `biome.json`, `README.md`, `LICENSE`. Not committed:
`npm/` (created by `napi create-npm-dirs` at pack time) and any `*.node`.

**napi config.** `binaryName: hyperuuid`, `packageName: @skunkwerkx/hyperuuid`,
`constEnum: false` (a `const enum` in a `.d.ts` is unusable from type-stripped TS), targets
`x86_64/aarch64-unknown-linux-gnu`, `x86_64/aarch64-unknown-linux-musl`,
`x86_64/aarch64-apple-darwin`, `x86_64/aarch64-pc-windows-msvc`. napi names them
`linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `darwin-x64`,
`darwin-arm64`, `win32-x64-msvc`, `win32-arm64-msvc`: the `.node` file, the `npm/<name>/`
dir and the `@skunkwerkx/hyperuuid-<name>` package each carry that suffix. The generated
loader prefers a local `./hyperuuid.<name>.node`, then the platform package, and detects musl
from `ldd`; `NAPI_RS_NATIVE_LIBRARY_PATH` overrides. `engines.node >= 22`.

**API (`src/lib.rs`, all in Rust).** Rust fn names use the Python spelling so napi's
camelCasing yields `newV4`, `newV5`, `newV7`, `newV7Batch`, `v7UnixMillis(id, layout?)`
(the `_in` pair merged), `version`, `variant`, `isRfc`, `getTimestamp`, `v7ToSqlOrder`,
`v7FromSqlOrder`, `nativeVersion`, plus `newV4Bytes`, `newV7Bytes`, `newV7BatchBytes`,
`fillV7(buf, time?)`, `v7Timestamp`. Strings are formatted into a stack `[u8; 36]` and
returned with `create_string_latin1` (one-byte V8 string, no UTF-8 validation). Ids in are
`Either<String, Uint8Array>`: 32 hex digits or 36 with hyphens, any case, or exactly 16 bytes;
that leniency lives in the binding, since the core's parser is hyphenated-only and part of the
persisted contract. Time in is `Option<Either<f64, Date>>`, `None` → `SystemTime::now()` in
Rust as `python_ext.rs` does. Dates out via `create_date`. Enums `Layout { Rfc9562 = 1,
SqlServer = 2 }` and `Variant`; consts `NIL`, `MAX`, `MAX_V7_BATCH`; `Namespaces` object.
Batch refuses `count > MAX_V7_BATCH` before allocating.

**Errors.** `Result<T, napi::Error<ErrorCode>>` where the code becomes `error.code`:
`ERR_HYPERUUID_RANDOM_SOURCE`, `_TIMESTAMP_RANGE`, `_BATCH_TOO_LARGE`, `_WRONG_VERSION`,
`_INVALID_ID`, `_INVALID_BUFFER`, `_INVALID_COUNT` (counts cross as `f64` and are validated
in the binding, since napi's `u32` is JavaScript's ToUint32 and would turn `1.5` into 1 and
`NaN` into 0), and `_UNAVAILABLE` from `index.js` when the addon did not load. A wrong JS type or an unknown `Layout` member is napi's own `InvalidArg` (`NumberExpected` for numeric and enum parameters), not a
HyperUuid code. Plain `Error` plus a stable `code` is Node's own convention; `index.d.ts` exports the union type, `HyperUuidError` and
`isHyperUuidError()`. Messages copy `python_ext.rs`. A caught panic is not one of these: napi raises it as a
`GenericFailure` error, so it reaches JS as an `Error` with `code === "GenericFailure"`,
outside the `ERR_HYPERUUID_*` union, and `isHyperUuidError()` returns false for it. The
scratch panic probe (not shipped) asserts that exact `code`, and the result is recorded in the PR report.

**Tests.** One tree, `tests/*.test.ts` on `node:test` + `node:assert/strict`, run by
`node --test` (type stripping is default from 22.18) and `bun test` (Bun implements the
in-process `node:test` API). No `enum`, parameter properties or decorators in test code.
`tests/corpus.test.ts` replays the five shared corpus files per `corpus/README.md` and throws
when the corpus directory is missing, as `python/tests/test_corpus.py` does;
`tests/esm.test.mts` proves `import { newV7 } from 'hyperuuid'`; `tsc --noEmit` is the typing
test. Fallback if Bun's `node:test` shim misbehaves: a ten-line harness picking `bun:test` when
`process.versions.bun` is set.

**Bench.** mitata (runs on both runtimes): `newV4` vs `crypto.randomUUID()`, `newV7` vs the
`uuid` package's `v7()`, `newV7Batch(1000)` vs a loop, `newV7BatchBytes`, timestamp
extraction. `crypto.randomUUID()` pools entropy and will likely be close or ahead on v4; the
README prints it either way.

**Lint/format.** Biome 2.x (one config) over `index.js`, tests, bench and JSON; `cargo fmt
--check` and `clippy -D warnings` in `typescript/`.

**check_exports.** Two new `SITES` entries: the `#[napi…]` functions in `typescript/src/lib.rs`
(regex on `#[napi…]` directly above `pub fn`; spelling = `python_spelling` with
`uuid_v7_unix_millis_in` merged away; extras `new_v4_bytes`, `new_v7_bytes`,
`new_v7_batch_bytes`, `fill_v7`, `v7_timestamp`) and the `export declare function` lines in
`binding.d.ts` (camelCase twin, like Python's `.pyi` site).

**Build legs** (from `typescript/` after `npm ci`: `napi build --release --platform --target
<triple> --js binding.js --dts binding.d.ts`):

| RID | Runner | Notes |
| --- | --- | --- |
| linux-x64, linux-arm64 | ubuntu-26.04, -arm | native; the `.node` links std, so re-measure the glibc floor rather than reuse the no_std core's 2.34 claim |
| linux-musl-x64, -arm64 | the existing Alpine leg | build in `node:<v>-alpine` the way `build-musl.sh` builds the core (`-crt-static`, static unwinder, `readelf -d` NEEDED gate); run `node --test` there and `bun test` in `oven/bun:alpine` |
| osx-arm64 | macos-latest | native |
| osx-x64 | macos-latest | `--target x86_64-apple-darwin`, `lipo -archs` check, test with `setup-node architecture: x64` under Rosetta (the napi template does this). This is the first binding suite on the osx-x64 leg |
| win-x64 | windows-latest | native MSVC |
| win-arm64 | windows-11-arm | native; Node only if Bun has no win-arm64 build (verify) |

**Pack and publish.** Tag run: `napi create-npm-dirs`, `napi artifacts` (places each `.node`
into `npm/<name>/`), `npm pack` in each `npm/<name>/` and at the root (`files` keeps `.node`
out of the root tarball), nine tarballs attested and listed in the inventory. Never
`napi prepublish`: it uploads and creates a GitHub release itself. Publish dispatch:
`npm publish <tgz> --access public` for the eight platform packages, then the root. npm
Trusted Publishing needs npm ≥ 11.5.1 (so Node 24 on the publish job), `id-token: write`, a
publisher registered on npmjs.com per package (nine entries, bare workflow filename
`release.yml`), and `repository.url` matching the OIDC repo exactly; provenance is automatic.
The first version of all nine goes up with a granular token because publishers attach to an
existing package. `optionalDependencies` are written by the pack step at the exact release version.

Unverified until first build, each with a fallback noted: `Option<JsDate>` and
`Vec<JsString>` return types; `&str` consts under `#[napi]`; in-place write through an owned
`Uint8Array` (else `Uint8ArraySlice`); `npm publish <tgz>` under OIDC with automatic
provenance (`--dry-run` does not exercise OIDC, so the first real release proves it).

## R binding (`r/`)

Facts checked 2026-10-26: R 4.6.1 is current (2026-06-24), 4.5.3 is oldrel; Rtools 4.5 serves
R ≥ 4.5 and ships no Rust, so CRAN's Windows checkers use rustup's `x86_64-pc-windows-gnu`
target (gifski and heck pass on every CRAN flavour, which is the practical proof). CRAN's
Rust policy: `cargo vendor` into an xz tarball, offline build, `-j 2`, `SystemRequirements:
Cargo (Rust's package manager), rustc`, configure checks `~/.cargo/bin` and prints the rustc
version, every vendored crate's authorship in DESCRIPTION. The staticlib dependency closure
(`cargo tree --no-default-features --features staticlib --target all`) is 14 crates:
getrandom, cfg-if, libc, r-efi, sha1, digest, block-buffer, crypto-common, hybrid-array,
typenum, cpufeatures, chacha20, cipher, inout. The reviewer built the no_std wrapper in a
scratch directory on this Mac and `cargo vendor` produced exactly those 14 crates. R is not
installed on this machine, so nothing R-side was executed; install it (rig or the CRAN pkg)
as the first implementation step.

**Mechanism.** `src/rust/Cargo.toml` is a wrapper crate `rcore` (`crate-type = ["staticlib"]`,
`[workspace]` to stop upward lookup, `panic = "abort"`, lto) depending on
`hyperuuid = { path = "core", default-features = false, features = ["staticlib"] }`. Its
`lib.rs` is `#![no_std]` plus `extern crate hyperuuid;` (no_std is mandatory, or std's panic
handler collides with the core's; the `extern crate` makes rustc keep the dependency's objects,
exactly gifski's shape). `src/rust/core/` is a real copy of `rust/` in both the checkout and
the tarball: gitignored, regenerated by `tools/vendor.sh`, which rsyncs the core (excluding
`target`, `.cargo`, `browser-test`), copies the five binding corpus files by name
(`v5.json`, `sql_order.json`, `sql_order_refused.json`, `timestamp.json`, `inspect.json`,
never a glob, so the Rust-only `v7_layout.json` stays out) to `tests/testthat/corpus/`, runs
`cargo vendor`, tars `vendor/` with reproducible flags into `vendor.tar.xz`, and regenerates
`inst/AUTHORS` from `cargo metadata`. Cargo vendors only the features the wrapper activates,
so the core manifest needs no trimming. One path, no symlinks, same recipe in all three repos
(the per-repo content is `CRATE=` in `vendor.sh`, the shim, the R files and the DESCRIPTION
text).

**Build.** `src/Makevars`: `CARGO_HOME=$(PWD)/.cargo`, `-j 2`, `--target-dir rust/target`,
`rustc --version` in the recipe, extract `vendor.tar.xz` and add `--offline` only when it
exists (an unvendored checkout builds online), `PKG_LIBS = -Lrust/target/release -lrcore`,
clean the intermediates after linking. `Makevars.win` adds `--target $(TARGET)` from
`tools/rustarch.R` (`x86_64-pc-windows-gnu`, `aarch64-pc-windows-gnullvm` on ARM) **and
[review 2]** links from the target-qualified directory, `PKG_LIBS =
-Lrust/target/$(TARGET)/release -lrcore`, because Cargo places an explicitly targeted build
under `<target-dir>/<triple>/<profile>`, not `<target-dir>/<profile>`. The Unix Makevars never
passes `--target`, so its un-qualified path stays correct; a comment in each file names the
other so the pair is not "fixed" back into agreement. No std means no `-lws2_32 -luserenv
-lbcrypt -lntdll`; keep them in a comment. `configure` / `configure.win` print cargo and rustc
versions or fail with the rustup pointer; `cleanup` / `cleanup.win` remove `src/rust/vendor`,
`src/.cargo`, `src/rust/target`. `.Rbuildignore` excludes `src/rust/target`,
`src/rust/vendor`, `.cargo`, `README.md`, `LICENSE.md`, `cran-comments.md` and never
`src/rust/core`, `Cargo.lock`, `vendor.tar.xz` or the corpus snapshot.

**Files.** `DESCRIPTION` (`Depends: R (>= 4.2)`, `License: MIT + file LICENSE` with CRAN's
two-line `LICENSE`, full text in `LICENSE.md`, `Suggests: testthat (>= 3.0.0), jsonlite`,
`Authors@R` including the vendored-crate contributors entry), `NAMESPACE` and `man/` from
roxygen2 (committed), `NEWS.md` (headings `# hyperuuid X.Y.Z`, derived from the CHANGELOG
section), `R/{generate,inspect,sql_order,convert,constants,native,hyperuuid-package}.R`,
`src/{core.h,hyperuuid.c,init.c,Makevars,Makevars.win}`, `src/rust/{Cargo.toml,src/lib.rs,
vendor-config.toml}`, `tests/testthat/`, `tools/{vendor.sh,vendor-authors.R,rustarch.R}`,
`cran-comments.md`, `README.md`.

**C shim.** `src/core.h` hand-declares the thirteen prototypes (a new `check_exports.py`
site, matched by the existing `C_PROTOTYPE` pattern). `hyperuuid.c` owns `parse16` (32 hex
digits or 8-4-4-4-12, any case) and `format36`, both table-driven and allocation-free; `init.c`
registers routines with `R_useDynamicSymbols(FALSE)`. Entries that return a character,
integer, logical or POSIXct vector take a `STRSXP` and map `NA_STRING` and unparsable input to
`NA` in that element. Only SQL-order conversion of a non-v7 (ABI code 5), random-source
failure (1), timestamp range (2) and the raw-conversion cases below raise with `Rf_error`.
No `printf`, `exit` or `abort` in the shim. The timestamp is computed in R
(`floor(as.numeric(as.POSIXct(time)) * 1000)`), which keeps "nothing behind the ABI reads a
clock" true.

**Missing values and the raw door [review 3].** A raw vector has no `NA`: coercing one yields
`00`, and sixteen zero bytes are the valid nil UUID, so a raw column cannot carry "missing"
without lying. The contract is therefore split by return type:

- Character, integer, logical and POSIXct results are NA-propagating, element-wise, as above.
- `uuid_to_raw(x)` is strict: an `NA_character_` or malformed element stops with an error
  naming the first offending index (`uuid_to_raw(): element 3 is NA` / `is not a UUID`).
  There is no validity-mask variant; a caller who needs one filters with `uuid_is_rfc()` or
  `is.na()` first, which the roxygen page says in one sentence. `uuid_from_raw(m)` requires a
  raw matrix with sixteen rows (or a raw vector of length 16k, reshaped) and errors on anything
  else; its output never contains `NA`.
- `uuid_v4_raw()` takes no input, so nothing can be missing. `uuid_v7()` and `uuid_v7_raw()`
  take `time`, and a missing or non-finite element (`NA`, `NaN`, `Inf`, in the scalar or the
  length-n form) is rejected in R with an error naming the index before anything is converted
  to the ABI's integer millisecond, for both the character and the raw variant alike: a bad
  timestamp is a caller error, never a missing id, and a raw column could not carry one
  anyway. `test-generate.R` covers the scalar and the vector case for both variants.
- Round trip: `uuid_from_raw(uuid_to_raw(x))` preserves the UUID bytes for every valid `x`
  and returns the canonical spelling (lowercase, hyphenated); exact string identity holds
  only for input that is already canonical, since `parse16` also accepts uppercase and
  unhyphenated text. `uuid_to_raw(uuid_nil)` is the all-zero column, which `uuid_from_raw`
  turns back into `uuid_nil`. The test round-trips one canonical, one uppercase and one
  unhyphenated spelling of the same id and expects the canonical string for all three. `test-convert.R` has one test with three inputs, `NA_character_`, a malformed
  string and `uuid_nil`, asserting the first two error with the documented message and the
  third round-trips to the nil UUID, so the three cases can never collapse into each other.

**R API.**

| Function | Behaviour |
| --- | --- |
| `uuid_v4(n = 1)`, `uuid_v4_raw(n = 1)` | n random ids; raw form is a 16×n raw matrix |
| `uuid_v7(n = 1, time = Sys.time())`, `uuid_v7_raw(...)` | scalar `time` → one `uuid_new_v7_batch` call (shared timestamp, ordered counter block); length-n `time` → n `uuid_new_v7` calls. `n > 2^26` with scalar time is refused, not chunked, because a second chunk would start a new counter block and break the batch ordering contract |
| `uuid_v5(namespace, name)` | vectorised over `name` (character, or a list of raw vectors for the bytes door); namespace a UUID string or `uuid_namespaces$dns` etc. |
| `uuid_to_raw(x)`, `uuid_from_raw(m)` | character ⇄ 16×n raw matrix; strict, never NA (see above) |
| `uuid_version(x, layout = c("rfc", "sql"))`, `uuid_variant(x)`, `uuid_is_rfc(x, version, layout)` | inspection; variant as `"ncs"`, `"rfc9562"`, `"microsoft"`, `"future"` |
| `uuid_timestamp(x, layout)` | POSIXct (UTC), NA where not v7; `uuid_v7_unix_millis(x, layout)` is the unchecked bit read the corpus replays |
| `uuid_to_sql_order(x)`, `uuid_from_sql_order(x)` | character in, character out; error on wrong version |
| `uuid_nil`, `uuid_max`, `uuid_namespaces`, `uuid_v7_max_batch` | constants |
| `hyperuuid_native_version()` | `package_version` unpacked from `hyperuuid_version()` |

**Tests.** testthat edition 3. `test-corpus.R` reads the staged snapshot in
`tests/testthat/corpus/` (ships in the tarball: 34 KB, and it makes CRAN's own flavours replay
the persisted-bytes contract) and falls back to walking up to the repo's `corpus/` in an
unvendored checkout. **[review 5]** When neither exists it stops with an error naming both
paths, never skips: a skipped corpus is exactly the packaging omission the CRAN check exists
to catch, and `python/tests/test_corpus.py` already raises in the same case. The five binding files
`corpus/README.md` lists as "every binding", `v5.json`, `sql_order.json`,
`sql_order_refused.json`, `timestamp.json` and `inspect.json`, are enumerated in the test
(`v7_layout.json` is Rust-only and is not shipped), and a missing or empty one is an error
too, not a skipped file. The same five names are the list the tarball check and the
inventory validator grep for. `jsonlite::fromJSON(simplifyVector = FALSE)`
so nulls and optional fields survive. Mirrors `python/tests/test_corpus.py` file by file.
`test-generate.R` (format, uniqueness, batch ordering, `n = 0`, the cap, raw dims, vector
`time`), `test-convert.R` (the three-way NA / malformed / nil test above, round trip),
`test-inspect.R`, `test-version.R` (native version equals `packageVersion()`, which catches a
bump that missed DESCRIPTION).

**`R CMD check --as-cran` watch list.** No hidden `.cargo`/`vendor` at build time; the
"compiled code should not call abort" NOTE if the panic handler's `abort` survives LTO
(check with `nm -g src/hyperuuid.so | grep -w abort`; if it survives, explain in
`cran-comments.md`); only API-sanctioned accessors in C; no `$(shell)` in the Unix Makevars;
`NEWS.md` must parse; `inst/AUTHORS` present.

**Bench.** `r/bench/bench.R` with `bench::mark` against `uuid::UUIDgenerate(n = ...)`, for
single ids and n = 1000; dated, machine-labelled, losses printed.

**Release.** The tag run runs `vendor.sh` then `R CMD build r`, **[review 5]** lists the
built tarball and fails unless all five corpus files are present under
`hyperuuid/tests/testthat/corpus/` (a `tar tzf` grep in the `pack-r` action, and the same
check in `vendor.sh --verify` for the local loop), then attests `hyperuuid_X.Y.Z.tar.gz`
into a `cran/` inventory group and uploads nothing. The owner submits by hand at the CRAN
form from the preserved tarball, with `cran-comments.md`, and confirms the email. Pre-flight
before the first submission: `devtools::check_win_devel()` (whether win-builder has cargo
today is unverified).

## Forge changes (`/Users/buvy/Developer/SkunkWerkx/HyperForge`)

Generic, input-driven, nothing HyperUuid-shaped. All of these have Forge unit tests
(`test_hyper_config.py`, `test_forge_actions.py`, `test_forge_producers.py`,
`test_package_inventory_metadata.py`, `test_release_inventory.py`) that enumerate defaults and
must move with them.

- `hyper-build-native.yml`: boolean inputs `node` and `r` beside the eight existing ones
  (default false, flipped to true in the cascade once every consumer ships the binding, so an unpinned Forge merge changes nothing for a sibling; HyperUuid passes true); `node_version` /
  `node_floor_versions` and `r_version` / `r_floor_version` inputs; a napi leg on the `ffi`
  shard after PyO3 (setup-node, `npm ci`, `napi build`, `node --test`, `bun test` via
  `oven-sh/setup-bun`, floor reruns, osx-x64 cross-build under Rosetta in
  `build-native-osx-x64`); an R leg on the `ffi` shard (dtolnay/rust-toolchain with the gnu
  target on Windows, `r-lib/actions/setup-r@v2` + `setup-r-dependencies@v2` +
  `check-r-package@v2`, `--as-cran`, `error-on: note`, release and oldrel); a `node)` case in
  `.github/actions/musl-suite/suite-musl.sh` building the musl `.node` in `node:<v>-alpine` with the
  `build-musl.sh` flags. The `mode` description's "all eight languages" becomes ten.
- `hyper_config.py` **[review 1]**: `default_version_files()` keeps its five unconditional
  entries and adds the new manifests only for bindings the consumer has:
  `r/DESCRIPTION` (`^Version: (.+)$`) when the `r/` directory exists at the root, and
  `typescript/package.json` when `typescript/` exists (the lockfile is regenerated, not
  bumped; see below). The
  key is the binding *directory*, not the manifest file, so a repo that has the binding but
  lost its manifest still fails loudly in `bump` (line 209's unconditional read is kept as
  is), while HyperCast and HyperTabular, which have neither directory today and use the
  defaults, keep bumping exactly as before until their cascade lands. The existence check
  needs the root, so `default_version_files()` takes it from `self.root` (already set by
  `HyperConfig.load`). A `version_files` key in `hyper.json` still overrides the whole map,
  unchanged. Tests: defaults with and without the directories, in a `tmp_path` root; the
  `bump` test asserts no `FileNotFoundError` for a root that has only the five classic files.
  The committed `package.json` carries one `version` line and no `optionalDependencies`
  (the pack step injects them at the exact version), so `bump` keeps its one-match rule;
  `hyper-prepare-release.yml` regenerates `typescript/package-lock.json` from it
  (`npm install --package-lock-only --ignore-scripts`, guarded on the file's existence) the
  way it refreshes the companion `Cargo.lock`s, and the companion loop gains
  `typescript/Cargo.lock` and `r/src/rust/Cargo.lock` (both wrapper crates path-depend on
  the core; `r/tools/vendor.sh --stage` runs first so the R copy exists). `NODE_PLATFORMS`
  (RID → napi suffix) in `shell()` and `json`.
- Release scripts: `release_inventory.py` gains `npm` and `cran` groups in `payload_files`,
  `validate_layout` (9 npm tarballs; 1 R tarball) and the signer rule (consumer-signed like
  `ruby/`); `REGISTRIES` splits into inventory groups and publishable registries so `cran` is
  attested but never planned for upload; `registry_plan` gets an `npm` branch comparing
  against `https://registry.npmjs.org/<name>/-/<file>.tgz`; `release_package_metadata.py`
  gains `npm_tarball` and `r_source_tarball` validators (the latter checks the five corpus
  members, **[review 5]**); `release_notes.py` and `finalize-release` name npm (and note the
  CRAN tarball). Fix the three places that ignore `hyper.json` while there (the 25-wheel
  literal, the duplicated `PLATFORMS`, Maven's `LIB_NAMES`).
- Composite actions: `pack-npm` (download the eight `.node` artifacts, verify their
  provenance, `create-npm-dirs` + `artifacts` + `npm pack` yielding nine tarballs, the
  platform packages plus the root, attest, upload `npm-packages`),
  `pack-r` (`vendor.sh`, `R CMD build`, tarball corpus check, attest, upload
  `cran-packages`), `publish-npm` with the publish-rubygems step shape (identity, download,
  inventory verify, registry-plan, OIDC exchange gated on `pending`, `npm publish <tgz>`
  gated on `pending && publish`, registry-confirm; Node 24 for npm ≥ 11.5.1). `release-gate`
  downloads both new artifacts.
- Docs: `docs/release-runbook.md` (first-publish rule for npm, OIDC list, "five registries"
  → six plus CRAN by hand, recovery per registry), `README.md`, `CLAUDE.md`,
  `docs/predicates/release-v1.md`. `hyper-check-core.yml` gains `lint_node` and `lint_r`
  inputs (Biome + cargo fmt/clippy in `typescript/`; `lintr` + `cargo fmt` in `r/`).

## This repo's wiring

- `ci.yml`: pass `node: true`, `r: true` and the version inputs to `build-native`; nothing
  else, since the legs live in the Forge.
- `release.yml`: `npm-pack` and `r-pack` jobs (composite actions, consumer identity, signed
  like `ruby-pack`), `npm-publish` job with the publish gate and `id-token: write`;
  `publication-gate` and `finalize-release` `needs` extended. `.github/scripts/tests/
  test_workflow_contracts.py` `PUBLISHERS`, `PACKERS`, `ACTION_JOBS` updated (HyperCast and
  HyperTabular carry the same file; they move with the cascade).
- `.github/hyper.json`: nothing new unless the Forge chooses an override key for the npm
  platform list.
- `check_exports.py`: three new `SITES` (R header, napi functions, `binding.d.ts`), plus the
  `catch_unwind` assertion on the napi site.
- `cdylib_smoke.py`: unchanged (no new exports). AOT and wasm smoke tests: unchanged.
- `dependabot.yml`: `npm` at `/typescript`, `cargo` at `/typescript` and `/r/src/rust`
  (the wrapper crate; its vendored copy is regenerated, not updated).
- `.gitignore`: `r/src/rust/core/`, `r/src/rust/vendor*`, `r/tests/testthat/corpus/`,
  `typescript/npm/`, `typescript/*.node`.
- `CLAUDE.md`: "seven other languages" → nine; the "adding a C export" list gains
  `r/src/core.h`, `typescript/src/lib.rs` and `binding.d.ts`; "Local loop" gains
  `r/tools/vendor.sh` and `npm run build`; the Lists-that-move-together gains npm's
  `optionalDependencies` and the first-publish-by-token rule; "Bytes that are persisted"
  gains one line on the R raw door being strict (no NA in raw).
- `README.md`: intro and "seven codebases" counts, the mechanism list (napi-rs, `.Call`),
  badges (npm, CRAN), platform table rows (TypeScript on all eight; R "source, CRAN builds"
  with win-arm64 experimental), WebAssembly table rows (TypeScript: not this release, napi's
  WASI fallback is the route; R: webR is Emscripten and could link the staticlib, not
  attempted), Layout block, Key features count, benchmark tables for both (measured on the
  current dev machine, labelled as such and kept apart from the WSL2 tables per the open
  follow-up). `CHANGELOG.md` header "All eight packages" → ten; `### Added` bullets under
  Unreleased tagged `*(R)*` and `*(TypeScript)*`. `rust/README.md` export-count prose and
  `corpus/README.md` "Replayed by" table gain both.

## Sequencing

1. Toolchains locally: R 4.6.1 + testthat/jsonlite/roxygen2/devtools; Node 26 + 24 + 22 via
   a version manager, Bun 1.4, `@napi-rs/cli`. Rust target `x86_64-apple-darwin` for the
   osx-x64 cross-build check.
2. TypeScript binding end to end locally: crate, API, tests on Node and Bun, corpus, bench,
   Biome, `check_exports.py`; resolve the five unverified napi points; the panic probe pair.
3. R package end to end locally: wrapper crate, shim, R API, testthat, `R CMD build` +
   `R CMD check --as-cran` clean on macOS; resolve the `abort` NOTE question; confirm the
   tarball carries the corpus.
4. Forge: inputs, legs, musl case, config (directory-conditional version files first, with
   its tests, so the siblings are safe before anything else merges), inventory, pack/publish
   actions, tests, docs.
5. This repo's workflows, scripts, tests, READMEs, CHANGELOG, CLAUDE.md.
6. Owner actions, by name, when asked: create the `@skunkwerkx` npm org; bootstrap-publish
   the nine npm packages by token and register `release.yml` as trusted publisher on each;
   CRAN maintainer email; then PR, CI, release 1.1.0, CRAN submission from the preserved
   tarball. Cascade to HyperCast and HyperTabular after 1.1.0 proves the recipes.

## Verification

- `typescript/`: `npm run build` then `npm test` on Node 22, 24 and 26 and `bun test`;
  `npm run typecheck`; `npm run lint`; `npm run bench` and `bench:bun` with the numbers
  pasted into the README table; negative controls: delete the `.node` and confirm
  `isAvailable()` is false and every call rethrows the load error with its `cause` chain;
  the scratch panic probe without `catch_unwind` takes the process down while the one with it
  throws.
- `r/`: `r/tools/vendor.sh`, `R CMD build r`, `tar tzf` shows the five corpus files,
  `R CMD check --as-cran` with zero NOTEs (or the one documented `abort` NOTE),
  `devtools::test("r")`; negative controls: temporarily flip a corpus byte and watch
  `test-corpus.R` fail; move the staged corpus aside in a tarball-built package and watch it
  error rather than skip; `nm` on the `.so` for `abort`; `uuid_to_raw(NA_character_)` errors.
- Parity: `python3 .github/scripts/check_exports.py` with and without a deliberately removed
  prototype in `r/src/core.h` (must fail without it), and with `catch_unwind` removed from
  one napi function (must fail).
- Forge: `unittest discover` there with the `bump` test over a root holding only the five
  classic version files (must pass) and over one with `r/` but no `r/DESCRIPTION` (must
  raise). Run `hyper_config.py version-files` against HyperCast and HyperTabular checkouts
  and confirm the list is unchanged from today's five.
- Workflow contracts: `python3 -m unittest discover .github/scripts/tests` here and the
  Forge's `unittest discover` there, `actionlint` over both.
- What only CI can prove, to be named in the PR report: the Windows gnu-target R build (and
  so the `$(TARGET)`-qualified link path), the musl `.node` NEEDED gate, the osx-x64 Rosetta
  suite, the win-arm64 Bun question.
- What only the 1.1.0 release can prove: `npm publish <tgz>` under OIDC with provenance, the
  CRAN submission itself.

## Review changes applied (2026-10-26)

All five findings were checked before being folded in.

1. **Forge version files** (verified in `hyper_config.py:209`, and neither sibling has `r/` or
   `typescript/`, nor a `version_files` override): the new entries are conditional on the
   binding directory existing at the consumer's root.
2. **Windows link dir** (Cargo's output rule): `Makevars.win` links from
   `rust/target/$(TARGET)/release`.
3. **Raw NA** (R raw vectors have no NA): `uuid_to_raw()` and `uuid_from_raw()` are strict and
   error; a three-way test separates `NA_character_`, malformed text and `uuid_nil`.
4. **Panics** (napi-rs docs: an uncaught panic in a sync function can terminate the process;
   `catch_unwind` is per-function and needs `panic = "unwind"`): every `#[napi]` function
   carries `catch_unwind`, enforced by the export-check test, proven by a scratch probe pair.
5. **Corpus absence** (`python/tests/test_corpus.py:21` raises): `test-corpus.R` errors
   instead of skipping, and the built tarball is checked for all five corpus files in `pack-r`,
   `vendor.sh --verify` and the inventory validator.

Second round, same day, four wording corrections, each checked:

6. **Corpus filenames**: `corpus/` holds no `v4.json` or `v7.json`; the five binding files
   are the ones `corpus/README.md` marks "every binding" and the plan now names them.
7. **Panic error code**: napi maps the status to `error.code`, so a caught panic reads
   `"GenericFailure"`, not "no code".
8. **Missing time in generation**: `uuid_v7_raw()` takes `time`; missing or non-finite
   elements are rejected before ABI conversion in both v7 variants, scalar and vector.
9. **Raw round trip**: bytes are preserved and the canonical spelling is returned; string
   identity only for canonical input.
