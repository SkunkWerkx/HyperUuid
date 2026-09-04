# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library. Two native backends, chosen
automatically by build tag, same public API either way: real cgo on darwin/linux
(`backend_cgo.go`) — 3-6x faster per call, see Benchmarks below — and
[purego](https://github.com/ebitengine/purego) (`backend_purego.go`) — dlopen/dlsym
plus per-arch call trampolines, no cgo and no C compiler required — everywhere else,
including Windows unconditionally and any darwin/linux build with `CGO_ENABLED=0`.
Bundles a native build for every supported platform (linux/darwin/windows ×
amd64/arm64) via `go:embed` and picks the right one at runtime, so `go get` is the whole
install. A third backend, opt-in behind `-tags hyperuuid_wasm`, runs the same core as a
WebAssembly module inside the process through wasmtime-go instead of dlopen'ing anything —
see [WebAssembly (wasmtime-go)](#webassembly-wasmtime-go).

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

created, err := hyperuuid.GetTimestamp(id) // version-agnostic: ErrNotTimeBased instead of assuming id is v6/v7
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
v6 limitation plain order already has. `NewV6AtTime`/`NewV7AtTime` accept a `time.Time`
directly in place of `NewV6At`/`NewV7At`'s raw millisecond count. `GetTimestamp` is
the version-agnostic counterpart to `V6Timestamp`/`V7Timestamp` — it checks
`id.Version()` itself and returns `ErrNotTimeBased` for anything but a genuine v6/v7
`uuid.UUID`, instead of assuming the caller already knows.

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
(`go/vX.Y.Z`, not a bare `vX.Y.Z` — those track this repo's other bindings' own release
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
native darwin/linux build. This repo's own CI (`ci.yml`) already runs
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

## WebAssembly (wasmtime-go)

The root README's WebAssembly table lists Go as a **structural** blocker, and that row is
still true: it is about compiling *this module* to wasm, and neither `cgo` nor `purego`
has a wasm target. This section is the inverse direction — the Rust core compiled to
`wasm32-wasip1` and run *inside* an ordinary Go process by
[wasmtime-go](https://github.com/bytecodealliance/wasmtime-go), with no native shared
library dlopen'd at all. Same public API, same suite, third backend:

```shell
go build -tags hyperuuid_wasm ./...
go test  -tags hyperuuid_wasm ./...
```

`backend_wasmtime.go` is gated on the `hyperuuid_wasm` tag and the other two backends
are gated on its absence, so exactly one is ever compiled in. It is opt-in only — never
selected automatically — because it is the right answer to two specific questions and a
worse answer to every other one:

- **A platform this module ships no native build for.** The embedded
  `native/wasm32-wasip1/hyperuuid.wasm` is one artifact for every OS and architecture
  wasmtime itself runs on; `currentTarget()` and the per-RID shared libraries are not
  consulted.
- **A deployment that must not write an executable to a temp file.** The native backends
  have to (see `native_extract.go`); this one instantiates the module straight from the
  embedded bytes.

Two costs, stated plainly:

**It is cgo throughout.** wasmtime-go links wasmtime's precompiled static library through
its C API, so a build with this tag needs a working C toolchain on every platform,
Windows included — which is exactly the story `backend_purego.go` exists to avoid (see
"cgo on darwin/linux, purego everywhere else" above). It is also a `require` in
`go.mod` regardless of tag, because Go has no tag-conditional requirements; it lands in
every consumer's module graph and `go.sum`, and compiles into a binary only with the tag.

**Every call crosses into a wasm guest, serialized under a mutex.** A wasmtime `Store`
is not safe for concurrent use, so one process-wide instance takes a lock per call. A
wasm guest sees only its own linear memory, so nothing is handed over by pointer either:
inputs are copied into a guest buffer obtained from the module's own exported `malloc`
(never a host-picked offset — the guest allocator claims the tail of the initial memory
on first use, and a batch written there was observed corrupted by its very next
allocation), and results are copied back out. The v7 counter lives inside that one
instance, so batch and single-call monotonicity hold exactly as they do against one
loaded shared library.

Measured on the same linux-arm64 machine as the tables below, `go test -tags
hyperuuid_wasm -bench=. -benchmem`:

| Call | cgo | wasmtime-go |
| --- | ---: | ---: |
| `NewV4` | 165 ns, 0 allocs | 2,677 ns, 9 allocs |
| `NewV5String` | 200 ns, 1 alloc | 4,286 ns, 14 allocs |
| `NewV7At` | 142 ns, 0 allocs | 3,127 ns, 11 allocs |
| `NewV7BatchAt(1000, ...)` | 33.6 µs, 2 allocs | 51.0 µs, 14 allocs |
| `FillV7At` (1000, existing slice) | 18.4 µs, 0 allocs | 40.3 µs, 13 allocs |
| `FillV7BytesAt` (1000, existing buffer) | 17.6 µs, 0 allocs | 41.3 µs, 13 allocs |

Per call it is roughly 20x the native crossing; per UUID inside a batch it is a little
over 2x, and the allocations are wasmtime-go's own per-call argument boxing, not this
module's. The advice the Destination-buffer fills section gives applies here with more
force, not less: if the workload can batch, batch.

## Destination-buffer fills

`FillV6`/`FillV7` (and the `At` variants) write into a slice you already own instead of allocating a fresh one per call:

```go
dst := make([]uuid.UUID, 1000)
for {
    if err := hyperuuid.FillV7(dst); err != nil { /* ... */ }
    // reuse dst next iteration — nothing allocated
}
```

Go gets the best version of this API in the whole project. `uuid.UUID` is `[16]byte`, so a `[]uuid.UUID` is contiguous 16-byte records in exactly the RFC order the native core writes — the batch lands directly in your slice with **no intermediate buffer and no per-element conversion**. (C# and Java can't do that; their UUID types aren't RFC byte order, so they must rebuild every element.)

`go test -bench=. -benchmem ./go/...`, 1000 UUIDs per op:

| method | ns/op | B/op | allocs/op |
| --- | ---: | ---: | ---: |
| `NewV7At` x1000 individually | 138,646 | 16,000 | 1000 |
| `NewV7BatchAt(1000)` | 22,989 | 16,384 | 1 |
| `FillV7At` into an existing slice | 18,355 | **0** | **0** |
| `FillV7BytesAt` into an existing buffer | 17,629 | **0** | **0** |

`FillV6Bytes`/`FillV7Bytes` take a `[]byte` for callers who want raw RFC-ordered bytes rather than `uuid.UUID` values — a wire buffer or a database parameter. In Go the two forms are within 4% of each other, since neither converts; the byte form exists for convenience, not speed.

`NewV6BatchAt`/`NewV7BatchAt` now delegate to the fills, so the array-returning API is a single allocation with no intermediate copy — existing callers got faster without changing a line.

### Raw-byte SQL-order transforms

`V6/V7ToSqlOrderBytes` and `V6/V7FromSqlOrderBytes` apply the same native permutation as `V7ToSqlOrder` in place on a caller's 16-byte slice. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same check every binding in this repo now makes against the one native implementation.

## Benchmarks

`go test -bench=. -benchmem ./...` — allocation tracking is built into `testing.B`,
no extra tooling needed. Measured on the same linux-arm64 machine, same run, both
backends (`go test -bench=.` for cgo, `CGO_ENABLED=0 go test -bench=.` for purego):

| Call | cgo before | cgo after | purego | Speedup (cgo after vs purego) |
| --- | ---: | ---: | ---: | ---: |
| `NewV4` | 174 ns, 1 alloc | **165 ns, 0 allocs** | 506 ns, 4 allocs | **3.1x** |
| `NewV5String` | 200 ns, 3 allocs | **200 ns, 1 alloc** | 731 ns, 7 allocs | **3.7x** |
| `NewV6At` | 133 ns, 1 alloc | **139 ns, 0 allocs** | 533 ns, 5 allocs | **3.8x** |
| `NewV7At` | 133 ns, 1 alloc | **142 ns, 0 allocs** | 543 ns, 5 allocs | **3.8x** |

Every purego call does 4-7 heap allocations; cgo now does none. An earlier edition
of this section called the one allocation cgo used to make "a structural floor for
this call shape (an out-param pointer into C)" — and it was a floor for *that*
shape, not for the ABI. `go build -gcflags=-m` is right that any Go pointer handed
to a cgo call is conservatively heap-allocated, so the shims stopped handing one
over: the C side keeps the sixteen bytes on its own stack and returns them as a
struct, and takes a UUID argument the same way, so nothing crosses by pointer except
a caller's own slice. The allocation is gone (the one `NewV5String` keeps is Go's own
`[]byte(name)` conversion); per-call time barely moves, because on these doors the
entropy fetch, not the crossing, is what costs. The same by-value shape took 30-50%
off every door in HyperCast, whose parsers had no such floor underneath.

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
| v6 | 58 ns, 0 allocs (was 75 ns, 1 alloc) | 437.4 ns, 4 allocs | 3.8-4.0 ns, 0 allocs |
| v7 | 57 ns, 0 allocs (was 74 ns, 1 alloc) | 443.7 ns, 4 allocs | 3.8-4.0 ns, 0 allocs |

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

## Verifying build provenance

(Not to be confused with the UUID-provenance point above — this is about the binary, not the
ID.) Go has no package registry to attest either — `go get` resolves straight from the
`go/vX.Y.Z` git tag against this repo. The native libraries committed under `go/native/`
(staged by `stage-native-binaries.yml`) each carry their own build-provenance attestation
from `hyper-build-native.yml`, which physically lives in `SkunkWerkx/.github` — so verifying
needs `--signer-repo` alongside `--repo`, or `gh` reports a bare `verifying with issuer
"sigstore.dev"` that reads like a bad signature but is only an identity mismatch:

```sh
gh attestation verify go/native/linux-x64/libhyperuuid.so \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.
