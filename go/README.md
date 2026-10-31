# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperUuid/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperUuid/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**The same [`google/uuid.UUID`](https://pkg.go.dev/github.com/google/uuid) type your code already uses — minted by a shared Rust core instead of Go's own generator, so a Go service and a Python/Ruby/C#/whatever-else service agree byte-for-byte on every ID they produce.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with a
version 8 SQL Server order for v7, from the `hyperuuid` core **compiled as Go source**: the
core built for `wasm32-wasip1`, translated to one Go file by
[wasm2go](https://github.com/ncruces/wasm2go) and committed as
[`internal/core/core.go`](internal/core/core.go). The Go compiler builds it like any other
package, so there is no cgo, no C compiler, no archive in git, nothing embedded, nothing
extracted, nothing `dlopen`ed and nothing that can fail to load. `go get` is the whole
install, `CGO_ENABLED=0` is fine, and the module builds for every GOOS and GOARCH Go has,
the browser and WASI included, under stock Go and TinyGo alike.

```go
package main

import (
	"fmt"
	"log"
	"time"

	hyperuuid "github.com/SkunkWerkx/HyperUuid/go"
)

func main() {
	id, err := hyperuuid.NewV4()
	if err != nil {
		log.Fatal(err)
	}
	id2, err := hyperuuid.NewV5String(hyperuuid.NamespaceDNS, "example.com")
	if err != nil {
		log.Fatal(err)
	}
	id3, err := hyperuuid.NewV7()
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("v4:", id)
	fmt.Println("v5:", id2)
	fmt.Println("v7:", id3)

	ts, err := hyperuuid.V7Timestamp(id3) // recover the embedded UTC time.Time
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("timestamp:", ts)
	millis, err := hyperuuid.V7UnixMillis(id3) // the same instant as Unix-epoch milliseconds
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("unix millis:", millis)
	created, err := hyperuuid.GetTimestamp(id3) // ErrNotTimeBased unless id3 is an RFC 9562 v7
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("checked timestamp:", created)
	fmt.Println("is v7:", hyperuuid.IsRfc(id3, 7)) // the RFC 9562 variant and version 7, in one call

	sqlOrdered, err := hyperuuid.V7ToSqlOrder(id3) // a version 8 SQL Server's uniqueidentifier sorts by creation order
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("sql order:", sqlOrdered)
	sqlIsV7, err := hyperuuid.IsRfcIn(sqlOrdered, 7, hyperuuid.LayoutSqlServer)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("sql is v7:", sqlIsV7)
	sqlCreated, err := hyperuuid.GetTimestampIn(sqlOrdered, hyperuuid.LayoutSqlServer) // read in place
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("sql timestamp:", sqlCreated)

	// One call into the core, one random draw, one counter reservation for the whole batch:
	unixMillis := uint64(time.Now().UnixMilli())
	batch, err := hyperuuid.NewV7BatchAt(1000, unixMillis)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("batch:", len(batch))
}
```

## Install

```sh
go get github.com/SkunkWerkx/HyperUuid/go@latest
```

Requires Go 1.26 or later (the `go` directive in `go.mod`); no C compiler, and `CGO_ENABLED`
makes no difference. The import path ends in `/go` while the package is named `hyperuuid`,
so spell the name out in the import, as above — `goimports` adds it for you, but a
hand-written `import "github.com/SkunkWerkx/HyperUuid/go"` reads as if the package were
called `go`.

Go modules have no separate registry to publish to — `go get` resolves straight from a git
tag, which *is* the real, complete publish story here, not a placeholder for one. This
module lives in a subdirectory of the monorepo, so its own semver tags are prefixed
(`go/v<version>`, not a bare `v<version>` — those track this repo's other bindings' own release
events instead). The module has two dependencies: `github.com/google/uuid` for the `uuid.UUID`
type, and `golang.org/x/sys`, whose `cpu` package decides the [random source](#randomness).

## What this binding is

The Rust core is one `wasm32-wasip1` module of about 9 KB whose only import is WASI's
`random_get`. wasm2go translates it into about 45 KB of Go with no dependencies beyond the
standard library, and that file is committed and compiled with the rest of the package. The
binding keeps an *instance* of it: the translated module, a random source serving its
`random_get`, and a scratch region in the module's linear memory where arguments are copied
in and results copied out. Every public function is an ordinary Go call into that code;
nothing crosses a foreign boundary, which is why a call costs what the core's own work costs
and nothing more.

wasm2go's generated code bounds-checks every memory access and keeps the call stack in Go,
so a core bug, a guest stack overflow included, is a Go panic and never a corrupted heap.
The guest's stack is 64 KiB: regenerating the core at halving stack sizes and running the
whole suite, the memory receipts below and the exhaustion package at each, 256 bytes is the
smallest size that passes everything and 128 is the first that traps (a bounds panic from the
generated code, never a wrong UUID), so the shipped size is a 256x margin over the tested
paths, measured rather than proved.

wasm2go is pre-1.0 (v0.4.19 today) with one maintainer. It is pinned in a tool module of its
own, [`internal/wasm2go/`](internal/wasm2go), so it never enters your module graph, and
because its output is committed, a dead wasm2go would cost this repository its regeneration
step and cost a consumer nothing: the binding keeps working from the committed file.

## Randomness

The core asks for random bytes through WASI's `random_get`, and the binding answers from one
of two sources, chosen once per instance:

- **An AES-256-CTR keystream**, where Go's own AES is hardware-accelerated. The source is
  selected only when `golang.org/x/sys/cpu` reports the features Go's assembly requires for
  the architecture, the same rule Go's standard library applies to itself (Go 1.27,
  `crypto/internal/fips140/aes/aes_asm.go` line 24 and `aes_s390x.go` line 38): on amd64
  `X86.HasAES && X86.HasSSE41 && X86.HasSSSE3`, on arm64 `ARM64.HasAES`, on s390x
  `S390X.HasAES && S390X.HasAESCBC`. ppc64 and ppc64le take the operating system instead: Go
  assumes POWER8 there with no runtime detection, so acceleration cannot be established from
  outside. The stream is never selected under the `purego` build tag, under TinyGo, or when
  `crypto/fips140.Enabled()` is true; `GODEBUG=cpu.aes=off` turns it off too, since
  `x/sys/cpu` honours it. The generic AES it refuses to run on is variable-time software,
  which is the reason for the rule.
- **The operating system**, through `crypto/rand`, one checked `io.ReadFull` per
  `random_get`, everywhere else, including every WebAssembly host.

The stream is keyed with 32 bytes from `crypto/rand` and a zero IV, and rekeyed after 1 MiB
of keystream or one second, whichever comes first. The byte budget is enforced inside a
draw, so a draw that would cross it is cut at the boundary and finished under the new key;
the time check happens on entry, so buffered keystream never outlives its key's second; and a
rekey discards whatever the 512-byte buffer still held. A rekey allocates twice (the cipher
and the CTR; `testing.AllocsPerRun` over the complete rekey path measures 2) and nothing
allocates between rekeys, so the steady state of every call is zero allocations. Go programs
cannot fork without exec, so the stream needs no fork guard. Stock Go's `crypto/rand.Read`
never returns an error (since 1.24 it crashes instead), but TinyGo's can fail or fall short,
so every read is checked: a failure or a short read is WASI's `EIO` to the core, which the
caller sees as `ErrRandomSource`, never a UUID made of bytes that were not written. A reader
that delivers its bytes in several short successful reads is fine, as it is for
`io.ReadFull`.

Why not ChaCha: Go's ChaCha20 has no amd64 assembly, `math/rand/v2`'s ChaCha8 is eight
rounds, and `crypto/aes` runs on the CPU's AES instructions wherever the rule above selects
it, which is what made the batch competitive.

## One primary and a pool

The v7 counter lives in the instance's memory, so `NewV7`, its batch and `Fill` forms run on
one *primary* instance behind its mutex: one process-wide stream, as in every other binding.
`NewV4`, `NewV5` and every inspection call carry no state, so they run on the primary when its
lock is free and otherwise on an instance from a `sync.Pool`, created on demand with its own
random source. The pool bounds nothing (it is per-P, cleared across two GCs), so the number of
live instances tracks the goroutines concurrently inside stateless calls, about `GOMAXPROCS`;
an instance returns to the pool only while its scratch is still the base page, and pooled
instances never run a v7 call, so their counters are never used. The primary is created on
first use; importing the package costs nothing.

Measured on the Apple M6 (Mac18,5, 12 cores, Go 1.27.3, `-count 6`, benchstat medians) with
`go test -bench 'Parallel' -benchmem`, `GOMAXPROCS` goroutines each:

| Benchmark | ns per call, aggregate | allocs | |
| --- | ---: | ---: | --- |
| `NewV4` on 12 goroutines | 56.8 (±45% within the run; an earlier `-count 6` run gave 16.7 ±74%: the spread is the number) | 0 | the cgo binding this replaced: 459 |
| `NewV7At` on 12 goroutines | 141 | 0 | one stream, one lock; cgo: 207 |
| v4, v5, v7 and `Version` round-robin on 12 goroutines | 113 | 0, 0 GCs, 4 MiB heap in use | |

The same on Linux arm64, in Docker on the same Mac (3 runs, medians, a leg the spike put at
up to ±26% noise): `NewV4` on 12 goroutines 10.2 ns a call aggregate, where the cgo binding's
archive, drawing from the core's native ChaCha20 pool, ran at 4.9 ns there. **That is a loss
under contention on Linux, 2x**, down from the 130 ns a single mutexed instance measured in
the spike; the pool closes most of the gap, not all of it. `NewV7At` 152 ns, the mixed
round-robin 111 ns, 0 allocations and 0 GCs in both
([`linux-arm64-docker-m6-shipped.txt`](../docs/superpowers/spikes/2026-10-30-go-cgo-free/results/linux-arm64-docker-m6-shipped.txt)).

Under TinyGo everything serializes on the primary: its scheduler is single-threaded and its
`sync.Mutex` has no `TryLock`, and a second instance would buy nothing.

## Memory

A fresh instance is 3 WebAssembly pages of linear memory, 192 KiB: the 64 KiB stack and the
core's data in 2 pages, plus one page of scratch. The scratch grows to hold the largest v7
batch asked for (16 bytes per UUID) or the longest v5 name, and never shrinks, because
WebAssembly memory cannot; a batch of `MaxV7Batch` therefore leaves the primary holding 1 GiB
for the life of the process. Your own result slice is separate and yours. Three numbers per
shape, measured in a fresh process each (`HYPERUUID_RECEIPTS=1 go test -run TestReceipts -v
.`, Apple M6, darwin/arm64, `ru_maxrss`): the guest memory's logical length, what Go's
`append` retained for it, and the process's peak resident size, your buffer included.

| Shape | logical | retained | peak RSS |
| --- | ---: | ---: | ---: |
| One batch of `MaxV7Batch` (67,108,864 UUIDs) | 1,048,704 KiB | 1,048,704 KiB | 2,108,240 KiB |
| A batch of 1,048,576, then one of 1,048,576 + 4,096 (16 MiB of scratch grown by one page) | 16,576 KiB | 20,648 KiB (24.6% over) | 78,080 KiB |
| One v5 of `MaxV5Name` bytes, on its one-off instance | 2,097,344 KiB | 2,097,344 KiB | 4,207,840 KiB |

A large grow retains about what it needs; a small incremental grow of a large slice retains
about 1.25x the logical length, Go's slice growth policy, and during any grow the old
backing array and the new one are both live. Two failures are distinct: a growth the guest
cannot address (past 65,536 pages, or past `math.MaxInt >> 16` pages on a 32-bit host) is an
error to the caller, `ErrBatchUnaddressable` or `ErrNameTooLong`, decided by arithmetic before
anything is allocated; a growth the host cannot allocate is a runtime fatal, exactly as your
own `make([]uuid.UUID, n)` of the same size would be. On a 32-bit WebAssembly host a maximum
batch is out of reach for that reason: 1 GiB of result plus 1 GiB of guest scratch do not fit
a 32-bit address space, and the host's failure is fatal (stock Go: the runtime's out of
memory; TinyGo: an `unreachable` trap). Batches of ordinary size are fine there, and the two
tests that mint a maximum batch skip on such hosts.

## `MaxV5Name` and `ErrNameTooLong`

A v5 name is copied into guest memory and read there through a slice whose length may not
exceed the guest's `isize::MAX`, 2^31 − 1 on wasm32, whatever the host's word size; so
`NewV5` and `NewV5String` take a name of at most `MaxV5Name` bytes, the smaller of that limit
and what the guest and host can address beside the instance's two base pages and the 32
bytes before the name. On a 64-bit host that is 2,147,483,647. A longer name returns
`ErrNameTooLong`, with the length and the limit in the message, before anything is allocated.
A name that fits the base page (65,504 bytes) runs on the primary or a pooled instance like
any other stateless call; a longer one runs on a one-off instance sized for it and discarded
afterwards, so neither the primary nor the pool keeps the pages, and that instance draws no
entropy at all. A streaming v5 through the core's C ABI would restore an unbounded name and
is not planned.

## API

Returns [`github.com/google/uuid`](https://pkg.go.dev/github.com/google/uuid)'s
`uuid.UUID` — already RFC 9562 network-byte-order-identical to what the core
writes, so there's no byte-swapping in this binding. `NamespaceDNS`/`NamespaceURL`/
`NamespaceOID`/`NamespaceX500` are re-exports of `google/uuid`'s own (already
RFC 9562 §6.6-identical) namespace constants, kept here for API-shape symmetry with
the other bindings' `Namespaces.*`; `Nil`/`Max` (RFC 9562 §5.9/§5.10) are the same
kind of re-export. `V7Timestamp` recovers the embedded UTC `time.Time` from a version 7
UUID. `NewV7BatchAt(count, unixMillis)` generates `count` UUIDs sharing one timestamp
capture and one call into the core, instead of `count` of each. `V7ToSqlOrder`/`V7FromSqlOrder`
convert a version 7 UUID to and from SQL Server order: an RFC 9562 version 8 (§5.8) whose
8 and RFC variant sit at octets 7 and 8, where SQL Server's text form and a .NET `Guid` show
them, with the other 122 bits filled in the order SQL Server's `uniqueidentifier` compares,
so it sorts by creation order. It is computed once in the Rust core (and verified
there, and independently against the real `System.Data.SqlTypes.SqlGuid` comparator in the
C# binding's test suite) rather than reimplemented per binding. Every version 7 converts,
whatever its timestamp. The two are exact inverses, and refuse what they can't convert with
`ErrWrongVersion`, returning the zero UUID: to SQL order, anything that isn't an RFC 9562
version 7; from SQL order, anything not converted from one. `V7ToSqlOrder` reads its input as
RFC order, so a value already in SQL order is converted a second time about one time in 16,
when it happens to carry a 7 and the RFC variant there; keep track of which order a value is
in. `NewV7AtTime` accepts a `time.Time` directly in place of `NewV7At`'s raw millisecond
count. `GetTimestamp` is the checked counterpart to `V7Timestamp` — one core call checks the
variant and version and reads the timestamp, and `GetTimestamp` returns `ErrNotTimeBased`
for anything that isn't an RFC 9562 version 7 UUID, instead of assuming the caller already
knows. The variant is checked too: a 7 nibble under another variant has no version, and so
no timestamp. A version 6 or version 1 UUID from another generator has none here either.

`NewV5` is the raw-byte form `NewV5String` converts into: a name is bytes, not text, and
nothing requires it to be valid UTF-8. An empty or nil name is valid and hashes the
namespace alone; the longest name is [`MaxV5Name`](#maxv5name-and-errnametoolong).

### Version, variant and layout

`Version(id)` is the RFC 9562 version nibble, 0–15 (0 for `Nil`, 15 for `Max`), the same
answer as `id.Version()` but read by the core. `Variant(id)` returns a `UuidVariant`:
`VariantNcs` (includes `Nil`), `VariantRfc9562`, `VariantMicrosoft` or `VariantFuture`
(includes `Max`), never `VariantUnspecified`. `IsRfc(id, version)` asks both at once — the
RFC variant and that version nibble, in one call — and is the guard to run before
trusting a value's version-specific fields. A version outside 0–15 is simply false. All
three read RFC 9562 order. `Variant` has no layout form at all: `IsRfcIn` with
`LayoutSqlServer` is the check for a SQL-ordered value or a `uniqueidentifier` read-back.

A SQL-ordered value can be inspected where it is, without converting it back first. The
`In` forms take a `UuidLayout`, `LayoutRfc9562` or `LayoutSqlServer` (the version 8 layout
`V7ToSqlOrder` returns, and what a `uniqueidentifier` column holds):

```go
sql, _ := hyperuuid.V7ToSqlOrder(id3)
v, err := hyperuuid.VersionIn(sql, hyperuuid.LayoutSqlServer)   // 7
ok, err := hyperuuid.IsRfcIn(sql, 7, hyperuuid.LayoutSqlServer) // true
created, err := hyperuuid.GetTimestampIn(sql, hyperuuid.LayoutSqlServer)
millis, err := hyperuuid.V7UnixMillisIn(sql, hyperuuid.LayoutSqlServer) // also V7TimestampIn
```

In `LayoutSqlServer` a version is the one the value was converted from: `VersionIn` checks
for the layout's 8 and variant and answers 7, and 0 for bytes without them. `IsRfcIn` asks the same question for one version. The layout itself is
yours to track: sixteen bytes carry no mark of their order, and RFC-ordered bytes can happen
to carry the 8 and the variant there — about one random v4 in 16 does. Carry it with the
value, as a column type already does, and pass it at every call site that can see a
SQL-ordered value. The forms without a layout read RFC order, so `IsRfc(sqlOrdered, 7)` reads
the wrong bytes; there is deliberately no layout-agnostic check. `GetTimestampIn`
returns `ErrNotTimeBased` for anything that isn't an RFC 9562 v7 in the given layout. The zero
value, `LayoutUnspecified`, and any undefined layout return `ErrInvalidLayout`; a layout is
never guessed. `VersionBytes`, `VariantBytes` and `IsRfcBytes` take 16 raw bytes already in
the given layout's order (`VariantBytes` is RFC order only). Every one of these is
allocation-free.

### Binding to SQL Server

Bind `V7ToSqlOrder`'s result as a `[]byte`. go-mssqldb 1.11 sends an `mssql.UniqueIdentifier`,
and a string, in SQL Server's mixed-endian text order by default, which stores the first three
groups byte-reversed and loses the creation order ([details](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering)). A `[]byte` goes in as
binary and lands byte for byte, and scanning the column into a `[]byte` reads it back
untouched (`db` is a `*sql.DB` opened with go-mssqldb):

```go
import "github.com/google/uuid"

sqlOrdered, err := hyperuuid.V7ToSqlOrder(id3)
_, err = db.Exec("INSERT INTO orders (id) VALUES (@p1)", sqlOrdered[:])

var raw []byte
err = db.QueryRow("SELECT id FROM orders").Scan(&raw)
stored, err := uuid.FromBytes(raw) // ready for LayoutSqlServer
```

With `guid conversion=true` in the connection string, `mssql.UniqueIdentifier` passes its bytes
through unchanged in both directions too, though `[]byte` then reads back reversed.

### Errors

Every function that can fail returns an `error`; nothing in this package panics on an input. The errors are the
core's own failures and a caller's mistakes — never a failed load, since there is none. The
sentinels, all matched with `errors.Is`:

| Error | Returned when |
| --- | --- |
| `ErrRandomSource` | the random source failed: a failed or short read of `crypto/rand` (stock Go's never fails; TinyGo's can), or a rekey of the AES stream whose key could not be read — `NewV4`, the v7 generators, and their batch and Fill forms. `NewV5` draws no entropy and never returns it |
| `ErrCounterExhausted` | the v7 stream can't advance without running more than 1000 ms ahead of the supplied timestamp or past 2^44 − 2 — every v7 generator, batch and Fill forms included. Nothing is reserved; a call whose timestamp is past the stream's millisecond succeeds |
| `ErrTimestampOutOfRange` | `unixMillis` doesn't fit version 7's 48 bits of Unix milliseconds — every v7 generator |
| `ErrWrongVersion` | a SQL-order conversion was given a value it doesn't take: to SQL order, anything but an RFC 9562 version 7; from SQL order, anything not converted from one |
| `ErrNotTimeBased` | `GetTimestamp`/`GetTimestampIn` was given a UUID that isn't an RFC 9562 version 7 (in that layout): another version, version 6 included, or another variant |
| `ErrNegativeCount` | `NewV7Batch`/`NewV7BatchAt` was given a negative count. A count of 0 returns a nil slice |
| `ErrBatchTooLarge` | a v7 batch or fill asked for more than `MaxV7Batch` UUIDs, checked before anything is allocated or written |
| `ErrBatchUnaddressable` | the guest can't address `count*16` bytes of scratch. No batch reaches it: `MaxV7Batch` UUIDs are 1 GiB, within the guest's page limit even on a 32-bit host, where what fails instead is the host's own allocation, fatally, as yours would |
| `ErrNameTooLong` | `NewV5`/`NewV5String` was given a name longer than [`MaxV5Name`](#maxv5name-and-errnametoolong) bytes, decided before anything is allocated |
| `ErrBufferNotWholeUUIDs` | a `FillV7Bytes`/`FillV7BytesAt` destination isn't a multiple of 16 bytes long |
| `ErrNotOneUUID` | a raw-byte SQL-order transform, or `VersionBytes`/`VariantBytes`/`IsRfcBytes`, was given a buffer that isn't exactly 16 bytes |
| `ErrInvalidLayout` | a function that takes a `UuidLayout` was given `LayoutUnspecified` or an undefined value |

## Destination-buffer fills

`FillV7`/`FillV7At` write into a slice you already own instead of allocating a fresh one per call:

```go
import "github.com/google/uuid"

dst := make([]uuid.UUID, 1000)
for {
	if err := hyperuuid.FillV7(dst); err != nil {
		log.Fatal(err)
	}
	// reuse dst next iteration — nothing allocated
}
```

Go gets the best version of this API in the whole project. `uuid.UUID` is `[16]byte`, so a `[]uuid.UUID` is contiguous 16-byte records in exactly the RFC order the core writes — the batch is copied out of the guest's scratch straight into your slice, with **no per-element conversion**. (C# and Java can't do that; their UUID types aren't RFC byte order, so they must rebuild every element.)

`go test -bench=. -benchmem ./...` from `go/`, Apple M6 (Mac18,5), Go 1.27.3, `-count 6`,
benchstat medians, 1000 UUIDs per op:

| method | ns/op | B/op | allocs/op |
| --- | ---: | ---: | ---: |
| `NewV7At` x1000 individually | 37,220 | 0 | 0 |
| `NewV7BatchAt(1000)` | 2,809 | 16,384 | 1 |
| `FillV7At` into an existing slice | 2,080 | **0** | **0** |
| `FillV7BytesAt` into an existing buffer | 2,080 | **0** | **0** |

`FillV7Bytes`/`FillV7BytesAt` take a `[]byte` for callers who want raw RFC-ordered bytes rather than `uuid.UUID` values — a wire buffer or a database parameter. In Go the two forms cost the same, since neither converts; the byte form exists for convenience, not speed.

`NewV7Batch`/`NewV7BatchAt` delegate to the fills, so the array-returning API is a single allocation with no intermediate copy.

### Batch size and order

One v7 batch or fill takes at most `MaxV7Batch` UUIDs — 67,108,864, every one of the 2^20 steps a millisecond holds at 64 UUIDs to a step — and every batch up to that size is in strictly increasing order. A batch of n consumes ⌈n/64⌉ steps of one process-wide stream, and one that crosses a millisecond's last step splits into the next, so its tail carries a later timestamp (at most 1000 ms past the one supplied) and still sorts after its head. A larger batch would have to reuse counter values within one millisecond, so it returns `ErrBatchTooLarge` before anything is allocated or written. A batch is minted whole in the guest's scratch and copied out on success, so a failed batch or fill leaves your destination untouched; see [Memory](#memory) for what the scratch keeps.

Every v7 call reserves the next step of that stream, which is re-seeded at random whenever a timestamp moves it forward. For timestamps up to 2^44 − 2 (the year 2527):

- A `unixMillis` within 10 s of the newest timestamp the stream has reserved at either fails with `ErrCounterExhausted` or sorts strictly after every stream UUID whose call completed before it began, on any goroutine. Overlapping calls are ordered by reservation, which need not be completion order. A timestamp up to 11 s behind the stream is clamped onto its millisecond, so the embedded timestamp can run up to 11 s ahead of the supplied one.
- A `unixMillis` more than 11 s behind the stream, or past 2^44 − 2, is minted at itself, outside the stream: it succeeds, embeds its timestamp exactly and is ordered against nothing. One call far in the future therefore advances the stream, and until the clock catches up, real-time calls are minted out of stream and unordered among themselves. A clock that steps back by more than 11 s does the same.
- A call that would take the stream more than 1000 ms ahead of the supplied timestamp, or past 2^44 − 2, returns `ErrCounterExhausted` and reserves nothing. It is a state error, not an argument error: a call whose timestamp is past the stream's millisecond succeeds.

[v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely) has the full contract.

### Raw-byte SQL-order transforms

`V7ToSqlOrderBytes` and `V7FromSqlOrderBytes` apply the same conversions as `V7ToSqlOrder`/`V7FromSqlOrder`, with the same refusals, in place on a caller's 16-byte slice; a refused buffer is left as it was. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same check every binding in this repo makes against the one core.

## The core: `Available`, `LoadError`, `NativeVersion`

The core is compiled into the binary, so there is nothing to load and nothing that can fail.
The three probes every binding carries are here so code written against them works unchanged:

- `Available()` — always `true`.
- `LoadError()` — always `nil`.
- `NativeVersion()` — `"major.minor.patch"` as the core reports it about itself, a literal
  compiled into the translated file from the crate's version, so a deployment can confirm
  which core went into the binary. It cannot fail.

```go
log.Printf("hyperuuid core %s", hyperuuid.NativeVersion())
```

[HyperCast](https://github.com/SkunkWerkx/HyperCast)'s Go binding has the same three entry
points.

## Platforms

Every GOOS and GOARCH Go has, with `CGO_ENABLED=0`, because the core is Go source. CI
cross-compiles and vets the package for these 23 pairs on every push:

| GOOS | GOARCH |
| --- | --- |
| `linux` | `amd64`, `386`, `arm`, `arm64`, `riscv64`, `ppc64le`, `s390x`, `loong64`, `mips64le` |
| `windows` | `amd64`, `arm64`, `386` |
| `darwin` | `amd64`, `arm64` |
| `freebsd`, `openbsd`, `netbsd`, `illumos`, `plan9` | `amd64` |
| `aix` | `ppc64` |
| `android` | `arm64` |
| `js`, `wasip1` | `wasm` |

and runs the whole suite natively on Linux and Windows on x64 and arm64, on macOS on arm64
(Intel macOS is built by the sweep, not run), on Alpine (musl x64 and arm64), in an iOS
simulator, in an Android emulator, and on all three WebAssembly hosts below. A
binary carries the core like any package, writes nothing to disk and starts without touching
the filesystem; glibc or musl makes no difference, since a cgo-free Go binary takes nothing
from the C library, and a `scratch` image runs it as it is. The [randomness rule](#randomness)
decides which of the two sources an architecture gets; everything else is identical.

### iOS

`GOOS=ios GOARCH=arm64` builds the package as it builds any other. What an iOS executable
needs is Xcode's linker, because Go's `ios/arm64` port links externally; that is Go's rule
for the platform, nothing of this binding's, and it is what
[`gomobile`](https://pkg.go.dev/golang.org/x/mobile/cmd/gomobile) arranges. For the
simulator, Go's own `misc/ios/clangwrap.sh` is the `CC`:

```sh
CGO_ENABLED=1 CC="$(go env GOROOT)/misc/ios/clangwrap.sh" GOOS=ios GOARCH=arm64 go build ./...
```

CI runs this suite in an iOS simulator through Go's `misc/ios/go_ios_exec.go` and builds it
for a device with the iPhoneOS SDK's clang. Mac Catalyst is not a Go target.

### Android

`GOOS=android GOARCH=arm64 go build ./...` needs no NDK: Go links it internally. `android/amd64`
(the emulator) is a platform Go itself links externally, so it takes a C linker for that
reason alone, the NDK's clang as `CC` with `CGO_ENABLED=1`, as any Go program for it does;
nothing of the core's is linked either way, and page alignment is the linker's business
(NDK r28 and later align to 16 KB pages by default). CI cross-builds this whole suite for
`android/amd64` and runs it in an x86_64 emulator whose image uses 16 KB pages
(`.github/scripts/android_build_suite.sh` and `android_device_test.sh`, which run the same
way against a local emulator), with `-short` so the one 1 GiB batch test is left to the other
platforms; `android/arm64` is built in the cross-compile sweep. One more flag there is the
linker's: an `android/amd64` binary run with `GODEBUG=fips140=on` panics at start with
`fips140: verification mismatch` unless it was linked with
`-ldflags=-extldflags=-Wl,--apply-dynamic-relocs`. Go's linker reads the FIPS section addresses
back out of the linked file; GNU ld writes dynamic-relocation addends into the data, and the
NDK's lld does not unless asked, so Go hashes four empty ranges and the check fails against the
real ones at start (reproduced on `linux/amd64` with `-fuse-ld=lld`, Go 1.26). The CI build
passes the flag, and so must any Go program lld links that is to run in FIPS mode; the module
itself is not involved.

### WebAssembly

The translated core is plain Go, so it builds for the browser and for WASI under stock Go
and TinyGo alike, with no archive and no `cgo`:

```sh
GOOS=js GOARCH=wasm go build -o main.wasm .          # stock Go, the browser or Node
cp "$(go env GOROOT)/lib/wasm/wasm_exec.js" .

tinygo build -target=wasm -no-debug -opt=z -o main.wasm .   # TinyGo, the browser
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

Use the `wasm_exec.js` of the toolchain that built the module. `GOOS=wasip1 GOARCH=wasm` and
`tinygo build -target=wasip1` do the same for a WASI runtime (wasmtime, for one); TinyGo's
`-target=wasip2` does not yet work, since it componentizes without the preview1 adapter.
The randomness is the runtime's `crypto/rand`, which `wasm_exec.js` backs with
`crypto.getRandomValues`; no WebAssembly host can establish hardware AES, so the operating
system's source serves every call there. The binding adds about 141 KB to a TinyGo `-opt=z
-no-debug` build and 818 KB to a stock Go `js/wasm` one built with `-ldflags='-s -w'`, on top
of `fmt` and the runtime. Guest memory grows in 64 KiB pages; a maximum batch is out of
reach on these 32-bit hosts ([Memory](#memory)).

CI runs the whole suite three ways on every push, under TinyGo on wasip1 under wasmtime
(with the checkout mounted and the corpus found through `HYPERUUID_CORPUS`), under stock Go on
`js/wasm` under Node and on `wasip1/wasm` under wasmtime, and builds
[`internal/tinygosmoke`](internal/tinygosmoke) with TinyGo and runs it in headless Chrome.
Measured on the Apple M6, `-count 3` medians, 2026-10-31, 0 allocations per call in every
row; `google/uuid` 1.6.0 in the same runs; the raw output is committed
([`macos-arm64-m6-shipped-tinygo-wasmtime.txt`](../docs/superpowers/spikes/2026-10-30-go-cgo-free/results/macos-arm64-m6-shipped-tinygo-wasmtime.txt),
[`-shipped-js-node.txt`](../docs/superpowers/spikes/2026-10-30-go-cgo-free/results/macos-arm64-m6-shipped-js-node.txt)):

| Call | TinyGo 0.44 on wasip1, wasmtime 50 | stock Go `js/wasm`, Node 24 |
| --- | ---: | ---: |
| `NewV4` | 154 ns (`NewRandom()` 60 ns: **2.6x slower**) | 537 ns (`NewRandom()` 491 ns: **1.1x slower**) |
| `NewV5String` | 258 ns | 282 ns |
| `NewV7At` | 161 ns (`NewV7()` 152 ns: level) | 559 ns (`NewV7()` 605 ns: 1.1x faster) |
| `NewV7BatchAt(1000)` | 17.7 µs | 13.0 µs |
| `FillV7At` (1000) | 13.9 µs | 11.2 µs |
| `V7Timestamp` | 97 ns (`id.Time()`: 2.4 ns) | 35 ns (`id.Time()`: 4.0 ns) |

On both hosts `google/uuid` is level or ahead per single call, and the table says so. Each
of its ids is one `crypto/rand` read, which is exactly what the binding's OS source does for
the core's `random_get`; the translated core's own work sits on top of that read, and under
TinyGo that work is most of the call. A batch pays the read once, which is where the binding
wins on wasm: 1000 v7s for 13 to 18 µs against 540 to 560 µs of single calls.

## Building and regenerating

Nothing to build: `go test ./...` runs on the committed core. The core is regenerated only
after a change to `rust/` or to the pins:

```sh
rustup toolchain install 1.99.1 --profile minimal --target wasm32-wasip1
go generate     # in go/: builds rust/ for wasm32-wasip1 and rewrites internal/core/core.go
```

[`internal/wasm2go`](internal/wasm2go) is the generator: it builds the crate with the rustc
named in its `rust-toolchain.toml` (1.99.1) and a 64 KiB guest stack
(`-C link-arg=-zstack-size=65536`), then runs the wasm2go pinned in its own `go.mod`. The
rustc is pinned as a reproducibility constraint, not a floor: the wasm's bytes follow rustc's
codegen, so one named toolchain makes the committed file reproducible, and a bump is a
deliberate regenerate-and-commit. `HYPERUUID_CHECK_CORE=1 go test -run
'TestCoreIsCurrent|TestRegenerationCarriesTheCrateVersion' .` regenerates into a temporary
file and diffs it against the committed one (without the variable the test skips when the
toolchain is missing); CI runs it on every push, and release preparation regenerates the
core at the new version, since the version is a literal in the file.

## Why not `google/uuid`'s own `NewV7`?

This binding depends on `google/uuid` for the `uuid.UUID` type itself — it's already
in your import graph, and it already ships a working `NewV7()` of its own, with a real
monotonic sub-millisecond sequence. Two reasons to reach for this binding's generator
instead. **Explicit, testable timestamps**: `google/uuid`'s `NewV7` always reads the system
clock internally with no way to inject a specific instant, where every time-based generator
here — `NewV7At`, `NewV7BatchAt` and the `FillV7At` family — takes `unixMillis` as an explicit
parameter, so tests can assert against a fixed RFC test vector instead of the wall clock. And
**the same engine as every other binding**: if your other services are in a different
language, the one generator that's byte-for-byte identical everywhere, SQL Server order
included. On the machine below it is also faster per call, by 4.5-5.4x, which is the random
source: `google/uuid` asks the operating system for every id (the v7 half compares `NewV7At`,
handed its timestamp, with `NewV7()`, which reads the clock, so part of that gap is the clock). Both libraries produce
spec-valid, mutually interoperable UUIDs.

## Benchmarks

`go test -bench=. -benchmem ./...` from `go/` — allocation tracking is built into `testing.B`,
no extra tooling needed. Apple M6 (Mac18,5), macOS, Go 1.27.3, `-count 6`, benchstat medians,
2026-10-31, the raw output committed as the receipt
([`results/macos-arm64-m6-shipped-host.txt`](../docs/superpowers/spikes/2026-10-30-go-cgo-free/results/macos-arm64-m6-shipped-host.txt));
`google/uuid` 1.6.0 in the same run. The cgo column is the 1.0 binding, measured
on the same machine the day before by the spike's harness, a different harness, read as such
([the spike](../docs/superpowers/spikes/2026-10-30-go-cgo-free/)); the x64 columns await the
owner's XPS run and are not printed here.

| Call | hyperuuid | `google/uuid` | the cgo binding (1.0) |
| --- | ---: | ---: | ---: |
| `NewV4` | 37.0 ns, 0 allocs | `NewRandom()` 167 ns, 1 alloc: **4.5x faster** | 184 ns |
| `NewV5String` | 101 ns, 0 allocs | | |
| `NewV7At` | 37.3 ns, 0 allocs | `NewV7()` 202 ns, 1 alloc: **5.4x faster** | 72 ns |
| `NewV7BatchAt(1000)` | 2.84 µs, 1 alloc | 1000 `NewV7At`: 37.3 µs, 13.1x faster | 1.84 µs: **1.5x slower** |
| `FillV7At`, `FillV7BytesAt` (1000) | 2.07 µs, 0 allocs | | |
| `V7Timestamp` | 6.6 ns | `id.Time()` 1.4 ns: **4.8x slower** | |

No call allocates; `NewV5String`'s `[]byte(name)` conversion stays on the stack. Where the
v4 went, by the spike's receipts on this machine: the crossing was 9 ns (`version`, a call
that does nothing else, 12.4 ns through cgo against 3.2 translated), and the rest was the
random source. The native core asks `CCRandomGenerateBytes` for every id on Apple platforms,
as `google/uuid` asks the operating system; the translated core measured 164 ns a v4 with
`crypto/rand` per call and 30 ns with the AES stream. On Linux the native core already drew
from its own ChaCha20 pool, which is why single calls there are level with `google/uuid` and
1.4x behind the cgo binding rather than ahead of it. What costs more is the batch: the core walks 1000 items as bounds-checked Go rather than
native Rust, so a batch of 1000 is **1.5x behind the cgo binding** on this machine, the one
loss printed above, and still 13x ahead of 1000 single calls. If your workload can batch,
batch. Contention has its own table under [One primary and a pool](#one-primary-and-a-pool),
with a 2x loss under contention on Linux, and the browser and WASI numbers are under
[WebAssembly](#webassembly).

Linux arm64, in Docker on the same Mac (`golang:1.27`, 3 runs, medians; the spike put this
leg's noise at up to ±26%; the receipt is
[`linux-arm64-docker-m6-shipped.txt`](../docs/superpowers/spikes/2026-10-30-go-cgo-free/results/linux-arm64-docker-m6-shipped.txt)): `NewV4` 55.1 ns
against `NewRandom()` 57.8 ns, level; `NewV7At` 54.9 ns against `NewV7()` 98.6 ns, 1.8x
faster; `NewV7BatchAt(1000)` 3.3 µs against the cgo binding's 3.46 µs there, level in this run
(an earlier run of the same count gave 5.0 µs: the spread is this leg's). Against the cgo
binding's own single calls there (41 and 40 ns) both are **1.3-1.4x behind**: on Linux the native core already drew from its ChaCha20 pool, so there was
no per-call syscall for the stream to remove.

### Extraction vs. `google/uuid`'s own `Time()`

`google/uuid` isn't just a source type here — it has real extraction logic of its
own (`UUID.Time()`, documented as defined for versions 1, 2, 6, and 7), so it's a
genuine head-to-head, not a strawman. Same machine, same run: `V7Timestamp` 6.6 ns against
`id.Time()` 1.4 ns, 0 allocations either way. `google/uuid`'s `Time()` wins, by 4.8x — it's
pure Go bit-shifting over bytes already in hand, where this binding copies the UUID into the
guest and calls the core. `google/uuid.UUID.Time()` works on *any* RFC-conformant v7 value
regardless of where it came from, so there's no provenance argument for reaching past it
here. Honestly: in Go specifically, prefer `id.Time()` over this binding's `V7Timestamp`
unconditionally. It and `GetTimestamp` exist for API symmetry with every other binding in
this repo, not because they're the better choice in Go.

## Verifying build provenance

(Not to be confused with the UUID-provenance point above — this is about the code, not the
ID.) Go has no package registry to attest, and this module ships no binary: `go get` resolves
straight from the `go/v<version>` git tag against this repo, and what it fetches is source,
the generated core included. That file's provenance is reproducible generation, claimed
exactly: `internal/core/core.go` is the deterministic output of wasm2go over the crate at that
tag built with the pinned rustc, CI regenerates it with those pins on every push and fails on
any diff, and anyone can repeat it with `go generate` in a checkout
([Building and regenerating](#building-and-regenerating)). No attestation is claimed for the
intermediate wasm, which is not a release asset. The root README's
[Provenance](../README.md#provenance) section has the picture for the artifacts the release
does ship: the inventory, which are signed from where, and the gates every byte passes before
publication.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
