# hyperuuid

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library via
[purego](https://github.com/ebitengine/purego) — dlopen/dlsym plus per-arch call
trampolines, no cgo and no C compiler required. Bundles a native build for every
supported platform (linux/darwin/windows × amd64/arm64) and picks the right one at
runtime, the same trick the Java binding uses.

```go
import (
	"github.com/google/uuid"
	"github.com/SkunkWerkx/HyperUuid/go"
)

id, err := hyperuuid.NewV4()
id, err = hyperuuid.NewV5String(hyperuuid.NamespaceDNS, "example.com")
id, err = hyperuuid.NewV6()
id, err = hyperuuid.NewV7()
batch, err := hyperuuid.NewV7BatchAt(1000, unixMillis)
sqlOrdered, err := hyperuuid.V7ToSqlOrder(id) // byte order SQL Server's uniqueidentifier needs to sort by creation order
```

Returns [`github.com/google/uuid`](https://pkg.go.dev/github.com/google/uuid)'s
`uuid.UUID` — already RFC 9562 network-byte-order-identical to what the native core
writes, so there's no byte-swapping in this binding. `NamespaceDNS`/`NamespaceURL`/
`NamespaceOID`/`NamespaceX500` are re-exports of `google/uuid`'s own (already
RFC 9562 §6.6-identical) namespace constants, kept here for API-shape symmetry with
the other bindings' `Namespaces.*`; `Nil`/`Max` (RFC 9562 §5.9/§5.10) are the same
kind of re-export. `V6Timestamp`/`V7Timestamp` recover the embedded UTC `time.Time`
from a version 6 or 7 UUID respectively. `NewV6BatchAt(count, unixMillis)`/
`NewV7BatchAt(count, unixMillis)` generate `count` UUIDs sharing one timestamp
capture and one native call, instead of `count` of each. `V7ToSqlOrder`/`V7FromSqlOrder`
convert a version 7 UUID to and from the byte order SQL Server's `uniqueidentifier`
needs on the wire to sort by creation order — computed once in the native Rust core
(and verified there, and independently against the real `System.Data.SqlTypes.SqlGuid`
comparator in the C# binding's test suite) rather than reimplemented per binding.
`V6ToSqlOrder`/`V6FromSqlOrder` do the same for version 6, though same-millisecond
v6 UUIDs aren't guaranteed to sort correctly afterward — v6 has no counter, so
`clock_seq`/`node` (not the timestamp) decide ties, the same pre-existing RFC 9562
v6 limitation plain order already has.

## Why not `google/uuid`'s own `NewV6`/`NewV7`?

This binding depends on `google/uuid` for the `uuid.UUID` type itself — it's already
in your import graph, and it already ships working `NewV6()`/`NewV7()` functions of
its own. Two real, checked-against-its-actual-source reasons to reach for this
binding's generators instead:

1. **Node ID privacy.** `google/uuid`'s `NewV6()` defaults to a real network
   interface's MAC address for the node ID field when one is available (see its own
   [`version6.go`](https://github.com/google/uuid/blob/master/version6.go) —
   `setNodeInterface`), which is exactly the hardware-identity leak RFC 9562 §6.9
   recommends against. `hyperuuid.NewV6`/`NewV6BatchAt` always use a random node ID
   with the multicast bit set, the same way this project's v6 works in every other
   binding.
2. **Explicit, testable timestamps.** `google/uuid`'s `NewV6`/`NewV7` always read the
   system clock internally with no way to inject a specific instant. Every time-based
   generator here — `NewV6At`, `NewV7At`, and both batch variants — takes
   `unixMillis` as an explicit parameter, so tests can assert against a fixed RFC
   test vector instead of the wall clock, and the same call works identically
   compiled to `wasm32`, which has no OS clock of its own.

(`google/uuid`'s own `NewV7()` *does* implement a real monotonic sub-millisecond
sequence — worth knowing if you're comparing the two, since a naively-random v7
generator would not.) Both libraries produce spec-valid, mutually interoperable
UUIDs; picking one over the other for v6/v7 generation is about these two
properties and, if your other services are in a different language, using the one
engine that's byte-for-byte identical everywhere.

Not yet published to a module proxy under a registered `SkunkWerkx` presence — for
now this is proven by CI building and testing the native core plus this binding on
real hardware for every platform leg. Consume via a direct `go get
github.com/SkunkWerkx/HyperUuid/go@<tag>` in the meantime.

## Benchmarks

`go test -bench=. -benchmem ./...` — allocation tracking is built into `testing.B`,
no extra tooling needed. Measured on linux-arm64: every call here does 4-7 heap
allocations (252-360 B/op), unlike the Rust core itself or the C# binding, which
are both genuinely allocation-free. Almost certainly `unsafe.Pointer` arguments
crossing into `purego`'s dynamically-generated call trampolines default Go's
escape analysis into moving the call's local variables to the heap — a real cost
of the "no cgo" approach, not something this binding does inefficiently on
purpose. Batch generation wins even bigger here than in the other bindings as a
result: `NewV7BatchAt(1000, ...)` was ~19x faster than 1000 individual `NewV7At`
calls (27µs vs 514µs), since it also collapses ~5000 of those allocations into 7.

**Would switching to cgo fix this?** Measured, not assumed: a real cgo prototype
against the same native library cut per-call allocations from 4-7 down to 1 (2 for
v5) and was faster outright (125-181 ns/op vs purego's 546-688 ns/op here) — but it
never reached zero. `go build -gcflags=-m` confirms why: any pointer crossing into
opaque foreign code — cgo included, not a purego-specific issue — is categorically
excluded from Go's escape analysis, so the compiler must conservatively heap-allocate
the pointee. That's a structural floor for this call shape (an out-param pointer into
C), independent of FFI mechanism; closing it fully would need a different API shape
(a caller-owned buffer passed by value, or a pool) rather than a different FFI
library. Given cgo would only close part of the gap while costing this binding its
one clean cross-platform story — `CGO_ENABLED=0`, no C toolchain, one module that
builds unmodified on all 6 platform legs — it isn't worth it here. The batch
functions already amortize allocations far more effectively than a cgo migration
would (7 allocations total for 1000 UUIDs, vs 5000 for individual calls).
