# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hyperuuid.svg)](https://crates.io/crates/hyperuuid)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hyperuuid.svg)](https://packagist.org/packages/skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**One [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html)-compliant UUID engine, written once in Rust, called directly — not wrapped, not shimmed — from C#, Java, Go, Swift, Ruby, PHP, and Python.**

Every other polyglot ID library either reimplements the same generation logic per language (drift risk: seven codebases that can each get the bit-twiddling subtly wrong in different ways) or ships a server/sidecar process to generate IDs centrally (a network round-trip for something that should cost nanoseconds). HyperUuid does neither: a single Rust core is compiled once and reached from inside each language's own process — over a plain C ABI (`P/Invoke`, FFM, `cgo`/`purego`, `Fiddle`, PHP's `FFI`) or linked directly into the language VM as a native extension (PyO3 for CPython, Magnus for CRuby) — sharing the *same* address space, the *same* generation logic, the *same* test vectors, on every platform. No runtime bridge, no serialization layer, no embedded interpreter.

And the scoreboard, measured rather than asserted: **every language in this roster except Go generates UUIDs faster through HyperUuid than through its own platform's built-in facility** — 5.7x faster than `Guid.NewGuid()`, 2.8x faster than `SecureRandom.uuid`, 4.2x faster than CPython 3.14's own `uuid.uuid7()`, faster in PHP than a naive inline `random_bytes` v4 that validates nothing. Go is the one honest exception, and it's the exception that proves the measurements are real — see [the control group](#the-control-group-go) below for exactly why, because the reason is interesting.

## Quick start

```csharp
// C# — dotnet add package HyperUuid (nuget.org)
var id = UuidGenerator.NewV7();                     // time-sortable, RFC 9562 §6.2
var ts = UuidGenerator.V7Timestamp(id);              // recover the embedded timestamp
var batch = UuidGenerator.NewV7Batch(1000);          // 1000 IDs, one native call
```

```go
// Go — go get github.com/SkunkWerkx/HyperUuid/go
id, err := hyperuuid.NewV7()
ts, err := hyperuuid.V7Timestamp(id)
```

```ruby
# Ruby — gem "hyperuuid" (rubygems.org)
id = HyperUuid.new_v7
id.timestamp
```

Every binding follows the same shape — `new_v4`/`new_v5`/`new_v6`/`new_v7`, batch variants for v6/v7, timestamp extraction for v6/v7, and the RFC's `Nil`/`Max` constants. See each language's own README (linked in the table below) for its exact idiom and install instructions.

## The scoreboard

Cutting to the chase, by language — real numbers, no adjustment for story, full receipts in each binding's own README.

| Language | Generation vs. the platform's own call | The platform's own call |
| --- | --- | --- |
| [Rust](rust/) | **13-16x faster** (v6/v7) | the `uuid` crate |
| [Java](java/) | **5-9x faster** | `UUID.randomUUID()` |
| [C#](csharp/) | **5.7-8.3x faster** | `Guid.NewGuid()` |
| [Ruby](ruby/) | **1.9-2.9x faster** | `SecureRandom.uuid` |
| [Python](python/) | **1.6-4.2x faster** | `uuid.uuid4()`-`uuid7()` |
| [PHP](php/) | **1.9-2x faster** | a naive inline v4 (PHP core has no UUID call at all) |
| [Swift](swift/) | **faster outright, every call** | `Foundation.UUID()` |
| [Go](go/) | slower per call — [the control group](#the-control-group-go) | `google/uuid` |

- **[C#](csharp/)** — 5.7-8.3x faster than `Guid.NewGuid()`, zero allocation on every call, and the only way to get a v7 with a real monotonic counter before .NET 9 — even on .NET 9+, `Guid.CreateVersion7()` still has no counter at all.
- **[Java](java/)** — 5-9x faster than `UUID.randomUUID()`, against no real competition: `java.util.UUID` has never shipped v5, v6, or v7. Proven under GraalVM Native Image too, not just the JVM.
- **[Rust](rust/)** — this *is* the engine. 13-16x faster than the `uuid` crate on v6/v7, allocation-free, asserted by a real counting-allocator test, not just claimed.
- **[Swift](swift/)** — every call beats `Foundation.UUID()` outright, while also being the only way to get v5/v6/v7 in Swift at all — Foundation only ever does v4.
- **[Python](python/)** — a clean sweep since the PyO3 native backend: 1.6x faster than `uuid.uuid4()`, 2.5x faster than `uuid.uuid5()`, **4.2x faster than 3.14's own `uuid.uuid6()`/`uuid.uuid7()`**, and timestamp extraction — previously an outright loss — now 2.5-2.8x faster than `UUID.time`. On 3.9-3.13, where stdlib has no v6/v7 at all, it's not even a comparison.
- **[Ruby](ruby/)** — the same mechanism swap as Python, same result: **2.8-2.9x faster than `SecureRandom.uuid`** for v4 and fixed-timestamp v6/v7, 1.9x for v5 — and `SecureRandom.uuid` only ever does random v4 anyway. This README used to call the Ruby gap "structural, not a bug to fix"; the receipts in [ruby/README](ruby/) print the correction.
- **[PHP](php/)** — generation beats a *naive inline pure-PHP v4* (three lines of `random_bytes` + bit twiddling, no RFC validation) by 1.9-2x — the whole native round trip costs less than PHP-level byte fiddling — and timestamp extraction beats `ramsey/uuid` by 48-74x. Still the only zero-Composer-dependency way to generate v4-v7 in PHP at all.

**Regardless of where your language lands above:** if SQL Server is your RDBMS, [SQL Server ordering](#sql-server-ordering) below might be reason enough to reach for this on its own — the only practical way to mint a client-side ID on a frontier device and have it arrive already sorted for clustering, something `NEWSEQUENTIALID()` structurally can't do (server-side only) and something `IDENTITY(1,1)` can't do at all for a value — a many-to-many bridge table's composite key, most concretely — that needs to exist before the row does.

## How the wins happened — two crossing strategies

The scoreboard above wasn't free, and the mechanism behind it is the actual finding of this project. There is no single trick; there are two, chosen per language by measuring where each one's boundary cost actually lives:

**Direct FFI, where the crossing floor is already nanoseconds.** C#'s `P/Invoke` and Java's FFM cost single-digit nanoseconds per call; PHP's built-in `ext-ffi` measures ~105ns. At those floors the engine's own speed dominates, so those bindings call the C ABI directly — and any remaining slowness is *wrapper*, which gets dieted, not excused. PHP is the proof: its per-call cost dropped from ~570ns to ~305ns purely by deleting wrapper (static scratch reused across calls, inputs crossing as zero-copy `const char *` strings) — no mechanism change at all, and that diet alone is what pushed it past the naive inline v4.

**A native extension, where the FFI mechanism itself was the cost.** CPython's `ctypes` used to price every call at ~1µs of interpreted marshalling; Ruby's `Fiddle` still does, at ~1.6µs. No diet fixes that — the mechanism is the bill. So those two bindings link the Rust core *directly into the language VM* as an ordinary native extension (PyO3, Magnus), turning the crossing into a plain C function call. The two bindings part ways from there: PyO3 ships one `abi3` wheel per platform that covers every CPython 3.9+ on that platform, so `pip` always resolves a native wheel and the `ctypes` fallback was dropped entirely — nothing left for it to buy. Magnus has no stable-ABI story across Ruby versions the way `abi3` gives PyO3 (a precompiled platform gem is tied to one Ruby minor version), so Ruby's gems are *fat* — one compiled extension per supported Ruby minor inside each platform gem — and Ruby keeps a real `Fiddle` fallback for whatever falls outside that grid: auto-selected on any platform/Ruby combination without a prebuilt Magnus gem, which today means Ruby 3.2/3.3, musl, and anything exotic — with the same test suite running green against both backends and cross-backend agreement pinned by tests, so the fallback is never a second implementation that can drift.

Which leaves exactly one language where neither strategy applies — and that's not an accident.

## The control group: Go

Go's per-call numbers lose to `google/uuid`, and the reason is worth stating precisely, because it's what makes the rest of the scoreboard credible.

Go's boundary cost isn't marshalling — it's `runtime.cgocall` defending Go's concurrency model. Goroutines run on tiny growable stacks the C ABI can't execute on, so every cgo call switches to the OS thread's system stack and does scheduler bookkeeping (so a blocking C call can't starve the scheduler), then unwinds it all on return: ~100ns, structural, and not diet-able. There is no PyO3-for-Go, because the thing being paid for isn't an interface layer that could be replaced — it's the runtime itself. purego pays the same toll plus trampoline overhead (4-7 heap allocations per call that defeat escape analysis); the cgo backend (default on darwin/linux since 2026-08-27, purego remaining the fallback on Windows and `CGO_ENABLED=0`) is 3-4x faster than purego per call and is already the best available door.

And on the far side of that boundary sits the only competition in the roster that plays by the same rules as the Rust core: `google/uuid` is pure compiled Go with no boundary at all, written by people who understand scale — a handful of bit shifts for `.Time()`, no culture machinery, no interpreter. When the native work costs tens of nanoseconds, a ~100ns toll can never amortize on a single call. Every other language's built-in lost to HyperUuid across a *smaller* boundary; Go's won across a *larger* one because its stdlib is genuinely that good. That's the control group: it demonstrates the benchmarks reward real speed, not story.

So the honest guidance is narrower for Go than for any other binding here: reach for it in exactly two situations. Either SQL Server is your RDBMS and you cluster on `uniqueidentifier` columns — the [SQL-order transforms](#sql-server-ordering) come from the same verified core as every other language's, which matters precisely when a Go service is minting IDs into the same tables a C# service reads — or you're bulk-generating, where the batch doors divide the toll by N and Go's numbers land right next to everyone else's (see [benchmarks](#benchmarks)). For everything else, use `google/uuid` — including its own `.Time()` for timestamp extraction, which wins outright even against the cgo backend. The binding stays in the roster as a thought experiment and the control baseline the other seven languages are measured against; pretending it's more than that would cost this README its credibility.

## RFC 9562 coverage

| Version | Purpose | RFC section |
| --- | --- | --- |
| **v4** | Cryptographically random | §5.4 |
| **v5** | Deterministic, namespace + name, SHA-1 (cross-language interoperable — the same `(namespace, name)` pair produces the same UUID everywhere, including against Python's own `uuid.uuid5`) | §5.5 |
| **v6** | Time-ordered, v1-field-compatible reordering for sort/index locality — no monotonic counter | §5.6 |
| **v7** | Time-ordered, 48-bit Unix-ms timestamp + 26-bit monotonic counter + random bits — strictly increasing even under concurrent generation | §6.2 |
| **Nil** / **Max** | The all-zero and all-one special values | §5.9 / §5.10 |

v1 (classic time-based, leaks a MAC-derived node ID) and v3 (MD5 name-based) are deliberately not implemented — RFC 9562 itself treats them as superseded by v6 and v5 respectively, so building them would just be completeness theater. v6 and v7 both embed a timestamp `*Timestamp`/`*_timestamp` can recover on every binding; v6's Gregorian-epoch tick count tops out around the year 5236, well short of any language's own datetime ceiling, so unlike v7 it can never realistically raise an overflow decoding it.

## State of the union

Every language, on every platform, proven for real: `.github/workflows/ci.yml`'s `build-native` matrix builds the Rust core fresh on each of 6 real-hardware legs, then runs that language's actual test suite against that leg's freshly-built native library — not just that it compiles.

| Language | linux-x64 | linux-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [crates.io](https://crates.io/crates/hyperuuid) |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [NuGet](https://www.nuget.org/packages/HyperUuid) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | `go get` (git tag) |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | `.package(url:)` (git tag) |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [RubyGems](https://rubygems.org/gems/hyperuuid) |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | ✅ | — | [Packagist](https://packagist.org/packages/skunkwerkx/hyperuuid) |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [PyPI](https://pypi.org/project/hyperuuid/) |

PHP skips win-arm64 deliberately: PHP has never shipped a native Windows ARM64 build, so it always runs under x64 emulation there regardless of host CPU — already exercised for real by the win-x64 leg.

Every leg also builds the core as a `wasm32-wasip1` module and runs the Java, Ruby and Python suites a second time through their in-process wasm backends, and the Go suite the same way on the non-Windows legs (wasmtime-go is cgo throughout and has no win-arm64 build) — see [WebAssembly](#webassembly).

**Published:** every binding. C#/Java/Ruby/PHP/Python/Rust all go through a real package registry (NuGet, Maven Central, RubyGems, Packagist, PyPI, crates.io); Go and Swift have no registry to publish to in the first place — both resolve dependencies straight from a git tag (`go get`, `.package(url:, from:)`), which *is* their real, complete publish story, not a placeholder for one. The JVM binding is plain Java, not Kotlin — `kotlin-stdlib` would otherwise be a real transitive dependency for every consumer, unlike every other binding here — and its AOT story is proven the same way C#'s is: a local GraalVM Native Image smoke test (`java/aot-smoke-test/`, `./gradlew :aot-smoke-test:nativeRun`) produces a genuine standalone native binary, no JVM required to run it. PHP's `composer.json` lives at [the repo root](composer.json) rather than `php/` — Packagist requires the manifest at the top of the git repository it watches, with no monorepo subdirectory support; Swift's root [`Package.swift`](Package.swift) exists for the identical reason. Ruby ships as real precompiled RubyGems "platform gems" (the Magnus native extension, auto-selected for linux-x64/arm64, osx-x64/arm64, x64-mingw-ucrt and aarch64-mingw-ucrt, each gem fat across Ruby 3.4 and 4.0 since a Magnus extension is tied to one Ruby minor) with an automatic fallback to a universal, zero-compile pure-Fiddle gem for everything outside that grid — Ruby 3.2/3.3, musl. Go's embedded native libraries and Swift's `NativeLibs` are committed straight into git — unlike every registry above (Ruby's own packing step included), a plain `go get`/`.package(url:)` consumer has no packing step of its own, so the binaries have to actually live in the tree the consumer's tool reads.

## Provenance

Every published artifact across all eight bindings — the package itself where a registry
has one, and the native binaries underneath it either way — carries a GitHub build-provenance
attestation, checkable with `gh attestation verify`. Which flags that needs depends on where
the signing workflow physically lives, not on which registry the artifact ended up in:
artifacts signed directly inside this repo's own `release.yml` — the RubyGems gem, the PyPI
wheel, and the published NuGet package — verify with plain `--repo SkunkWerkx/HyperUuid`.
Artifacts signed by a reusable workflow hosted in `SkunkWerkx/.github` — the crates.io crate,
the Maven jar, the pre-push NuGet package, every native library (which is the entire
story for Go, Swift, and PHP, none of which has a package-level attestation of its own), and
the `wasm32-wasip1` module that rides inside the jar, the gems and `go/native/` —
need `--signer-repo SkunkWerkx/.github` added, or `--owner SkunkWerkx` in place of both
flags. Get it wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch.

See each binding's own README for its exact verify command and artifact:
[Rust](rust/#verifying-provenance), [C#](csharp/#native-binary-provenance),
[Java](java/#verifying-provenance), [Ruby](ruby/#verifying-provenance),
[Python](python/#verifying-provenance), [PHP](php/#verifying-provenance),
[Swift](swift/#verifying-provenance), [Go](go/#verifying-build-provenance).

## WebAssembly

There are two directions a binding can meet WebAssembly, and they have nothing in common
mechanically:

- **The binding runs inside wasm.** The whole consumer app is compiled to wasm (Blazor, a
  wasm32 Rust crate) and the Rust core has to be linked into that build. Two of eight do this
  today, and four are blocked for reasons that are the ecosystems', not this repo's (table
  below).
- **wasm runs inside the binding.** The process stays native; what changes is that the Rust
  core arrives as a `wasm32-wasip1` module and a wasm engine already available in that
  ecosystem runs it in-process. No `dlopen`, no per-platform binary, the same twelve C-ABI
  exports. Four of eight do this today, each behind the binding's existing backend switch,
  with the engine as an optional dependency the consumer adds only if they want it.

**The binding inside wasm: 2 of 8 proven, live today**

- **Rust** — the core crate itself runs correctly under `wasm32-wasip1` via [`wasmtime`](https://wasmtime.dev/): real WASI randomness (`random_get`), not just "compiles for the target." No clock: `now_v7` is compiled out on `wasm32`, and every v6/v7 door takes the host's timestamp, which is exactly how the four in-process backends below drive it. That build, with the crate's own `.cargo/config.toml` exporting `malloc`/`free`, *is* the `hyperuuid.wasm` those four ship.
- **C#** — genuinely turnkey. `dotnet add package HyperUuid` into a Blazor WebAssembly project is enough; no `<NativeFileReference>`, no hand-written P/Invoke. See [`csharp/README.md`](csharp/README.md)'s WebAssembly (Blazor) section for exactly how (two builds of the same assembly, an auto-imported `.targets` file supplying the native reference) and the one real caveat that survives it — a `wasm-opt`/rustc version-skew bug in every current `wasm-tools` SDK band, filed upstream as [dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858) with a verified workaround.

**wasm inside the binding: 4 of 8 proven, live today** — one artifact, `hyperuuid.wasm`,
built from the same crate with wasi-libc's `malloc`/`free` exported (a linker flag in
`rust/.cargo/config.toml`, no source change), shipped beside the native libraries in the
jar, the gems and the wheels, and committed under `go/native/` like the rest of Go's
binaries. CI runs every binding's full suite a second time through it on every leg.

| Binding | Engine | Select it | `new_v7`, one call | 1000-UUID batch | Native, same box |
| --- | --- | --- | ---: | ---: | --- |
| Java | [GraalWasm](https://www.graalvm.org/webassembly/) (`org.graalvm.polyglot:wasm`, `compileOnly`, never in the POM) | `-Dhyperuuid.backend=wasm`, or automatic when the jar has no native build for the platform | 420 ns on GraalVM CE 25 (JIT), 181 ns under Native Image, 3.1 µs on Temurin 25 (interpreter only, with a warning) | 15.9 µs (JIT) | 64 ns / 15.8 µs |
| Ruby | [wasmtime gem](https://github.com/bytecodealliance/wasmtime-rb) (development dependency only) | `HYPERUUID_WASM=1`, or automatic when no Fiddle library exists for the platform | 867 ns | 40.6 µs | ~450 ns / 24 µs |
| Python | [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py) (`pip install hyperuuid[wasm]`) | `HYPERUUID_WASM=1`, or automatic when the PyO3 extension fails to import (today every wheel and the sdist carry it, so in practice you set the variable; a pure-Python wheel would change that and is not built yet) | 6.2 µs (the bare crossing is 3.1 µs by going underneath wasmtime-py's public call, which re-fetches the function type per call and costs 38 µs) | 41 µs | 0.85 µs / 18.7 µs |
| Go | [wasmtime-go](https://github.com/bytecodealliance/wasmtime-go) (cgo throughout; no win-arm64 build) | `-tags hyperuuid_wasm` | 3.1 µs | 41 µs | 142 ns / 17.6 µs |

Two facts every one of those four shares, both learned the hard way in the same afternoon.
The host must take its buffers from the guest's own allocator: a host-picked offset past the
data segments looked free and was not, because dlmalloc claims the tail of the initial
memory on first use, and the next allocation overwrote a batch mid-buffer, intermittently,
depending on what it read back as a chunk header. And every call is serialized under a lock,
because neither a GraalWasm `Context` nor a wasmtime `Store` is safe for concurrent use; the
native backends stay lock-free. The per-call numbers are the engines' host-call overhead, not
wasm execution — the same module costs 0.9 µs from Ruby and 38 µs from Python's public API —
which is why the batch doors close most of the gap and the single-call doors do not. Every
number here was measured through the shipped binding, not a harness beside it; each
binding's README has its own section with the mechanics and the exact loop.

**1 of 8 proven once, then deliberately dropped:** Python's Pyodide path worked — the Rust core built as a genuine Emscripten *side module*, loaded at runtime in a real [Pyodide](https://pyodide.org/) session via plain `ctypes.CDLL` — but that proof-of-concept existed specifically to justify keeping the `ctypes` fallback backend alive. Once PyO3's `abi3` wheels made `ctypes` unnecessary for every real install (see [State of the union](#state-of-the-union)), the fallback — and the smoke test that proved it — was removed with it, nothing structural stops resurrecting it if a real Pyodide use case shows up. The wasmtime backend above is the other direction entirely: wasm inside CPython, not CPython inside wasm.

**The binding inside wasm: 5 of 8 investigated and currently blocked** — not from a lack of trying, from real gaps checked directly against each ecosystem's own tooling. Four of these five now have the inverse direction working (above); Swift and PHP have neither.

| Binding | Blocker | Why |
| --- | --- | --- |
| Go | Structural | Neither of Go's two native backends works: `cgo` is unavailable for any wasm target (architectural, not a flag), and `purego`'s own supported-platform list has no wasm entry — its whole model is runtime `dlopen`, which doesn't exist in WASM. `go:wasmexport`/`go:wasmimport` (Go 1.24+) let a Go wasm module talk to its host, not link a separately-compiled Rust wasm module. |
| Swift | Structural | swift.org ships real, official WASM SDKs since Swift 6.2 — but its own docs state dynamic linking "is not formally specified for `wasip1` triples and tooling for it is not available yet," and there's no documented static-lib-linking path to a Rust `.a` either (nothing like C#'s `NativeFileReference`). No wasm engine ships as a Swift package with a stable API either, so the inverse direction has nothing to stand on yet. |
| Ruby | Structural | `ruby.wasm` is official (bundled with CRuby since 3.2) but ships as one statically-linked component with no runtime library search — confirmed Fiddle itself only resolves libraries known at build time, not arbitrary runtime paths. The Magnus extension doesn't change this: `ruby.wasm` links C extensions statically at build time too. |
| PHP | Structural | The actively maintained WASM build (WordPress Playground's `@php-wasm`, not the stale `oraoto/pib`) loads extensions build-time/startup-only; no indication the FFI extension this binding needs is available there at all. There is no maintained wasm engine for PHP to embed either, so the inverse direction is closed too. |
| Java | Functional gap | No official OpenJDK path. The one Oracle-backed option, GraalVM Native Image's Web Image (`--tool:svm-wasm`), is explicitly labeled experimental and its feature list never mentions the Foreign Function & Memory API this binding is built on; neither third-party compiler (TeaVM, CheerpJ) supports FFM either. This one would mean rewriting the interop layer against experimental tooling with a real hole in it, not a packaging exercise. |

Go/Swift/Ruby/PHP hit the same underlying wall from four different angles: this project's whole architecture — one native core, every binding `dlopen`s the same compiled artifact at runtime — assumes dynamic library loading exists. WASM sandboxes generally don't have one. C#'s working story isn't an exception to that; it's a different mechanism entirely (link-time static linking via `NativeFileReference`), which happens to have a genuine, well-supported analog in .NET's tooling that these four don't (yet) have in theirs. Java's gap is different in kind — not the loading mechanism, but a real missing capability (FFM support) in the compilers that exist at all. The wasm-inside-the-binding backends sidestep that wall rather than climb it: the engine is the loader.

## Why not your platform's built-in UUID call?

Most languages *do* already have one — `Guid.NewGuid()`, `java.util.UUID.randomUUID()`, `uuid.uuid4()`, `SecureRandom.uuid`. HyperUuid isn't arguing you should never use those. It's for the specific, common situation where you need more than plain v4 randomness gives you:

1. **Time-sortable IDs with real ordering guarantees.** A v7 ID minted a microsecond after another one you generated on the same thread will sort after it — HyperUuid's monotonic counter (RFC 9562 §6.2 Method 1) guarantees that even under concurrent generation. Most stdlib v4 generators have no time-ordering story at all, and even a stdlib that *does* offer v7 (C#'s `Guid.CreateVersion7`, added in .NET 9) doesn't implement the counter, so two IDs minted in the same millisecond sort randomly relative to each other — exactly the index-fragmentation problem v7 adoption is meant to solve in the first place.
2. **One generation engine across a polyglot system.** If your API is C#, your batch jobs are Go, and your data pipeline is Python, plain per-language UUID libraries give you three independent implementations that all *should* agree bit-for-bit on RFC 9562 semantics but have no structural reason to. HyperUuid's v5 namespace UUIDs are verified in CI to match byte-for-byte with Python's own `uuid.uuid5` — because it's the literal same Rust code minting them everywhere, not three ports of the same spec.
3. **Batch throughput, and a byte-level door under it.** Need to backfill a million IDs? `NewV7Batch`/`new_v7_batch`/`NewV7BatchAt` (binding-dependent naming) shares one timestamp capture and one contiguous counter reservation across the whole batch instead of paying per-item overhead N times — 1.3-19.6x faster than the equivalent loop depending on binding. Underneath that, every binding now also exposes a raw-bytes form that constructs no UUID objects at all, which is worth up to **103x** in PHP and 35x in Python and lands every language within a few microseconds of the same floor ([numbers below](#skipping-object-construction-entirely)). Most stdlib UUID facilities have no batch API at all, let alone both.
4. **It's not slower for the trouble — it's faster.** Generation beats the platform's own call outright in every roster language except Go (see [the scoreboard](#the-scoreboard)).

The honest trade-off: this is one more native dependency to ship (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll`, or a prebuilt extension for the Python/Ruby fast paths) versus a UUID call that's already sitting in your standard library. If you only need plain v4 randomness and don't care about cross-language consistency, the stdlib call is simpler and that's a completely reasonable choice.

## SQL Server ordering

`V7ToSqlOrder`/`v7ToSqlOrder`/`v7_to_sql_order` (C#/Java/Go/Swift/Python's version-explicit naming — Ruby's `#to_sql_order`/PHP's `->toSqlOrder()` stay polymorphic across both versions, matching their existing `#timestamp`/`->timestamp()` convention) converts an RFC 9562-ordered version 7 UUID to the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order, and the `FromSqlOrder`/`fromSqlOrder`/`from_sql_order` counterpart converts it back. `V6ToSqlOrder`/`v6ToSqlOrder`/`v6_to_sql_order` do the same for version 6. `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed, non-sequential byte significance order. For v7, this moves the timestamp and counter (the two fields that determine creation order) into that comparison's most-significant bytes, and moves the trailing entropy, which carries no ordering information, into the least-significant ones as one intact block — the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use for C#. v6 has no counter, so only its 60-bit timestamp is sort-relevant; `clock_seq`/`node` (random per call, not a counter) get relocated the same way v7's entropy does. Both are computed once in the Rust core and exported over FFI so every binding gets them from the same verified source instead of a seven-times-reimplemented one. Verified against the real `System.Data.SqlTypes.SqlGuid` comparator (not a hand-rolled stand-in) in the C# test suite; every other binding verifies the same sort behavior against a comparator replicating `SqlGuid`'s documented byte order.

**When this actually matters:** SQL Server already ships its own native answer to GUID-clustering fragmentation — [`NEWSEQUENTIALID()`](https://learn.microsoft.com/sql/t-sql/functions/newsequentialid-transact-sql) — but it only runs *inside* SQL Server, as a column default at insert time. It can't help a frontier device (a mobile client, an edge node, anything generating records before it ever talks to the database) that needs to mint its own final row ID *before* that insert happens. This feature is what closes that gap: generate a v7 UUID on the device, convert it to SQL order, and the exact same value that was minted on the frontier is what lands in the clustered `uniqueidentifier` column — no round trip to the server first just to obtain a key, and no swapping identities between a client-side temp ID and a server-assigned real one.

That value arrives at the server somewhat out of strict minting order — real sync/queue/wire latency between device and database means insert order isn't quite generation order — so it's not as perfectly gap-free as a value SQL Server assigned to itself the instant before insert. It's still a bounded, mostly-monotonic disorder window, not the fully uniform randomness of a v4 GUID spread across the entire 128-bit space — a materially smaller number of page splits than random insert order produces, even with realistic sync delay. And it solves a problem `IDENTITY(1,1)` can't solve at all, not just less efficiently: an `IDENTITY` value doesn't exist until *after* the row is physically inserted, so anything that needs to reference that row before then — most concretely, a many-to-many bridge/junction table's composite key, built at the same time as the rows it links — has to insert first and come back for the generated key. A client-generated v7 ID needs no such round trip; it's already known at the moment it's needed everywhere, the bridge table included.

That's also the load-bearing condition for this feature to matter at all — a random or naively-generated GUID only fragments a clustered index because SQL Server always maintains a clustered index's physical sort order on every insert; nothing has to be "turned on" for that to happen, but nothing here helps if the column isn't the clustered key in the first place. And plenty of real-world SQL Server schemas sidestep the whole problem by never clustering on the GUID at all — an `IDENTITY`/sequence integer as the clustered key, with the GUID kept as an ordinary non-clustered unique column — which remains a completely reasonable choice if there's no frontier-generated-ID requirement driving the decision.

Meaningful only for a genuine version 6 or 7 UUID, respectively. **v6 caveat:** two v6 UUIDs minted at the same millisecond have identical timestamp bits — with no counter to break the tie, their relative order after conversion isn't guaranteed to match creation order, the same limitation plain RFC order already has for v6, not something this transform introduces; every binding's v6 sort-correctness test therefore only exercises strictly increasing timestamps. **Java caveat:** this is verified at the raw-byte level against .NET's own `Guid` wire format (which ADO.NET passes through unchanged), not against any specific JDBC driver's `uniqueidentifier` parameter binding — check your driver, or bind the bytes directly, before relying on it there. **Ruby/PHP caveat:** converting *back* from SQL order can't tell v6 and v7 apart from the version nibble alone (it sits at a different byte offset per version) — both bindings resolve this deterministically by checking a byte position/field that's provably collision-free between the two versions, but PHP's `fromSqlOrder()` also accepts an explicit `$version` argument for when you already know it.

## Benchmarks

The "high-performance, allocation-free" claim is measured, not just asserted — each binding with a mature benchmarking ecosystem has its own harness (BenchmarkDotNet, JMH, criterion, `testing.B`, package-benchmark, benchmark-ips, phpbench, pyperf), the numbers agree with each other, and the losses print next to the wins (all measured on linux-arm64; regenerate with the commands in each binding's README on your own hardware). Two measurement lessons are baked into the current numbers, both learned the hard way: PHP benchmarks must run with `XDEBUG_MODE=off` (a loaded Xdebug inflates everything ~14x uniformly — an earlier edition of the PHP tables was contaminated exactly that way, and says so), and time-based generation benchmarks carry explicit-timestamp variants because a wall-clock read is priced by the OS, not the binding — the WSL2 measurement box pays ~1µs per `clock_gettime(CLOCK_REALTIME)` where bare-metal Linux pays tens of nanoseconds.

### C# vs. `Guid.NewGuid()`

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*` (BenchmarkDotNet, `[MemoryDiagnoser]`):

| Method | Mean | Allocated |
| --- | ---: | ---: |
| `Guid.NewGuid()` | 630.26 ns | 0 B |
| `UuidGenerator.NewV4()` | 111.20 ns (**5.67x faster**) | 0 B |
| `UuidGenerator.NewV5()` | 131.31 ns (4.80x faster) | 0 B |
| `UuidGenerator.NewV6()` | 75.57 ns (**8.34x faster**) | 0 B |
| `UuidGenerator.NewV7()` | 82.10 ns (7.68x faster) | 0 B |

Every one of these is genuinely zero-allocation now — including `NewV5(Guid, string)`, which used to allocate 40 B encoding the name to UTF-8. Fixed by UTF-8-encoding into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique already used by the batch methods (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library).

### The interpreted tier, after the mechanism swap

The headline single-call numbers from the [Ruby](ruby/) and [PHP](php/) READMEs, worth restating here because they used to be this README's asterisks:

| Call | Time | The platform comparison |
| --- | ---: | --- |
| Ruby `HyperUuid.new_v4` (Magnus) | 459 ns | `SecureRandom.uuid` 1.29 µs — **2.8x faster** |
| Ruby `HyperUuid.new_v7` (Magnus, explicit ms) | 439 ns | — **2.9x faster** |
| PHP `HyperUuid::newV4()` | 308 ns | naive inline `random_bytes` v4 595 ns — **1.9x faster** |
| PHP `->timestamp()` (v7) | 380 ns | `ramsey/uuid` `getDateTime()` 18.3 µs — **48x faster** |

The zero-compile `Fiddle` fallback (`HYPERUUID_PURE=1`, and automatic on any platform without a prebuilt Magnus gem) keeps its own honest numbers in the [Ruby README](ruby/) — slower, mechanism-bound, and still fully supported. Python's own `ctypes` fallback is gone entirely; PyO3's `abi3` wheels made it redundant.

### Batch generation vs. an equivalent loop

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*`, `cargo bench` (`rust/benches/`), `go test -bench=. -benchmem ./go/...`:

| Binding | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| Rust — v7 | 61.7 µs | 16.9 µs | **3.6x** |
| Rust — v6 | 52.5 µs | 20.7 µs | 2.5x |
| C# — v7 | 93 µs | 24 µs | **3.9x** |
| C# — v6 | 84 µs | 27 µs | 3.1x |
| Go (cgo, darwin/linux default) — v7 | 147.3 µs | 33.6 µs | **4.4x** |
| Go (cgo, darwin/linux default) — v6 | 140.7 µs | 34.1 µs | 4.1x |
| Go (purego, Windows/`CGO_ENABLED=0`) — v7 | 574.8 µs | 29.4 µs | **19.6x** |
| Go (purego, Windows/`CGO_ENABLED=0`) — v6 | 565.9 µs | 33.0 µs | 17.2x |

Go's two native backends tell different stories here, and both are worth knowing before picking where to put your hot path (the wasmtime-go backend behind `-tags hyperuuid_wasm` has its own numbers in the [Go README](go/#webassembly-wasmtime-go)). Every individual purego call does 4-7 heap allocations (252-360 B/op via `go test -bench=. -benchmem`) — unlike Rust and C#, which are both genuinely zero-allocation per call — almost certainly `unsafe.Pointer` arguments crossing into purego's dynamically-generated call trampolines defeating Go's escape analysis. Batch generation collapses ~5000 of those allocations into 7, which is why purego's batch win looks the largest of any binding. Since 2026-08-27, darwin/linux builds default to a real cgo backend instead (`go/backend_cgo.go`, purego remaining the automatic fallback on Windows and any `CGO_ENABLED=0` build — see `go/README.md` for the full tradeoff and why Windows stays on purego unconditionally): cgo cuts per-call allocations to zero (one for v5, Go's own `[]byte(name)`) and individual calls 3-4x faster outright, which *shrinks* the batch-vs-individual gap rather than widening it, since batch generation was already amortizing most of the cost purego was paying per call. Net effect: cgo wins for individual-call-heavy workloads, purego and cgo land in the same place for batch-heavy ones — and batch is exactly where [the control group](#the-control-group-go) stops being the exception.

Rust's own allocation-free claim isn't just asserted either — `rust/tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v6/v7 and asserts zero allocations, then asserts the batch functions' scratch buffer *does* allocate, confirming it's the one deliberate exception documented in `v6.rs`/`v7.rs`.

### Skipping object construction entirely

The batch doors above still hand back a collection of the language's own UUID type. Every binding now also offers a form that hands back **raw RFC 9562-ordered bytes** — a destination buffer to fill, or one contiguous byte string — constructing no UUID objects at all. For the interpreted tier that turns out to matter far more than the batching did:

| Binding | batch → objects | raw bytes | speedup | API |
| --- | ---: | ---: | ---: | --- |
| PHP | 2260 µs | **21.9 µs** | **103x** | `newV7BatchBytes` |
| Python | 650 µs | **18.5 µs** | **35x** | `fill_v7(bytearray)` |
| Ruby | 370 µs | **24.1 µs** | **15x** | `new_v7_batch_bytes` |
| Swift | 77.0 µs | **16.0 µs** | **4.8x** | `fillV7(into: raw bytes)` |
| Java | 31.7 µs | **18.8 µs** | 1.7x | `fillV7(byte[])` |
| Go | 23.0 µs | **17.6 µs** | 1.3x, and 0 allocs | `FillV7BytesAt` |
| C# | 21.9 µs | **18.2 µs** | 1.2x, and 0 allocs | `FillV7(Span<byte>)` |

Read the right-hand column, not the speedup column: **every binding converges on roughly 18–24 µs per 1000 UUIDs**, because that is what the work actually costs. The native call was never the bottleneck in any of them. What varied was the price each language charges to wrap those 16000 bytes in a thousand objects — 2.2 ms of it in PHP, essentially none in Go.

Swift and Java are the instructive middle. Neither is an interpreted language, yet Swift gains 4.8x — it was paying for a result array plus a per-element `UUID(rfcBytes:)` construction, and dropping both matters. Java gains only ~1.5x, and the reason is visible in its own numbers: filling a `UUID[]` measures 27.7 µs against `newV7Batch`'s 26.1 µs, statistically identical, because `java.util.UUID` is two `long`s and every element has to be rebuilt regardless of who allocated the array. Only its `byte[]` form escapes that. The dividing line is not compiled-versus-interpreted; it is whether the language's UUID type is already RFC-ordered bytes.

That also explains why Go and C# barely move: they were already at the floor. Their win is allocation, not time — `FillV7` writes into a buffer you already own, so a hot loop allocates nothing at all. In Go and Swift it needs no per-element conversion either, since `uuid.UUID` is `[16]byte` and Foundation's `UUID` wraps `uuid_t`, both already in RFC order; C# and Java must rebuild each element because `System.Guid` is mixed-endian and `java.util.UUID` is two longs.

**One caveat, and it inverts the advice** — documented on every method in the three dynamic bindings, because getting it wrong is a pessimization: this is only faster if bytes are the *destination*. In Python, filling a buffer and then building `uuid.UUID` objects from it measures ~1210 µs, roughly twice as slow as `new_v7_batch`, because the extension constructs them through a faster path internally than anything callable from Python. Use the byte forms for a bind parameter, a wire format, or a bulk `COPY` — not as a step on the way to objects.

## Key features

- **RFC 9562 compliant** — correct version nibble and variant bits on every UUID, from every binding, because they all come from the same Rust core
- **One implementation, seven call sites** — no per-language reimplementation to drift out of sync; v5's SHA-1 hashing, v7's monotonic counter, and v6's Gregorian-epoch math are each written exactly once
- **Faster than the platform's own call** — in every roster language except Go, measured per binding with that ecosystem's own benchmark harness
- **Monotonically increasing v7** — a process-global counter (RFC 9562 §6.2 Method 1) guarantees strict ordering under concurrency, continued correctly across individual *and* batch calls
- **Batch generation** — `*Batch`/`*_batch` for v6/v7 amortizes timestamp capture, counter reservation, and the random-bytes fetch across the whole batch
- **SQL Server byte ordering** — `*ToSqlOrder`/`*_to_sql_order` for both v6 and v7, computed once in the Rust core and exported to every binding, verified against the real `System.Data.SqlTypes.SqlGuid` comparator
- **No runtime bridge** — direct FFI (`P/Invoke`, FFM, `cgo`/`purego`, `Fiddle`, PHP `FFI`) or the Rust core linked directly into the VM as a native extension (PyO3, Magnus), never a serialization protocol — with Ruby's zero-compile `Fiddle` fallback kept fully supported and test-verified against the Magnus fast path. The one deliberate exception is opt-in: Java, Ruby, Python and Go can each run the same core as a `wasm32-wasip1` module inside the process (GraalWasm, wasmtime) for a platform with no native build, still the same twelve exports, still the same test suite — see [WebAssembly](#webassembly)
- **Genuinely allocation-free where it counts** — verified with a counting allocator in Rust and `[MemoryDiagnoser]` in C#, not just claimed
- **AOT-friendly** — C# publishes cleanly under `PublishAot`; Java's JVM binding survives a real GraalVM Native Image build into a standalone native binary, no JVM required to run it
- **CI-proven, not CI-claimed** — 6 real-hardware platforms × 8 language/runtime targets, each running that language's actual test suite against a freshly-built native library on every dispatch, and the Java/Ruby/Python/Go suites a second time on every leg through a freshly-built `wasm32-wasip1` module

## Contributing

Pull requests and issues are welcome. `.github/workflows/ci.yml` builds and tests every binding on every platform — a PR should stay green there before merging.

## License

[MIT](LICENSE)
