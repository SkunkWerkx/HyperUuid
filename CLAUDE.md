# HyperUuid — notes for agents

One RFC 9562 UUID engine in Rust (`rust/`), reached over a C ABI or as a native extension
from nine other languages, and from C and C++ through the release's C archives. `README.md`
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
to wins. Go is the deliberate control group, slower per call and said so. Don't adjust
numbers without re-running the harness, and don't soften the Go section.

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
  among them R's `r/src/core.h` (one prototype per line; `test_check_exports.py` asserts it
  equals the core's export set) and TypeScript's `typescript/src/lib.rs` and
  `typescript/binding.d.ts` (every `#[napi]` function carries `catch_unwind`, which
  `check_exports.py` enforces and `test_check_exports.py` pins), Java's two `NativeCore`
  trees, `java/jni/src/lib.rs` (its extern block and its `RegisterNatives` table) and its
  `examples/no_panic.rs` (every JNI entry point in `java/jni/src/lib.rs` carries `no_panic`,
  which `check_exports.py`'s `jni_no_panic` enforces and `test_check_exports.py` pins),
  `cdylib_smoke.py`, the C++ header `include/hyperuuid.hpp` (hand-written; its `::`-qualified
  calls are a `check_exports.py` site) and the C smoke consumers in `c/smoke/`, the prefixes
  in `.github/hyper.json` (the Forge's symbol check reads them), the AOT and wasm smoke
  tests, and the export count in the READMEs. The Blazor archive needs no export list: the
  SDK's pinvoke table links every P/Invoke by symbol.
- npm's nine packages: the root `hyperuuid` and eight `@skunkwerkx/hyperuuid-<suffix>`, one
  per RID, mapped by `NODE_PLATFORMS` in the Forge's `hyper_config.py` (a `node.platforms`
  override in `.github/hyper.json` if this repo ever differs) and listed as `napi.targets` in
  `typescript/package.json`. The root's `optionalDependencies` are injected at pack time,
  never committed, and nothing runs `napi prepublish`. The first publish of every package,
  a new platform's included, is the owner's, by hand with a granular token: npm Trusted
  Publishing attaches only to a package that exists, and `release.yml` is registered as its
  trusted publisher after.
- The Node floor: `engines.node` (`>=22`) in `typescript/package.json`, the Forge's
  `node_floor_versions` default (`"22 24"`, an input override in `ci.yml` if this repo ever
  differs) and the floor sentence in `typescript/README.md`. Node 22 reaches EOL on
  2027-04-30: then drop it from all three.
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
- The PHP floor: `php` in `composer.json` and `php/composer.json`, `php_floor_version` in
  `ci.yml` (an explicit input, since the Forge's default serves the siblings),
  and the floor sentences in `php/README.md` and the root README. PHP 8.3 reaches EOL on 2027-12-31: then drop it from all
  of them.
- The Python sdist: built in the Forge's `hyper-build-wheels.yml` when `.github/hyper.json`
  says `sdist: true`; one more pypi inventory file, `hyperuuid-<version>.tar.gz`, beside the 17
  wheels. Its Rust floor is `rust-version`, proven at exactly that toolchain by the build job.
- The C archives: `C_RIDS` in the Forge's `c_package.py`, the platform list in `c/README.md`
  and the C row of README 'Platform support'. The shared library's own name is set in three
  places that must agree: `rust/build.rs` (install name and soname), `IMPORTED_SONAME` in
  `c/cmake/hyperuuidConfig.cmake`, and the `needed` check in `c/smoke/run.sh`.

## Never hand-edit

- `go/staticlib/` and `php/src/native/` are CI-built binaries committed by
  `stage-native-binaries.yml`, which also writes the release version and the two Swift zips'
  checksums into the root `Package.swift`. Its commit trailers are what `release.yml` uses to
  find the CI run.
- `include/hyperuuid.h` and the Swift bundle's copy are generated by cbindgen from
  `rust/src/ffi.rs`: `HYPERUUID_BLESS=1 cargo test --test header` in `rust/` rewrites both.
- Versions: bump through `prepare-release.yml`. The libraries embed their version, so a
  hand bump ships binaries reporting the old one.

## Toolchains and floors

Each language's primary toolchain is its current release; check endoflife.date rather than
assuming. Exceptions must be commented where pinned: a real platform constraint, or an
older floor shipped beside the primary and dropped at its EOL. Floors match HyperCast and
HyperTabular and move together. PHP is the exception for now: 8.3 here, ahead of the siblings,
which stay on 8.2 through the Forge default until they thaw; don't cascade it.

Rust's one exception is the Blazor archive: it is built on stable, but with `-Zbuild-std`,
because only that leaves out the prebuilt sysroot's legacy exception handling and the
features .NET 10's wasm-opt rejects (`rust/.cargo/config.toml` says why). Stable cargo takes
the nightly flag only with `RUSTC_BOOTSTRAP=1`, which the Forge sets on that one build step
and nowhere else; never set it more widely. The flag has no stability promise, so every
stable bump is re-proven by CI's guard and Blazor legs. When build-std stabilizes, the
variable goes.

## Forge coupling and releases

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. `release.yml` resolves master to one
  commit per run (`find-ci-run`'s `forge_sha`), checks the Forge out at it as `.hyperforge`
  and runs its composite actions from there (`release-guard`, `pack-gems`, `pack-npm`,
  `pack-r`, `verify-wheels`, `release-gate`, `publish-nuget`, `publish-rubygems`,
  `publish-npm`, `publish-pypi`, `finalize-release`);
  the five reusable workflows (NuGet, crate and Maven packing; crate and Maven publishing)
  are called at master and resolved when the run is queued. The inventory records the
  commit and recovery requires it unchanged. Don't merge Forge changes while a release
  is in flight.
- Fail fast: in the Forge's `hyper-build-native.yml` a new binding's steps go immediately
  after the Rust compile steps, ahead of every binding that has already shipped, in the
  desktop job and in the musl suite list alike. The newest code is where a run fails, and a
  failure in the first minutes lets the owner cancel the rest of the tree and re-run. Once a
  binding has been through a release, the next new one goes in front of it (today Java's JNI
  path leads, then TypeScript and R).
- `prepare-release.yml`, `stage-native-binaries.yml` and ci.yml's `check-core` are one
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

Go and PHP otherwise run against the committed binaries, which are refreshed only at release
and so lag every core change, and Swift has no core until one is built:
`.github/scripts/local-core.sh` builds the checkout's own (Swift's into the gitignored
`swift/HyperUuidCore.artifactbundle/`), then `go test -tags hyperuuid_local ./...`,
`swift test`, and PHP with `HYPERUUID_NATIVE_LIBRARY`. CI builds each leg's core before its
suites, so it never sees the committed ones. Outside CI, Go's and PHP's suites refuse to run
against a committed core older than `rust/` (the core freshness check), and `local-core.sh`
finds `../HyperForge` on its own. Go caches a pass from before a core change; `-count=1` runs
the check again.

The Blazor archive needs stable's sources: `rustup component add rust-src` and `rustup target
add wasm32-unknown-emscripten`, emsdk on PATH, then `RUSTC_BOOTSTRAP=1 cargo wasm-staticlib`
in `rust/`, after deleting `rust/target/.rustc_info.json` and
`rust/target/wasm32-unknown-emscripten/` (cargo caches rustc's cfg without regard to the
variable, and a cache from a plain cargo run for that target builds a different archive); the
Forge's `wasm_archive_guard.py` checks the result.

TypeScript builds its own addon, a napi-rs crate path-depending on `rust/`: from
`typescript/`, `npm ci && npm run build && npm test`, then `npm run test:bun` and `npm run
lint`. `npm run build` passes `--js binding.js --dts binding.d.ts`; a `napi build` without
them (a hand-run `--target` cross build, say) writes napi's own loader over the hand-written
`index.js` and `index.d.ts`, and CI fails on that diff.

Java's JNI path needs its own library: `cargo build --release --locked --lib` in `java/jni/`,
after `cargo cdylib`. Gradle stages both and refuses a stale one. Then, from `java/`,
`./gradlew test testFloor checkSuiteParity javadoc`.

R builds a staged copy of the core, not `rust/`: `r/tools/vendor.sh` stages the core and the
corpus, derives the wrapper crate's `r/src/rust/Cargo.lock` from `rust/Cargo.lock` (gitignored,
never committed; only the CRAN tarball carries it), vendors the crates and rewrites
`inst/AUTHORS`, and has to run again after any change in `rust/` or `corpus/`. Then, from the repo root, `Rscript -e 'devtools::test("r")'`, `R CMD
check --as-cran` on the tarball `R CMD build r` makes, and `clang-format --dry-run -Werror
r/src/*.c r/src/*.h`. Lint against a scratch install (`R CMD INSTALL --library=<dir> r`,
then `R_LIBS=<dir>`): lintr's object-usage linter loads the package it lints, and without an
installed copy every export is a false positive. Without LaTeX a local check fails only at
the PDF manual; `--no-manual` skips it, as CI's check does, so only CRAN builds that manual.

C#'s test project multi-targets net10.0 and net11.0, so `dotnet test` needs both runtimes, and
its `--project` is relative to where you run it: from `csharp/HyperUuid.Tests`, or with an
absolute path (a repo-root `--project csharp/HyperUuid.Tests/...` doubles the path). Two local
Apple link traps predate the multi-target and are not consumer-visible: a gitignored
`runtimes/browser-wasm` archive left under `csharp/HyperUuid` is packed by the csproj's
`runtimes/**` item, and the csproj copies `rust/target/release/libhyperuuid.dylib` when no
library is staged, so stale ones link into the wrong build.

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

- Benchmark tables (`docs/benchmarks.md` and each binding's README) were measured on the
  WSL2 i9-11900H box, except TypeScript's and R's, which were measured on the Apple M6
  (Mac18,5), last on 2026-10-30, and are labelled so. When the dev
  machine changes, re-measure every harness there before relabelling a table; never mix
  machines, and say which tables predate a core change. A benchmarking workflow is an
  idea, not started.
- Core: the ChaCha20 pool is Linux and Android only. Apple platforms take
  `CCRandomGenerateBytes`, Windows `ProcessPrng`, and the rest `getrandom`.
- The linux-gnu `.node` links std, so the core's glibc 2.34 floor isn't its floor. CI prints
  the newest glibc symbol it references and never fails on it; state the floor in the
  TypeScript README from that, then enforce it.
- CRAN submission is by hand, from the R tarball the tag run preserved; no job uploads it.
- 1.1.0 is the first release with npm and CRAN. What only it can prove: npm publishing
  under OIDC with provenance (after the owner's `0.0.1` placeholders and trusted-publisher
  registration on all nine names), the nine attested npm tarballs and the CRAN tarball in
  the inventory and its release attestation, and the hand CRAN submission from those bytes;
  and the first Maven Central publish of the multi-release jar, sixteen native libraries with their
  `jni-{rid}` attestations verified at pack.
- Java does not reach Android yet: ART has JNI, but it would need an AAR and an Android
  loader on top of the JNI library (Android builds of `java/jni/`, `System.loadLibrary` from
  the APK, API 26 for `java.time`). Python, Ruby and PHP don't reach Android either.
- The Blazor archive's `RUSTC_BOOTSTRAP=1` goes when `-Zbuild-std` stabilizes. HyperCast and
  HyperTabular still pin the nightly in `.github/hyper.json`, which the Forge now ignores;
  they move over when they thaw. Firefox and Safari are not tested. See
  `docs/superpowers/specs/2026-10-28-wasm-archive-net10-design.md`.
