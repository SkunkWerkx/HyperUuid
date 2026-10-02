# Changelog

All eight packages in this repository — the `hyperuuid` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Go — Windows links the core in, like Linux and macOS.** A cgo build on Windows now names
  `go/staticlib/windows_amd64` (or `windows_arm64`) on its link line: the same MSVC archive,
  17 KB, that the C# package links under Native AOT. MinGW's linker reads MSVC's objects, and
  the archive carries its own import stub for `ProcessPrng`, so the link line names nothing
  else and there is one Windows archive per architecture for both bindings. The suite passes
  on windows/amd64 built with MinGW-w64 gcc. *(`go get`)*

### Removed

- **Go — every backend but the linked one.** The purego backend (`CGO_ENABLED=0`, and all of
  Windows until now), the loading cgo backend (`-tags hyperuuid_dynamic`) and the wasmtime
  backend (`-tags hyperuuid_wasm`) are gone, and with them `go/native/`: the nine shared
  libraries and the wasm module every non-linked build embedded, the copy each process wrote
  to a temp directory and never removed, and the libc detection that picked one. None of them
  reached a platform the linked build does not — wasmtime-go needs cgo itself and ships
  engines only for the same five platforms — and Go compiled to WebAssembly never worked:
  its toolchain has no cgo and no external linker. `go.mod` requires only
  `github.com/google/uuid`, and `go/` (which the Composer archive carries too, for the Go
  proxy) is about a third of its 0.5.0 size. *(`go get`, Packagist)*

### Changed

- **Every shipped library is stripped.** The release profile now drops the symbol table and
  debug info from what it links, never the exports. Against what 0.5.0 shipped, a stripped
  local build puts the linux-x64 shared library at 16,632 bytes (from 19,048), the
  wasm32-wasip1 module at 40,617 (from 48,986) and the Linux Magnus extension at 313,680
  (from 390,624, and a platform gem carries two). The static libraries keep their symbols,
  since a consumer's linker resolves the core through them, and `cargo bench` keeps its own
  for profilers. *(every package that carries a native library)*
- **Ruby — each platform gem carries only the native libraries its platform can load.**
  Through 0.5.0 all five carried every RID's library beside their Magnus extensions, though
  a platform gem only installs where its own platform matches. Now `x86_64-linux` and
  `aarch64-linux` keep their glibc and musl libraries (Alpine installs them too, and falls
  back to Fiddle over the musl one), the other three keep their one, and every gem keeps the
  wasm module; the universal gem still carries everything. Repacking 0.5.0's own bytes this
  way puts `x86_64-linux` at 407,040 bytes (from 459,264), `aarch64-linux` at 416,768 (from
  458,752), `arm64-darwin` at 375,808 (from 435,712), `x64-mingw-ucrt` at 363,520 (from
  417,792) and `aarch64-mingw-ucrt` at 382,976 (from 437,760). *(RubyGems)*

### Fixed

- **C# — a Blazor WebAssembly app no longer gets the 3 MB wasm archive in its output.**
  Restore resolves `runtimes/browser-wasm/nativeassets/` as a copy-local native asset, so
  through 0.5.0 `libhyperuuid.a` was copied into `bin/` and the publish root of every
  browser-wasm consumer, outside `wwwroot`, never served and never read. The package's
  targets take it back out of the copy-local list; the link, which names the archive by
  path, is unchanged. *(NuGet)*
- **Rust — the `no-panic` proof from a consumer's crate is documented.** It needs `lto = true`
  in the consumer's own release profile: Cargo ignores a dependency's profile, and without
  fat LTO the generators fail the link. *(crates.io docs)*
- **Swift — a Linux or WebAssembly build no longer carries the macOS and Windows libraries.**
  SwiftPM resources take no platform condition, so through 0.5.0 every Linux build staged
  `HyperUuid_HyperUuid.resources/NativeLibs/` beside the executable, and every WebAssembly
  build carried it too: 104 KB of dylibs and DLLs that the statically linked core never
  loads. They now live in a resource-only target of their own, `HyperUuidNativeLibs`, which
  only macOS and Windows depend on; Linux and WebAssembly stage no resources from the
  package at all. On macOS and Windows the directory to deploy beside the executable is
  renamed to `HyperUuid_HyperUuidNativeLibs.bundle` (`.resources` on Windows before Swift
  6.4). The public API is unchanged. *(`.package(url:)`)*

### Upgrade note

Go is the one package with a breaking change, and the reason this is a minor release. The
module now builds only under cgo, on Linux, macOS and Windows on amd64 and arm64, which takes
a C compiler where it is built: gcc or clang on Linux (`build-base` on Alpine), the Xcode
command-line tools on macOS, MinGW-w64 gcc on Windows (llvm-mingw on arm64). A build with
`CGO_ENABLED=0`, for `GOOS=wasip1` or `js`, or for any other platform stops at compile time on
`undefined: hyperuuid_needs_cgo_and_a_C_compiler_…`, which names the fix; drop
`-tags hyperuuid_dynamic` and `-tags hyperuuid_wasm`, which no longer select anything.
Cross-compiling needs a C cross-compiler, e.g. `CC=x86_64-w64-mingw32-gcc GOOS=windows
CGO_ENABLED=1`. The API is unchanged: `Available` is always `true`, `LoadError` always `nil`,
and `ErrNativeUnavailable`, which nothing returns any more, is deprecated. Go compiled to
WebAssembly cannot use this module; `github.com/google/uuid` is pure Go and builds there.

One deployment step changes for Swift on macOS and Windows: the directory copied beside the
executable is now `HyperUuid_HyperUuidNativeLibs.bundle` (`.resources` on Windows before
Swift 6.4), not `HyperUuid_HyperUuid.*`. Nothing changes for a SwiftPM build that runs where
it was built, or on Linux and WebAssembly.

## [0.5.0] — 2026-10-02

Three themes. *Proven panic-free*: every public function in the crate and every C export
is checked at link time, and the panics that check turned up are fixed — among them a
36-character string with a stray hyphen that made `Uuid::from_str` index past its input,
and a short batch buffer that panicked instead of returning an error. *A twentieth the
size*: the shared library every binding loads no longer carries Rust's standard library,
and link-time optimization finally reaches it, so linux-x64 goes from 427 KB to 19 KB.
*Python's wheels are tested before they ship*: they are built, installed and attested on
every CI run, and the release publishes those exact files. One change breaks Rust code
that matches on the batch errors exhaustively (see the upgrade note); every other package
is a drop-in.

### Added

- **Rust — the public API is proven panic-free at link time.** A new `no-panic` feature puts
  [`#[no_panic]`](https://docs.rs/no-panic) on every generation, parsing and conversion
  function and every C export, and CI's new `check-no-panic` job links a release binary that
  calls each one (`examples/no_panic.rs`) on Linux and Windows, so a panic path the optimizer
  cannot remove fails the build and names the function. That includes `v7::now_v7`, which on
  Unix now reads the clock with `clock_gettime` directly (`libc`, already a dependency through
  `getrandom`) because std's `SystemTime::now` unwraps that call. The feature is off by
  default and changes no code a consumer runs. *(crates.io)*
- **Rust — `NewV6Error::BufferTooSmall` and `NewV7Error::BufferTooSmall`**, returned by
  `new_v6_batch`/`new_v7_batch` when `out` is shorter than `count * 16` bytes, which used to
  panic (see Fixed). Both enums are now `#[non_exhaustive]`, so a later variant is not a
  breaking change. *(crates.io)*
- **C ABI — return code `3`** from `uuid_new_v6_batch`/`uuid_new_v7_batch` when
  `count * 16` overflows `usize`. Only a 32-bit target can reach it, and only with a count no
  buffer in its address space could hold, so no binding sees it: each one allocates the
  buffer before the call. *(every package that carries a native library)*

### Changed

- **The native libraries are a twentieth the size.** The shared library every binding
  loads is now built `#![no_std]`, with the same abort-on-panic handler the static
  libraries already had. What std added was its runtime — the unwinder, the backtrace
  symbolizer and the allocator — which no C ABI export can reach. And link-time
  optimization now reaches it: Cargo only passes `-C lto` for a cdylib built as an
  invocation's one crate type, so `lto = true` never applied while the manifest listed
  `["cdylib", "rlib"]` (see the next entry). As shipped, linux-x64 goes from 426,856 bytes
  to 19,048 (346,544 with LTO alone) and win-x64 from 124,928 to 17,920, and the Linux
  libraries depend on libc alone, so the musl builds no longer need libgcc_s. The exports
  are the same 13 symbols over the same code: every one takes its timestamp from the
  caller, and `getrandom` reads the OS entropy source the same way without std. They are
  proved panic-free, so the abort handler is never even linked. The LTO step alone was
  measured at the same speed (v4, v5 and v7 through the C ABI, alternating runs, within
  noise). The wasm32-wasip1
  module keeps std, whose allocator its hosts call into, and the Python, Ruby and PHP
  extensions keep std and unwinding, so a panic in one still surfaces as a host exception.
  *(every package that carries a native library)*
- **Rust — the shared library is built by naming its crate type, not listed in the
  manifest.** `[lib]` now declares only the rlib, which is what fixes `default-features =
  false` (see Fixed). The library every binding loads is built with `cargo cdylib` (an
  alias for `cargo rustc --release --crate-type cdylib` with std off and the `cdylib`
  feature on), the way the static libraries already were; the new `cdylib` feature is that build's no_std panic
  handler, off by default and never for an rlib consumer. In this repository `cargo cdylib`
  replaces `cargo build --release` in every dev loop, and the new `cargo wasm-module` builds
  the wasm32-wasip1 module; a plain `cargo build` now produces the rlib and no shared
  library. *(crates.io, and every dev loop)*
- **Python — the wheels are built, installed and attested in CI, and the release publishes
  them unchanged.** Through 0.4.0 they were built in `release.yml` at the tag, so the first
  time a wheel was ever installed was after other registries had published; 0.4.0's osx-x64
  wheel failed there. All eight are now built on every CI run by the forge's
  `hyper-build-wheels.yml`, installed on their own platform and called into, and
  `release.yml` verifies their count, version and provenance before uploading them. Because
  the forge signs them, `gh attestation verify` on a wheel now takes
  `--signer-repo SkunkWerkx/.github` (or `--owner SkunkWerkx`); wheels up to 0.4.0 verify
  with `--repo` alone. *(PyPI)*
- **PHP — the Composer package no longer carries the other bindings.** A `.gitattributes`
  `export-ignore` list keeps the C#, Java, Python, Ruby, Rust and Swift trees, the
  workflows and PHP's own tests out of the archive Packagist serves: 2.6 MB against
  3.5 as downloaded, 6.1 MB against 8.5 unpacked. `go/` stays in, because
  the Go module proxy builds its zip from the same kind of archive. *(Packagist)*
- **Rust — the crate is rustfmt-clean and CI keeps it that way.** `cargo fmt` had drifted
  across most of `src`, the tests, the benchmarks and the no-panic example; it is applied
  throughout, and a new `check-fmt` job runs `cargo fmt --check` on every PR. The
  `clippy::chunks_exact_to_as_chunks` warnings are gone too: the Python batch path and the
  batch tests use `as_chunks::<16>()`, which also drops an `unwrap` from the former.
  Formatting only otherwise; no behavior changes. *(repository only)*

### Fixed

- **Rust — `default-features = false` builds on every target, not only bare metal.** Cargo
  builds every crate type a dependency lists, and a no_std cdylib has no panic handler, so
  through 0.4.0 a `default-features = false` consumer failed with "`#[panic_handler]`
  function required, but not found" on any target that can produce a cdylib: the
  developer's own machine and `wasm32-unknown-unknown`. Bare-metal targets drop the crate
  type, which is why the `thumbv7em` check in CI never saw it. CI now builds a real
  `default-features = false` consumer on the host and for `wasm32-unknown-unknown`.
  *(crates.io)*
- **Rust — `Uuid::from_str` no longer panics on a stray hyphen.** The parser skipped hyphens
  wherever they fell, so a 36-character string with an extra one in the last group shifted
  the hex pairing and read one byte past the end. `00000000-0000-0000-0000--00000000000` is
  the shortest example. The digit pairs are now read at their fixed offsets, so a misplaced
  hyphen is just a non-hex digit and the result is `ParseUuidError`. *(crates.io)*
- **Rust — a short buffer passed to `new_v6_batch`/`new_v7_batch` is an error, not a panic.**
  It returns the new `BufferTooSmall` and leaves the buffer untouched. The bindings were never
  exposed: each sizes its own buffer. *(crates.io)*
- **Rust — `v7::now_v7` no longer panics on a clock set before 1970.** It returns
  `TimestampOutOfRange`, the same as a clock so far ahead that its milliseconds overflow a
  `u64` (which used to be silently truncated). *(crates.io)*
- **Rust — `Timestamp::to_unix_millis` saturates instead of overflowing.** For more than
  `u64::MAX / 1000` seconds it panicked in a debug build, and in a release build it wrapped
  into a valid-looking timestamp, so `new_v6_at`/`new_v7_at` minted a UUID for the wrong
  time. It now saturates at `u64::MAX`, which both generators reject as out of range.
  *(crates.io)*
- **Ruby — the native extension cannot panic on an uninitialized exception cache.** It
  falls back to `RuntimeError` rather than panicking. `init` fills the
  cache before defining any method, so the fallback is unreachable in practice. *(RubyGems)*

### Upgrade note

The Rust crate is the one package with a breaking change, and the reason this is a minor
release rather than a patch: `NewV6Error` and `NewV7Error` gained a variant and became
`#[non_exhaustive]`, so a `match` on either one outside the crate needs a wildcard arm. Every
other binding is a drop-in: the C ABI only gained a return code no binding can receive, and
the bindings' own APIs are unchanged.

## [0.4.0] — 2026-10-01

Six themes. *A load probe everywhere*: the core now reports its own version, and every
binding fronts it with an availability check, the pair HyperCast already had. *musl*:
`linux-musl-x64` and `linux-musl-arm64` are built, attested and shipped, so Alpine gets a
native library instead of a glibc one that cannot load. *Only supported runtimes*: every
floor that had reached end of life is raised, and the floors are now tested. *The core
links in*: Swift on Linux, which is what adds musl and WebAssembly to that binding, and Go's
cgo build and C# Native AOT, which now carry no shared library to extract or load.
*Faster where it was slow for no reason*: Java's FFM calls are compiled rather than
interpreted in a GraalVM Native Image, and Python and Ruby calls cost about half what they
did. *Measured again*: every benchmark table is re-measured on x86-64, and most headline
ratios are smaller for it. Around those, the fixes from a full audit of all eight bindings,
and the test and dev-loop setup carried back from HyperCast's wasm port.

### Added

- **Python — `v6_unix_millis` and `v7_unix_millis`.** The embedded timestamp as the integer
  the core returns, Unix-epoch milliseconds, with no `datetime` built around it: 161 ns
  against 252 for `v7_timestamp` and 401 for stdlib's `UUID.time`. Unlike `v7_timestamp`
  it cannot overflow — the whole 48-bit field fits an `int`. Both backends. *(PyPI)*
- **Swift — musl Linux and WebAssembly.** The binding builds with Swift's static Linux SDK
  (`--swift-sdk x86_64-swift-linux-musl`, and arm64) and with its WebAssembly SDK
  (`wasm32-unknown-wasip1`). Neither target can open a shared library, so the core is
  linked in: `swift/HyperUuidCore.artifactbundle` carries one no_std static library per
  triple, about 20 KB each, built by `cargo staticlib` and attested like every other native
  binary. CI runs the suite under WasmKit and a smoke executable (`swift/StaticSmokeTest`)
  through the musl SDK, on Swift 6.4 and on the 6.2 floor. Through 0.3.0 a musl build
  stopped at a compile error. *(`.package(url:)`)*
- **`hyperuuid_version` — the load probe, in the core and in every binding.** A
  zero-argument export returning the core's version packed `major << 16 | minor << 8 |
  patch`, the same shape as `hypercast_version`, so a host can prove the library it loaded
  is the one its binding was built against before minting anything. Each binding fronts it
  with a check that never throws and the loaded version as text: C#
  `UuidGenerator.IsAvailable`/`NativeVersion`, Java `isAvailable()`/`nativeVersion()`, Go
  `Available()`/`LoadError()`/`NativeVersion()` with `ErrNativeUnavailable` for
  `errors.Is`, Swift `isAvailable`/`nativeVersion()` with a public `NativeLibraryError`,
  Ruby `HyperUuid.available?`/`native_version`, PHP `HyperUuid::isAvailable()`/
  `nativeVersion()`, Python `native_version()`, and `hyperuuid::hyperuuid_version()` in
  Rust. On every backend, wasm included. A library that predates the export (0.3.0 and
  earlier) reads as unavailable. *(every package)*
- **musl (Alpine): `linux-musl-x64` and `linux-musl-arm64`.** Built inside an Alpine
  container with the unwinder linked statically, so the library depends on musl's libc and
  nothing else and loads on a bare `alpine`, `python:alpine` or `golang:alpine` image. In
  the NuGet package, the jar, the gems, `go/native/` and `php/src/native/`, and as
  `musllinux_1_2` wheels. Each binding resolves it for a process that has a musl loader
  mapped; Ruby runs on its Fiddle backend there. Through 0.3.0 Alpine got the glibc library,
  which does not load under musl. Swift has no dynamic loader on musl and reaches it
  by linking the core in instead (above). *(every package)*
- **C# — `NewV5(Guid, ReadOnlySpan<char>)`.** The `string` overload's path for a name held
  as a slice; nothing is materialized first. *(NuGet)*
- **Java — `Automatic-Module-Name: io.github.skunkwerkx.hyperuuid`**, and a README section
  on `--enable-native-access`. *(Maven Central)*
- **Go — `ErrNegativeCount`.** The batch functions return it for a negative count instead of
  panicking in `make`. `Example*` tests for pkg.go.dev, and the zero-allocation claims held
  by `testing.AllocsPerRun` tests. *(`go get`)*
- **Swift — `fillV6(into:)`/`fillV7(into:)` over an `inout [UUID]` at the current time**,
  which the raw-buffer forms already had. *(`.package(url:)`)*
- **PHP — `Uuid` implements `JsonSerializable`**, so `json_encode` carries the string
  instead of `{}`. *(Packagist)*
- **Python — the package is typed.** `py.typed` ships with a stub for the extension module,
  so mypy and pyright see `uuid.UUID` and `datetime` instead of `Any`; a test type-checks a
  consumer under `mypy --strict`. *(PyPI)*
- **Ruby — `rake native:dev`.** Builds the Magnus extension for the running Ruby and stages
  it where `require` looks, the step `cargo ruby-ext` alone never did. *(dev only)*
- **The declared floors are tested, and so is every wheel.** CI's new musl job runs the PHP,
  Ruby and Python suites on the oldest version each package declares as well as the newest;
  the Go purego backend now runs on Linux and macOS, where only cgo did; and the release
  installs each wheel and calls into it before anything is published. *(dev only)*
- **The AOT and browser smoke tests cross every native entry point** in C#, and Java's AOT
  smoke test calls every public method; each covered about half before, while claiming
  more. *(dev only)*
- **A repeatable Native Image proof of the wasm path.** `./gradlew :aot-smoke-test:nativeRun
  -Pwasm` puts GraalWasm on the smoke test's classpath and runs the binary with
  `-Dhyperuuid.backend=wasm`; the binary prints which backend it took. The 0.3.0 receipt for
  this came from a one-off hand build against the published jar. *(dev only)*
- **JMH through the wasm backend.** `./gradlew :benchmarks:jmh -Pwasm` runs the same suite
  through GraalWasm, with the longer warmup Truffle's runtime compilation needs — HyperCast
  saw error bars wider than the values at the FFM suite's 3×1s — and `-PjmhInclude=<regex>`
  runs a subset. Under GraalVM CE 25.4's JIT that run puts `newV7` at 93 ns against 40 ns
  FFM in the same session, and the README's wasm table is now that run rather than a hand
  loop. *(dev only)*
- **A dev loop for the native library and the wasm module.** The Java build stages
  `rust/target/release` and `rust/target/wasm32-wasip1/release` onto the classpath when
  nothing has been placed under `src/main/resources/native` explicitly, and the Ruby and
  Python backends fall back to the same in-repo builds when the packaged file is absent, so
  `./gradlew test testWasm`, `HYPERUUID_WASM=1 bundle exec rspec` and `HYPERUUID_WASM=1
  pytest` need nothing copied by hand. The Ruby fallback also counts for backend selection:
  `fiddle_library_available?` sees the in-repo build too. Same dev trap as HyperCast's
  README already documents: an extension-feature build overwrites the plain cdylib, and the
  fallback will dlopen it and fail on unresolved `Py*` symbols until a plain
  `cargo build --release` puts it back. `cargo ruby-ext` and `cargo php-ext` (aliases in
  `rust/.cargo/config.toml`) and `python/.cargo/config.toml` now build the extensions into
  their own target directories, so that no longer happens. *(dev only)*
- **A browser proof of the C# WebAssembly package.** `csharp/HyperUuid.WasmSmokeTest` is
  rewritten to import the shipped `build/HyperUuid.targets` and call v4, v5, v6, v7
  and a 1000-id batch through the public `UuidGenerator` API (it previously declared its
  own P/Invokes and no longer worked). `./check.sh` publishes it, loads it in headless
  Chromium and requires `PASS`. *(dev only)*

### Changed

- **Every benchmark table was re-measured on x86-64, and most headline ratios are smaller.**
  The published figures came from an arm64 WSL2 machine whose wall-clock read cost about a
  microsecond and whose JDK random source ran six times slower than this one's — and both
  sat on the far side of the comparisons that produced the largest numbers. Re-measured on
  linux-x64 (an Intel Core i9-11900H) with each binding's own harness: against the `uuid`
  crate, 2x on v5 and v7 and level on v4 and v6 (was 13–16x on v6/v7); against
  `UUID.randomUUID()`, 2.4–4.5x (was 11–17x); against `Guid.NewGuid()`, 5.7–8.1x
  (unchanged); against `Foundation.UUID()`, 9.5–13x; against a naive inline PHP v4, level to
  1.2x (was 1.9–2x). Batch-over-loop multiples that included a clock read per call shrank
  the same way (Swift's from 42–50x to 6–7.5x), and PHP's byte batch is 8x its object batch,
  not 100x. Python and Ruby were measured after the two entries below and are stated
  there. Go's table gained the
  linked-in backend beside the loading one and purego. Every README names the machine and
  the runtime it measured. *(docs)*
- **Java — GraalWasm 25.4.4.1.1.** The wasm backend's optional engine moves from 25.3.4.1
  to the release that matches GraalVM 25.4, in the build, the benchmarks, the AOT smoke
  test and the README's dependency snippet. The two have to match: on a 25.4 JDK the older
  artifacts ran the module interpreted on the JVM, with no error, and failed a Native Image
  build outright. *(docs, dev only)*
- **Python — every call costs about half what it did.** Nearly all of a call was the
  `uuid.UUID` or `datetime` it returned, and the extension built those by calling Python
  callables: `UUID.__new__` and two `object.__setattr__` calls for one, the eight-argument
  constructor for the other. It now allocates the instance and sets its slots through the
  C API (`PyType_GenericAlloc`, `PyObject_GenericSetAttr`) and builds a `datetime` from the
  packed state its own pickling uses, all inside the stable ABI, so the wheels are
  unchanged; and `new_v6`/`new_v7` skip their argument conversion for no argument or a
  plain `int`. On CPython 3.14: `new_v4()` 540 ns to 243, `new_v7(ms)` 726 to 354,
  `v7_timestamp` 466 to 252. Against stdlib that is 3.0–4.2x on generation (was 1.4–2.2x)
  and 1.6–1.9x on timestamp extraction, which used to trail `UUID.time`. *(PyPI)*
- **Ruby — v6 and v7 cost what v4 does.** `Uuid.new` copied the sixteen bytes the core had
  just handed it, and the timestamp argument was range-checked against a 64-bit constant —
  a bignum comparison that cost more than the mint. `new_v4` 317 ns to 234, `new_v7` 512
  to 284 on the Magnus backend: 4.9x and 4.0x `SecureRandom.uuid` (was 3.1x and 1.9x).
  `Uuid.new` still copies a caller's String. *(RubyGems)*
- **Only upstream-supported runtimes.** PHP's floor is 8.2 (8.1 ended 2025-12-31), Ruby's is
  3.3 (3.2 ended 2026-03-31), and Java's is JDK 25 (22, 23 and 24 are end of life; 25 is the
  first LTS with the final FFM API, and the jar is compiled `--release 25`). A consumer on
  an older runtime keeps resolving 0.3.0. *(Packagist, RubyGems, Maven Central)*
- **Python — the floor is 3.11.** CPython 3.9 reached end of life in October 2025 and 3.10
  does on 2026-10-31, so `requires-python` is `>=3.11` and the wheels are built
  `abi3-py311`: still one wheel per platform, covering every CPython from 3.11 up, and the
  same floor HyperCast has. A 3.9 or 3.10 interpreter keeps resolving 0.3.0. A floor past
  3.10 also puts `PyUnicode_AsUTF8AndSize` in the stable ABI, so `new_v5` with a `str` name now hashes the string's own UTF-8 in place
  instead of encoding and copying it on every call. *(PyPI)*
- **A release rebuilds the native libraries at the version it ships.** The libraries now
  report their own version, so the last green CI run from before the version bump is no
  longer reusable: `prepare-release` dispatches CI on the bump commit, staging follows that
  run automatically, and `release.yml` refuses a tag whose CI run or committed libraries
  were built at any other version. *(release machinery)*
- **An architecture with no native build is no longer taken for x64.** Go and PHP report an
  unsupported platform, Swift refuses to compile for it, and Java resolves to no native
  build and falls back to wasm, as its README always said it would; Java also falls back
  when a bundled library will not load. PHP on Windows always loads the x64 library, since
  PHP there is an x64 process even on ARM hardware. *(Maven Central, `go get`,
  `.package(url:)`, Packagist)*
- **Caller errors are the same exception on every backend, and name the mistake.**
  Out-of-range timestamps and batch counts, wrong argument types and null names were
  whatever the backend happened to raise: `NullReferenceException` and `OverflowException`
  in C#, `RangeError`/`NoMemoryError` or a silent wrap in Ruby, `OverflowError` or silent
  coercion in Python, an FFI error in PHP. Each binding now checks once, above its backends.
  Ruby's exceptions are `HyperUuid::TimestampOutOfRangeError`/`RandomSourceError` (the
  `Runtime::` names remain as aliases). *(NuGet, RubyGems, PyPI, Packagist)*
- **`Uuid.parse` takes the 8-4-4-4-12 form only** in Ruby and PHP, matching the core. Both
  used to delete every hyphen first, so bare and misplaced-hyphen strings parsed.
  *(RubyGems, Packagist)*
- **Python — a `datetime` is truncated to its millisecond**, where it was rounded through a
  float and could stamp a UUID up to half a millisecond late. *(PyPI)*
- **Swift and Go open the native library `RTLD_LOCAL`**, and Swift opens it in place: every
  process used to copy it to a fresh temp file and leave it behind. A negative Swift batch
  count is a precondition failure, not an empty array. *(`.package(url:)`, `go get`)*
- **Swift — the floor is Swift 6.2, and Linux links the core in.** On Linux the package
  no longer loads a shared library: the core is a static library SwiftPM links into the
  consumer's executable (a binary target, SE-0482, which is what sets the floor). Nothing
  has to be deployed beside the executable, `UuidGenerator.isAvailable` is always `true` there, and
  `NativeLibraryError` is never thrown. macOS and Windows still load a bundled shared
  library, and find its resource directory under both names SwiftPM uses: `.bundle` (Swift
  6.4's default build system) and `.resources` (6.2 and 6.3 on Windows). A toolchain older
  than 6.2 keeps resolving 0.3.0. *(`.package(url:)`)*
- **Go — a cgo build links the core in.** On Linux and macOS, amd64 and arm64, the core
  is a static library on the cgo link line (`go/staticlib/`), not a shared library embedded
  for every platform, written to a temp file and `dlopen`ed on first use. A program that
  does nothing else is 3.1 MB instead of 6.2 MB, starts without touching the filesystem, and
  runs with no writable temp directory — including fully static, in an empty read-only
  container. One archive serves glibc and musl. `Available()` is always `true` in this
  build. `CGO_ENABLED=0`, Windows and cross-compiles still load through purego, unchanged;
  `-tags hyperuuid_dynamic` keeps cgo and loads the shared library as before. *(`go get`)*
- **C# — a Native AOT publish links the core in.** The package carries a static library
  per RID (`staticlibs/`), and its targets file hands the right one to the AOT linker and
  binds the P/Invokes as direct calls, so the publish directory is one executable with no
  `libhyperuuid` beside it. `<HyperUuidStaticLink>false</HyperUuidStaticLink>` restores the old
  behaviour; a JIT process is unaffected. *(NuGet)*
- **Intel macOS (`osx-x64`) is built and core-tested, with no CI leg of its own.** The
  library is cross-compiled on the Apple silicon runner, attested and shipped in every
  package as before, and the Rust core's own suite runs on it under Rosetta 2. No binding's
  suite runs there any more, and there is no `x86_64-darwin` precompiled gem: Ruby on an
  Intel Mac installs the universal gem and runs on Fiddle, the slower backend (2,299 ns
  against 416 ns per `new_v4` where both were last measured side by side, on win-arm64). The `osx-x64` wheel is cross-built on the same runner and still installed
  and called into, under an x64 Python, before it is published. The leg took 37 minutes
  against 8 on Apple silicon, on hardware Apple stopped selling in 2023.
  *(RubyGems; CI for everything else)*
- **CI builds on Ubuntu 26.04 and tests Swift on 6.4.** The Linux legs name `ubuntu-26.04`
  and `ubuntu-26.04-arm` rather than `ubuntu-latest`. The glibc floor of the shared
  libraries is unchanged at 2.34, and CI now fails a Linux leg whose library references
  anything newer.
- **C# — a Blazor WebAssembly project below .NET 11 gets warning `HYPERUUID001` and no
  native link**, instead of the wasm wiring applying to every browser project. *(NuGet)*

### Fixed

- **Java — every FFM call in a GraalVM Native Image went through the method-handle
  interpreter.** A native image built from this jar ran `newV7(long)` in 6.4 µs where the
  JVM takes 36 ns, 180 times slower and forty times slower than the wasm backend in the
  same binary. The downcall handles were `static final` but bound to the library's
  addresses, so their class initialized at run time, and Native Image only compiles a call
  through a handle that is a constant when the image is built. The handles are now one per
  C signature, created without an address in a holder class
  (`UuidGenerator.Downcalls`) that a `native-image.properties` in the jar initializes at
  image build time; each call passes the export's address as its first argument. The
  big-endian layout a UUID is read through moved with them for the same reason. In a native
  image: `newV7(long)` 78 ns, a 1000-UUID byte fill 12.8 µs (was 19), a `UUID[]` fill
  31 µs (was 162). Nothing changes on the JVM, and a consumer's `native-image` build
  inherits the setting with no configuration. *(Maven Central)*
- **PHP — the batch benchmark timed the library load.** `bench/UuidBatchBench.php` ran one
  unwarmed revolution per iteration, and phpbench runs each iteration in a fresh process,
  so the ~0.7 ms it takes to load the native library sat inside every figure: the README's
  batch table read 0.64 ms against 0.87 ms, a 1.4x gain, where the batch is 76 µs against
  285 µs for the loop, 3.8x. The suite now warms up before it measures and covers the byte
  batches too, so the README's bytes-versus-objects figure is reproducible from the repo.
  *(dev only)*
- **C# — a Blazor WebAssembly app could not use HyperUuid and HyperCast together.** Each
  package's wasm static library bundled its own copy of Rust's standard library, and the
  two collided at link time: `wasm-ld: duplicate symbol: rust_eh_personality`. The library
  is now built without std (`cargo wasm-staticlib`: `--no-default-features` plus a
  `staticlib` feature that supplies the panic handler std would have, with panics
  aborting), so there is nothing to collide. Proven by linking both packed
  packages into one Blazor app and running it in headless Chromium; CI fails the build if
  the library ever defines `rust_eh_personality` again. *(NuGet, `hyperuuid` crate)*
- **C# — a Blazor WebAssembly app that reached the package through a class library got no
  native link at all.** NuGet imports `build/` only into a project that references a package
  directly, so an app depending on a library that depends on HyperUuid received the managed
  assembly and none of the wasm wiring. The package now also ships `buildTransitive/`,
  which flows to every project downstream, and an app using both HyperUuid and HyperCast is
  handed the exception-handling translation flag once instead of twice. The file now sits at `build/HyperUuid.targets`, with no target-framework folder, since it gates itself. Proven with
  a packed `.nupkg`, a class library and a Blazor app in headless Chromium. *(NuGet)*
- **An empty v5 name crossed the C ABI as a null pointer.** C#, Go, Ruby's Fiddle backend
  and Swift's raw-buffer form hand over null for a zero-length name, and `uuid_new_v5` built
  a slice from it, which is undefined behaviour. The export no longer touches the pointer
  when the length is zero, and every binding pins the empty-name vector. *(every package)*
- **Java — a batch count past `Integer.MAX_VALUE / 16` wrote past a Java array.**
  `count * 16` wrapped to a small or zero length while the core still wrote the full batch.
  Such a count, or a negative one, is `IllegalArgumentException` on both backends.
  *(Maven Central)*
- **Python — `new_v6_batch`/`new_v7_batch` narrowed `count` to 32 bits.** `2**32 + 1`
  minted one UUID and returned it as the batch, and a count the allocator refused aborted
  the interpreter. Out of range is `ValueError`, unallocatable is `MemoryError`. On the wasm
  backend a failed buffer regrow left a dangling guest pointer. *(PyPI)*
- **Python — a `bytes` name cost about a microsecond more than a `str` one in `new_v5`.**
  The derived PyO3 extractor built and discarded a `TypeError` on every `bytes` call; a
  hand-written one checks the type directly. *(PyPI)*
- **Swift — `newV6(_: Date)`/`newV7(_: Date)` trapped on a date before 1970**, and a missing
  resource bundle crashed the process through SwiftPM's generated accessor. The first throws
  `timestampOutOfRange`; the second is a thrown `NativeLibraryError` and
  `isAvailable == false`, with the deployment requirement documented. *(`.package(url:)`)*
- **Go — a core missing a symbol panicked on the purego backend** and left the package
  half-initialised; it is a load error, as on cgo. `NewV5` could report `ErrRandomSource`,
  which version 5 cannot produce. *(`go get`)*
- **Ruby — on Alpine, 0.3.0 loaded the glibc library and the first call raised
  `Fiddle::DLError`**; the wasm backend failed with `NoMethodError` on a module missing an
  export. *(RubyGems)*
- **Java — selecting wasm without GraalWasm now says what to add.** The message naming the
  two artifacts existed but was unreachable. *(Maven Central)*
- **Docs that had drifted from the code.** C#'s shipped XML docs said the build "ships
  linux-arm64 only"; the Python READMEs described an sdist that is not published; the root
  README still said the batch functions allocate; Go's and PHP's READMEs gave commands and
  paths that do not exist. PHP's README now says how to enable FFI under a web SAPI.
  *(docs only)*
- **C# — Blazor WebAssembly on .NET 11 failed in the browser.** .NET 11 links browser-wasm
  with the new exception-handling encoding while the precompiled Rust standard library
  inside the static library uses the legacy one; the link succeeded and the browser then
  refused the module (`module uses a mix of legacy and new exception handling
  instructions`). `HyperUuid.targets` now appends Binaryen's translate-to-exnref pass to the
  SDK's post-link `wasm-opt`. WebAssembly is documented as .NET 11 and later only.

## [0.3.0] — 2026-09-03

Two themes. The first is *one core, one more way in*: Java, Ruby, Python and Go can now run
the Rust core as a `wasm32-wasip1` module inside the process, through a wasm engine the
ecosystem already has, so a platform with no native build in the package still has a working
backend and nothing has to be `dlopen`'d at all. The second is the carrier diet HyperCast
0.2.0 ran, ported back to the three bindings here that had the same shape underneath: a
confined arena or a heap array wrapped around a native call that never needed one. Measured
before and after on one machine in one session; the core and every UUID it produces are
untouched.

### Added

- **A wasm backend in Java, Ruby, Python and Go.** The core built as a `wasm32-wasip1`
  module, `hyperuuid.wasm`, ships beside the native libraries in the jar, the gems and the
  wheels, and is committed under `go/native/`; a wasm engine the ecosystem already has runs it
  in-process, behind each binding's existing backend switch, with the engine an optional
  dependency the consumer adds only if they want this path:
  - **Java** — [GraalWasm](https://www.graalvm.org/webassembly/), `-Dhyperuuid.backend=wasm`,
    or automatic when the jar has no native build for the platform. `org.graalvm.polyglot:wasm`
    is `compileOnly` and never in the POM; `UuidGenerator.backend()` reports which path won.
  - **Ruby** — the [wasmtime](https://rubygems.org/gems/wasmtime) gem, `HYPERUUID_WASM=1`, or
    automatic when no native library exists for the platform. `HyperUuid::BACKEND` reports
    `:wasm`; `spec/wasm_backend_spec.rb` pins the outputs byte-for-byte against Fiddle.
  - **Python** — [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py) via
    `pip install hyperuuid[wasm]`, `HYPERUUID_WASM=1`, or automatic when the PyO3 extension
    fails to import. `hyperuuid.BACKEND` reports `"wasm"` or `"native"`.
  - **Go** — [wasmtime-go](https://github.com/bytecodealliance/wasmtime-go) behind
    `-tags hyperuuid_wasm`, opt-in only and never selected automatically; the tag compiles in
    exactly one backend. cgo throughout, so no win-arm64 build.

  Measured on one box, through each shipped binding: `new_v7` at 420 ns under GraalVM's JIT
  and 181 ns under Native Image (3.1 µs interpreter-only on a stock JDK), 867 ns from Ruby,
  6.2 µs from Python, 3.1 µs from Go, against 64 / ~450 / 850 / 142 ns native; the 1000-UUID
  byte fills land at 15.9 / 40.6 / 41 / 41 µs against 15.8 / 24 / 18.7 / 17.6 µs native. Every
  call is serialized under a lock, because neither a GraalWasm `Context` nor a wasmtime
  `Store` is safe for concurrent use; the native backends stay lock-free. The module exports
  wasi-libc's `malloc`/`free` through two linker flags in `rust/.cargo/config.toml`, because a
  host-picked offset into the guest's initial memory collides with dlmalloc and corrupted a
  batch mid-buffer. CI builds the module on every leg and runs the four suites a second time
  through it. *(Maven Central, RubyGems, PyPI, `go get`)*
- **`newV5(namespace:name:)` over an `UnsafeRawBufferPointer`** in Swift — the primitive the
  `String` and `[UInt8]` forms now wrap. *(`.package(url:)`)*
- **The wasm module is attested like every native library.** `hyperuuid.wasm` carries the
  same build-provenance attestation as the six native builds, signed by the reusable workflow
  in `SkunkWerkx/.github`, and `stage-native-binaries.yml` refuses to commit it under
  `go/native/` unless that attestation verifies. Every README now has a Verifying provenance
  section with the exact `gh attestation verify` command and flags for its artifact.
  *(all packages; docs and release machinery)*

### Changed

- **Java: nothing is copied on the way across.** Every downcall is linked
  `Linker.Option.critical(true)`, so a caller's `byte[]` — a v5 name, a batch destination,
  sixteen bytes to reorder in place — is pinned and handed to the native side directly; the
  single-UUID doors use one per-thread 16-byte in/out scratch instead of an
  `Arena.ofConfined()` opened and torn down per call, written and read as two big-endian
  longs. `reachability-metadata.json` registers the option and the GraalVM Native Image
  smoke test passes on it. JMH: `newV4` 155 → **102 ns**, `newV5` 230 → **102 ns**, `newV6`
  128 → **67 ns**, `newV7` 125 → **77 ns**, each 112 → 32 B/op; `fillV7(byte[])` now
  **0 B/op** — the caller's array is written in place. *(Maven Central)*
- **Go: the UUID crosses by value.** The cgo shims keep the sixteen bytes on their own stack
  and return them as a struct, and take a UUID argument the same way, so no Go pointer
  crosses except a caller's own slice: `NewV4`/`NewV6At`/`NewV7At` **1 → 0 allocs**,
  `NewV5String` 3 → 1 (Go's own `[]byte(name)`), `V6`/`V7UnixMillis` 75 → **58 ns, 0 allocs**.
  Per-call time on the generators barely moves, because entropy, not the crossing, is what
  those doors cost; the README's "structural floor" claim is corrected. purego is unchanged.
  *(`go get`)*
- **Swift: zero mallocs per call.** `uuid_t` on the stack is the scratch and the result —
  no heap `[UInt8]` for the out-value or the inputs — the v5 name crosses via `withUTF8`,
  the batch object doors fill their result array in place through the existing fill path,
  and the library handle is a class reference rather than a 13-field struct copied per call.
  `newV4` 1 → **0 mallocs**, `newV5` 3 → **0**, `newV7Batch(1000)` 86 → **17 µs** and 1002 →
  **1** malloc. *(`.package(url:)`)*
- **Ruby: the gemspec declares no wasmtime.** The engine is a Gemfile group for this repo's
  own suite, not a development dependency of the gem, so `gem install hyperuuid` and
  `bundle install` against the gem pull in nothing new. *(RubyGems)*

### Upgrade note

Drop-in for every binding. Nothing is removed or renamed. The wasm backends are opt-in and
change nothing until asked for: no new runtime dependency in any package (Java's GraalWasm
is `compileOnly`, Ruby's wasmtime a Gemfile group for the suite, Python's an extra, Go's behind a
build tag — though wasmtime-go does now appear in `go.mod`, so it enters a consumer's module
graph without entering their binary). The Rust crate's source is unchanged since 0.2.1; the
only addition under `rust/` is the `.cargo/config.toml` that exports `malloc`/`free` on
`wasm32-wasip1`, which applies to builds run from that directory and to nothing a consumer
compiles. The crate takes the coordinated version like every other package.

## [0.2.1] — 2026-09-02

A release-machinery fix. No API changes in any binding — but **0.2.0 did not reach Maven
Central**, so this is the version Java consumers want, and it is the first release whose crate
is signed.

### Fixed

- **The Java binding now builds its javadoc**, unblocking the Maven Central publish that
  failed on 0.2.0. Sixteen `@param` tags were missing from the destination-buffer and
  raw-byte SQL-order methods added in 0.2.0, and `javadoc -Xwerror` — set in
  `java/build.gradle.kts` — correctly refused them. **0.2.0 is absent from Maven Central and
  will stay absent**; it cannot be published now that the version is spent elsewhere. Java
  consumers should go straight from 0.1.1 to 0.2.1, which carries everything 0.2.0 added.
  *(Maven Central)*
- **The published crate is attested again.** On 0.2.0 the attestation step ran *after*
  `cargo publish` and could not find the packaged `.crate`, so the crate uploaded and the
  signing failed — and a crates.io publish is irreversible, which left 0.2.0 permanently
  unsigned. The release pipeline now packages, attests, and only then publishes, so the same
  failure would stop the release while it is still reversible. **The 0.2.0 crate has no
  provenance attestation and cannot be given one**; its integrity is still checkable against
  the index checksum cargo verifies on every download, and 0.2.1 restores full provenance.
  *(crates.io)*

### Changed

- **crates.io publishing is tokenless**, using Trusted Publishing over OIDC rather than a
  stored API token — short-lived credentials, minted per run and revoked when the job ends.
  Consumer-invisible; recorded because it changes what a compromise of this repository's
  secrets could reach. *(crates.io)*
- **CI runs `javadoc` on every pull request.** The gate that caught this existed all along —
  it simply never ran outside a release. C# and Rust get their doc enforcement from
  compilation CI already performs (`CS1591` with warnings-as-errors, `#![deny(missing_docs)]`);
  Java's `-Xwerror` only fired during the Maven publish, so undocumented members passed every
  PR and failed the release instead. *(CI only, no package change)*

### Upgrade note

Drop-in from 0.2.0 for every binding except Java, where it is the first available 0.2.x.
Nothing else changed: same API, same behaviour, same native core.

If you verify provenance, note the one gap this release closes and the one it cannot:

```sh
# 0.2.1 — every package attested, including the crate
gh attestation verify hyperuuid-0.2.1.gem --repo SkunkWerkx/HyperUuid

# 0.2.0 — the .crate alone has no attestation; every other package does
```

## [0.2.0] — 2026-09-02

The theme is *stop paying for objects you didn't ask for*. Every binding already made one
native call per batch; what cost real time was the per-item object construction wrapped around
it. Eight packages now expose the raw bytes directly, and the wins scale with how expensive
each language's object construction is — 73x in PHP, 35x in Python, ~11x in Ruby.

Alongside that: Ruby's compiled Magnus extension now ships for **both** Windows architectures,
so the slow `Fiddle` fallback is no longer the only option anywhere mainstream, and every
registry's package now carries build provenance rather than just NuGet's.

### Added

- **Raw-byte and destination-buffer APIs, across all eight packages.** One cross-binding parity
  pass, each shaped to what the language can actually do:
  - **C#** — a non-throwing `Try*` twin for every fallible operation (`TryNewV4`/`V6`/`V7`,
    `TryFillV6`/`V7`), so a `Result<T>`-shaped gateway no longer needs a `try`/`catch` per call;
    `Span<byte>` overloads for `V6`/`V7To`/`FromSqlOrder`; `Span<byte>` overloads for
    `FillV6`/`V7`. *(NuGet)*
  - **Go** — `FillV6`/`V7` and `FillV6`/`V7Bytes`, each with an `At` variant, plus
    `V6`/`V7To`/`FromSqlOrderBytes` rewriting a caller's 16 bytes in place. `uuid.UUID` is
    `[16]byte`, so a whole batch lands in the caller's slice in one native call with no
    per-element conversion: `FillV7At` measures 18,355 ns / **0 B / 0 allocs** per 1000, against
    138,646 ns and 1000 allocs for individual calls. *(`go get`)*
  - **Swift** — `fillV6`/`fillV7` over both an `UnsafeMutableRawBufferPointer` and an
    `inout [UUID]`, plus `v6`/`v7To`/`FromSqlOrder(bytes:)`. Foundation's `UUID` wraps `uuid_t`,
    already RFC 9562-ordered, so the array form needs no conversion either — asserted with a
    `precondition` on `MemoryLayout<UUID>` rather than assumed. Adds
    `Error.bufferNotWholeUUIDs`. *(`.package(url:)`)*
  - **Java** — `fillV6`/`fillV7` over both `UUID[]` and `byte[]`, plus
    `v6`/`v7To`/`FromSqlOrder(byte[])` in place. `java.util.UUID` is two `long`s rather than 16
    ordered bytes, so — like C# and unlike Go/Swift — the `UUID[]` form removes the allocation
    but still rebuilds each element; the `byte[]` form is the one that removes real work, and
    both say so. *(Maven Central)*
  - **Python** — `fill_v6`/`fill_v7` write into a caller's `bytearray`: **~35x** on a
    1000-UUID batch, 650 µs → 18.5 µs. The largest proportional gain of the typed languages,
    because `new_v7_batch` was building 1000 `uuid.UUID` instances around a single native call.
    Takes a `bytearray` specifically, not the general buffer protocol, which needs a
    `Py_buffer` that only entered the stable ABI in 3.11. *(PyPI)*
  - **Ruby** — `new_v6_batch_bytes`/`new_v7_batch_bytes`: **~11x**, 400 µs → 35 µs. No Rust
    change at all; the Runtime layer already had the native core's bytes as one binary `String`
    and was immediately slicing them into objects. *(RubyGems)*
  - **PHP** — `newV6BatchBytes`/`newV7BatchBytes`: **73x**, 2147 µs → 29.3 µs, the largest
    speedup in the repo. PHP's per-object construction cost is the steepest here, so removing
    it gains the most. *(Packagist)*
- **`Guid.Timestamp`** — a nullable extension property recovering the UTC timestamp embedded in
  a version 6 or 7 UUID, `null` for any other version. Written as a C# 14 `extension` block,
  the only form that can express a *property*: reading a timestamp out of bits the value
  already holds is a projection, not an action. It re-spells `UuidGenerator.GetTimestamp`,
  which keeps the logic, and a test pins the two to identical results on every version so they
  cannot drift. Works on any `Guid`, including one from `Guid.CreateVersion7()`. *(NuGet)*
- **Precompiled Ruby platform gems for Windows** — `x64-mingw-ucrt` and `aarch64-mingw-ucrt`,
  joining the existing linux and macOS ones. Windows is where the `Fiddle` fallback cost the
  most, and both architectures now get the compiled Magnus extension instead: measured on
  win-x64, `new_v4` in 406ns against Fiddle's 2407ns (**5.9x**) and `new_v7` 595ns against
  2759ns (**4.6x**); on win-arm64, 416ns against 2299ns (**5.5x**) and 621ns against 2474ns
  (**4.0x**). Windows-on-ARM had been the one mainstream platform still on the fallback.
  *(RubyGems)*
- **Fat platform gems.** A Magnus extension is bound to a single Ruby minor — there is no
  `abi3` equivalent to collapse that axis the way PyO3 does — so each platform gem now carries
  one compiled extension per supported Ruby, under `lib/hyperuuid/<minor>/`, and picks at
  `require` time. Ruby 3.4 and 4.0 today; anything outside that grid still resolves the
  universal zero-compile Fiddle gem automatically. *(RubyGems)*
- **Build provenance on every registry's package, not just NuGet's.** The gem, the wheel, the
  crate and the jars all shipped unsigned at 0.1.1 even though the native binaries inside them
  were signed. Each is now attested, and the gates that were missing on the way in are in
  place: the RubyGems job verifies all ten native artifacts before packing rather than
  trusting them, attests `pkg/*.gem` before the push so a failure stops the release while it is
  still reversible, then re-fetches each gem from the CDN and records attested-vs-served
  digests — turning "the registry stores an upload verbatim" into a per-release measurement
  instead of a belief. Verify with
  `gh attestation verify <file> --repo SkunkWerkx/HyperUuid`. *(all registries)*

### Changed

- **Both Windows Magnus extensions build the `gnullvm` Rust target rather than `gnu`** — same
  mingw-w64/UCRT ABI, LLVM instead of GCC. `rb-sys`'s own table maps `x64-mingw-ucrt` to
  `x86_64-pc-windows-gnu`, but that describes the toolchain it cross-compiles *with*, not an
  ABI requirement. The GCC target statically links libgcc; the LLVM one uses compiler-rt. Net
  effect on the shipped extension: **1,612,742 → 342,016 bytes, 79% smaller**, with `.text`
  landing next to the arm64 build's. Both the load into RubyInstaller's GCC-built Ruby and
  unwinding across the boundary (magnus turns Rust panics into Ruby exceptions, and this swaps
  the unwinder) were tested on real hardware rather than reasoned about. *(RubyGems)*
- **The release profile enables `lto = true` and `codegen-units = 1`.** Applies to builds of
  this repo — every binding's cdylib and the three extension features — and never to a
  downstream crates.io consumer, who gets their own workspace's profile. *(all packages)*
- **Ruby platform gems declare `required_ruby_version >= 3.4, < 4.1`**, narrower than the
  gemspec's own `>= 3.2`. A platform gem is only correct on the ABIs actually inside it, and
  RubyGems declining it is the only guard that runs *first* — a wrong-ABI extension must never
  be installed at all. On Windows it would at least fail to load cleanly, but Linux extensions
  don't link libruby, so one can load successfully against the wrong ABI and misbehave later.
  *(RubyGems)*

### Upgrade note

The new byte-returning forms are **only** faster when bytes are the destination — a database
bind parameter, a wire format, a bulk `COPY`. This inverts the usual "batch is faster" advice
and is worth reading before switching: in Python, filling and then constructing `uuid.UUID`
objects measures ~1210 µs against `new_v7_batch`'s 650 µs, roughly twice as slow, because the
extension's internal fast path beats anything callable from Python. Ruby and PHP are the same
story — slicing the returned string yourself only relocates the identical allocations into your
own code. If you want objects, keep using the existing batch methods. Nothing is deprecated.

## [0.1.1] — 2026-08-31

### Added

- **The Rust core is genuinely `#![no_std]`** under a new default-on `std` feature, so the
  `no-std` category the crate has published since 0.1.0 is compiler-enforced rather than
  asserted. Default-on rather than unconditional because the crate also builds the `cdylib`
  every other binding dlopens, and a linked artifact needs a `#[panic_handler]` only std
  supplies. *(`hyperuuid` crate)*
- **The crate no longer links `alloc` either**, and now carries the `no-std::no-alloc` category
  alongside `no-std`. *(`hyperuuid` crate)*
- LICENSE and README are now bundled inside every package that can carry them, so the license
  text ships with the artifact rather than only being named in its metadata.
  *(NuGet, Maven, PyPI, RubyGems)*
- CI gained a `check-no-std` job. Every other cargo invocation in the pipeline compiles the
  *std* configuration, so nothing else would notice a `use std::` creeping back into the core.
- New tests: `tests/v7_counter_race.rs` (the monotonic counter under 8-thread contention,
  including its first concurrent calls while the seed is landing) and per-item entropy
  placement assertions for both batch functions. *(`hyperuuid` crate)*

### Changed

- `NewV6Error`/`NewV7Error` implement `core::error::Error` instead of `std::error::Error`. These
  are the same trait — `std::error::Error` is a re-export — so callers see no difference.
  *(`hyperuuid` crate)*
- `getrandom`'s std-only error impl now rides the `std` feature instead of being pinned on
  unconditionally, so it is no longer forced into every consumer's dependency graph.
  *(`hyperuuid` crate)*
- `v7::now_v7` is compiled out without the `std` feature, joining the `wasm32` gate it already
  had. It is the only API that reads a system clock. *(`hyperuuid` crate)*
- `v7`'s process-global monotonic counter replaced `std::sync::OnceLock` with a lock-free seed
  fold over `core::sync::atomic`. The seed is *added* rather than stored, so it commutes with
  concurrent increments instead of clobbering one; the RFC 9562 §6.2 ordering guarantee is
  unchanged. *(`hyperuuid` crate)*
- `new_v6_batch`/`new_v7_batch` no longer allocate. Their single `getrandom` call now draws into
  the caller's own output buffer, and each item's entropy is moved to its final octets as the
  batch is written backwards — no scratch buffer on the heap, and no fixed stack frame either.
  Benchmarked against the previous implementation on the same machine: v7 slightly faster, v6
  within noise; the published batch speedups still hold. *(`hyperuuid` crate)*
- **Behavior change on an error path.** The batch functions draw entropy before assembling any
  UUID, so on `NewV6Error::Random`/`NewV7Error::Random` no UUID has been written but the front of
  `out` may hold partial bytes from the failed draw. Previously `out` was left untouched. Treat
  the buffer as clobbered rather than intact when a batch call returns `Err`.
  *(`hyperuuid` crate)*
- Documentation corrected across the bindings where it still described the retired ctypes
  backend, and each binding's README now stays about that binding. Registry and CI badges added
  to all of them. *(docs only, all packages)*

### Upgrade note

For essentially everyone this is a drop-in patch release: default features are on and the public
API is unchanged. The one exception is a `Cargo.toml` that already said
`default-features = false` — in 0.1.0 the crate had no default features, so that line was inert
and you still got a full std build. It now means what it says, and `v7::now_v7` disappears.
Either drop the line, or take the no_std path deliberately, which additionally needs a
`getrandom` custom backend and your own timestamps.

The C ABI is untouched — all 12 exported symbols are identical — so the seven non-Rust bindings
carry no API or behavior change beyond the packaging and documentation entries above.

## [0.1.0] — 2026-08-30

First coordinated release — all eight packages published together from one tag, and the first
tag to go out through the repository's own release pipeline rather than by hand. Full notes:
[v0.1.0 release](https://github.com/SkunkWerkx/HyperUuid/releases/tag/v0.1.0).

### Added

- One RFC 9562 UUID engine written in Rust and called directly from C#, Java, Go, Swift, Ruby,
  PHP and Python — published to crates.io, NuGet, Maven Central, PyPI, RubyGems, Packagist, and
  git tags for Swift and Go.
- v4 (random), v5 (deterministic, namespace-based), v6 and v7 (time-sortable, with a real
  monotonic counter for v7) on every binding, plus batch generation, timestamp extraction, and
  SQL Server byte-ordering for clustered `uniqueidentifier` columns.
- Go's module is tagged separately as `go/v0.1.0`, a Go modules requirement for a subdirectory
  module, pushed alongside the bare tag.

### Notes

- v1 and v3 are deliberately not implemented; RFC 9562 treats them as superseded by v6 and v5.
- Every binding was verified by pulling it from its real live registry into a fresh scratch
  project and generating a UUID — not by a passing CI job alone. The same `(DNS, "example.com")`
  input produces a byte-identical v5 UUID on all eight.
- Go is a deliberate control group and is *slower* per call than `google/uuid`, which is pure
  compiled Go with no FFI boundary. Reported rather than omitted.
- Known gaps at this release: free-threaded CPython (3.13t/3.14t) unsupported; WebAssembly proven
  for Rust and C# only; PHP skips win-arm64, which PHP itself has never shipped a native build
  for.

[Unreleased]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/SkunkWerkx/HyperUuid/releases/tag/v0.1.0
