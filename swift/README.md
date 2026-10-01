# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperUuid?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperUuid/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**Foundation's `UUID()` initializer only ever produces random v4 UUIDs — no v5, no v6, no v7 (a [Swift Forums pitch](https://forums.swift.org/t/pitch-uuid-v7-other-improvements/85427) to add v7 is still at the pitch stage). This package is the whole RFC, today.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library via `dlopen`/`dlsym`
(Linux/macOS) or `LoadLibraryW`/`GetProcAddress` (Windows) plus an `@convention(c)`
function-pointer cast — no runtime bridge, no shim. Bundles a native build for every
supported platform (linux/macOS/Windows × x64/arm64) since SwiftPM's native binary-
distribution mechanism (a `binaryTarget`/XCFramework) is Apple-only; picks the right
one at compile time.

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
the loaded library's own `major.minor.patch` (`hyperuuid_version`), so a caller can prove
the binary it resolved is the one this binding was built against before minting the first
UUID; `UuidGenerator.isAvailable` is the non-throwing form of the same question — see
[Loading and deployment](#loading-and-deployment).

## Why not Foundation's `UUID()`?

There's no real comparison to make for v5/v6/v7 — Foundation's `UUID` type has never generated anything but random v4:

1. **Full RFC 9562 coverage.** v4, v5 (SHA-1 namespace-based, verified in CI to match Python's own `uuid.uuid5` byte-for-byte), v6, and v7 — none of which `UUID()` can produce at all.
2. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency — not something `UUID()` needs, since it has no v7 to order in the first place.
3. **Batch generation.** `newV7Batch(count:unixMillis:)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead N times.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — a Swift service and a service written in any of those languages agree byte-for-byte on the same `(namespace, name)` pair.

The honest trade-off: this package bundles a native library per platform instead of being pure Swift, and every call is `throws` where `UUID()` never fails. If plain v4 randomness is all you need, `UUID()` is simpler, has no native dependency, and is already there — a completely reasonable choice.

## Destination-buffer fills

`fillV6`/`fillV7` write into storage you already own instead of allocating a fresh array per call, over either an `inout [UUID]` or an `UnsafeMutableRawBufferPointer`:

```swift
var dst = [UUID](repeating: UUID(), count: 1000)
try UuidGenerator.fillV7(into: &dst, unixMillis: ms)   // reuse dst; nothing allocated
try UuidGenerator.fillV7(into: &dst)                    // the same, at the current time
```

Swift gets the good version of this, alongside Go. Foundation's `UUID` wraps `uuid_t` — 16 bytes already in RFC 9562 order — so the native core writes the whole batch straight into the array's storage with **no per-element conversion**. (C# and Java must rebuild every element, because their UUID types aren't RFC byte order.) That soundness condition is checked with a `precondition` on `MemoryLayout<UUID>`'s size and stride rather than assumed.

`swift package benchmark`, p50 wall clock, 1000 UUIDs per op:

| Benchmark | before | after | mallocs after |
| --- | ---: | ---: | ---: |
| `newV7` x1000 individually | 947 µs | **844 µs** | 1000 → **0** |
| `newV7Batch(count: 1000)` | 86 µs | **17 µs** | 1002 → **1** |
| `fillV7(into: [UUID])` | 24 µs | **19 µs** | 1 |
| `fillV7(into: raw bytes)` | 16 µs | **16 µs** | 1 |

The gap between `newV7Batch` and the fills is gone, and the before column says why it was there: `newV7Batch` was paying for a scratch buffer plus a per-element `UUID(rfcBytes:)` construction over it. It now allocates its result array and fills it in place through the same path `fillV7(into:)` uses — one native call, one allocation, no per-element work.

The raw-buffer overload is for callers who want RFC-ordered bytes rather than `UUID` values — a wire buffer or a database parameter. A destination whose length isn't a whole multiple of 16 throws `Error.bufferNotWholeUUIDs`.

### Raw-byte SQL-order transforms

`v6/v7ToSqlOrder(bytes:)` and `v6/v7FromSqlOrder(bytes:)` apply the same native permutation in place on a caller's 16 bytes. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here now makes against the one native implementation.

## Benchmarks

Measured with [`package-benchmark`](https://github.com/ordo-one/package-benchmark) (`swift package benchmark run` in `Benchmarks/`, release build, linux-arm64, p50 of 10,000 samples):

| Call | before | after | Malloc (total) |
|---|---:|---:|---:|
| `Foundation.UUID()` | 3,101 ns | 3,201 ns | 0 |
| `UuidGenerator.newV4()` | 1,000 ns | **900 ns** | 1 → **0** |
| `UuidGenerator.newV5(namespace:name:)` | 1,500 ns | **1,100 ns** | 3 → **0** |
| `UuidGenerator.newV6()` | 1,700 ns | **1,600 ns** | 1 → **0** |
| `UuidGenerator.newV7()` | 1,700 ns | **1,600 ns** | 1 → **0** |

Every HyperUuid call here is faster than `Foundation.UUID()` on this machine — the `dlopen`/`@convention(c)` call path is cheap — and none of them allocates. The before column is a heap `[UInt8]` for the out-value and one more per input, neither of which this shape needs: Foundation's `UUID` wraps `uuid_t`, sixteen bytes already in RFC 9562 order, so a `uuid_t` on the stack is both the scratch every door needs and the value the result is built from. The v5 name crosses as a view of the string's own UTF-8 (`withUTF8`) rather than an `Array` copy, and `newV5(namespace:name:)` takes an `UnsafeRawBufferPointer` as the primitive the `String` and `[UInt8]` forms wrap. Zero mallocs per call, measured by the harness rather than claimed.

Batch generation amortizes the native call over the whole batch, and no longer pays a per-element construction on top:

| Call | before | after | Per-UUID after |
|---|---:|---:|---:|
| `newV6()` × 1000 (individual) | 991 µs | **842 µs** | 842 ns |
| `newV6Batch(count: 1000)` | 91 µs | **20 µs** | 20 ns |
| `newV7()` × 1000 (individual) | 947 µs | **844 µs** | 844 ns |
| `newV7Batch(count: 1000)` | 86 µs | **17 µs** | 17 ns |

**≈42x for v6, ≈50x for v7** — one native call and one allocation instead of a thousand of each, with the batch doors now landing on the same floor the fills reach.

## Requirements

- **Swift.** Tested on Swift 6.4 — every CI leg runs `swift test` on it, and the Linux legs
  run it a second time with `--build-system native`, the build system Swift 6.3 and earlier
  use. The manifests declare `swift-tools-version:5.9`: that is the floor SwiftPM will accept
  and the oldest language version the sources are written against, but no CI leg builds on
  it, so anything below 6.4 is declared rather than proven.
- **Platforms.** glibc Linux, macOS and Windows, each on x86_64 and arm64 — the six native
  builds under `NativeLibs/`. No `platforms:` floor is declared, so macOS takes SwiftPM's
  default deployment target.
- **Not supported: musl Linux.** The other bindings in this repo ship `linux-musl-x64` and
  `linux-musl-arm64` builds; this one deliberately does not. Swift's musl target is the fully
  static Linux SDK, and a statically linked executable has no dynamic loader to `dlopen` a
  shared library with, so there is nothing a bundled musl library could be loaded by. A musl
  build stops at a compile-time `#error` that says so. This is deferred, not impossible:
  the core could be linked in statically instead of loaded, as a SwiftPM binary
  static-library target (SE-0482, Swift 6.2 and later), and that path is not built yet.
- **Not supported: everything else.** iOS, tvOS, watchOS, visionOS, Android, and any other
  architecture on the three supported systems have no native build here and stop at the same
  kind of `#error` — at compile time, rather than being handed a library that can't load.

## Loading and deployment

The native library travels as a SwiftPM resource. `swift build` stages `NativeLibs/` into a
directory beside the built products, and the first call `dlopen`s this platform's library
straight out of it — nothing is extracted, copied or left behind in a temp directory. The
directory's name depends on the toolchain: `HyperUuid_HyperUuid.bundle` on Swift 6.4 and later
(and on macOS with any version), `HyperUuid_HyperUuid.resources` on Linux and Windows with Swift
6.3 and earlier or with `--build-system native`. The loader accepts either.

**That directory has to ship with your executable.** A deployment that copies only the binary
— the usual multi-stage Dockerfile — has no native library to load:

```dockerfile
COPY --from=build /src/.build/release/MyServer /app/
# Swift 6.4 and later. On 6.3 and earlier the directory is HyperUuid_HyperUuid.resources.
COPY --from=build /src/.build/release/HyperUuid_HyperUuid.bundle /app/HyperUuid_HyperUuid.bundle
```

The loader looks beside the executable first, then in the main bundle's resources, which is
where an app bundle carries it. On the machine that built the package it also falls back to
the package's own checkout, so an executable copied out of `.build` keeps working *there* —
which is exactly why a missing directory tends to show up only after deployment. Test the
deployed layout, not the build tree.

When the library can't be found or loaded, nothing crashes. Every call throws
`NativeLibraryError` — a public type, distinct from `UuidGenerator.Error`, naming the path
it looked for or the export it couldn't resolve — and `UuidGenerator.isAvailable` answers
the same question without a `do`/`catch`:

```swift
guard UuidGenerator.isAvailable else {
    return UUID()                               // your fallback; v4 is all Foundation has
}

do {
    return try UuidGenerator.newV7()
} catch let error as NativeLibraryError {
    // .openFailed(path:reason:) or .symbolNotFound(name:) — the library, not the call
} catch let error as UuidGenerator.Error {
    // the call ran and was refused: .timestampOutOfRange, .randomSourceFailure(code:), …
}
```

The load is attempted once per process and its outcome kept, so `isAvailable` costs nothing
after the first answer, and a failed load throws the same error from every later call.

## WebAssembly

None today, in either direction. Compiling *this binding* to wasm: swift.org ships real WASM
SDKs since Swift 6.2, but its own docs say dynamic linking "is not formally specified for
`wasip1` triples and tooling for it is not available yet," and there is no documented path
to link a Rust `.a` statically either — this binding is `dlopen`/`@convention(c)` all the
way down. Running the core as wasm *inside* Swift, the way the Java, Ruby, Python and Go
bindings now do: no wasm engine ships as a Swift package with a stable API today, so there
is nothing to embed. The root README's [WebAssembly section](../README.md#webassembly)
tracks both directions for every binding; if either changes for Swift, this section is
where it lands.

## Verifying provenance

Like PHP, there's no separate package registry to attest here — SwiftPM resolves a git tag
directly against this repo. The native libraries bundled under
`swift/Sources/HyperUuid/NativeLibs/` (staged by `stage-native-binaries.yml`) each carry
their own build-provenance attestation from `hyper-build-native.yml`, which physically lives
in `SkunkWerkx/.github` — so verifying needs `--signer-repo` alongside `--repo`, or `gh`
reports a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is
only an identity mismatch:

```sh
gh attestation verify swift/Sources/HyperUuid/NativeLibs/osx-arm64/libhyperuuid.dylib \
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
`path:` rather than duplicating them. The native libraries under
`Sources/HyperUuid/NativeLibs/{rid}/` are committed straight into git: unlike a real package
registry, SwiftPM has no packing step of its own — whatever's literally in the git tree at
the resolved tag is what a consumer's build bundles as resources.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
