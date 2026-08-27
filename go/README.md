# hyperuuid

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
capture and one native call, instead of `count` of each.

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
