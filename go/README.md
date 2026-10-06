# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperUuid/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperUuid/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `hyperuuid` core — **linked into your binary through cgo**
(`backend_static.go`). The core is a ~20 KB static library on the link line: nothing is
embedded, nothing is extracted, nothing is `dlopen`ed, nothing can fail to load, and the
binary runs from a read-only filesystem or a `scratch` image. `go get` is the whole install;
a C compiler at build time is the one requirement (see [Requirements](#requirements)).

```go
import (
	"github.com/google/uuid"

	hyperuuid "github.com/SkunkWerkx/HyperUuid/go"
)

id, err := hyperuuid.NewV4()
id, err = hyperuuid.NewV5String(hyperuuid.NamespaceDNS, "example.com")
id, err = hyperuuid.NewV6()
id, err = hyperuuid.NewV7()
batch, err := hyperuuid.NewV7BatchAt(1000, unixMillis)
sqlOrdered, err := hyperuuid.V7ToSqlOrder(id) // byte order SQL Server's uniqueidentifier needs to sort by creation order

created, err := hyperuuid.GetTimestamp(id) // version-agnostic: ErrNotTimeBased instead of assuming id is v6/v7
```

## Install

```sh
go get github.com/SkunkWerkx/HyperUuid/go
```

Requires Go 1.26 or later (the `go` directive in `go.mod`). The import path ends in `/go`
while the package is named `hyperuuid`, so spell the name out in the import, as above —
`goimports` adds it for you, but a hand-written `import "github.com/SkunkWerkx/HyperUuid/go"`
reads as if the package were called `go`.

Go modules have no separate registry to publish to — `go get` resolves straight from a git
tag, which *is* the real, complete publish story here, not a placeholder for one. This
module lives in a subdirectory of the monorepo, so its own semver tags are prefixed
(`go/vX.Y.Z`, not a bare `vX.Y.Z` — those track this repo's other bindings' own release
events instead). The core's static libraries under `staticlib/{goos}_{goarch}/` are
committed straight into git: unlike a real package registry, `go get`/`go build` has no
packing step of its own, so whatever is in the git tree at the resolved module version is
what a consumer links. `github.com/google/uuid` is the module's only dependency.

## Requirements

cgo, and so a C compiler wherever the module is **built** — nothing at run time:

| Building on / for | C compiler |
| --- | --- |
| Linux x64 / arm64, glibc or musl | gcc or clang (`build-essential`; on Alpine, `apk add build-base`) |
| macOS x64 / arm64 | the Xcode command-line tools (`xcode-select --install`) |
| Windows x64 | MinGW-w64 gcc |
| Windows arm64 | [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) |

Every other build fails at compile time, by name:

```
undefined: hyperuuid_needs_cgo_and_a_C_compiler_on_linux_darwin_or_windows_amd64_arm64__set_CGO_ENABLED_1__for_wasm_build_with_TinyGo
```

That is `CGO_ENABLED=0` (which is also Go's default for a cross-compile — see
[Building and cross-compiling](#building-and-cross-compiling)), any OS or architecture
outside the six above, and stock Go compiled to WebAssembly (`GOOS=wasip1`, `GOOS=js`): Go's
wasm toolchain links Go code only, with no cgo, so a foreign library has nowhere to go.
For WebAssembly, build with [TinyGo](#in-the-browser-tinygo), which links the core there,
browser included.

## In the browser (TinyGo)

[TinyGo](https://tinygo.org) 0.42 or later compiles this module to WebAssembly with the core
linked in, the way the C# package does for Blazor: TinyGo links with `wasm-ld` and has cgo,
so `backend_tinygo.go` names `staticlib/wasm/libhyperuuid.a` — the core built for
`wasm32-wasip1` — on its link line. No code changes and no extra files to ship: the core is
inside your `.wasm`.

```sh
tinygo build -target=wasm -no-debug -opt=z -o main.wasm .
cp "$(tinygo env TINYGOROOT)/targets/wasm_exec.js" .
```

```html
<script src="wasm_exec.js"></script>
<script>
  const go = new Go();
  WebAssembly.instantiateStreaming(fetch("main.wasm"), go.importObject)
    .then(result => go.run(result.instance));
</script>
```

Use TinyGo's own `wasm_exec.js`, not stock Go's: the core's one import, WASI's
`random_get`, is supplied by TinyGo's from `crypto.getRandomValues`. The core and this
binding add about 43 KB to an `-opt=z` build. `-target=wasip1` works the same way under a
WASI runtime (wasmtime, for one). `-target=wasip2` does not yet: TinyGo componentizes without
the preview1 adapter, and the core's `random_get` is a preview1 import. CI builds
[`internal/tinygosmoke`](internal/tinygosmoke) this way on every pull request and runs it
in headless Chrome.

TinyGo's cgo is narrower than Go's — no build constraints on `#cgo` lines, no `${SRCDIR}`,
no C structs by value — which is why the browser build has a backend file of its own
rather than more lines in `backend_static.go`; the file's header has the details.

## API

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

`NewV5` is the raw-byte form `NewV5String` converts into: a name is bytes, not text, and
nothing requires it to be valid UTF-8. An empty or nil name is valid and hashes the
namespace alone.

### Errors

Every function returns an `error`; nothing in this package panics. The errors are the
core's own failures and a caller's mistakes — never a failed load, since there is none. The
sentinels, all matched with `errors.Is`:

| Error | Returned when |
| --- | --- |
| `ErrRandomSource` | the core's random source failed — `NewV4`, the v6 and v7 generators, and their batch and Fill forms. `NewV5` draws no entropy and never returns it |
| `ErrTimestampOutOfRange` | `unixMillis` doesn't fit the version's own timestamp field — every v6 and v7 generator. Version 7 holds 48 bits of Unix milliseconds; version 6's 60-bit count of 100 ns ticks since 1582-10-15 runs out earlier, in the year 5236 |
| `ErrNotTimeBased` | `GetTimestamp` was given a UUID that isn't version 6 or 7 |
| `ErrNegativeCount` | `NewV6Batch`/`NewV7Batch` (and their `At` forms) were given a negative count. A count of 0 returns a nil slice |
| `ErrBufferNotWholeUUIDs` | a `FillV6Bytes`/`FillV7Bytes` destination isn't a multiple of 16 bytes long |
| `ErrNotOneUUID` | a raw-byte SQL-order transform was given a buffer that isn't exactly 16 bytes |

`ErrNativeUnavailable` is deprecated and never returned; it stays so code that tests for it
keeps compiling.

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

`go test -bench=. -benchmem ./...` from `go/`, 1000 UUIDs per op:

| method | ns/op | B/op | allocs/op |
| --- | ---: | ---: | ---: |
| `NewV7At` x1000 individually | 65,413 | 0 | 0 |
| `NewV7BatchAt(1000)` | 7,107 | 16,384 | 1 |
| `FillV7At` into an existing slice | 3,624 | **0** | **0** |
| `FillV7BytesAt` into an existing buffer | 3,599 | **0** | **0** |

`FillV6Bytes`/`FillV7Bytes` take a `[]byte` for callers who want raw RFC-ordered bytes rather than `uuid.UUID` values — a wire buffer or a database parameter. In Go the two forms are within 1% of each other, since neither converts; the byte form exists for convenience, not speed.

`NewV6BatchAt`/`NewV7BatchAt` now delegate to the fills, so the array-returning API is a single allocation with no intermediate copy — existing callers got faster without changing a line.

### Raw-byte SQL-order transforms

`V6/V7ToSqlOrderBytes` and `V6/V7FromSqlOrderBytes` apply the same native permutation as `V7ToSqlOrder` in place on a caller's 16-byte slice. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same check every binding in this repo now makes against the one native implementation.

## The native library: `Available`, `LoadError`, `NativeVersion`

The core is part of the binary, so there is nothing to load and nothing that can fail. The
three probes every binding carries are here so code written against them works unchanged:

- `Available()` — always `true`.
- `LoadError()` — always `nil`.
- `NativeVersion()` — `"major.minor.patch"` as the linked core reports it about itself, read
  through the ABI rather than from this module, so a deployment can confirm which build of
  the archive went into the binary. Its error is always `nil`.

```go
version, _ := hyperuuid.NativeVersion()
```

[HyperCast](https://github.com/SkunkWerkx/HyperCast)'s Go binding has the same three entry
points.

## Platforms

| Platform | Archive linked | Also links |
| --- | --- | --- |
| Linux x64 / arm64, glibc and musl (Alpine) | `staticlib/linux_amd64`, `staticlib/linux_arm64` | the C library |
| macOS x64 / arm64 | `staticlib/darwin_amd64`, `staticlib/darwin_arm64` | the C library |
| Windows x64 / arm64 | `staticlib/windows_amd64`, `staticlib/windows_arm64` | nothing (the C runtime) |
| WebAssembly under [TinyGo](#in-the-browser-tinygo) — browser, WASI | `staticlib/wasm` | wasi-libc, which TinyGo links anyway |

Each build names one archive on its link line and that is all it takes from this module:
the binary carries about 20 KB of core for its own platform, writes nothing to disk, and
starts without touching the filesystem.

**One archive serves glibc and musl.** cgo has no build constraint that tells the two C
libraries apart, so the Linux archives are the core built for the musl target, which asks
the C library for nothing glibc and musl have not both had for a decade (`getrandom`, and
`open`/`read`/`poll` for the fallback).

**Windows links the MSVC archive**, the same bytes the C# package's Native AOT publish
links. MinGW's linker reads MSVC's COFF objects, and the archive carries its own import stub
for `ProcessPrng`, the entropy source on Windows, so the link line names nothing else.

### Deploying

- **A fully static Linux binary** — `go build -ldflags '-linkmode external -extldflags
  -static'` gives a binary with no dependencies at all, which runs in an empty, read-only
  `scratch` container. It needs a static C library to link against; Alpine's musl has one.
- **Shipping to Alpine** — build on Alpine (`apk add build-base`), or link fully statically
  as above. An ordinary dynamically linked build from a glibc machine needs the builder's
  glibc, which a bare Alpine image does not have.
- **Anywhere else** — the binary needs only the C library it was linked against.

## Building and cross-compiling

`CGO_ENABLED` defaults to `1` on a native build and to `0` the moment `GOOS`/`GOARCH`
differ from the host:

```
$ go env CGO_ENABLED                # native
1
$ GOARCH=arm64 go env CGO_ENABLED   # cross, same OS, different arch
0
$ GOOS=windows go env CGO_ENABLED   # cross, different OS
0
```

So a plain cross-compile lands on the compile error above. Cross-compiling takes a cross
C compiler and `CGO_ENABLED=1` set explicitly:

```sh
CC=x86_64-w64-mingw32-gcc GOOS=windows GOARCH=amd64 CGO_ENABLED=1 go build ./...
CC=aarch64-linux-gnu-gcc  GOOS=linux   GOARCH=arm64 CGO_ENABLED=1 go build ./...
```

A native build with no C compiler installed fails in `runtime/cgo` with `C compiler "gcc"
not found` — install the one for your platform from [Requirements](#requirements).
GitHub's `ubuntu-latest` and `macos-latest` runner images ship one by default (`gcc` and the
Xcode command-line tools' `clang`).

This repo's own CI runs `go test ./...` natively, never cross-compiled, on every leg —
Linux and Windows on x64 and arm64, macOS on arm64 — and on Alpine.

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
   test vector instead of the wall clock.

(`google/uuid`'s own `NewV7()` *does* implement a real monotonic sub-millisecond
sequence — worth knowing if you're comparing the two, since a naively-random v7
generator would not.) Both libraries produce spec-valid, mutually interoperable
UUIDs; picking one over the other for v6/v7 generation is about these two
properties and, if your other services are in a different language, using the one
engine that's byte-for-byte identical everywhere.

## Benchmarks

`go test -bench=. -benchmem ./...` — allocation tracking is built into `testing.B`,
no extra tooling needed. Measured on linux-x64 (an Intel Core i9-11900H, Go 1.27), one
session, median of three runs:

| Call | Time, allocations |
| --- | ---: |
| `NewV4` | 60 ns, 0 allocs |
| `NewV5String` | 98 ns, 1 alloc |
| `NewV6At` | 54 ns, 0 allocs |
| `NewV7At` | 65 ns, 0 allocs |

No call allocates (the one `NewV5String` keeps is Go's own `[]byte(name)` conversion).
`go build -gcflags=-m` is right that any Go pointer handed to a cgo call is conservatively
heap-allocated, so the shims never hand one over: the C side keeps the sixteen bytes on its
own stack and returns them as a struct, and takes a UUID argument the same way, so nothing
crosses by pointer except a caller's own slice. The same by-value shape took 30-50% off
every door in HyperCast.

**Batch is where the per-call toll goes away.** Most of those ~60 ns is the cgo crossing
itself (about 40 ns — [the root README](../README.md#the-control-group-go) has why), not the
core's work, so one crossing for 1000 UUIDs instead of 1000
crossings is the whole win:

| Call | Time, allocations |
| --- | ---: |
| `NewV6BatchAt(1000, ...)` | 7.9 µs, 1 alloc |
| `NewV7BatchAt(1000, ...)` | 7.1 µs, 1 alloc |

against 65.4 µs for 1000 individual `NewV7At` calls (see
[Destination-buffer fills](#destination-buffer-fills), where `FillV7At` drops the one
allocation too). If your workload can batch, batch.

### Extraction vs. `google/uuid`'s own `Time()`

`google/uuid` isn't just a source type here — it has real extraction logic of its
own (`UUID.Time()`, documented as defined for versions 1, 2, 6, and 7), so it's a
genuine head-to-head, not a strawman. Same machine, same run:

| Call | hyperuuid | `google/uuid`'s `id.Time()` |
| --- | ---: | ---: |
| v6 | 30 ns, 0 allocs | 1.3 ns, 0 allocs |
| v7 | 29 ns, 0 allocs | 1.7 ns, 0 allocs |

`google/uuid`'s `Time()` wins outright, by roughly 20x — it's pure Go bit-shifting over
bytes already in the process, with no FFI boundary to cross.
`google/uuid.UUID.Time()` works on *any* RFC-conformant v6/v7 value regardless of
where it came from — it's pure bit math, not tied to how the value was minted — so
there's no provenance argument for reaching past it here. Honestly: in Go
specifically, prefer `id.Time()` over this binding's `V6Timestamp`/`V7Timestamp`
unconditionally. They exist for API symmetry with every other
binding in this repo, not because they're the better choice in Go.

## Verifying build provenance

(Not to be confused with the UUID-provenance point above — this is about the binary, not the
ID.) Go has no package registry to attest either — `go get` resolves straight from the
`go/vX.Y.Z` git tag against this repo. The static libraries committed under `go/staticlib/`
(staged by `stage-native-binaries.yml`, which verifies each one before committing it) each
carry their own build-provenance attestation from the build in `SkunkWerkx/.github` — so
verifying needs `--signer-repo` alongside `--repo`, or `gh` reports a bare `verifying with
issuer "sigstore.dev"` that reads like a bad signature but is only an identity mismatch:

```sh
gh attestation verify go/staticlib/linux_amd64/libhyperuuid.a \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
