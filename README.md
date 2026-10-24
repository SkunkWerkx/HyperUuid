# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hyperuuid.svg)](https://crates.io/crates/hyperuuid)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperUuid/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperUuid/go)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperUuid?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperUuid/tags)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hyperuuid.svg)](https://packagist.org/packages/skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**One [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html)-compliant UUID engine, written once in Rust, called directly — not wrapped, not shimmed — from C#, Java, Go, Swift, Ruby, PHP, and Python.**

Every other polyglot ID library either reimplements the same generation logic per language (drift risk: seven codebases that can each get the bit-twiddling subtly wrong in different ways) or ships a server/sidecar process to generate IDs centrally (a network round-trip for something that should cost nanoseconds). HyperUuid does neither: a single Rust core is compiled once and reached from inside each language's own process — over a plain C ABI (`P/Invoke`, FFM, `cgo`, `Fiddle`, PHP's `FFI`) or linked directly into the language VM as a native extension (PyO3 for CPython, Magnus for CRuby) — sharing the *same* address space, the *same* generation logic, the *same* test vectors, on every platform. No runtime bridge, no serialization layer, no embedded interpreter.

And the scoreboard, measured rather than asserted: **in every language in this roster except Go, generating a UUID through HyperUuid is as fast as the platform's own facility or faster, and usually several times faster** — 16x faster than `Foundation.UUID()`, 7.9x faster than `Guid.NewGuid()`, 5.5x faster than `SecureRandom.uuid`, 3.8x faster than CPython 3.14's own `uuid.uuid7()`, and in PHP ahead of a naive inline `random_bytes` v4 that validates nothing. Go is the one honest exception, and it's the exception that proves the measurements are real — see [the control group](#the-control-group-go) below for exactly why, because the reason is interesting.

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

Every binding follows the same shape — `new_v4`/`new_v5`/`new_v7`, a v7 batch variant, v7 timestamp extraction, the SQL Server order conversions, the RFC's `Nil`/`Max` constants, and a probe that says whether the native core loaded and which version it is (`UuidGenerator.IsAvailable`/`NativeVersion` in C#, and the same pair in each language's idiom), so a consumer with a fallback gates on the probe rather than catching a load failure. See each language's own README (linked in the table below) for its exact idiom and install instructions.

## The scoreboard

Cutting to the chase, by language — real numbers, no adjustment for story, full receipts in each binding's own README.

| Language | Generation vs. the platform's own call | The platform's own call |
| --- | --- | --- |
| [Swift](swift/) | **11-16x faster** | `Foundation.UUID()` |
| [C#](csharp/) | **7.5-7.9x faster** | `Guid.NewGuid()` |
| [Ruby](ruby/) | **2.1-5.5x faster** | `SecureRandom.uuid` |
| [Python](python/) | **3.4-4.2x faster** | `uuid.uuid4()`-`uuid7()` |
| [Java](java/) | **2.8-5.1x faster** | `UUID.randomUUID()` |
| [Rust](rust/) | **1.7-2.4x faster** | the `uuid` crate |
| [PHP](php/) | **1.2-1.4x faster** | a naive inline v4 (PHP core has no UUID call at all) |
| [Go](go/) | slower per call — [the control group](#the-control-group-go) | `google/uuid` |

- **[C#](csharp/)** — 7.5-7.9x faster than `Guid.NewGuid()` for v4 and v7, zero allocation on every call, and the only way to get a v7 with a real monotonic counter before .NET 9 — even on .NET 9+, `Guid.CreateVersion7()` still has no counter at all.
- **[Java](java/)** — 2.8-5.1x faster than `UUID.randomUUID()`, against no real competition: `java.util.UUID` has never shipped v5 or v7. Proven under GraalVM Native Image too, not just the JVM.
- **[Rust](rust/)** — this *is* the engine. 1.7-2.4x faster than the `uuid` crate on v4, v5 and v7, and 9x at reading a timestamp back out, allocation-free, asserted by a real counting-allocator test, not just claimed.
- **[Swift](swift/)** — every call beats `Foundation.UUID()` by 11-16x, while also being the only way to get v5/v7 in Swift at all — Foundation only ever does v4.
- **[Python](python/)** — every call ahead of stdlib's own C-accelerated ones since the PyO3 native backend: 3.4x faster than `uuid.uuid4()`, 4.2x faster than `uuid.uuid5()`, 3.8x faster than 3.14's own `uuid.uuid7()`, and timestamp extraction 1.6x faster than `UUID.time` as a `datetime`, 2.4x as a plain integer. On 3.11-3.13, where stdlib has no v7 at all, it's not even a comparison.
- **[Ruby](ruby/)** — the same mechanism swap as Python, same result: **5.5x faster than `SecureRandom.uuid`** for v4, 3.8-4.0x for v7, 2.1x for v5 — and `SecureRandom.uuid` only ever does random v4 anyway.
- **[PHP](php/)** — a v4 costs less than a *naive inline pure-PHP v4* (three lines of `random_bytes` + bit twiddling, no RFC validation), by 1.4x, and a v7 by 1.2x — the whole native round trip for less than the price of PHP-level byte fiddling — and timestamp extraction beats `ramsey/uuid` by 45x. Still the only zero-Composer-dependency way to generate v4, v5 and v7 in PHP at all.

**Regardless of where your language lands above:** if SQL Server is your RDBMS, [SQL Server ordering](#sql-server-ordering) below might be reason enough to reach for this on its own — the only practical way to mint a client-side ID on a frontier device and have it arrive already sorted for clustering, something `NEWSEQUENTIALID()` structurally can't do (server-side only) and something `IDENTITY(1,1)` can't do at all for a value — a many-to-many bridge table's composite key, most concretely — that needs to exist before the row does.

## How the wins happened — two crossing strategies

The scoreboard above wasn't free, and the mechanism behind it is the actual finding of this project. There is no single trick; there are two, chosen per language by measuring where each one's boundary cost actually lives:

**Direct FFI, where the crossing floor is already nanoseconds.** C#'s `P/Invoke` and Java's FFM cost on the order of ten nanoseconds per call; PHP's built-in `ext-ffi` measured ~105ns. At those floors the engine's own speed dominates, so those bindings call the C ABI directly — and any remaining slowness is *wrapper*, which gets dieted, not excused. PHP is the proof: its per-call cost roughly halved (~570ns to ~305ns, where the diet was measured) purely by deleting wrapper (static scratch reused across calls, inputs crossing as zero-copy `const char *` strings) — no mechanism change at all, and that diet alone is what brought it level with the naive inline v4.

**A native extension, where the FFI mechanism itself was the cost.** CPython's `ctypes` used to price every call at ~1µs of interpreted marshalling; Ruby's `Fiddle` still charges over a microsecond. No diet fixes that — the mechanism is the bill. So those two bindings link the Rust core *directly into the language VM* as an ordinary native extension (PyO3, Magnus), turning the crossing into a plain C function call. The two bindings part ways from there: PyO3 ships one `abi3` wheel per platform that covers every CPython 3.11+ on that platform, beside two for free-threaded CPython (`cp314t`, and PEP 803's `abi3t` for 3.15+, where the extension runs with the GIL off), so `pip` always resolves a native wheel and the `ctypes` fallback was dropped entirely — nothing left for it to buy. Magnus has no stable-ABI story across Ruby versions the way `abi3` gives PyO3 (a precompiled platform gem is tied to one Ruby minor version), so Ruby's gems are *fat* — one compiled extension per supported Ruby minor inside each platform gem — and Ruby keeps a real `Fiddle` fallback for whatever falls outside that grid: auto-selected on any platform/Ruby combination without a prebuilt Magnus gem, which today means Ruby 3.3, a Ruby newer than the release, Intel macOS, and any platform outside the eight RIDs — shipped only in the universal gem, the last resort — with the same test suite running green against both backends and cross-backend agreement pinned by tests, so the fallback is never a second implementation that can drift.

Which leaves exactly one language where neither strategy applies — and that's not an accident.

## The control group: Go

Go's per-call numbers lose to `google/uuid`, and the reason is worth stating precisely, because it's what makes the rest of the scoreboard credible.

Go's boundary cost isn't marshalling — it's `runtime.cgocall` defending Go's concurrency model. Goroutines run on tiny growable stacks the C ABI can't execute on, so every cgo call switches to the OS thread's system stack and does scheduler bookkeeping (so a blocking C call can't starve the scheduler), then unwinds it all on return: ~40ns on the box measured here, structural, and not diet-able. There is no PyO3-for-Go, because the thing being paid for isn't an interface layer that could be replaced — it's the runtime itself. The binding links the core straight into the binary and passes UUIDs to and from C by value, so a call allocates nothing — cgo with the toll and nothing else is already the best available door.

And on the far side of that boundary sits the only competition in the roster that plays by the same rules as the Rust core: `google/uuid` is pure compiled Go with no boundary at all, written by people who understand scale — a handful of bit shifts for `.Time()`, no culture machinery, no interpreter. When the native work costs tens of nanoseconds, a toll of the same size again can never amortize on a single call. Every other language's built-in lost to HyperUuid across a *smaller* boundary; Go's won across a *larger* one because its stdlib is genuinely that good. That's the control group: it demonstrates the benchmarks reward real speed, not story.

So the honest guidance is narrower for Go than for any other binding here: reach for it in exactly two situations. Either SQL Server is your RDBMS and you cluster on `uniqueidentifier` columns — the [SQL-order transforms](#sql-server-ordering) come from the same verified core as every other language's, which matters precisely when a Go service is minting IDs into the same tables a C# service reads — or you're bulk-generating, where the batch doors divide the toll by N and Go's numbers land right next to everyone else's (see [benchmarks](#benchmarks)). For everything else, use `google/uuid` — including its own `.Time()` for timestamp extraction, which wins outright even against this binding. The binding stays in the roster as a thought experiment and the control baseline the other seven languages are measured against; pretending it's more than that would cost this README its credibility.

## RFC 9562 coverage

| Version | Purpose | RFC section |
| --- | --- | --- |
| **v4** | Cryptographically random | §5.4 |
| **v5** | Deterministic, namespace + name, SHA-1 (cross-language interoperable — the same `(namespace, name)` pair produces the same UUID everywhere, including against Python's own `uuid.uuid5`) | §5.5 |
| **v7** | Time-ordered, 48-bit Unix-ms timestamp + 26-bit monotonic counter + random bits — strictly increasing even under concurrent generation (see [v7 ordering, precisely](#v7-ordering-precisely)) | §6.2 |
| **v8** | Only as the [SQL Server order](#sql-server-ordering) of a v7: the same 122 bits in the order `uniqueidentifier` sorts by | §5.8 / §6.11 |
| **Nil** / **Max** | The all-zero and all-one special values | §5.9 / §5.10 |
| **Inspection** | Version, variant (NCS / RFC 9562 / Microsoft / Future), and a one-call "is this an RFC 9562 UUID of version N" guard, in RFC order or straight from [SQL Server order](#sql-server-ordering) | §4.1 / §4.2 |

v1 (classic time-based, leaks a MAC-derived node ID), v3 (MD5 name-based) and v6 (v1's fields reordered to sort) are deliberately not implemented. RFC 9562 treats v3 as superseded by v5, and v6 exists for systems that already speak v1: "Systems that do not involve legacy UUIDv1 SHOULD use UUIDv7 (Section 5.7) instead" (§5.6). A v7 embeds a timestamp `*Timestamp`/`*_timestamp` can recover on every binding; a v1 or v6 minted elsewhere still reports its version, and carries no timestamp here.

Inspection is layout knowledge, so it lives in the core like everything else. Every binding exposes the same three doors through it, each in its own spelling: `Version`, `Variant` and `IsRfc`. The version-agnostic `GetTimestamp` applies the same guard: a 7 nibble under another variant is no RFC version, so it carries no timestamp. The guard is what a wrap constructor or a deserializer runs on every value: `IsRfc(id, 7)` is true only for the RFC variant with version nibble 7. A platform's own `Guid.Version` or `UUID.version()` reads the nibble alone, so a Microsoft-variant value with a 7 in it passes that check.

### v7 ordering, precisely

The v7 counter is one process-wide sequence, seeded once and advanced atomically by every call and every batch, so concurrent callers in the same millisecond still get strictly increasing values. It is 26 bits wide and never resets, so it wraps back to 0 every 67,108,864 UUIDs. That has three consequences:

- **A batch is always in strictly increasing order.** A batch that straddles the wrap stamps its UUIDs from the wrap on with the supplied timestamp plus one millisecond, as RFC 9562 §6.2 allows on counter overflow. No embedded timestamp is ever more than a millisecond ahead of the supplied one.
- **A batch takes at most 67,108,864 UUIDs** (`MaxV7Batch` and its spellings). A larger one would have to reuse counter values within one millisecond, so every binding refuses it with an argument error before allocating anything.
- **Two individual calls in the same millisecond on either side of the wrap sort in reverse.** Individual calls share no state but the counter, so they have nothing to roll forward with. This happens at most once per 67,108,864 calls, and only when the two calls land in the same millisecond.
- **A batch's rolled-forward tail runs ahead of whatever comes next in the real millisecond.** The roll-forward orders one batch, not the stream: the next batch or call in the same millisecond starts its counter just past the wrap, carries the supplied timestamp, and so sorts before the previous batch's tail stamped a millisecond later. Same cause as the point above, and just as rare.

The timestamp is the caller's, trusted as given: nothing detects a clock that goes backwards, and a v7 minted after one sorts before the values minted ahead of it.

## Platform support

Every language, on every platform, proven for real: `.github/workflows/ci.yml`'s `build-native` matrix builds the Rust core fresh on each of 5 real-hardware legs, then runs that language's actual test suite against that leg's freshly-built native library — not just that it compiles. A second job does the same for the two musl RIDs inside real Alpine containers, on the language's own official `*-alpine` image. Intel macOS has no leg of its own; see the osx-x64 note under the table.

| Language | linux-x64 | linux-arm64 | linux-musl-x64 | linux-musl-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [crates.io](https://crates.io/crates/hyperuuid) |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [NuGet](https://www.nuget.org/packages/HyperUuid) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `go get` (git tag) |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `.package(url:)` (git tag) |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | Fiddle, built | ✅ | ✅ | ✅ | [RubyGems](https://rubygems.org/gems/hyperuuid) |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | — | [Packagist](https://packagist.org/packages/skunkwerkx/hyperuuid) |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [PyPI](https://pypi.org/project/hyperuuid/) |

The cells that are not a plain ✅, and why each is deliberate:

- **osx-x64 (Intel macOS): built, and tested at the core only.** There is no Intel macOS CI leg. The library is cross-compiled on the Apple silicon runner, attested, and shipped in every package exactly as before, and the Rust core's own suite runs on it there under Rosetta 2, which is why that row keeps its ✅. No binding's suite runs on Intel macOS; each still runs on Apple silicon against the same source. Ruby there installs the universal gem and runs on Fiddle, since no precompiled gem is built for it. The leg took 37 minutes against 8 on Apple silicon, for hardware Apple stopped selling in 2023 on runners GitHub retires by Fall 2027.
- **PHP on win-arm64.** PHP has never shipped a native Windows ARM64 build, so it runs as an x64 process there regardless of host CPU and loads the win-x64 library — already exercised for real by the win-x64 leg.

Three bindings link the core into the consumer's executable where they can, instead of loading a shared library at run time. Swift always does, on every platform in the table, so there is nothing to deploy beside the executable: its musl cells are Swift's static Linux SDK, which CI proves with a smoke executable built and run in Swift's own containers (that SDK ships no XCTest), and the same mechanism compiles the binding to WebAssembly; see [WebAssembly](#webassembly). A platform with no prebuilt core fails to compile. Go always does, through cgo on Linux, macOS and Windows, so a binary carries ~20 KB of core for its own platform and loads nothing; building it takes a C compiler, and `CGO_ENABLED=0` or any other target is a compile error — except WebAssembly under TinyGo, which links the same way; see [WebAssembly](#webassembly). C# does for a Native AOT publish, on every RID, so the result is one executable. Everything else — the JIT, the JVM, the interpreters — loads the shared library, as before.

All three reach past the table that way. **iOS and Mac Catalyst** load no libraries at all, so C#, Swift and Go link the core into the app there, from static libraries cross-compiled in the same job as the rest: C# for `ios-arm64`, `iossimulator-arm64`, `maccatalyst-arm64` and `maccatalyst-x64`, where a .NET iOS, MAUI or Mac Catalyst app needs nothing but the package reference; Swift for the three arm64 ones, as an XCFramework; and Go for all four, chosen by build tag, since Go builds every one of them as `GOOS=ios`. CI's `test-apple-mobile` job builds them on a Mac from that run's archives: C# and Swift run in an iOS simulator and as a Mac Catalyst process, Go's suite runs in the simulator, and each links an iOS device build. See [`csharp/README.md`](csharp/README.md#platform-support), [`swift/README.md`](swift/README.md#linking-and-deployment) and [`go/README.md`](go/README.md#ios-and-mac-catalyst).

**Android**, arm64 and x86_64, is reached by the same three bindings, so that a .NET MAUI app has the core on every platform MAUI targets and a Go or Swift Android build links it as on any other platform. C# carries a shared library cross-built with the NDK at API level 21, its segments aligned to the 16 KB pages Android 15 devices may use and Google Play requires: an app on CoreCLR, .NET 11's Android runtime, loads it out of the APK like any Linux library, and a Native AOT publish links it in from a static archive like every other RID. Go links its own pair of archives under `GOOS=android` with the NDK's clang, and Swift links them from its artifact bundle with the [Swift SDK for Android](https://www.swift.org/documentation/articles/swift-sdk-for-android-getting-started.html) (Swift 6.3+, API 28+). CI's `test-android` job builds the C# app both ways for both RIDs from a package packed in that run, cross-builds the Go and Swift suites, runs everything x86_64 in an emulator whose image uses 16 KB pages, and links or inspects the arm64 builds. Java, Python, Ruby and PHP do not reach Android. See [`csharp/README.md`](csharp/README.md#platform-support), [`go/README.md`](go/README.md#android) and [`swift/README.md`](swift/README.md#linking-and-deployment).

The musl libraries are built so that they depend on musl's libc and nothing else — the unwinder is linked statically — which is what lets them load on a bare `alpine` or `python:alpine` image with no `libgcc` installed. The glibc libraries need glibc 2.34 or newer.

**Runtime floors** follow upstream support: a version that has reached end of life is not a floor. Today that is .NET 11, JDK 25, Go 1.26, Python 3.11, Ruby 3.3 and PHP 8.2, and the musl job runs the PHP, Ruby and Python suites on those oldest versions as well as the newest. Swift's floor is 6.2 for a different reason: it is the first release whose package manager can link a static library, which is how the binding reaches musl and WebAssembly at all. CI runs it on 6.2 as well as 6.4. The crate's own floor is Rust 1.85 (`rust-version` in its manifest, edition 2024's own floor), and CI builds the library and the `no_std` shared library on exactly that toolchain.

**Published:** every binding. C#/Java/Ruby/PHP/Python/Rust all go through a real package registry (NuGet, Maven Central, RubyGems, Packagist, PyPI, crates.io); Go and Swift have no registry to publish to in the first place — both resolve dependencies straight from a git tag (`go get`, `.package(url:, from:)`), which *is* their real, complete publish story, not a placeholder for one. The JVM binding is plain Java, not Kotlin — `kotlin-stdlib` would otherwise be a real transitive dependency for every consumer, unlike every other binding here — and its AOT story is proven the same way C#'s is: a local GraalVM Native Image smoke test (`java/aot-smoke-test/`, `./gradlew :aot-smoke-test:nativeRun`) produces a genuine standalone native binary, no JVM required to run it. PHP's `composer.json` lives at [the repo root](composer.json) rather than `php/` — Packagist requires the manifest at the top of the git repository it watches, with no monorepo subdirectory support; Swift's root [`Package.swift`](Package.swift) exists for the identical reason. Ruby ships as real precompiled RubyGems "platform gems" (the Magnus native extension, auto-selected on seven RIDs — `x86_64-linux-gnu`, `aarch64-linux-gnu`, `x86_64-linux-musl`, `aarch64-linux-musl`, `arm64-darwin`, `x64-mingw-ucrt` and `aarch64-mingw-ucrt` — each gem fat across Ruby 3.4 and 4.0 since a Magnus extension is tied to one Ruby minor, and carrying no Fiddle library) with an automatic fallback to a universal, zero-compile pure-Fiddle gem for everything outside that grid — Ruby 3.3, a Ruby newer than the release, Intel macOS, and any other platform. A third kind, the `hyperuuid-wasm` gem, carries the same extension prebuilt for ruby.wasm, where `rbwasm build` links it into the interpreter ([Ruby in the browser](ruby/README.md#ruby-in-the-browser)). Go's and Swift's static libraries are committed straight into git — unlike every registry above (Ruby's own packing step included), a plain `go get`/`.package(url:)` consumer has no packing step of its own, so the binaries have to actually live in the tree the consumer's tool reads.

## Provenance

Every file a release ships verifies from the tag alone. One release attestation, signed by
this repository's own `release.yml` at the tag, names every registry package (the `.nupkg` as
packed and as nuget.org serves it, the gems, the wheels, the crate, the Maven files) and every
native library committed for Go, Swift and PHP, and its predicate is the release inventory:
every file's path, size, SHA-256, the commit it was built from and the repository that signed
its build provenance. One command, any file:

```sh
gh attestation verify <file> --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Underneath it, every artifact also carries GitHub build provenance from the run that made it,
and there the flags depend on where the signing workflow lives, not on the registry. The gems,
`inventory.json` and the published NuGet package are signed inside this repository's
`release.yml` and verify with plain `--repo SkunkWerkx/HyperUuid`. The crate, the jar, the
wheels, the pre-push NuGet package, every native library (the whole story for Go, Swift and
PHP, which have no package of their own) and the `wasm32-wasip1` module inside the jar are
signed by reusable workflows in `SkunkWerkx/.github`: the crate and the Maven files at the
tagged commit, the wheels, the natives and the module at the preparation commit on master they
were built from. Add `--signer-repo SkunkWerkx/.github` (and `--source-digest <commit>` to pin
either), or `--owner SkunkWerkx` in place of both. Get the
signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which reads like a
bad signature but is only an identity mismatch. PyPI additionally holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel, filed by the
publish action and shown on each file's PyPI page.

The gates run on the way in, not just on the way out: `stage-native-binaries.yml` verifies each
native library's attestation before committing it for the Go, Swift and PHP consumers, the gem
job verifies every native artifact it packs before building a gem, every package is attested
and preserved before anything is uploaded, and the upload itself is a second, deliberate
dispatch of `release.yml` from those preserved bytes ([RELEASING.md](.github/RELEASING.md)), so
a failed check stops the release while nothing is yet irreversible. Each binding's README shows
the command on its own artifact: [Rust](rust/#verifying-provenance),
[C#](csharp/#native-binary-provenance), [Java](java/#verifying-provenance),
[Ruby](ruby/#verifying-provenance), [Python](python/#verifying-provenance),
[PHP](php/#verifying-provenance), [Swift](swift/#verifying-provenance),
[Go](go/#verifying-build-provenance).

## WebAssembly

WebAssembly meets a binding in one of two directions, and they share nothing mechanically:

- **The core runs as wasm inside the binding.** The process stays native, and a wasm engine
  the ecosystem already has runs the core as a `wasm32-wasip1` module in-process. No binding
  ships this today: the column says, per binding, why not.
- **The binding is compiled to wasm.** The whole consumer app becomes a wasm module
  (Blazor, a `wasm32` Rust crate, Python under Pyodide) and the Rust core has to be linked
  into that build by the ecosystem's own toolchain.

Where each of the eight stands, today:

| Binding | Core as wasm inside the binding | Binding compiled to wasm |
| --- | --- | --- |
| Rust | Not applicable: the crate *is* the core. | **Yes.** Runs under [wasmtime](https://wasmtime.dev/) on `wasm32-wasip1` with real WASI randomness (`random_get`), not merely "compiles for the target." No clock: `now_v7` is compiled out on `wasm32`, and every v7 door takes the host's timestamp. `wasm32-unknown-unknown` needs `getrandom`'s `wasm_js` feature on the consumer's side; see [`rust/README.md`](rust/README.md#webassembly). |
| C# | Not built. | **Yes, on .NET 11+.** `dotnet add package HyperUuid` into a Blazor WebAssembly project is enough, given the `wasm-tools` workload (the build stops with `HYPERUUID002` without it): no `<NativeFileReference>`, no hand-written P/Invoke, and the shipped `.targets` also applies the exception-handling translation .NET 11 needs. Proven in headless Chromium by `csharp/HyperUuid.WasmSmokeTest`; see [`csharp/README.md`](csharp/README.md#webassembly-blazor). |
| Java | Not built. The jar reaches the core through FFM downcalls into the native library it bundles for the platform, and a JVM on a platform without one has no core. | Blocked. No Java-to-wasm compiler supports the Foreign Function & Memory API this binding is built on: GraalVM's Web Image (`--tool:svm-wasm`) is labeled experimental and never lists it, and neither TeaVM nor CheerpJ has it. Loading the core as a module of its own and calling it through Web Image's JavaScript interop would work, but that is a second binding with its own glue, not this one; revisit when Web Image is mature and can call or link native code. |
| Ruby | Not built. The `wasmtime` gem ships precompiled only for platforms the universal gem already carries a native library for; anywhere else it compiles from source with a Rust toolchain, so a wasm backend would reach nothing the Fiddle backend does not. | **Yes, through `rbwasm build`.** ruby.wasm links extensions into the interpreter when it is built, so a browser app lists `hyperuuid-wasm` instead of `hyperuuid` in the Gemfile it hands `rbwasm build` (ruby_wasm 2.10+, Ruby 3.4 or 4.0). That gem carries the same Magnus extension prebuilt for `wasm32-wasip1`, one archive per Ruby minor, so the consumer needs no Rust toolchain, and it shares an interpreter with HyperCast's `hypercast-wasm`. CI links it into both minors' interpreters and runs a smoke test under Node and in headless Chrome; see [`ruby/README.md`](ruby/README.md#ruby-in-the-browser). |
| Python | Not built. Only platform wheels are published, each carrying the PyO3 extension, and no sdist, so there is no install a wasm backend could fill in for. | **Yes, on [Pyodide](https://pyodide.org/) 314.x.** `await micropip.install("hyperuuid")` in the browser: the PyO3 extension built for Pyodide's Emscripten target ships to PyPI as a twenty-fifth wheel (`cp311-abi3-pyemscripten_2026_0_wasm32`, ~74 KB), the same native backend with no JavaScript bridge. CI runs the binding's whole pytest suite inside Pyodide, under Node and in headless Chrome. Each Pyodide ABI year needs its own wheel; see [`python/README.md`](python/README.md#in-the-browser-pyodide). |
| Go | Not built. The binding links the core in on every platform it supports, so there is no platform for a wasm backend to fill in for. | **Yes, through [TinyGo](https://tinygo.org) 0.42+.** `tinygo build -target=wasm` links the core in from the module's own `staticlib/wasm` archive (the `wasm32-wasip1` build), about 43 KB with the binding at `-opt=z`, and TinyGo's `wasm_exec.js` supplies its randomness from `crypto.getRandomValues`; CI runs a smoke program in headless Chrome on every PR. Stock Go cannot: its wasm toolchain links Go code only, so `GOOS=wasip1`/`js` is a compile error. |
| Swift | Not built. No wasm engine ships as a Swift package with a stable API, so there is nothing to embed. | **Yes, on Swift 6.2+.** `swift build --swift-sdk` with swift.org's WebAssembly SDK links the core in as a static library (a SwiftPM binary target with a `wasm32-unknown-wasip1` archive), so there is nothing to load. The result is a plain `wasm32-wasip1` command module, so it also runs in the browser through a WASI shim such as [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim), which supplies `random_get` from `crypto.getRandomValues`. CI runs the binding's suite under WasmKit on Swift 6.4, a smoke executable on 6.2, and the same smoke executable in headless Chrome; see [`swift/README.md`](swift/README.md#webassembly). |
| PHP | Not built. There is no maintained wasm engine PHP can embed. | Proven, not shipped. The `ext-php-rs` extension spike loads as a side module into WordPress Playground's prebuilt `@php-wasm` runtime (PHP 8.5, JSPI) and runs in node and headless Chrome; shipping it means a module per PHP minor and a ~4 GB build image in CI, so it waits on demand. [php/README.md](php/README.md#webassembly) has the full recipe and the two upstream issues it found ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800), [wordpress-playground#4377](https://github.com/WordPress/wordpress-playground/issues/4377)). |

The two directions are blocked, where they are blocked, for different reasons. Compiling a
binding to wasm needs the ecosystem's toolchain to link a Rust static library into its own
wasm build. .NET has a supported mechanism for exactly that (`NativeFileReference`, which this
package's `.targets` injects for you), so does Swift from 6.2 (a SwiftPM binary
static-library target), so does TinyGo (cgo, linked by `wasm-ld`), and so does Pyodide,
which loads a CPython extension module built as an Emscripten side module just as CPython
loads a native one, and so does ruby.wasm, whose `rbwasm build` links each gem's extension
into the interpreter it makes. WordPress Playground's PHP does too — it loads a Zend
extension built as an Emscripten side module — which is proven for PHP but not shipped (see
its row); stock Go does not. Java's gap is different in kind: the loading mechanism is not the problem, the compilers
that exist have no FFM.

## Why not your platform's built-in UUID call?

Most languages *do* already have one — `Guid.NewGuid()`, `java.util.UUID.randomUUID()`, `uuid.uuid4()`, `SecureRandom.uuid`. HyperUuid isn't arguing you should never use those. It's for the specific, common situation where you need more than plain v4 randomness gives you:

1. **Time-sortable IDs with real ordering guarantees.** A v7 ID minted a microsecond after another one you generated on the same thread will sort after it — HyperUuid's monotonic counter (RFC 9562 §6.2 Method 1) guarantees that even under concurrent generation. Most stdlib v4 generators have no time-ordering story at all, and even a stdlib that *does* offer v7 (C#'s `Guid.CreateVersion7`, added in .NET 9) doesn't implement the counter, so two IDs minted in the same millisecond sort randomly relative to each other — exactly the index-fragmentation problem v7 adoption is meant to solve in the first place.
2. **One generation engine across a polyglot system.** If your API is C#, your batch jobs are Go, and your data pipeline is Python, plain per-language UUID libraries give you three independent implementations that all *should* agree bit-for-bit on RFC 9562 semantics but have no structural reason to. HyperUuid's v5 namespace UUIDs are verified in CI to match byte-for-byte with Python's own `uuid.uuid5` — because it's the literal same Rust code minting them everywhere, not three ports of the same spec.
3. **Batch throughput, and a byte-level door under it.** Need to backfill a million IDs? `NewV7Batch`/`new_v7_batch`/`NewV7BatchAt` (binding-dependent naming) shares one timestamp capture and one contiguous counter reservation across the whole batch instead of paying per-item overhead N times — 1.3-27x faster than the equivalent loop depending on binding. Underneath that, every binding now also exposes a raw-bytes form that constructs no UUID objects at all, which is worth up to **20x** in Ruby and 15x in Python and lands every language within a few microseconds of the same floor ([numbers below](#skipping-object-construction-entirely)). Most stdlib UUID facilities have no batch API at all, let alone both.
4. **It's not slower for the trouble — it's faster.** Generation beats the platform's own call outright in every roster language except Go (see [the scoreboard](#the-scoreboard)).

The honest trade-off: this is one more native dependency to ship (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll`, or a prebuilt extension for the Python/Ruby fast paths) versus a UUID call that's already sitting in your standard library. If you only need plain v4 randomness and don't care about cross-language consistency, the stdlib call is simpler and that's a completely reasonable choice.

## SQL Server ordering

`V7ToSqlOrder`/`v7ToSqlOrder`/`v7_to_sql_order` (C#/Java/Go/Swift/Python; Ruby's `#to_sql_order` and PHP's `->toSqlOrder()` are methods on the value) converts an RFC 9562 version 7 UUID to the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order, and the `FromSqlOrder`/`fromSqlOrder`/`from_sql_order` counterpart converts it back. `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed, non-sequential byte significance order. The converted value is an RFC 9562 **version 8**. RFC 9562 defines v7's layout in network byte order and sends any other byte order to version 8 (§6.11), so the conversion writes an 8 and the RFC variant where SQL Server's text form and a .NET `Guid` show them, and fills the other 122 bits in the comparator's order: the timestamp and counter (the two fields that determine creation order) in the most-significant bytes, then the trailing 48 random bits, which carry no ordering information, as one intact block in the least-significant ones. That is the permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) uses for C#, with an 8 where SequentialGuid keeps the 7. It is computed once in the Rust core and exported over FFI so every binding gets them from the same verified source instead of a seven-times-reimplemented one. Verified against the real `System.Data.SqlTypes.SqlGuid` comparator (not a hand-rolled stand-in) in the C# test suite; every other binding verifies the same sort behavior against a comparator replicating `SqlGuid`'s documented byte order.

**When this actually matters:** SQL Server already ships its own native answer to GUID-clustering fragmentation — [`NEWSEQUENTIALID()`](https://learn.microsoft.com/sql/t-sql/functions/newsequentialid-transact-sql) — but it only runs *inside* SQL Server, as a column default at insert time. It can't help a frontier device (a mobile client, an edge node, anything generating records before it ever talks to the database) that needs to mint its own final row ID *before* that insert happens. This feature is what closes that gap: generate a v7 UUID on the device, convert it to SQL order, and the exact same value that was minted on the frontier is what lands in the clustered `uniqueidentifier` column — no round trip to the server first just to obtain a key, and no swapping identities between a client-side temp ID and a server-assigned real one.

That value arrives at the server somewhat out of strict minting order — real sync/queue/wire latency between device and database means insert order isn't quite generation order — so it's not as perfectly gap-free as a value SQL Server assigned to itself the instant before insert. It's still a bounded, mostly-monotonic disorder window, not the fully uniform randomness of a v4 GUID spread across the entire 128-bit space — a materially smaller number of page splits than random insert order produces, even with realistic sync delay. And it solves a problem `IDENTITY(1,1)` can't solve at all, not just less efficiently: an `IDENTITY` value doesn't exist until *after* the row is physically inserted, so anything that needs to reference that row before then — most concretely, a many-to-many bridge/junction table's composite key, built at the same time as the rows it links — has to insert first and come back for the generated key. A client-generated v7 ID needs no such round trip; it's already known at the moment it's needed everywhere, the bridge table included.

**Why version 8.** The 8 keeps the value honest wherever it is read as a UUID: SQL Server's text, a .NET `Guid` and a driver's UUID type all show a version 8, a custom layout, so nothing reads a version 7 timestamp out of bytes that no longer hold one there. It costs nothing: a v7 and a v8 have the same 122 free bits, so every v7 converts and the conversion back is exact. The conversion refuses only a value that isn't a v7 going in, or doesn't carry the layout's 8 and variant coming back, with the binding's invalid-argument error.

That's also the load-bearing condition for this feature to matter at all — a random or naively-generated GUID only fragments a clustered index because SQL Server always maintains a clustered index's physical sort order on every insert; nothing has to be "turned on" for that to happen, but nothing here helps if the column isn't the clustered key in the first place. And plenty of real-world SQL Server schemas sidestep the whole problem by never clustering on the GUID at all — an `IDENTITY`/sequence integer as the clustered key, with the GUID kept as an ordinary non-clustered unique column — which remains a completely reasonable choice if there's no frontier-generated-ID requirement driving the decision.

**Binding it outside .NET.** SQL Server reads a UUID's text form mixed-endian and stores the first three groups byte-reversed. Every driver outside .NET that was tested sends its own UUID type the same way it sends text. So binding the converted value as a UUID, or as a string, stores bytes other than the ones `to_sql_order` produced, and the column does not sort by creation. Against SQL Server 2022 that held for mssql-jdbc 13.6, go-mssqldb 1.11 (by default), pyodbc 5.3, mssql-python 1.15, pymssql 2.4, tiberius 0.13, pdo_sqlsrv 5.13 and tiny_tds 3.4. Bind the 16 bytes as binary instead: SQL Server's binary-to-`uniqueidentifier` conversion keeps them as they are. Reading has the same trap in reverse, because those drivers hand back a UUID with its first three groups reversed. Select `CAST(col AS binary(16))`, or use the driver's raw-bytes getter, to get the bytes the SQL Server layout reads take. Each binding's README shows its own form. .NET is the exception: ADO.NET sends a `Guid` as its `ToByteArray()` bytes, so the `Guid` binds as it is.

**Reading a value still in SQL order.** A value read back from a `uniqueidentifier` column (as raw bytes outside .NET, per the paragraph above) can be validated and its timestamp read without permuting it back first. `Version`, `IsRfc`, `V7UnixMillis`, `V7Timestamp` and `GetTimestamp` each take a layout (`UuidLayout.SqlServer` and its spellings), and the core reads the permuted bytes in place: one native call per question instead of a permute and then a read. In SQL Server layout `Version` answers 7, the version a value was converted from. Bytes without the layout's 8 and variant read as version 0 with no timestamp. The layout is the caller's to know, not something a value can reveal: bytes in RFC order can happen to carry an 8 and the variant where the layout puts them (a random v4 does one time in 16), so keep the layout alongside the value, the way a column or a converter already does. For the same reason, a conversion handed raw bytes in the wrong order succeeds about one time in 16: converting a value already in SQL order to SQL order again, or converting a v7 still in RFC order back from it. C#'s `Guid` overloads refuse both every time, because a `Guid` puts its version nibble where the layout puts the 8: a SQL-ordered `Guid` reads as a version 8, and an RFC-ordered v7 shows its 7 there.

**The layout is persisted data.** A value written in SQL order sits in a clustered index, so moving one bit in the layout would silently reorder every existing row. Every property test (round trips, sort order) would still pass. The [conformance corpus](corpus/) pins the exact bytes instead, in both directions, along with the refusals. An oracle derives them in bit strings from RFC 9562's field table, and they are cross-checked against a port of SequentialGuid's permutation, which derives them from .NET's mixed-endian `Guid` layout and differs only in the 8.

## Benchmarks

The "high-performance, allocation-free" claim is measured, not just asserted — each binding with a mature benchmarking ecosystem has its own harness (BenchmarkDotNet, JMH, criterion, `testing.B`, package-benchmark, benchmark-ips, phpbench, pyperf), the numbers agree with each other, and the losses print next to the wins (all measured on linux-x64, an Intel Core i9-11900H; regenerate with the commands in each binding's README on your own hardware). Three measurement rules are baked into these numbers. PHP benchmarks run with `XDEBUG_MODE=off` (a loaded Xdebug inflates everything ~14x uniformly). Every case is warmed before it is timed, so a first call's library load is never in a figure. And **the operating system prices part of every one of these calls** — the wall-clock read and the random source — so a ratio with either on one side of it belongs to the machine as much as to the code: where a clock read costs a microsecond instead of tens of nanoseconds, every comparison against a platform call that reads the clock internally looks several times better than it does here. The time-based benchmarks carry explicit-timestamp variants for that reason, and the figures printed are from a machine where those calls are cheap.

### C# vs. `Guid.NewGuid()`

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*` (BenchmarkDotNet, `[MemoryDiagnoser]`):

| Method | Mean | Allocated |
| --- | ---: | ---: |
| `Guid.NewGuid()` | 292.84 ns | 0 B |
| `UuidGenerator.NewV4()` | 36.94 ns (**7.93x faster**) | 0 B |
| `UuidGenerator.NewV5()` | 69.27 ns (4.23x faster) | 0 B |
| `UuidGenerator.NewV7()` | 38.88 ns (7.53x faster) | 0 B |

Every one of these is genuinely zero-allocation now — including `NewV5(Guid, string)`, which used to allocate 40 B encoding the name to UTF-8. Fixed by UTF-8-encoding into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique already used by the batch methods (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library).

### The interpreted tier, after the mechanism swap

The headline single-call numbers from the [Ruby](ruby/) and [PHP](php/) READMEs, worth restating here because they used to be this README's asterisks:

| Call | Time | The platform comparison |
| --- | ---: | --- |
| Ruby `HyperUuid.new_v4` (Magnus) | 191 ns | `SecureRandom.uuid` 1.06 µs — **5.5x faster** |
| Ruby `HyperUuid.new_v7` (Magnus, explicit ms) | 264 ns | — **4.0x faster** |
| PHP `HyperUuid::newV4()` | 224 ns | naive inline `random_bytes` v4 308 ns — **1.4x faster** |
| PHP `->timestamp()` (v7) | 293 ns | `ramsey/uuid` `getDateTime()` 13.2 µs — **45x faster** |

The zero-compile `Fiddle` fallback (`HYPERUUID_PURE=1`, and automatic on any platform without a prebuilt Magnus gem) keeps its own honest numbers in the [Ruby README](ruby/) — slower, mechanism-bound, and still fully supported. Python's own `ctypes` fallback is gone entirely; PyO3's `abi3` wheels made it redundant.

### Batch generation vs. an equivalent loop

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*`, `cargo bench` (from `rust/`), `go test -bench=. -benchmem ./...` (from `go/`):

| Binding | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| Rust — v7 | 25.2 µs | 4.40 µs | **5.7x** |
| C# — v7 | 37.5 µs | 6.06 µs | **6.2x** |
| Go — v7 | 69.8 µs | 6.97 µs | **10.0x** |

Go's batch win is the largest of the three because its per-call toll is the largest: nearly all of a single call is `runtime.cgocall` (see [the control group](#the-control-group-go)), and a batch pays it once for 1000 UUIDs. The individual calls allocate nothing (v5 aside, which pays for Go's own `[]byte(name)`), so the win is time alone — and batch is exactly where the control group stops being the exception.

Rust's own allocation-free claim isn't just asserted either — `rust/tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v7 and around the batch function, and asserts zero allocations for all of them. Batches used to be the one documented exception; they now draw their entropy through the caller's own buffer, which is what lets the crate build without `alloc` at all.

### Skipping object construction entirely

The batch doors above still hand back a collection of the language's own UUID type. Every binding now also offers a form that hands back **raw RFC 9562-ordered bytes** — a destination buffer to fill, or one contiguous byte string — constructing no UUID objects at all. For the interpreted tier that turns out to matter far more than the batching did:

| Binding | batch → objects | raw bytes | speedup | API |
| --- | ---: | ---: | ---: | --- |
| Ruby | 202 µs | **4.9 µs** | **41x** | `new_v7_batch_bytes` |
| Python | 132 µs | **3.7 µs** | **36x** | `fill_v7(bytearray)` |
| PHP | 67.1 µs | **4.2 µs** | **16x** | `newV7BatchBytes` |
| Java | 9.3 µs | **3.6 µs** | 2.6x | `fillV7(byte[])` |
| Go | 7.0 µs | **3.7 µs** | 1.9x, and 0 allocs | `FillV7BytesAt` |
| C# | 6.1 µs | **3.8 µs** | 1.6x, and 0 allocs | `FillV7(Span<byte>)` |
| Swift | 3.7 µs | **3.8 µs** | level | `fillV7(into: raw bytes)` |

Read the right-hand column, not the speedup column: **every binding converges on roughly 3.6–4.9 µs per 1000 UUIDs**, because that is what the work actually costs. The native call was never the bottleneck in any of them. What varied was the price each language charges to wrap those 16000 bytes in a thousand objects — 200 µs of it in Ruby, 130 µs in Python, essentially none in Swift.

Java is the instructive middle. It is not an interpreted language, yet it gains 2.6x where C# gains 1.6x and Swift nothing, and the reason is visible in its own numbers: filling a `UUID[]` measures 12.3 µs against `newV7Batch`'s 9.3 µs, no cheaper for skipping the array allocation, because `java.util.UUID` is two `long`s and every element has to be rebuilt regardless of who allocated the array. Only its `byte[]` form escapes that. The dividing line is not compiled-versus-interpreted; it is whether the language's UUID type is already RFC-ordered bytes.

That also explains why Go, C# and Swift barely move: they are already at or near the floor. (Swift was not, through 0.2: `newV7Batch` paid for a scratch buffer and a per-element `UUID(rfcBytes:)` construction, and cost five times what it has since 0.3.0's carrier rewrite.) Their win is allocation, not time — `FillV7` writes into a buffer you already own, so a hot loop allocates nothing at all. In Go and Swift it needs no per-element conversion either, since `uuid.UUID` is `[16]byte` and Foundation's `UUID` wraps `uuid_t`, both already in RFC order; C# and Java must rebuild each element because `System.Guid` is mixed-endian and `java.util.UUID` is two longs.

**One caveat, and it inverts the advice** — documented on every method in the three dynamic bindings, because getting it wrong is a pessimization: this is only faster if bytes are the *destination*. In Python, filling a buffer and then building `uuid.UUID` objects from it measures ~890 µs, more than six times as slow as `new_v7_batch`, because the extension constructs them through a faster path internally than anything callable from Python. Use the byte forms for a bind parameter, a wire format, or a bulk `COPY` — not as a step on the way to objects.

## Key features

- **RFC 9562 compliant** — correct version nibble and variant bits on every UUID, from every binding, because they all come from the same Rust core
- **One implementation, seven call sites** — no per-language reimplementation to drift out of sync; v5's SHA-1 hashing, v7's monotonic counter and the SQL Server permutation are each written exactly once
- **Faster than the platform's own call** — in every roster language except Go, measured per binding with that ecosystem's own benchmark harness
- **Monotonically increasing v7** — a process-global counter (RFC 9562 §6.2 Method 1) guarantees strict ordering under concurrency, continued correctly across individual *and* batch calls, and every batch stays in order across the counter's wrap ([precisely](#v7-ordering-precisely))
- **Batch generation** — `NewV7Batch`/`new_v7_batch` amortizes timestamp capture, counter reservation, and the random-bytes fetch across the whole batch
- **SQL Server byte ordering** — `V7ToSqlOrder`/`v7_to_sql_order`, as an RFC 9562 version 8, computed once in the Rust core and exported to every binding, verified against the real `System.Data.SqlTypes.SqlGuid` comparator, and validated and read in place without converting back
- **Inspection** — version, variant and a one-call RFC guard, in either layout, from the core
- **One conformance corpus** — [`corpus/*.json`](corpus/) pins v5 derivation, the v7 field layout, the SQL Server layout and its refusals, timestamp extraction and inspection. The Rust suite and every binding's suite replay it, and `corpus/oracle.py` re-derives every vector from code that shares nothing with the core
- **No runtime bridge** — direct FFI (`P/Invoke`, FFM, `cgo`, `Fiddle`, PHP `FFI`) or the Rust core linked directly into the VM as a native extension (PyO3, Magnus), never a serialization protocol — with Ruby's zero-compile `Fiddle` fallback kept fully supported and test-verified against the Magnus fast path
- **Genuinely allocation-free where it counts** — verified with a counting allocator in Rust and `[MemoryDiagnoser]` in C#, not just claimed
- **AOT-friendly** — C# publishes cleanly under `PublishAot`, with the core linked into the executable rather than sitting beside it; Java's JVM binding survives a real GraalVM Native Image build into a standalone native binary, no JVM required to run it
- **CI-proven, not CI-claimed** — 5 real-hardware platforms plus two musl RIDs in Alpine containers × 8 language/runtime targets (Intel macOS is cross-built and tested at the core only), each running that language's actual test suite against a freshly-built native library on every dispatch

## Layout

```
rust/       the core: twelve uuid_* exports plus hyperuuid_version, allocation-free and no_std; a cdylib (and two for Android) and fifteen static archives (four for iOS and Mac Catalyst, two for Android)
corpus/     the conformance vectors every suite replays, and the independent oracle that derives them
csharp/     the .NET 11 binding: UuidGenerator over LibraryImport, AOT smoke test, Blazor wasm
java/       the JDK 25+ binding: FFM downcalls, Native Image smoke test
python/     the 3.11+ binding: PyO3 native extension (abi3 and free-threaded wheels, Pyodide in the browser)
swift/      the SwiftPM binding: the core linked in as a static library on every platform
go/         the Go binding: the core linked in through cgo, and under TinyGo in the browser
ruby/       the 3.3+ binding: Magnus extension + Fiddle fallback
php/        the 8.2+ binding: ext-ffi (an ext-php-rs extension spike, proven in the browser, unshipped)
```

## Why "Hyper"

The SkunkWerkx Hyper* series — HyperUuid, [HyperCast](https://github.com/SkunkWerkx/HyperCast), [HyperTabular](https://github.com/SkunkWerkx/HyperTabular) — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## Contributing

Pull requests and issues are welcome. `.github/workflows/ci.yml` builds and tests every binding on every platform — a PR should stay green there before merging.

## License

[MIT](LICENSE)
