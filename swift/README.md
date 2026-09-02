# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)

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
millisecond count, and `getTimestamp(_:)` is the version-agnostic counterpart to
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
RFC 9562 v6 limitation plain order already has.

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
```

Swift gets the good version of this, alongside Go. Foundation's `UUID` wraps `uuid_t` — 16 bytes already in RFC 9562 order — so the native core writes the whole batch straight into the array's storage with **no per-element conversion**. (C# and Java must rebuild every element, because their UUID types aren't RFC byte order.) That soundness condition is checked with a `precondition` on `MemoryLayout<UUID>`'s size and stride rather than assumed.

`swift package benchmark`, p50 wall clock, 1000 UUIDs per op:

| Benchmark | p50 µs |
| --- | ---: |
| `newV7` x1000 individually | 955.0 |
| `newV7Batch(count: 1000)` | 77.0 |
| `fillV7(into: [UUID])` | 19.0 |
| `fillV7(into: raw bytes)` | **16.0** |

Worth noting how large that gap is: `fillV7` is roughly **4x faster than `newV7Batch`**, not the marginal improvement the same change produces in C#. Swift was paying real time for the result array plus a per-element `UUID(rfcBytes:)` construction, and dropping both lands it at 16 µs — the same floor every other binding here reaches.

The raw-buffer overload is for callers who want RFC-ordered bytes rather than `UUID` values — a wire buffer or a database parameter. A destination whose length isn't a whole multiple of 16 throws `Error.bufferNotWholeUUIDs`.

### Raw-byte SQL-order transforms

`v6/v7ToSqlOrder(bytes:)` and `v6/v7FromSqlOrder(bytes:)` apply the same native permutation in place on a caller's 16 bytes. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here now makes against the one native implementation.

## Benchmarks

Measured with [`package-benchmark`](https://github.com/ordo-one/package-benchmark) (`swift package benchmark run` in `Benchmarks/`, release build, linux-arm64, p50 of 10,000 samples):

| Call | Time (wall clock) | Malloc (total) |
|---|---|---|
| `Foundation.UUID()` | 3,101 ns | 0 |
| `UuidGenerator.newV4()` | 1,000 ns | 1 |
| `UuidGenerator.newV5(namespace:name:)` | 1,100 ns | 3 |
| `UuidGenerator.newV6()` | 1,700 ns | 1 |
| `UuidGenerator.newV7()` | 2,201 ns | 1 |

Every HyperUuid call here is faster than `Foundation.UUID()` on this machine — the `dlopen`/`@convention(c)` call path is cheap. The honest asterisk: unlike `Foundation.UUID()` itself, these calls aren't allocation-free — each one heap-allocates the fixed-size `[UInt8]` marshaling buffers Swift's `Array` always backs with a heap allocation (Swift has no Span-style stack buffer to reach for here). 1-3 small, fixed-size allocations per call is cheap in absolute terms, but it's real, and worth saying plainly rather than inheriting a zero-alloc claim that doesn't hold on this side of the boundary.

Batch generation amortizes both the native call and that allocation over the whole batch:

| Call | Time for 1,000 UUIDs | Per-UUID |
|---|---|---|
| `newV6()` × 1000 (individual) | 944 µs | 944 ns |
| `newV6Batch(count: 1000)` | 86 µs | 86 ns |
| `newV7()` × 1000 (individual) | 943 µs | 943 ns |
| `newV7Batch(count: 1000)` | 90 µs | 90 ns |

**≈11x for v6, ≈10.5x for v7** — one native call and one marshaling allocation instead of a thousand of each.

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
entry with whatever version requirement suits you. SwiftPM resolves the newest release that
satisfies it, so there is no version to copy from here and none to go stale.

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
