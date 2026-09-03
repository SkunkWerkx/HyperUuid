# Changelog

All eight packages in this repository — the `hyperuuid` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.3.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/SkunkWerkx/HyperUuid/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/SkunkWerkx/HyperUuid/releases/tag/v0.1.0
