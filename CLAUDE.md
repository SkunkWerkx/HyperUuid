# HyperUuid — notes for agents

One RFC 9562 UUID engine in Rust (`rust/`), reached over a C ABI, as a native extension, or,
for Go, as Go source translated from its WebAssembly build, from nine other languages, and
from C and C++ through the release's C archives. `README.md`
has the why, each binding's README has its own mechanics, and all CI and release logic lives
in the Forge (`SkunkWerkx/.github`). This file holds what the code alone won't tell you.

## Bytes that are persisted

- The SQL Server layout (the v8 form of a v7) and v5 output sit in other people's clustered
  indexes and keys. Moving one bit silently reorders or re-derives every stored row, and
  every property test would still pass. `corpus/*.json` pins the exact bytes.
- Corpus vectors are add-only. Regenerate them only with `python3 corpus/oracle.py
  --write`, never by hand. The oracle deliberately shares no code with the core (it works
  from RFC field tables in bit strings); keep it that way. Rationale: `corpus/README.md`
  and README "The layout is persisted data".
- v7's counter semantics (width, wrap, batch roll-forward) are a documented contract:
  README "v7 ordering, precisely".
- R's raw doors are strict: a raw vector has no `NA`, so `uuid_to_raw` errors on an `NA` or
  malformed element instead of writing bytes that lie. Every data input to the other R doors
  propagates `NA`; arguments (time, namespace, count) error.

## Claims are measured

Every performance claim is a dated, machine-specific receipt, and losses are printed next
to wins: Go's batch, and its Linux single-call and contention numbers, are losses against
the cgo binding it replaced, and its browser batch against the TinyGo archive path it
replaced; all stay printed beside its wins. Don't adjust numbers without
re-running the harness, and don't drop a loss.

## Lists that move together

- Ruby ABIs: `ABIS` in `ruby/Rakefile`, `ruby_compat_version` in `ci.yml`, and in the Forge
  the `ruby.abis` default in `hyper_config.py` (an `.github/hyper.json` override if this repo
  ever differs) and the `ruby_wasm_abis` default. Magnus
  extensions are tied to one Ruby minor, so gems are fat across these ABIs and Fiddle
  covers everything outside them, including other platforms. Fiddle must never be
  restricted to old Rubies. Ruby 3.4 reaches EOL on 2028-03-31: then drop it from those
  lists and clear `ruby_compat_version`.
- Adding or removing a C export: every binding, C#'s three `LibraryImport` declarations
  (`"hyperuuid"`, `"*"`, `"__Internal"`), the sites in `.github/scripts/check_exports.py`,
  among them Go's two (`go/internal/core/core.go`, every export translated, which `go
  generate` rewrites, and `go/core.go`, every export called), R's `r/src/core.h` (one prototype per line; `test_check_exports.py` asserts it
  equals the core's export set) and TypeScript's `typescript/src/lib.rs` and
  `typescript/binding.d.ts` (every `#[napi]` function carries `catch_unwind`, which
  `check_exports.py` enforces and `test_check_exports.py` pins), Java's two `NativeCore`
  trees, `java/jni/src/lib.rs` (its extern block and its `RegisterNatives` table) and its
  `examples/no_panic.rs` (every JNI entry point in `java/jni/src/lib.rs` carries `no_panic`,
  which `check_exports.py`'s `jni_no_panic` enforces and `test_check_exports.py` pins), PHP's
  `rust/src/php_ext/` and its committed stub (not a C ABI site: every PHP-callable body starts
  with `guard(`, which `check_exports.py`'s `php_guard` enforces and `test_check_exports.py`
  pins), `cdylib_smoke.py`, the C++ header `include/hyperuuid.hpp` (hand-written; its
  `::`-qualified calls are a `check_exports.py` site) and the C smoke consumers in `c/smoke/`,
  the prefixes in `.github/hyper.json` (the Forge's symbol check reads them), the AOT and wasm
  smoke tests, and the export count in the READMEs. The Blazor archive needs no export list:
  the SDK's pinvoke table links every P/Invoke by symbol.
- npm's ten packages: the root `hyperuuid`, eight `@skunkwerkx/hyperuuid-<suffix>`, one per
  RID, mapped by `NODE_PLATFORMS` in the Forge's `hyper_config.py` (a `node.platforms`
  override in `.github/hyper.json` if this repo ever differs) and listed as `napi.targets` in
  `typescript/package.json`, and the WebAssembly fallback `@skunkwerkx/hyperuuid-wasm32-wasip1`
  (`NODE_WASI_PLATFORM` there, the last `napi.targets` entry, `node.wasi` in `hyper.json` to
  override, `""` for none). The root's `optionalDependencies` are injected at pack time, never
  committed, and nothing runs `napi prepublish`; the WASI package is the root's plain
  dependency, with no `cpu`/`os`. The first publish of every package, a new platform's
  included, is the owner's, by hand with a granular token: npm Trusted Publishing attaches
  only to a package that exists, and `release.yml` is registered as its trusted publisher
  after.
- The WASI package's runtime: `@napi-rs/wasm-runtime`, `@emnapi/core` and `@emnapi/runtime`
  (`WASI_RUNTIME` in the Forge's `release_package_metadata.py`), with `emnapi` beside them, all
  exact devDependencies in `typescript/package.json` and pinned by its `package-lock.json`,
  from which the Forge's `npm_package.py` writes the WASI package's `dependencies` at pack time
  (napi's own manifest carries a range). Dependabot moves them with `@napi-rs/cli`; the emnapi
  trio must stay at the one version the CLI accepts.
- The Node floor: `engines.node` (`^22.13.0 || >=23.5.0`, the WASI runtime's floor) in
  `typescript/package.json`, the same string in `WASI_NODE_ENGINES` in the Forge's
  `npm_package.py` (the packer refuses a WASI manifest that differs), the Forge's
  `node_floor_versions` default (`"22 24"`, an input override in `ci.yml` if this repo ever
  differs), `FLOOR_IMAGE`'s default `node:22.13.0-alpine` and `LOCKFILE_NPM` (`10.9.2`, the npm that Node
  bundles, the one npm bug 4828 still reproduces with) in `typescript/consumer-tests/run.sh`,
  and the floor sentences in `typescript/README.md`,
  `README.md` and the CHANGELOG. Node 22 reaches EOL on 2027-04-30: then drop it from all of
  them.
- The JDK floor: `jdkFloor` (`21`) in `java/build.gradle.kts`, `jdk_floor_version` in
  `ci.yml`, and the floor sentences in `java/README.md` and README 'Platform support'. JDK 21
  reaches EOL at Temurin on 2029-12-31. Then the floor is 25: the JNI base, `java/jni/`,
  `jni-{rid}`, the input and the JNI sites in `check_exports.py` go, and the FFM `NativeCore`
  moves back into the base.
- The .NET floor: `TargetFrameworks` in `HyperUuid.csproj`, `HyperUuid.Tests.csproj`,
  `HyperUuid.AotSmokeTest.csproj` and `HyperUuid.WasmSmokeTest.csproj` (with its
  per-framework references and `check.sh`'s net10 legs and header), ci.yml's
  `csharp_aot_frameworks` and `csharp_extra_runtime`, the `HYPERUUID003` gate and the
  `build/net10.0/` folder name (which `csharp/HyperUuid.PlatformGateTest` imports by path),
  `test-nuget-package`'s layout step (its `lib/` and `build/` paths and the
  no-`build/net11.0` check), its AOT loop (`net10.0 net11.0`) and control (`-f net10.0`), the
  needles `test_workflow_contracts.py` pins for that job, its floor step (a net9.0 restore
  must fail with NU1202), the probe steps that hard-code `net10.0-*` (the Apple and Android
  probes, and `test-nuget-package`'s gate step, which admits Blazor WebAssembly on both
  frameworks and reads `net10.0-android` as its control), `wasm-tools-net10` in the
  `HYPERUUID002` text, `csharp/README.md` and `README.md`, the Forge `test-blazor` job's .NET
  10 SDK and `wasm-tools-net10` steps (unconditional, so nothing else prompts them), and the
  floor sentences in `csharp/README.md` and `README.md`. .NET 10
  reaches EOL on 2028-11-14: then drop it from all of them, and the targets folder moves to
  the new lowest TFM.
- R's vendored-crate credits: `r/inst/AUTHORS` is regenerated by `r/tools/vendor.sh` from the
  vendored crates' manifests, and a crate with no `authors` field stops it until it has an
  entry in the `undeclared` map in `r/tools/vendor-authors.R`. Never edit `AUTHORS` by hand.
- The R wrapper's core pin: `hyperuuid = { version = "=X.Y.Z"` in `r/src/rust/Cargo.toml` is a
  version file in the Forge's `default_version_files` (when `r/` exists), bumped with
  `r/DESCRIPTION` by `prepare-release.yml`; `r/tools/vendor.sh` refuses a pin that differs
  from `rust/Cargo.toml`, since cargo would otherwise vendor the published core in silence.
- The PHP floor: `php` in `composer.json` and `php/composer.json`, `php_floor_version` in
  `ci.yml` (an explicit input, since the Forge's default serves the siblings), the lowest of
  the PHP minors below, and the floor sentences in `php/README.md` and the root README. PHP
  8.3 reaches EOL on 2027-12-31: then drop it from all of them.
- The PHP minors the extension is built for (8.3, 8.4, 8.5; an extension is tied to one
  minor): the Forge's `php_versions` default in `hyper-build-native.yml` and
  `hyper-build-php-extension.yml`, `MINORS` in its `php_ext_package.py` (with
  `WINDOWS_COMPILERS`, the `vsNN` each minor's Windows build uses, and `WINDOWS_TOOLSETS`, the
  MSVC toolset each `vsNN` links with: PHP refuses a DLL linked newer than its core), the
  `minors` default in `hyper-pie-acceptance.yml` (`test_forge_producers.py` holds those
  equal), and the platform and minor table in `php/README.md`. A new minor is 6 more zips on the
  release; the count (18 today) is in `php/README.md`, `.github/RELEASING.md` and the CHANGELOG.
  No Intel macOS zip and no leg that compiles a PHP: a leg takes the PHP its platform packages,
  or there is no leg (the Forge's `CLAUDE.md` has the rule; never build a language runtime).
- The Python sdist: built in the Forge's `hyper-build-wheels.yml` when `.github/hyper.json`
  says `sdist: true`; one more pypi inventory file, `hyperuuid-<version>.tar.gz`, beside the 17
  wheels. Its Rust floor is `rust-version`, proven at exactly that toolchain by the build job.
- The Go core's regeneration pins: the rustc in `go/internal/wasm2go/rust-toolchain.toml` (a
  reproducibility constraint, not a floor: the committed core follows its codegen) and
  wasm2go's version in `go/internal/wasm2go/go.mod`, each moved by regenerating and
  committing the core. The AES stream's per-architecture feature rule in `go/random_aes.go`
  copies Go's own (`crypto/internal/fips140/aes/aes_asm.go` line 24 and `aes_s390x.go` line
  38 at Go 1.27) and is re-read at every Go floor bump; the floor is `go` in `go/go.mod`
  (`1.26.0`) and the floor sentence in `go/README.md`.
- The C archives: `C_RIDS` in the Forge's `c_package.py`, the platform list in `c/README.md`
  and the C row of README 'Platform support'. The shared library's own name is set in three
  places that must agree: `rust/build.rs` (install name and soname), `IMPORTED_SONAME` in
  `c/cmake/hyperuuidConfig.cmake`, and the `needed` check in `c/smoke/run.sh`.

## Never hand-edit

- `go/internal/core/core.go` is wasm2go's translation of the core's `wasm32-wasip1` build:
  `go generate` in `go/` rewrites it with the rustc pinned in
  `go/internal/wasm2go/rust-toolchain.toml` (`rustup toolchain install <pin> --target
  wasm32-wasip1`), and `HYPERUUID_CHECK_CORE=1 go test -run TestCoreIsCurrent .` diffs it. A
  wasm2go or rustc bump that changes the output turns that check red; regenerate and commit
  the new core with the bump. `prepare-release.yml` regenerates it at the version, since the
  core's version is a literal in the file.
- The root `Package.swift`'s release version and the two Swift zips' checksums are written by
  `stage-native-binaries.yml`; its commit trailers are what `release.yml` uses to find the CI
  run. No binary is committed anywhere.
- `include/hyperuuid.h` and the Swift bundle's copy are generated by cbindgen from
  `rust/src/ffi.rs`: `HYPERUUID_BLESS=1 cargo test --test header` in `rust/` rewrites both.
- `php/stubs/hyperuuid.stub.php` is generated from the extension by `php
  tools/generate-stubs.php` in `php/` (Linux or macOS), with cargo-php pinned to the
  `CARGO_PHP_VERSION` that script names (0.2.1; the Forge's stub leg installs the same one).
  `StubTest` regenerates it and compares when `HYPERUUID_CHECK_STUBS=1`.
- Versions: bump through `prepare-release.yml`. The libraries embed their version, so a
  hand bump ships binaries reporting the old one.

## Toolchains and floors

Each language's primary toolchain is its current release; check endoflife.date rather than
assuming. Exceptions must be commented where pinned: a real platform constraint, or an
older floor shipped beside the primary and dropped at its EOL. Floors match HyperCast and
HyperTabular and move together. PHP is the exception for now: 8.3 here, ahead of the siblings,
which stay on 8.2 through the Forge default until they thaw; don't cascade it.

Rust has two exceptions, both on stable with `RUSTC_BOOTSTRAP=1`, which the Forge sets on
exactly two build steps and nowhere else; never set it more widely. Neither has a stability
promise, so every stable bump is re-proven by the legs that use it.

- The Blazor archive (`hyper-build-wasm.yml`) is built with `-Zbuild-std`, because only that
  leaves out the prebuilt sysroot's legacy exception handling and the features .NET 10's
  wasm-opt rejects (`rust/.cargo/config.toml` says why). Stable cargo takes the nightly flag
  only with the variable; CI's guard and Blazor legs re-prove it. When build-std stabilizes,
  the variable goes from that step.
- The Windows PHP extension (`hyper-build-php-extension.yml`'s Windows build step): ext-php-rs
  needs the unstable `abi_vectorcall` there (rust-lang/rust#124485), and PIE takes only
  prebuilt DLLs on Windows. The win-x64 legs re-prove it. When the ABI stabilizes, the
  variable goes from that step.

## Forge coupling and releases

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. `release.yml` resolves master to one
  commit per run (`find-ci-run`'s `forge_sha`), checks the Forge out at it as `.hyperforge`
  and runs its composite actions from there (`release-guard`, `pack-gems`, `pack-npm`,
  `pack-r`, `verify-wheels`, `release-gate`, `publish-nuget`, `publish-rubygems`,
  `publish-npm`, `publish-pypi`, `finalize-release`);
  the six reusable workflows (NuGet, crate and Maven packing; crate and Maven publishing;
  the PIE acceptance, `release.yml`'s last job) are called at master and resolved when the run
  is queued. The inventory records the
  commit and recovery requires it unchanged. Don't merge Forge changes while a release
  is in flight.
- The board is one Forge workflow per concern: `core` (`hyper-core.yml`, the seven core legs,
  each with its cdylib export check and C smoke), `core-checks` (`hyper-core-checks.yml`,
  the static checks on three hosts), `lint-<binding>` (one job on `ubuntu-26.04-arm`) and
  `test-<binding>` (the binding's suite on its hosts), gated on what the pull request touched
  (`changes`, from `.github/scripts/pr_changes.py`; `pr-board` is the one status to read, red
  when a job failed or was skipped while its gate said run). The Forge's fail-fast ordering
  rule (a new binding's steps first in `hyper-build-native.yml`) is moot here: every binding
  is its own job, and a red lint starts no suite. Off a pull request nothing is gated and the
  whole release-mode board runs. A new binding is one `lint-` and one `test-` caller, one
  class in `pr_changes.py` and one gate in `pr_board.py` (its tests hold the three together).
- `prepare-release.yml`, `stage-native-binaries.yml` and ci.yml's `core-checks` are one
  `uses:` each of a Forge reusable workflow; this repo keeps triggers, permissions and gates.
  `.github/hyper.json` is what the Forge knows about this project (crate and project names,
  the export rule, the cdylib smoke script; the ABI probe is `.github/scripts/abi_probe.c`,
  the Forge's default path, and every other name is derived in the Forge's `hyper_config.py`). `.github/scripts/tests` holds the workflow shape contracts
  and the probe tests; the generic release-guard tests live in the Forge.
- The release is two acts at one tag: the tag push packs, attests (build provenance and the
  release attestation over every shipped file) and preserves without uploading; `gh workflow
  run release.yml --ref vX.Y.Z -f prepared_run_id=<tag run>` publishes from the preserved
  bytes. The finalizer writes the GitHub Release from the CHANGELOG's `## [X.Y.Z]` section,
  and the tag run refuses a CHANGELOG without it.
- Third-party actions are pinned to a commit with the version in a trailing comment, and
  `.github/dependabot.yml` moves them with every other ecosystem in one monthly pull request.
  It ignores the Forge and `dtolnay/rust-toolchain` (branch heads, refreshed by hand).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`. Recover by re-dispatching from the preserved
  packages, never by retagging or rebuilding.

## Local loop

The PR board runs a binding's lint and suite only when its tree, the core (`rust/`,
`include/`, `c/`, `corpus/`), or `.github/` changed; `python3 .github/scripts/pr_changes.py`
classifies a `filename<TAB>previous_filename` list from stdin the way the `changes` job does
(`--all` is the release board), and `python3 .github/scripts/pr_board.py` with `NEEDS`,
`CHANGES` and `GITHUB_EVENT_NAME` set gives the verdict.

C builds an archive from local libraries with the Forge's packer, then the smoke consumer
against it (CMake and pkg-config on PATH), after `cargo cdylib` and `local-core.sh`:

```sh
python3 ../HyperForge/.github/scripts/c_package.py assemble --rid osx-arm64 --out <dir> \
  --static swift/HyperUuidCore.artifactbundle/arm64-apple-macosx/libhyperuuid.a \
  --shared rust/target/release/libhyperuuid.dylib
c/smoke/run.sh <dir>/hyperuuid-<version>-osx-arm64 <version>
```

`SMOKE_ARCH=x86_64` runs an osx-x64 archive under Rosetta, and `c/smoke/apple-mobile.sh <dir>
<version>` takes the four Apple mobile archives (a booted iOS simulator, Rosetta for
maccatalyst-x64). Windows' import library needs LLVM's `llvm-readobj` and `llvm-dlltool`.

Build `cargo cdylib` in `rust/` before running a binding's tests. A raw `cargo rustc
--release --crate-type cdylib --features python` (or `ruby`, or `php`) in `rust/` overwrites
the same `target/release` library, and the other bindings then fail on missing symbols until
`cargo cdylib` runs again; maturin in `python/` and the `ruby-ext`/`php-ext` aliases build
elsewhere and don't (`rust/README.md`, "Local dev trap").

Go needs nothing built: `go test ./...` in `go/` runs on the committed translated core
(`CGO_ENABLED=0`, as CI runs it). After a change in `rust/`, `go generate` in `go/`
regenerates `internal/core/core.go` with the pinned rustc, and `HYPERUUID_CHECK_CORE=1 go test
-count=1 -run 'TestCoreIsCurrent|TestRegenerationCarriesTheCrateVersion' .` is CI's check (it
skips without the toolchain unless the variable is set; `-count=1`, since `go test` replays a
cached pass). The wasm legs: `tinygo test -target=wasip1 -c -o suite.wasm .`, then `wasmtime
run --dir "$PWD/..::/repo" --env HYPERUUID_CORPUS=/repo/corpus --env
HYPERUUID_CRATE_VERSION=<version> suite.wasm -test.v`; and `GOOS=js GOARCH=wasm` or
`GOOS=wasip1 GOARCH=wasm` `go test ./...` with `$(go env GOROOT)/lib/wasm` on PATH. The
memory receipts: `HYPERUUID_RECEIPTS=1 go test -run TestReceipts -v .`. Swift has no core
until one is built: `.github/scripts/local-core.sh` builds the checkout's own into the
gitignored `swift/HyperUuidCore.artifactbundle/` (it finds `../HyperForge` on its own) and
stages the same archive in `csharp/HyperUuid/staticlibs/<rid>/` for a local Native AOT publish,
then `swift test`. It starts with `.github/scripts/local-clean.sh`, which removes every
binding's staged build artifact (C#'s `runtimes/` and `staticlibs/`, the Swift bundle's
variants, Java's `build/`, Python's in-tree extension, Ruby's staged bundles, R's vendor
tarball, lockfile, corpus and objects, TypeScript's addon, WASI module and bundles) and names
what rebuilds each, so nothing a suite loads predates the core being built; run it alone before
any local build after a `rust/` change. Only this checkout has that problem: CI builds each
leg's core into an empty tree before its suites.

PHP is the extension built from `rust/`: `cargo php-ext` there, then `composer test` in `php/`
(`composer bench` for the benchmarks), both loading `rust/target/php/release/` with `-d
extension=`, or the build `HYPERUUID_PHP_EXTENSION` names. The Forge derives that variable's
name and `HYPERUUID_CHECK_STUBS`'s from the crate (`<CRATE>_PHP_EXTENSION`,
`<CRATE>_CHECK_STUBS`), as it derives `<CRATE>_VERSION`; the stub check needs cargo-php and a C
compiler, so locally run it with `HYPERUUID_CHECK_STUBS=1 composer test`. The Forge's
`php_ext_package.py` packs and checks a zip against PIE's own naming code (`pie-pin` names the
PIE release it pins).

The Blazor archive needs stable's sources: `rustup component add rust-src` and `rustup target
add wasm32-unknown-emscripten`, emsdk on PATH, then `RUSTC_BOOTSTRAP=1 cargo wasm-staticlib`
in `rust/`, after deleting `rust/target/.rustc_info.json` and
`rust/target/wasm32-unknown-emscripten/` (cargo caches rustc's cfg without regard to the
variable, and a cache from a plain cargo run for that target builds a different archive); the
Forge's `wasm_archive_guard.py` checks the result.

TypeScript builds its own addon, a napi-rs crate path-depending on `rust/`: from
`typescript/`, `npm ci && npm run build && npm test`, then `npm run test:bun` and `npm run
lint`. Both build scripts pass `--platform --js binding.js --dts binding.d.ts`; a `napi build`
without the last two (a hand-run `--target` cross build, say) writes napi's own loader over the
hand-written `index.js` and `index.d.ts`, and CI fails on that diff; without `--platform` the
WASI build deletes `binding.js` as a stale managed file. The WebAssembly fallback (`rustup
target add wasm32-wasip1`): `npm run build:wasi`, then `NAPI_RS_FORCE_WASI=error npm test`
(the suite on the fallback, failing rather than falling back; Bun 1.4.4 cannot load it). The
consumer tests, `typescript/consumer-tests/README.md`: pack the ten tarballs with the Forge's
`npm_package.py pack` (`--allow-missing-natives` locally, never in CI; CI's PR mode passes
`--skip osx-x64`, since no osx-x64 addon is built there, as `pack-c` does) and run
`consumer-tests/run.sh <dir> <scenarios>`, which serves them from Verdaccio on port 4873 (one
run at a time) and installs them in fresh consumers; `floor` and `s390x` need Docker
(`REGISTRY_HOST=host.docker.internal` on Docker Desktop), and `mac-lockfile` needs the
darwin-arm64 tarball in the pack (it reproduces npm bug 4828 with the floor's npm, 10.9.2,
which it installs itself; npm 11.20.0 has the fix). The browser page, `typescript/browser-smoke/`, is CI's: a Vite
build against the packed WASI tarball, run in headless Chrome and judged by `check-dom.mjs`.

Java's JNI path needs its own library: `cargo build --release --locked --lib` in `java/jni/`,
after `cargo cdylib`. Gradle stages both and refuses a stale one. Then, from `java/`,
`./gradlew test testFloor checkSuiteParity javadoc`.

R builds the core from its vendor tarball, never from `rust/` directly: the wrapper crate
depends on the `hyperuuid` crate by exact version, as a crates.io consumer does, and
`r/tools/vendor.sh` resolves that version from the checkout's `rust/`, vendors the core
beside the other crates into `r/src/rust/vendor.tar.xz`, derives the wrapper's
`r/src/rust/Cargo.lock` from `rust/Cargo.lock` (both gitignored, never committed; only the
CRAN tarball carries them), stages the corpus and rewrites `inst/AUTHORS`, and has to run
again after any change in `rust/` or `corpus/` (the Makevars stop without the tarball rather
than fetch the published core). Then, from the repo root, `Rscript -e 'devtools::test("r")'`, `R CMD
check --as-cran` on the tarball `R CMD build r` makes, and `clang-format --dry-run -Werror
r/src/*.c r/src/*.h`. Lint against a scratch install (`R CMD INSTALL --library=<dir> r`,
then `R_LIBS=<dir>`): lintr's object-usage linter loads the package it lints, and without an
installed copy every export is a false positive. Without LaTeX a local check fails only at
the PDF manual; `--no-manual` skips it, as CI's check does, so only CRAN builds that manual.

C#'s test project multi-targets net10.0 and net11.0, so `dotnet test` needs both runtimes, and
its `--project` is relative to where you run it: from `csharp/HyperUuid.Tests`, or with an
absolute path (a repo-root `--project csharp/HyperUuid.Tests/...` doubles the path). The
csproj packs whatever sits in the gitignored `runtimes/` and `staticlibs/` under
`csharp/HyperUuid`, the targets link a `staticlibs/<rid>/` archive whenever one is present, and
the csproj copies `rust/target/release/libhyperuuid.dylib` only when no library is staged, so a
stale staging links into the wrong build and is not consumer-visible: `local-clean.sh` clears
it, and `local-core.sh` restages the host's archive.

## Working with the owner

- The owner drives git. Don't commit, push, tag, open pull requests or dispatch workflows
  unless asked for that step by name; a go on the edits is not a go on git. Leave the work
  in the working tree, verified, and report it.
- Agreeing that an approach is sound, or stating a preference, is a design decision, not
  an instruction to start. Reading to scope the work is fine; edits wait for the go. During
  a release the owner may be acting in parallel: `git fetch` and compare with origin before
  editing.
- Prove everything locally first, installing toolchains under `~` as needed. Every push to
  a PR branch runs the full matrix, so only what cannot run locally goes to CI, and the
  report says which parts those were.
- Fix stale docs, comments and links on sight; that permission is standing. READMEs state
  the current state only, history goes in the CHANGELOG, and README examples are verified
  by hand in the same pass. No CI step for docs.
- A change reaches every binding (C++'s header included) and the sibling repos in the same
  pass, with parity in behaviour, API and wording. Where a binding can't follow, say why for that binding.
  Never leave a repo half-edited.
- Pre-1.0, a breaking change is a plain CHANGELOG entry under Unreleased; no semver
  callout, no upgrade hand-wringing.
- Install snippets keep `<version>` (Go: `@latest`); a concrete version there is the bug.
- Drafting release notes includes the pre-publish fixes: rename `[Unreleased]`, add the
  bullet for every package the diff touched, and set `r/NEWS.md`'s heading to the version,
  then draft from the fixed changelog.
- Never report commit, push or release state from memory; check with git or gh in the same
  turn, or leave it out.
- A verification counts only when its negative control fails. Copy the flags the repo's own
  alias or CI step uses before trusting a check.
- Formatters and their CI checks cover src, tests, benchmarks and smoke tests in every
  language.
- Tags are lightweight and unsigned (`git -c tag.gpgsign=false tag`); commits are always
  signed.
- Remove agent worktrees and their branches once the work is in the main checkout;
  `.claude/` holds only the tracked `settings.json` (it enables the superpowers plugin), so
  `git add` picks up any worktree left in `.claude/worktrees/`.

## Open follow-ups (as of 2026-10-28)

- Benchmark tables (`docs/benchmarks.md` and each binding's README) were measured on the WSL2
  i9-11900H box, except TypeScript's and R's (Apple M6, Mac18,5, 2026-10-30) and Go's (the
  M6 on 2026-10-31, natively and in Docker for Linux arm64; its x64 columns await the XPS
  run), which are labelled so. PHP's predate the extension (they measured 1.0's
  FFI package) and are marked so until `composer bench` runs on that box. When the dev machine
  changes, re-measure every harness there before relabelling a table; never mix machines, and
  say which tables predate a core change. A benchmarking workflow is an idea, not started.
- Core: the ChaCha20 pool is Linux only (Android keys a ChaCha20 stretch for batches and
  takes `getrandom` for single ids). Apple platforms take
  `CCRandomGenerateBytes`, Windows `ProcessPrng`, and the rest `getrandom`.
- The linux-gnu `.node` links std, so the core's glibc 2.34 floor isn't its floor. CI prints
  the newest glibc symbol it references and never fails on it; state the floor in the
  TypeScript README from that, then enforce it.
- CRAN submission is by hand, from the R tarball the tag run preserved; no job uploads it.
- 1.1.0 is the first release with npm and CRAN. What only it can prove: npm publishing
  under OIDC with provenance (after the owner's `0.0.1` placeholders and trusted-publisher
  registration on all ten names), the ten attested npm tarballs and the CRAN tarball in
  the inventory and its release attestation, and the hand CRAN submission from those bytes;
  and the first Maven Central publish of the multi-release jar, sixteen native libraries with their
  `jni-{rid}` attestations verified at pack.
- The PHP extension's first PIE install from Packagist (the `v1.1.0` pretty-version segment
  in the zip names, `bsdlibc`, `vs16`/`vs17`) is proved only at the rehearsal or 1.1.0.
- Java does not reach Android yet: ART has JNI, but it would need an AAR and an Android
  loader on top of the JNI library (Android builds of `java/jni/`, `System.loadLibrary` from
  the APK, API 26 for `java.time`). Python, Ruby and PHP don't reach Android either.
- The Blazor archive's `RUSTC_BOOTSTRAP=1` goes when `-Zbuild-std` stabilizes. HyperCast and
  HyperTabular still pin the nightly in `.github/hyper.json`, which the Forge now ignores;
  they move over when they thaw. Firefox and Safari are not tested. See
  `docs/superpowers/specs/2026-10-28-wasm-archive-net10-design.md`.
