# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperUuid?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperUuid/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Foundation's `UUID()` initializer only ever produces random v4 UUIDs — no v5, no v6, no v7 (a [Swift Forums pitch](https://forums.swift.org/t/pitch-uuid-v7-other-improvements/85427) to add v7 is still at the pitch stage). This package is the whole RFC, today.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `hyperuuid` Rust core through `@convention(c)` function
pointers — no runtime bridge, no shim. On every platform it supports — Linux (glibc and
musl), macOS, Windows and WebAssembly — the core is linked into your executable as a static
library, so there is nothing to load at run time and nothing to deploy beside it.

```swift
import HyperUuid

let id = try UuidGenerator.newV4()
let id2 = try UuidGenerator.newV5(namespace: Namespaces.dns, name: "example.com")
let id3 = try UuidGenerator.newV6()
let id4 = try UuidGenerator.newV7()

let created = try UuidGenerator.v7Timestamp(id4) // recover the embedded UTC Date
let maybeCreated = try UuidGenerator.getTimestamp(id4) // Date?, nil instead of assuming id4 is v6/v7
let sqlOrdered = try UuidGenerator.v7ToSqlOrder(id4) // byte order SQL Server's uniqueidentifier needs to sort by creation order

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
let batch = try UuidGenerator.newV7Batch(count: 1000)
```

Returns Foundation's `UUID`. `Namespaces.dns`/`url`/`oid`/`x500` are RFC 9562
Section 6.6's well-known namespaces; `WellKnownUuids.nilUUID`/`maxUUID` are the
§5.9/§5.10 special values. `UuidGenerator.v6Timestamp(_:)`/`v7Timestamp(_:)` recover
the embedded UTC `Date` from a version 6 or 7 UUID respectively; `newV6(_:)`/`newV7(_:)`
accept a `Date` directly in place of `newV6(unixMillis:)`/`newV7(unixMillis:)`'s raw
millisecond count (a date before 1970, or one that isn't a finite instant, throws
`timestampOutOfRange` like any other timestamp the field can't hold), and
`getTimestamp(_:)` is the version-agnostic counterpart to
`v6Timestamp`/`v7Timestamp` — it checks the version nibble itself and returns `nil`
for anything but a genuine v6/v7 UUID, instead of assuming the caller already knows.
`UuidGenerator.newV6Batch(count:unixMillis:)`/`newV7Batch(count:unixMillis:)` generate
`count` UUIDs sharing one timestamp capture and one native call, instead of `count`
of each. `UuidGenerator.v7ToSqlOrder(_:)`/`v7FromSqlOrder(_:)` convert a version 7
UUID to and from the byte order SQL Server's `uniqueidentifier` needs on the wire to
sort by creation order — computed once in the native Rust core rather than
reimplemented in Swift, and verified there (and independently against the real
`System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite).
`v6ToSqlOrder(_:)`/`v6FromSqlOrder(_:)` do the same for version 6, though
same-millisecond v6 UUIDs aren't guaranteed to sort correctly afterward — v6 has no
counter, so `clock_seq`/`node` (not the timestamp) decide ties, the same pre-existing
RFC 9562 v6 limitation plain order already has. `UuidGenerator.nativeVersion()` reports
the linked core's own `major.minor.patch` (`hyperuuid_version`), so a caller can prove
the binary it resolved is the one this binding was built against before minting the first
UUID — see [Linking and deployment](#linking-and-deployment).

## Why not Foundation's `UUID()`?

There's no real comparison to make for v5/v6/v7 — Foundation's `UUID` type has never generated anything but random v4:

1. **Full RFC 9562 coverage.** v4, v5 (SHA-1 namespace-based, verified in CI to match Python's own `uuid.uuid5` byte-for-byte), v6, and v7 — none of which `UUID()` can produce at all.
2. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency — not something `UUID()` needs, since it has no v7 to order in the first place.
3. **Batch generation.** `newV7Batch(count:unixMillis:)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead N times.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — a Swift service and a service written in any of those languages agree byte-for-byte on the same `(namespace, name)` pair.

The honest trade-off: this package links a prebuilt native library per platform instead of being pure Swift, and every call is `throws` where `UUID()` never fails. If plain v4 randomness is all you need, `UUID()` is simpler, has no native dependency, and is already there — a completely reasonable choice.

## Destination-buffer fills

`fillV6`/`fillV7` write into storage you already own instead of allocating a fresh array per call, over either an `inout [UUID]` or an `UnsafeMutableRawBufferPointer`:

```swift
var dst = [UUID](repeating: UUID(), count: 1000)
try UuidGenerator.fillV7(into: &dst, unixMillis: ms)   // reuse dst; nothing allocated
try UuidGenerator.fillV7(into: &dst)                    // the same, at the current time
```

Swift gets the good version of this, alongside Go. Foundation's `UUID` wraps `uuid_t` — 16 bytes already in RFC 9562 order — so the native core writes the whole batch straight into the array's storage with **no per-element conversion**. (C# and Java must rebuild every element, because their UUID types aren't RFC byte order.) That soundness condition is checked with a `precondition` on `MemoryLayout<UUID>`'s size and stride rather than assumed.

`swift package benchmark`, p50 wall clock, 1000 UUIDs per op (linux-x64, an Intel Core i9-11900H, Swift 6.3):

| Benchmark | p50 | mallocs |
| --- | ---: | ---: |
| `newV7` x1000 individually | 76 µs | 0 |
| `newV7Batch(count: 1000)` | **10 µs** | 1 |
| `fillV7(into: [UUID])` | 12 µs | 1 |
| `fillV7(into: raw bytes)` | 12 µs | 1 |

There is no gap between `newV7Batch` and the fills: `newV7Batch` allocates its result array and fills it in place through the same path `fillV7(into:)` uses — one native call, one allocation, no per-element work.

The raw-buffer overload is for callers who want RFC-ordered bytes rather than `UUID` values — a wire buffer or a database parameter. A destination whose length isn't a whole multiple of 16 throws `Error.bufferNotWholeUUIDs`.

### Raw-byte SQL-order transforms

`v6/v7ToSqlOrder(bytes:)` and `v6/v7FromSqlOrder(bytes:)` apply the same native permutation in place on a caller's 16 bytes. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here now makes against the one native implementation.

## Benchmarks

Measured with [`package-benchmark`](https://github.com/ordo-one/package-benchmark) (`swift package benchmark run` in `Benchmarks/`, release build, linux-x64 on an Intel Core i9-11900H, Swift 6.3, p50 of 10,000 samples):

| Call | p50 | vs. `Foundation.UUID()` | Malloc (total) |
|---|---:|---:|---:|
| `Foundation.UUID()` | 1,413 ns | baseline | 0 |
| `UuidGenerator.newV4()` | 107 ns | **13x faster** | 0 |
| `UuidGenerator.newV5(namespace:name:)` | 135 ns | **10x faster** | 0 |
| `UuidGenerator.newV6()` | 148 ns | **9.5x faster** | 0 |
| `UuidGenerator.newV7()` | 117 ns | **12x faster** | 0 |

Every HyperUuid call here is faster than `Foundation.UUID()` on this machine — the call path is cheap, a direct call to a linked-in symbol — and none of them allocates. Each used to: a heap `[UInt8]` for the out-value and one more per input, neither of which this shape needs. Foundation's `UUID` wraps `uuid_t`, sixteen bytes already in RFC 9562 order, so a `uuid_t` on the stack is both the scratch every door needs and the value the result is built from. The v5 name crosses as a view of the string's own UTF-8 (`withUTF8`) rather than an `Array` copy, and `newV5(namespace:name:)` takes an `UnsafeRawBufferPointer` as the primitive the `String` and `[UInt8]` forms wrap. Zero mallocs per call, measured by the harness rather than claimed.

Batch generation amortizes the native call over the whole batch, and no longer pays a per-element construction on top:

| Call | p50 | Per UUID |
|---|---:|---:|
| `newV6()` × 1000 (individual) | 70 µs | 70 ns |
| `newV6Batch(count: 1000)` | **12 µs** | 12 ns |
| `newV7()` × 1000 (individual) | 76 µs | 76 ns |
| `newV7Batch(count: 1000)` | **10 µs** | 10 ns |

**≈6x for v6, ≈7.5x for v7** — one native call, one clock read and one allocation instead of a thousand of each, with the batch doors landing on the same floor the fills reach. The multiple is the machine's as much as the binding's: the individual calls each read the wall clock, so where a clock read is expensive the loop costs far more and the batch, which reads it once, does not.

## Requirements

- **Swift 6.2 or later.** The manifests declare `swift-tools-version:6.2`: the first release
  whose package manager can link a static library as a binary target (SE-0482), which is how
  the core reaches every platform. CI runs `swift test` on Swift 6.4 on every platform, and
  runs Linux (glibc and musl) and WebAssembly again on 6.2 in Swift's own containers. macOS
  and Windows are tested on 6.4 only.
- **Platforms.** Linux on glibc and on musl (Swift's static Linux SDK), macOS and Windows,
  each on x86_64 and arm64, and WebAssembly (`wasm32-unknown-wasip1`), in WASI hosts and in
  the browser. No `platforms:` floor is declared, so macOS takes SwiftPM's default
  deployment target.
- **Not supported: everything else.** iOS, tvOS, watchOS, visionOS, Android, and any other
  architecture on the supported systems have no prebuilt core here, so the build stops at
  compile time with no `HyperUuidCore` module (Swift Build first warns that the artifact
  bundle has no matching variant) — never at run time.

## Linking and deployment

There is nothing for you to do. The package declares the core as a SwiftPM binary target —
one static library per triple, in `HyperUuidCore.artifactbundle` — and SwiftPM links the one
for your target into your executable. There is no shared library to find at run time, no
resource bundle, and nothing to deploy beside the binary: a multi-stage Dockerfile that
copies only the executable works, so does a fully static build with
`swift build --swift-sdk x86_64-swift-linux-musl`, and so does copying a macOS or Windows
executable on its own. The core adds roughly 10–20 KB to it.

Every call `throws` only `UuidGenerator.Error` — a native call that ran and was refused
(`.timestampOutOfRange`, `.randomSourceFailure(code:)`, …). `UuidGenerator.isAvailable` is
always `true`, and the `NativeLibraryError` type is deprecated and has no cases: both are
left from when macOS and Windows loaded a shared library, so existing code that checks
them still compiles.

On Swift 6.3 with `--build-system swiftbuild` (opt-in there), a package that depends on
this one fails with `missing required module 'HyperUuidCore'`: that release's Swift Build
drops a binary target's module map when it is reached through a product
([swift-build#1295](https://github.com/swiftlang/swift-build/pull/1295), fixed in 6.4).
The default build system of every release from 6.2 on is unaffected, and so is 6.4's
Swift Build (its default); 6.2's opt-in Swift Build predates static-library artifact
bundles altogether.

## WebAssembly

The binding compiles to WebAssembly from Swift 6.2: install swift.org's WebAssembly SDK
and build with it.

```sh
swift sdk install <the Wasm SDK URL and checksum from swift.org/install>
swift build --swift-sdk swift-6.4.0-RELEASE_wasm     # `swift sdk list` names yours
```

The core is linked in as a static library — the same binary target Linux uses, with a
`wasm32-unknown-wasip1` archive — so there is no module to load and no engine to embed.
Randomness is WASI's `random_get`, which every WASI host and browser shim provides. The
core itself has no clock, on any platform; `newV6()` and `newV7()` read the time through
Foundation, which works under WASI. CI runs the whole suite under WasmKit on Swift 6.4, and
a smoke executable on 6.2, whose XCTest does not start under WASI.

### In the browser

What the WebAssembly SDK builds is a plain `wasm32-wasip1` command module (it imports only
`wasi_snapshot_preview1`), so a browser runs it through a WASI shim — no JavaScriptKit and
no change to the package. With [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim),
whose `random_get` is `crypto.getRandomValues`:

```js
import { WASI, File, OpenFile, ConsoleStdout } from "@bjorn3/browser_wasi_shim";

const wasi = new WASI(["MyTool"], [], [
  new OpenFile(new File([])),                       // stdin
  ConsoleStdout.lineBuffered(console.log),          // stdout
  ConsoleStdout.lineBuffered(console.error),        // stderr
]);
const { instance } = await WebAssembly.instantiateStreaming(
  fetch("MyTool.wasm"), { wasi_snapshot_preview1: wasi.wasiImport });
const exitCode = wasi.start(instance);
```

CI builds the smoke executable for WebAssembly on Swift 6.4 and runs it this way in headless
Chrome on every PR, failing unless it exits 0. An executable that imports Foundation is
large (tens of MB, most of it ICU data); `-c release` and `wasm-opt` bring that down.

The other direction — running the core as wasm *inside* a native Swift process, the way the
Java binding does — is not built: no wasm engine ships as a Swift
package with a stable API, and nothing here needs one, since every platform this binding
supports has the core natively. The root README's
[WebAssembly section](../README.md#webassembly) tracks both directions for every binding.

## Verifying provenance

Like PHP, there's no separate package registry to attest here — SwiftPM resolves a git tag
directly against this repo. The native binaries the package carries — the static libraries
under `swift/HyperUuidCore.artifactbundle/`, staged by `stage-native-binaries.yml` — each
carry their own build-provenance attestation from `hyper-build-native.yml`, which physically lives
in `SkunkWerkx/.github` — so verifying needs `--signer-repo` alongside `--repo`, or `gh`
reports a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is
only an identity mismatch:

```sh
gh attestation verify swift/HyperUuidCore.artifactbundle/arm64-apple-macosx/libhyperuuid.a \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
gh attestation verify swift/HyperUuidCore.artifactbundle/x86_64-unknown-linux-gnu/libhyperuuid.a \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.

## Install

Add the package URL as a dependency:

```
https://github.com/SkunkWerkx/HyperUuid
```

In Xcode that's File ▸ Add Package Dependencies; in a `Package.swift` it's a `.package(url:)`
entry with whatever version requirement suits you, plus the product on each target that
uses it:

```swift
dependencies: [
    .package(url: "https://github.com/SkunkWerkx/HyperUuid", from: "…"),   // the tag on the badge above
],
targets: [
    .target(
        name: "MyTarget",
        dependencies: [.product(name: "HyperUuid", package: "HyperUuid")]
    ),
]
```

SwiftPM resolves the newest release that satisfies the requirement, so there is no version
to copy from here and none to go stale.

Swift Package Manager has no separate registry to publish to — `.package(url:, from:)`
resolves straight from a git tag, which *is* the real, complete publish story here, not a
placeholder for one (Swift Package Index, a discovery/documentation site rather than a
functional registry, is a separate, optional listing — not needed for this to work). SPM
requires `Package.swift` at the repository root with no monorepo subdirectory support, same
constraint Packagist has for `composer.json` — [the repo root's own `Package.swift`](../Package.swift)
exists for that reason, with its targets pointed at the real sources under `swift/` via
`path:` rather than duplicating them. The native binaries — the static libraries under
`HyperUuidCore.artifactbundle/{triple}/` (`hyperuuid.lib` for the two Windows triples,
`libhyperuuid.a` for the rest) — are committed straight into git: unlike a real package
registry, SwiftPM has no packing step of its own — whatever's literally in the git tree at
the resolved tag is what a consumer's build links.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
