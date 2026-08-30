# hyperuuid

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library. Two backends, chosen
automatically by build tag, same public API either way: real cgo on darwin/linux
(`backend_cgo.go`) — 3-6x faster per call, see Benchmarks below — and
[purego](https://github.com/ebitengine/purego) (`backend_purego.go`) — dlopen/dlsym
plus per-arch call trampolines, no cgo and no C compiler required — everywhere else,
including Windows unconditionally and any darwin/linux build with `CGO_ENABLED=0`.
Bundles a native build for every supported platform (linux/darwin/windows ×
amd64/arm64) and picks the right one at runtime, the same trick the Java binding
uses.

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

```sh
go get github.com/SkunkWerkx/HyperUuid/go
```

Go modules have no separate registry to publish to — `go get` resolves straight from a git
tag, which *is* the real, complete publish story here, not a placeholder for one. This
module lives in a subdirectory of the monorepo, so its own semver tags are prefixed
(`go/v0.0.1`, not a bare `v0.0.1` — those track this repo's other bindings' own release
events instead). The native libraries under `native/{rid}/` are committed straight into git:
unlike a real package registry, `go get`/`go build` has no packing step of its own — whatever
`go:embed` finds in the git tree at the resolved module version is what a consumer gets.

## cgo on darwin/linux, purego everywhere else

Earlier versions of this binding used purego unconditionally, on the reasoning
that a real cgo prototype only closed part of the allocation gap (see below) while
costing the module its one clean cross-platform story. Revisited: cgo is now the
default on darwin/linux, gated by `//go:build cgo && (darwin || linux)`
(`backend_cgo.go`) with purego as the automatic fallback
(`//go:build !(cgo && (darwin || linux))`, `backend_purego.go`) — same public API,
selected entirely at compile time, no code changes needed by a consumer either way.

**Why Windows stays on purego unconditionally**, even when `CGO_ENABLED=1`: a cgo
build there needs a MinGW-class C toolchain, and the mainline MinGW-w64 distribution
has no arm64 support at all — only `llvm-mingw`/MSYS2's `clangarm64`, neither bundled
by default. Windows Go servers are a small slice of this module's likely audience
next to Linux/macOS, so trading a real perf win there for the arm64 toolchain pain
wasn't worth it.

**Why this doesn't reintroduce the cross-compilation risk that ruled cgo out the
first time:** Go disables cgo by default the moment `GOOS`/`GOARCH` differ from the
host — confirmed directly, not assumed:

```
$ go env CGO_ENABLED            # native (linux/arm64 here)
1
$ GOARCH=amd64 go env CGO_ENABLED   # cross, same OS, different arch
0
$ GOOS=windows go env CGO_ENABLED   # cross, different OS
0
```

A consumer cross-compiling this module — `GOOS=linux GOARCH=arm64 go build` from an
amd64 CI runner, a multi-arch Docker build, whatever — lands on `backend_purego.go`
automatically, with zero action on their part; cgo only activates on a genuine
native darwin/linux build. This repo's own CI (`build-packages.yml`) already runs
`go test ./...` natively on every leg (real ubuntu/macOS/Windows runners per
architecture, never cross-compiled), so it exercises the cgo backend for real on
4 of 6 legs, not just purego via Windows — and both GitHub's `ubuntu-latest` and
`macos-latest` images ship a working C toolchain by default (`gcc` and Xcode
Command Line Tools' `clang` respectively, confirmed against
[actions/runner-images](https://github.com/actions/runner-images)' own published
tool manifests), so no CI changes were needed to pick this up.

**The real caveat this doesn't cover: a native darwin/linux build with no C
compiler installed at all.** Cross-compiling protects you automatically (above);
building natively without one doesn't. Verified directly, not assumed — pointing
`CC` at a nonexistent binary on this native linux/arm64 machine:

```
$ CC=/nonexistent/no-such-cc go build ./...
# runtime/cgo
cgo: C compiler "/nonexistent/no-such-cc" not found: exec: "/nonexistent/no-such-cc": stat /nonexistent/no-such-cc: no such file or directory
```

`CGO_ENABLED` defaults to `1` on a native darwin/linux build regardless of whether
a compiler is actually present, so this module now hard-fails to build in that
specific situation — a minimal/distroless-style Linux container without
`build-essential`, or a macOS box without Xcode Command Line Tools installed.
Before this backend split, purego being unconditional meant this module built
with zero C toolchain requirement, full stop, on every darwin/linux machine
regardless of what was installed. That guarantee is now conditional: it holds for
every cross-compile and for GitHub's own `ubuntu-latest`/`macos-latest` runners
(both confirmed to ship a compiler by default, see above), but not for an
arbitrary native build environment you don't control. If you hit this, the fix is
one env var: `CGO_ENABLED=0 go build ./...` forces the purego fallback on any
platform, native or not.

## Benchmarks

`go test -bench=. -benchmem ./...` — allocation tracking is built into `testing.B`,
no extra tooling needed. Measured on the same linux-arm64 machine, same run, both
backends (`go test -bench=.` for cgo, `CGO_ENABLED=0 go test -bench=.` for purego):

| Call | cgo | purego | Speedup |
| --- | ---: | ---: | ---: |
| `NewV4` | 170.4 ns, 1 alloc | 531.9 ns, 4 allocs | **3.1x** |
| `NewV5String` | 210.3 ns, 3 allocs | 731.4 ns, 7 allocs | **3.5x** |
| `NewV6At` | 140.5 ns, 1 alloc | 532.6 ns, 5 allocs | **3.8x** |
| `NewV7At` | 151.0 ns, 1 alloc | 562.7 ns, 5 allocs | **3.7x** |

Every purego call did 4-7 heap allocations; cgo cuts that to 1 (3 for v5, which
marshals a variable-length name buffer). `go build -gcflags=-m` explains why cgo
doesn't reach zero allocations either: any pointer crossing into opaque foreign
code — cgo included, not a purego-specific issue — is categorically excluded from
Go's escape analysis, so the compiler must conservatively heap-allocate the
pointee. That's a structural floor for this call shape (an out-param pointer into
C), independent of FFI mechanism; closing it fully would need a different API shape
(a caller-owned buffer passed by value, or a pool), not just a different FFI
library.

**Batch generation is the one place cgo doesn't help — worth stating plainly rather
than only reporting the numbers where it wins:**

| Call | cgo | purego |
| --- | ---: | ---: |
| `NewV6BatchAt(1000, ...)` | 34.1 µs, 2 allocs | 33.0 µs, 7 allocs |
| `NewV7BatchAt(1000, ...)` | 33.6 µs, 2 allocs | 29.4 µs, 7 allocs |

Batch generation already collapses ~5000 individual-call allocations down to a
handful regardless of FFI mechanism (one native call, one random-bytes fetch, one
counter reservation for the whole 1000), so the marginal allocation win cgo brings
per-call has nothing left to amortize — purego is a statistical wash here, and
edges ahead on v7. If your workload is batch-heavy, the backend choice doesn't
matter; if it's dominated by individual calls, cgo's 3-4x per-call win is real.

### Extraction vs. `google/uuid`'s own `Time()`

`google/uuid` isn't just a source type here — it has real extraction logic of its
own (`UUID.Time()`, documented as defined for versions 1, 2, 6, and 7), so it's a
genuine head-to-head, not a strawman. Same machine, same run, both backends:

| Call | cgo | purego | `google/uuid`'s `id.Time()` |
| --- | ---: | ---: | ---: |
| v6 | 76.1 ns, 1 alloc | 437.4 ns, 4 allocs | 3.8-4.0 ns, 0 allocs |
| v7 | 85.0 ns, 1 alloc | 443.7 ns, 4 allocs | 3.8-4.0 ns, 0 allocs |

cgo closes most of the gap to purego (5.2-5.8x faster here) but `google/uuid`'s
`Time()` still wins outright either way, by roughly 20x against cgo and two orders
of magnitude against purego — it's pure Go bit-shifting over bytes already in the
process, zero FFI boundary to cross regardless of which backend this binding uses.
`google/uuid.UUID.Time()` works on *any* RFC-conformant v6/v7 value regardless of
where it came from — it's pure bit math, not tied to how the value was minted — so
there's no provenance argument for reaching past it here. Honestly: in Go
specifically, prefer `id.Time()` over this binding's `V6Timestamp`/`V7Timestamp`
unconditionally, cgo backend or not. They exist for API symmetry with every other
binding in this repo, not because they're the better choice in Go.
