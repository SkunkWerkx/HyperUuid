# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperUuid/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperUuid/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation,
calling directly into the native `hyperuuid` core. Which way is chosen by the build, with
the same public API every time:

- **cgo on Linux and macOS links the core in** (`backend_static.go`). The core is a 20 KB
  static library on the link line: nothing is embedded, nothing is extracted, nothing is
  `dlopen`ed, and the binary runs from a read-only filesystem or a `scratch` image.
- **Everywhere else loads it** through [purego](https://github.com/ebitengine/purego)
  (`backend_purego.go`) — no cgo and no C compiler required. That is Windows always, and
  any build with `CGO_ENABLED=0`, which includes every cross-compile. This build embeds a
  shared library for every supported platform via `go:embed` and picks one at run time.
- **`-tags hyperuuid_wasm`** runs the same core as a WebAssembly module inside the process
  through wasmtime-go — see [WebAssembly (wasmtime-go)](#webassembly-wasmtime-go).

Either way `go get` is the whole install. cgo is also 4-6x faster per call than purego;
see Benchmarks below.

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
events instead). The native binaries — the shared libraries under `native/{rid}/` and the
static ones under `staticlib/{goos}_{goarch}/` — are committed straight into git: unlike a
real package registry, `go get`/`go build` has no packing step of its own, so whatever is in
the git tree at the resolved module version is what a consumer links or embeds.

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

Every function returns an `error`; nothing in this package panics. The sentinels, all
matched with `errors.Is`:

| Error | Returned when |
| --- | --- |
| `ErrNativeUnavailable` | the native library could not be loaded — any function, see [the next section](#the-native-library-available-loaderror-nativeversion) |
| `ErrRandomSource` | the core's random source failed — `NewV4`, the v6 and v7 generators, and their batch and Fill forms. `NewV5` draws no entropy and never returns it |
| `ErrTimestampOutOfRange` | `unixMillis` doesn't fit the version's own timestamp field — every v6 and v7 generator. Version 7 holds 48 bits of Unix milliseconds; version 6's 60-bit count of 100 ns ticks since 1582-10-15 runs out earlier, in the year 5236 |
| `ErrNotTimeBased` | `GetTimestamp` was given a UUID that isn't version 6 or 7 |
| `ErrNegativeCount` | `NewV6Batch`/`NewV7Batch` (and their `At` forms) were given a negative count. A count of 0 returns a nil slice |
| `ErrBufferNotWholeUUIDs` | a `FillV6Bytes`/`FillV7Bytes` destination isn't a multiple of 16 bytes long |
| `ErrNotOneUUID` | a raw-byte SQL-order transform was given a buffer that isn't exactly 16 bytes |

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
| `NewV7At` x1000 individually | 76,630 | 0 | 0 |
| `NewV7BatchAt(1000)` | 14,425 | 16,384 | 1 |
| `FillV7At` into an existing slice | 9,803 | **0** | **0** |
| `FillV7BytesAt` into an existing buffer | 9,446 | **0** | **0** |

`FillV6Bytes`/`FillV7Bytes` take a `[]byte` for callers who want raw RFC-ordered bytes rather than `uuid.UUID` values — a wire buffer or a database parameter. In Go the two forms are within 4% of each other, since neither converts; the byte form exists for convenience, not speed.

`NewV6BatchAt`/`NewV7BatchAt` now delegate to the fills, so the array-returning API is a single allocation with no intermediate copy — existing callers got faster without changing a line.

### Raw-byte SQL-order transforms

`V6/V7ToSqlOrderBytes` and `V6/V7FromSqlOrderBytes` apply the same native permutation as `V7ToSqlOrder` in place on a caller's 16-byte slice. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same check every binding in this repo now makes against the one native implementation.

## The native library: `Available`, `LoadError`, `NativeVersion`

In a cgo build on Linux or macOS the core is part of the binary, so there is nothing to
load and nothing that can fail: `Available()` is always `true` and `LoadError()` always
`nil`. Every other build loads the core once, on first use, and caches the outcome for the
life of the process. When that load fails, every function returns an error wrapping
`ErrNativeUnavailable` around the specific reason — an unsupported platform, no embedded
build for it, a failed extraction or `dlopen`, a core that doesn't export the ABI this
binding was built against. Three entry points probe the same outcome up front, without
generating anything:

- `Available()` — `true` when the native library (or, under `-tags hyperuuid_wasm`, the
  wasm module) loaded and exports the ABI this binding was built against: every symbol
  resolved and `hyperuuid_version` answered. A `false` is permanent for the process.
- `LoadError()` — `nil` when it loaded; otherwise the same error every other function
  returns, reason included.
- `NativeVersion()` — `"major.minor.patch"` as the loaded core reports it about itself, so
  a mismatch against the version this module was built for can be named before the first
  UUID. Returns the `LoadError` if the library did not load.

```go
if err := hyperuuid.LoadError(); err != nil {
	// errors.Is(err, hyperuuid.ErrNativeUnavailable) == true, and the message says why:
	//   hyperuuid: native library unavailable: dlopen failed: libgcc_s.so.1: cannot open
	//   shared object file: No such file or directory
	log.Fatal(err)
}
version, _ := hyperuuid.NativeVersion()
```

[HyperCast](https://github.com/SkunkWerkx/HyperCast)'s Go binding has the same three entry
points and the same `ErrNativeUnavailable`, with one difference that follows from its API:
its doors return `(value, *Fault)`, so they panic with that error where these functions
return it.

Where the core is loaded — purego, or cgo with `-tags hyperuuid_dynamic` — loading means
extracting the embedded library to a temp file and `dlopen`ing it from there
(`native_extract.go`). The file is created in `os.TempDir()` — `TMPDIR` moves it — once per
process, and is not removed at exit. A build that links the core in does none of that.

## Platforms

| Platform | cgo build | `CGO_ENABLED=0` (purego) |
| --- | --- | --- |
| Linux x64 / arm64, glibc | linked in: `staticlib/linux_amd64`, `staticlib/linux_arm64` | loads `native/linux-x64`, `native/linux-arm64` — needs glibc 2.34 or newer, and `libgcc_s.so.1` |
| Linux x64 / arm64, musl (Alpine) | linked in, the same two archives | loads `native/linux-musl-x64`, `native/linux-musl-arm64` — needs musl libc, nothing else |
| macOS x64 / arm64 | linked in: `staticlib/darwin_amd64`, `staticlib/darwin_arm64` | loads `native/osx-x64`, `native/osx-arm64` |
| Windows x64 / arm64 | purego regardless of cgo | loads `native/win-x64`, `native/win-arm64` |

Anything else — another OS, or an architecture such as 386 or riscv64 — is reported as
`unsupported platform {GOOS}/{GOARCH}` inside `ErrNativeUnavailable`, never guessed at.

**Linked in.** A cgo build names one static library on its link line and that is all it
takes from this module: the binary carries about 20 KB of core for its own platform, where
a build that loads carries every platform's shared library — 3.1 MB against 6.2 MB for a
program that does nothing else. It needs nothing at run time beyond the C library it was
linked against, so `-ldflags '-linkmode external -extldflags -static'` gives a binary with
no dependencies at all, which runs in an empty, read-only container. One archive serves
glibc and musl alike: cgo has no build constraint that tells them apart, and the archive
asks the C library for nothing both have not had for a decade. CI runs the suite against it
on Debian and on Alpine, on both architectures.

**Loaded, on glibc.** The glibc shared libraries reference symbols up to `GLIBC_2.34`, which
is Debian 12, Ubuntu 22.04, RHEL 9 and Amazon Linux 2023 or later. On an older glibc —
Debian 11's 2.31, say — the load fails with the loader's own ``version `GLIBC_2.33' not
found``. They also link `libgcc_s.so.1`, which every mainstream glibc distribution ships and
a minimal image may not: on `gcr.io/distroless/base` the load fails with `libgcc_s.so.1:
cannot open shared object file`, and on `gcr.io/distroless/cc` it works. None of this
applies to a build that links the core in.

**Loaded, on musl.** Which Linux shared library is loaded is decided at run time, by what
the process is actually running on: if `/proc/self/maps` shows a musl loader mapped
(`ld-musl-*` or `libc.musl-*`), the `linux-musl-*` build is used; otherwise, or if the file
can't be read, the glibc one. The musl builds depend on musl libc alone — no `libgcc`, no
`gcompat`. What that means for a build:

- **Built on Alpine** — every backend works. The default cgo build needs a C compiler
  (`apk add build-base`) at build time only, and links the core in. `CGO_ENABLED=0`
  (purego) needs none.
- **Built on a glibc machine, shipped to Alpine** — build with `CGO_ENABLED=0`, or link
  fully statically; an ordinary cgo build links the builder's glibc, which a bare Alpine
  image does not have. With `CGO_ENABLED=0`, Go's linker still writes glibc's loader into
  the binary by default, so on a bare Alpine image it fails to start (`not found`, exit
  127) before this module is ever reached. Name musl's loader instead and it runs:
  `CGO_ENABLED=0 go build -ldflags '-I /lib/ld-musl-x86_64.so.1' ./...` (verified on x64;
  arm64's loader is `/lib/ld-musl-aarch64.so.1`). Installing `gcompat` in the image also
  works, and the musl build is still the one selected.
- **The wasm backend** (`-tags hyperuuid_wasm`) does not link on musl: wasmtime-go's
  precompiled static library targets glibc.
- A version of this module from before the musl builds were added has no `linux-musl-*`
  directory to embed; there, a musl process gets `ErrNativeUnavailable` naming the
  missing file rather than a failed `dlopen` of the glibc build.

## cgo on darwin/linux, purego everywhere else

Earlier versions of this binding used purego unconditionally, on the reasoning
that a real cgo prototype only closed part of the allocation gap (see below) while
costing the module its one clean cross-platform story. Revisited: cgo is the
default on darwin/linux, with purego as the automatic fallback
(`//go:build !(cgo && (darwin || linux))`, `backend_purego.go`) — same public API,
selected entirely at compile time, no code changes needed by a consumer either way.

There are two cgo backends, and a build gets exactly one:

- `backend_static.go` — the default on amd64 and arm64. The core is linked in.
- `backend_cgo.go` — behind `-tags hyperuuid_dynamic`. It `dlopen`s the embedded shared
  library the way every cgo build did through 0.3.0, for a build that has to pick the core
  up at run time rather than at link time. Same speed per call; it is the loading that
  differs.

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
3 of 5 legs, not just purego via Windows — and both GitHub's `ubuntu-latest` and
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
- **A deployment that must not write an executable to a temp file, and cannot use cgo.** The
  purego backend has to (see `native_extract.go`); this one instantiates the module straight
  from the embedded bytes. Where cgo is available the default build already writes nothing:
  it links the core in.

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

Measured on the same linux-x64 machine as the tables below, `go test -tags
hyperuuid_wasm -bench=. -benchmem`:

| Call | cgo | wasmtime-go |
| --- | ---: | ---: |
| `NewV4` | 87 ns, 0 allocs | 2,380 ns, 9 allocs |
| `NewV5String` | 111 ns, 1 alloc | 2,808 ns, 14 allocs |
| `NewV7At` | 82 ns, 0 allocs | 2,384 ns, 11 allocs |
| `NewV7BatchAt(1000, ...)` | 14.4 µs, 1 alloc | 25.9 µs, 14 allocs |
| `FillV7At` (1000, existing slice) | 9.8 µs, 0 allocs | 21.3 µs, 13 allocs |
| `FillV7BytesAt` (1000, existing buffer) | 9.4 µs, 0 allocs | 21.4 µs, 13 allocs |

Per call it is 25-30x the native crossing; per UUID inside a batch it is a little
over 2x, and the allocations are wasmtime-go's own per-call argument boxing, not this
module's. The advice the Destination-buffer fills section gives applies here with more
force, not less: if the workload can batch, batch.

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

## Benchmarks

`go test -bench=. -benchmem ./...` — allocation tracking is built into `testing.B`,
no extra tooling needed. Measured on linux-x64 (an Intel Core i9-11900H, Go 1.27), one
session, median of three runs, each backend by the build that selects it (`go test
-bench=.` for cgo with the core linked in, `-tags hyperuuid_dynamic` for cgo loading it,
`CGO_ENABLED=0` for purego):

| Call | cgo, linked in (the default) | cgo, loading | purego | Speedup (linked vs purego) |
| --- | ---: | ---: | ---: | ---: |
| `NewV4` | **87 ns, 0 allocs** | 94 ns, 0 allocs | 378 ns, 4 allocs | **4.4x** |
| `NewV5String` | **111 ns, 1 alloc** | 127 ns, 1 alloc | 535 ns, 7 allocs | **4.8x** |
| `NewV6At` | **70 ns, 0 allocs** | 82 ns, 0 allocs | 406 ns, 5 allocs | **5.8x** |
| `NewV7At` | **82 ns, 0 allocs** | 84 ns, 0 allocs | 395 ns, 5 allocs | **4.8x** |

Linking the core in is worth 2-16 ns a call over loading it. That is a side effect, not
the reason it is the default — the reason is everything it removes from the binary and
from start-up, above.

Every purego call does 4-7 heap allocations; cgo now does none. An earlier edition
of this section called the one allocation cgo used to make "a structural floor for
this call shape (an out-param pointer into C)" — and it was a floor for *that*
shape, not for the ABI. `go build -gcflags=-m` is right that any Go pointer handed
to a cgo call is conservatively heap-allocated, so the shims stopped handing one
over: the C side keeps the sixteen bytes on its own stack and returns them as a
struct, and takes a UUID argument the same way, so nothing crosses by pointer except
a caller's own slice. The allocation is gone (the one `NewV5String` keeps is Go's own
`[]byte(name)` conversion); per-call time barely moved, because the allocation was
never the expensive part of the call. The same by-value shape took 30-50%
off every door in HyperCast, whose parsers had no such floor underneath.

**Batch generation is the one place cgo doesn't help — worth stating plainly rather
than only reporting the numbers where it wins:**

| Call | cgo | purego |
| --- | ---: | ---: |
| `NewV6BatchAt(1000, ...)` | 16.6 µs, 1 alloc | 16.5 µs, 6 allocs |
| `NewV7BatchAt(1000, ...)` | 14.4 µs, 1 alloc | 14.3 µs, 6 allocs |

Batch generation already collapses ~5000 individual-call allocations down to a
handful regardless of FFI mechanism (one native call, one random-bytes fetch, one
counter reservation for the whole 1000), so the marginal allocation win cgo brings
per-call has nothing left to amortize — purego is a statistical wash here on both
versions. If your workload is batch-heavy, the backend choice doesn't matter; if
it's dominated by individual calls, cgo's 4-6x per-call win is real.

### Extraction vs. `google/uuid`'s own `Time()`

`google/uuid` isn't just a source type here — it has real extraction logic of its
own (`UUID.Time()`, documented as defined for versions 1, 2, 6, and 7), so it's a
genuine head-to-head, not a strawman. Same machine, same run, both backends:

| Call | cgo | purego | `google/uuid`'s `id.Time()` |
| --- | ---: | ---: | ---: |
| v6 | 34 ns, 0 allocs | 326 ns, 4 allocs | 1.4 ns, 0 allocs |
| v7 | 32 ns, 0 allocs | 325 ns, 4 allocs | 1.9 ns, 0 allocs |

cgo is about 10x faster than purego here (42 ns when it loads the core rather than
links it), but `google/uuid`'s
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

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
