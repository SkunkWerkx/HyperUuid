# HyperUuid

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
let sqlOrdered = try UuidGenerator.toSqlOrder(id4) // byte order SQL Server's uniqueidentifier needs to sort by creation order

// One native call, one random-bytes fetch, one counter reservation for the whole batch:
let batch = try UuidGenerator.newV7Batch(count: 1000)
```

Returns Foundation's `UUID`. `Namespaces.dns`/`url`/`oid`/`x500` are RFC 9562
Section 6.6's well-known namespaces; `WellKnownUuids.nilUUID`/`maxUUID` are the
§5.9/§5.10 special values. `UuidGenerator.v6Timestamp(_:)`/`v7Timestamp(_:)` recover
the embedded UTC `Date` from a version 6 or 7 UUID respectively.
`UuidGenerator.newV6Batch(count:unixMillis:)`/`newV7Batch(count:unixMillis:)` generate
`count` UUIDs sharing one timestamp capture and one native call, instead of `count`
of each. `UuidGenerator.toSqlOrder(_:)`/`fromSqlOrder(_:)` convert a version 7 UUID
to and from the byte order SQL Server's `uniqueidentifier` needs on the wire to sort
by creation order — computed once in the native Rust core rather than reimplemented
in Swift, and verified there (and independently against the real
`System.Data.SqlTypes.SqlGuid` comparator in the C# binding's test suite).

## Why not Foundation's `UUID()`?

There's no real comparison to make for v5/v6/v7 — Foundation's `UUID` type has never generated anything but random v4:

1. **Full RFC 9562 coverage.** v4, v5 (SHA-1 namespace-based, verified in CI to match Python's own `uuid.uuid5` byte-for-byte), v6, and v7 — none of which `UUID()` can produce at all.
2. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency — not something `UUID()` needs, since it has no v7 to order in the first place.
3. **Batch generation.** `newV7Batch(count:unixMillis:)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch instead of paying per-item overhead N times.
4. **Cross-language consistency.** The same Rust core mints v5 namespace UUIDs for Python, Go, C#, Ruby, and every other binding in this repo — a Swift service and a service written in any of those languages agree byte-for-byte on the same `(namespace, name)` pair.

The honest trade-off: this package bundles a native library per platform instead of being pure Swift, and every call is `throws` where `UUID()` never fails. If plain v4 randomness is all you need, `UUID()` is simpler, has no native dependency, and is already there — a completely reasonable choice.

## Benchmarks

Measured with [`package-benchmark`](https://github.com/ordo-one/package-benchmark) (`swift package benchmark run` in `Benchmarks/`, release build, linux-arm64, p50 of 10,000 samples):

| Call | Time (wall clock) | Malloc (total) |
|---|---|---|
| `Foundation.UUID()` | 3,101 ns | 0 |
| `UuidGenerator.newV4()` | 1,000 ns | 1 |
| `UuidGenerator.newV5(namespace:name:)` | 1,100 ns | 3 |
| `UuidGenerator.newV6()` | 1,700 ns | 1 |
| `UuidGenerator.newV7()` | 2,201 ns | 1 |

Every HyperUuid call here is faster than `Foundation.UUID()` on this machine — the `dlopen`/`@convention(c)` call path is cheap. The honest asterisk: unlike the Rust core, the C# binding, and `UUID()` itself, these calls aren't allocation-free — each one heap-allocates the fixed-size `[UInt8]` marshaling buffers Swift's `Array` always backs with a heap allocation (Span-style stack buffers aren't available to Swift the way C#'s `stackalloc`/`ReadOnlySpan<byte>` are). 1-3 small, fixed-size allocations per call is cheap in absolute terms, but it's real, and worth saying plainly rather than claiming zero-alloc across every binding uniformly.

Batch generation amortizes both the native call and that allocation over the whole batch:

| Call | Time for 1,000 UUIDs | Per-UUID |
|---|---|---|
| `newV6()` × 1000 (individual) | 944 µs | 944 ns |
| `newV6Batch(count: 1000)` | 86 µs | 86 ns |
| `newV7()` × 1000 (individual) | 943 µs | 943 ns |
| `newV7Batch(count: 1000)` | 90 µs | 90 ns |

**≈11x for v6, ≈10.5x for v7** — one native call and one marshaling allocation instead of a thousand of each.

## Install

Not yet published to the Swift Package Registry under a registered `SkunkWerkx`
presence — for now this is proven by CI building and testing the native core plus
this package on real hardware for every platform leg. Consume via a direct
`.package(url: "https://github.com/SkunkWerkx/HyperUuid", branch: "...")` (scoped to
this `swift/` subdirectory) in the meantime.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
