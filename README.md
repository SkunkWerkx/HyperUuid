# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hyperuuid.svg)](https://crates.io/crates/hyperuuid)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)
[![PyPI](https://img.shields.io/pypi/v/hyperuuid.svg)](https://pypi.org/project/hyperuuid/)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperUuid/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperUuid/go)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperUuid?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperUuid/tags)
[![RubyGems](https://img.shields.io/gem/v/hyperuuid.svg)](https://rubygems.org/gems/hyperuuid)
[![npm](https://img.shields.io/npm/v/hyperuuid.svg)](https://www.npmjs.com/package/hyperuuid)
[![CRAN](https://img.shields.io/cran/v/hyperuuid.svg)](https://cran.r-project.org/package=hyperuuid)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hyperuuid.svg)](https://packagist.org/packages/skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**One [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html)-compliant UUID engine, written once in Rust, called in-process — no bridge process, no per-language reimplementation — from C#, Java, Go, Swift, Ruby, PHP, Python, TypeScript, R, C and C++.**

What it gives every one of those languages, from the same compiled code:

- **The SQL Server order.** A v7 converted to the byte order a `uniqueidentifier` column sorts by (an RFC 9562 version 8), so an id minted on a client, before any insert, lands in a clustered index in creation order. [SQL Server ordering](#sql-server-ordering)
- **v7 ordering, with its limits written down.** One process-wide stream, strictly increasing across threads, single calls and batches, with every bound on clock steps, capacity and what the counter leaks stated. [v7 ordering, precisely](#v7-ordering-precisely)
- **Byte-identical v5.** The same namespace and name give the same 16 bytes in every language, checked against Python's own `uuid.uuid5`. A [conformance corpus](corpus/), derived by an oracle that shares no code with the core, pins v5, v7's fields and the SQL Server layout, and every binding's suite replays it.

Every other polyglot ID library either reimplements the same generation logic per language (drift risk: nine codebases that can each get the bit-twiddling subtly wrong in different ways) or ships a server/sidecar process to generate IDs centrally (a network round-trip for something that should cost nanoseconds). HyperUuid does neither: a single Rust core is compiled once and reached from inside each language's own process — over a plain C ABI (`P/Invoke`, FFM, `cgo`, `Fiddle`, PHP's `FFI`, and `.Call` into a vendored static library for R) or linked directly into the language VM as a native extension (PyO3 for CPython, Magnus for CRuby, napi-rs for Node and Bun) — sharing the *same* address space, the *same* generation logic, the *same* test vectors, on every platform. No runtime bridge, no serialization layer, no embedded interpreter.

Speed comes with it, measured rather than asserted: **in every language measured on linux-x64 except Go, generating a UUID through HyperUuid is as fast as the platform's own facility or faster, and usually several times faster** — 16x faster than `Foundation.UUID()`, 7.9x faster than `Guid.NewGuid()`, 5.5x faster than `SecureRandom.uuid`, 3.8x faster than CPython 3.14's own `uuid.uuid7()`, and in PHP ahead of a naive inline `random_bytes` v4 that validates nothing. Go is the one honest exception there, and it's the exception that proves the measurements are real — see [the control group](#the-control-group-go) below for exactly why, because the reason is interesting. TypeScript and R have so far been measured only on an Apple M6 Mac, and they lose calls there: TypeScript its v4 to `crypto.randomUUID()` on Node and Bun and its v7 to the `uuid` package on Bun (on Node it is 2.3x faster), and R its time-based ids, to the `uuid` package's v1, while its v4 is 12.7x faster than that package's ([their tables](docs/benchmarks.md#typescript-and-r-on-an-apple-m6)).

## Quick start

```csharp
// C# — dotnet add package HyperUuid (nuget.org)
using HyperUuid;

var id = UuidGenerator.NewV7();                     // time-sortable, RFC 9562 §6.2
var ts = UuidGenerator.V7Timestamp(id);              // recover the embedded timestamp
var batch = UuidGenerator.NewV7Batch(1000);          // 1000 IDs, one native call
```

```go
// Go — go get github.com/SkunkWerkx/HyperUuid/go@latest
import hyperuuid "github.com/SkunkWerkx/HyperUuid/go"

id, err := hyperuuid.NewV7()
ts, err := hyperuuid.V7Timestamp(id)
```

```ruby
# Ruby — gem "hyperuuid" (rubygems.org)
require "hyperuuid"

id = HyperUuid.new_v7
id.timestamp
```

Every binding follows the same shape — `new_v4`/`new_v5`/`new_v7`, a v7 batch variant, v7 timestamp extraction, the SQL Server order conversions, the RFC's `Nil`/`Max` constants, and a probe that says whether the native core loaded and which version it is (`UuidGenerator.IsAvailable`/`NativeVersion` in C#, and the same pair in each language's idiom), so a consumer with a fallback gates on the probe rather than catching a load failure. R has only the version half, `hyperuuid_native_version()`: its package does not load at all without its compiled code. See each language's own README (linked in the table below) for its exact idiom and install instructions.

## SQL Server ordering

`V7ToSqlOrder`/`v7ToSqlOrder`/`v7_to_sql_order` (C#/Java/Go/Swift/Python/TypeScript; Ruby's `#to_sql_order` and PHP's `->toSqlOrder()` are methods on the value, and R's is the vectorised `uuid_to_sql_order()`) converts an RFC 9562 version 7 UUID to the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order, and the `V7FromSqlOrder`/`v7FromSqlOrder`/`v7_from_sql_order` counterpart converts it back. `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed, non-sequential byte significance order. The converted value is an RFC 9562 **version 8**. RFC 9562 defines v7's layout in network byte order and sends any other byte order to version 8 (§6.11), so the conversion writes an 8 and the RFC variant where SQL Server's text form and a .NET `Guid` show them, and fills the other 122 bits in the comparator's order: the timestamp and counter (the two fields that determine creation order) in the most-significant bytes, then the trailing 48 random bits, which carry no ordering information, as one intact block in the least-significant ones. That is the permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) uses for C#, with an 8 where SequentialGuid keeps the 7. It is computed once in the Rust core and exported over FFI so every binding gets them from the same verified source instead of a nine-times-reimplemented one. Verified against the real `System.Data.SqlTypes.SqlGuid` comparator (not a hand-rolled stand-in) in the C# test suite; every other binding verifies the same sort behavior against a comparator replicating `SqlGuid`'s documented byte order.

**When this actually matters:** SQL Server already ships its own native answer to GUID-clustering fragmentation — [`NEWSEQUENTIALID()`](https://learn.microsoft.com/sql/t-sql/functions/newsequentialid-transact-sql) — but it only runs *inside* SQL Server, as a column default at insert time. It can't help a frontier device (a mobile client, an edge node, anything generating records before it ever talks to the database) that needs to mint its own final row ID *before* that insert happens. This feature is what closes that gap: generate a v7 UUID on the device, convert it to SQL order, and the exact same value that was minted on the frontier is what lands in the clustered `uniqueidentifier` column — no round trip to the server first just to obtain a key, and no swapping identities between a client-side temp ID and a server-assigned real one.

That value arrives at the server somewhat out of strict minting order — real sync/queue/wire latency between device and database means insert order isn't quite generation order — so it's not as perfectly gap-free as a value SQL Server assigned to itself the instant before insert. It's still a bounded, mostly-monotonic disorder window, not the fully uniform randomness of a v4 GUID spread across the entire 128-bit space — a materially smaller number of page splits than random insert order produces, even with realistic sync delay. And it solves a problem `IDENTITY(1,1)` can't solve at all, not just less efficiently: an `IDENTITY` value doesn't exist until *after* the row is physically inserted, so anything that needs to reference that row before then — most concretely, a many-to-many bridge/junction table's composite key, built at the same time as the rows it links — has to insert first and come back for the generated key. A client-generated v7 ID needs no such round trip; it's already known at the moment it's needed everywhere, the bridge table included.

**Why version 8.** The 8 keeps the value honest wherever it is read as a UUID: SQL Server's text, a .NET `Guid` and a driver's UUID type all show a version 8, a custom layout, so nothing reads a version 7 timestamp out of bytes that no longer hold one there. It costs nothing: a v7 and a v8 have the same 122 free bits, so every v7 converts and the conversion back is exact. The conversion refuses only a value that isn't a v7 going in, or doesn't carry the layout's 8 and variant coming back, with the binding's invalid-argument error.

That's also the load-bearing condition for this feature to matter at all — a random or naively-generated GUID only fragments a clustered index because SQL Server always maintains a clustered index's physical sort order on every insert; nothing has to be "turned on" for that to happen, but nothing here helps if the column isn't the clustered key in the first place. And plenty of real-world SQL Server schemas sidestep the whole problem by never clustering on the GUID at all — an `IDENTITY`/sequence integer as the clustered key, with the GUID kept as an ordinary non-clustered unique column — which remains a completely reasonable choice if there's no frontier-generated-ID requirement driving the decision.

**Binding it outside .NET.** SQL Server reads a UUID's text form mixed-endian and stores the first three groups byte-reversed. Every driver outside .NET that was tested sends its own UUID type the same way it sends text. So binding the converted value as a UUID, or as a string, stores bytes other than the ones `to_sql_order` produced, and the column does not sort by creation. Against SQL Server 2022 that held for mssql-jdbc 13.6, go-mssqldb 1.11 (by default), pyodbc 5.3, mssql-python 1.15, pymssql 2.4, tiberius 0.13, pdo_sqlsrv 5.13 and tiny_tds 3.4. Bind the 16 bytes as binary instead: SQL Server's binary-to-`uniqueidentifier` conversion keeps them as they are. Reading has the same trap in reverse, because those drivers hand back a UUID with its first three groups reversed. Select `CAST(col AS binary(16))`, or use the driver's raw-bytes getter, to get the bytes the SQL Server layout reads take. Each binding's README shows its own form. .NET is the exception: ADO.NET sends a `Guid` as its `ToByteArray()` bytes, so the `Guid` binds as it is.

**Reading a value still in SQL order.** A value read back from a `uniqueidentifier` column (as raw bytes outside .NET, per the paragraph above) can be validated and its timestamp read without permuting it back first. `Version`, `IsRfc`, `V7UnixMillis`, `V7Timestamp` and `GetTimestamp` each take a layout (`UuidLayout.SqlServer` and its spellings), and the core reads the permuted bytes in place: one native call per question instead of a permute and then a read. In SQL Server layout `Version` answers 7, the version a value was converted from. Bytes without the layout's 8 and variant read as version 0 with no timestamp. The layout is the caller's to know, not something a value can reveal: bytes in RFC order can happen to carry an 8 and the variant where the layout puts them (a random v4 does one time in 16), so keep the layout alongside the value, the way a column or a converter already does. For the same reason, a conversion handed raw bytes in the wrong order succeeds about one time in 16: converting a value already in SQL order to SQL order again, or converting a v7 still in RFC order back from it. C#'s `Guid` overloads refuse both every time, because a `Guid` puts its version nibble where the layout puts the 8: a SQL-ordered `Guid` reads as a version 8, and an RFC-ordered v7 shows its 7 there.

**The layout is persisted data.** A value written in SQL order sits in a clustered index, so moving one bit in the layout would silently reorder every existing row. Every property test (round trips, sort order) would still pass. The [conformance corpus](corpus/) pins the exact bytes instead, in both directions, along with the refusals. An oracle derives them in bit strings from RFC 9562's field table, and they are cross-checked against a port of SequentialGuid's permutation, which derives them from .NET's mixed-endian `Guid` layout and differs only in the 8.

## v7 ordering, precisely

Every v7 call reserves the next step of one process-wide stream. The stream is re-seeded at random whenever a timestamp moves it forward, so counters do not count UUIDs across milliseconds. Each millisecond holds 2^20 steps, a single call consumes one step and a batch of n consumes ⌈n/64⌉. These guarantees hold on targets with 64-bit atomics, for timestamps up to 2^44 − 2 (the year 2527). Let S be the newest timestamp among the stream reservations that have succeeded; failed and out-of-stream calls never change it.

- **Ordering.** A call whose timestamp is within 10 s of S either fails with `CounterExhausted` or reserves from the stream, and its UUIDs then sort strictly after every UUID from a stream reservation that completed before it began, on any thread. Overlapping reservations, singles and batches alike, are ordered by reservation, which need not be completion order. A timestamp up to 11 s behind the stream is clamped onto the stream's millisecond, and a millisecond whose steps run out splits into the next, so a batch that crosses the split is still strictly increasing.
- **Embedded timestamp.** It runs at most 11 s ahead of the supplied one: a clamped call carries the stream's millisecond, a split's tail carries the millisecond after it (at most 1000 ms ahead of the supplied timestamp), and an out-of-stream call carries its own timestamp exactly.
- **Backfill and future-dated calls.** A timestamp more than 11 s behind the stream, or past 2^44 − 2, is minted at itself, outside the stream: it succeeds, leaves the stream alone and is ordered against nothing. One call far in the future therefore advances the stream, and until the clock catches up, real-time calls fall behind it and are minted out of stream, unordered among themselves. A clock that steps back by more than 11 s does the same.
- **Capacity and exhaustion.** A millisecond entered by an advance holds at least 2^19 + 1 steps, and one entered by a split holds 2^20. Demand above that splits forward, and the stream may drift up to 1000 ms ahead of the supplied timestamp. A split that would take it further, or past 2^44 − 2, fails with `CounterExhausted` (return code 6 over the C ABI) before anything is reserved. It is a state error, not an argument error: it reserves nothing, and a call whose timestamp is past the stream's millisecond succeeds. No throughput floor is claimed.
- **What the counter leaks.** Only consumed steps, and only for stream-generated values. Within one millisecond the step difference between two values is exactly the number of steps consumed between them, and the raw counters of adjacent singles differ by 1 to 127. Across a split, from step s_a at one millisecond to step s_b at the next, the rollover-adjusted difference (2^20 − 1 − s_a) + (s_b + 1) is exactly the steps consumed between them; splits happen only while the stream is overloaded and ahead of real time. Across a real advance the re-seed hides the count. A lone step value shows at least step − (2^19 − 1) steps consumed in its millisecond. An out-of-stream value draws its own seed and relates to nothing.
- **A batch takes at most 67,108,864 UUIDs** (`MaxV7Batch` and its spellings). Every binding refuses a larger one with an argument error before allocating anything.
- **No 64-bit atomics.** Targets without them (`thumbv7em-none-eabi` and the like) keep a never-reset 26-bit counter that wraps every 67,108,864 UUIDs. None of the guarantees above apply there: a batch that straddles the wrap is stamped from the wrap on a millisecond later, and individual calls either side of the wrap sort in reverse.

The timestamp is the caller's, not read from the clock, so a clock that goes backwards by less than 11 s is absorbed by the clamp, and one that goes back further is minted out of stream.

## RFC 9562 coverage

| Version | Purpose | RFC section |
| --- | --- | --- |
| **v4** | Cryptographically random | §5.4 |
| **v5** | Deterministic, namespace + name, SHA-1 (cross-language interoperable — the same `(namespace, name)` pair produces the same UUID everywhere, including against Python's own `uuid.uuid5`) | §5.5 |
| **v7** | Time-ordered, 48-bit Unix-ms timestamp + a counter re-seeded each millisecond + random bits — strictly increasing within one process's stream, even under concurrent generation (see [v7 ordering, precisely](#v7-ordering-precisely)) | §6.2 |
| **v8** | Only as the [SQL Server order](#sql-server-ordering) of a v7: the same 122 bits in the order `uniqueidentifier` sorts by | §5.8 / §6.11 |
| **Nil** / **Max** | The all-zero and all-one special values | §5.9 / §5.10 |
| **Inspection** | Version, variant (NCS / RFC 9562 / Microsoft / Future), and a one-call "is this an RFC 9562 UUID of version N" guard, in RFC order or straight from [SQL Server order](#sql-server-ordering) | §4.1 / §4.2 |

v1 (classic time-based, leaks a MAC-derived node ID), v3 (MD5 name-based) and v6 (v1's fields reordered to sort) are deliberately not implemented. RFC 9562 treats v3 as superseded by v5, and v6 exists for systems that already speak v1: "Systems that do not involve legacy UUIDv1 SHOULD use UUIDv7 (Section 5.7) instead" (§5.6). A v7 embeds a timestamp `*Timestamp`/`*_timestamp` can recover on every binding; a v1 or v6 minted elsewhere still reports its version, and carries no timestamp here.

Inspection is layout knowledge, so it lives in the core like everything else. Every binding exposes the same three doors through it, each in its own spelling: `Version`, `Variant` and `IsRfc`. The version-agnostic `GetTimestamp` applies the same guard: a 7 nibble under another variant is no RFC version, so it carries no timestamp. The guard is what a wrap constructor or a deserializer runs on every value: `IsRfc(id, 7)` is true only for the RFC variant with version nibble 7. A platform's own `Guid.Version` or `UUID.version()` reads the nibble alone, so a Microsoft-variant value with a 7 in it passes that check.

## Why not your platform's built-in UUID call?

Most languages *do* already have one — `Guid.NewGuid()`, `java.util.UUID.randomUUID()`, `uuid.uuid4()`, `SecureRandom.uuid`. HyperUuid isn't arguing you should never use those. It's for the specific, common situation where you need more than plain v4 randomness gives you:

1. **Time-sortable IDs with real ordering guarantees.** A v7 ID minted a microsecond after another one you generated on the same thread will sort after it — HyperUuid's process-wide v7 stream (RFC 9562 §6.2 Method 1) guarantees that even under concurrent generation. Most stdlib v4 generators have no time-ordering story at all, and even a stdlib that *does* offer v7 (C#'s `Guid.CreateVersion7`, added in .NET 9) doesn't implement the counter, so two IDs minted in the same millisecond sort randomly relative to each other — exactly the index-fragmentation problem v7 adoption is meant to solve in the first place.
2. **One generation engine across a polyglot system.** If your API is C#, your batch jobs are Go, and your data pipeline is Python, plain per-language UUID libraries give you three independent implementations that all *should* agree bit-for-bit on RFC 9562 semantics but have no structural reason to. HyperUuid's v5 namespace UUIDs are verified in CI to match byte-for-byte with Python's own `uuid.uuid5` — because it's the literal same Rust code minting them everywhere, not three ports of the same spec.
3. **Batch throughput, and a byte-level door under it.** Need to backfill a million IDs? `NewV7Batch`/`new_v7_batch`/`NewV7BatchAt` (binding-dependent naming) shares one timestamp capture and one contiguous counter reservation across the whole batch instead of paying per-item overhead N times — 1.3-15x faster than the equivalent loop depending on binding ([numbers below](docs/benchmarks.md#batch-generation-vs-an-equivalent-loop) and in each binding's README). Underneath that, every binding also exposes a raw-bytes form that constructs no UUID objects at all, which beats its own batch by **41x** in Ruby, 36x in Python and 16x in PHP and lands every language measured on linux-x64 on the same floor, 3.6-4.9 µs per 1000 UUIDs ([numbers below](docs/benchmarks.md#skipping-object-construction-entirely)). Most stdlib UUID facilities have no batch API at all, let alone both. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)
4. **It's not slower for the trouble — it's faster, mostly.** Generation beats the platform's own call outright in every language measured on linux-x64 except Go. TypeScript, measured so far only on an Apple M6 Mac, loses its v4 there on both runtimes and its v7 on Bun, and R's time-based ids lose to the `uuid` package's v1 (see [the scoreboard](#the-scoreboard)).

The honest trade-off: this is one more native dependency to ship (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll`, or a prebuilt extension for the Python/Ruby fast paths) versus a UUID call that's already sitting in your standard library. If you only need plain v4 randomness and don't care about cross-language consistency, the stdlib call is simpler and that's a completely reasonable choice.

## The scoreboard

By language — real numbers, no adjustment for story, the receipts in [docs/benchmarks.md](docs/benchmarks.md) and each binding's own README.

| Language | Generation vs. the platform's own call | The platform's own call |
| --- | --- | --- |
| [Swift](swift/) | **11-16x faster** | `Foundation.UUID()` |
| [C#](csharp/) | **7.5-7.9x faster** | `Guid.NewGuid()` |
| [Ruby](ruby/) | **2.1-5.5x faster** | `SecureRandom.uuid` |
| [Python](python/) | **3.4-4.2x faster** | `uuid.uuid4()`-`uuid7()` |
| [Java](java/) | **2.8-5.1x faster** | `UUID.randomUUID()` |
| [Rust](rust/) | **1.7-2.4x faster** | the `uuid` crate |
| [PHP](php/) | **1.2-1.4x faster** | a naive inline v4 (PHP core has no UUID call at all) |
| [R](r/) | **12.7-29x faster** for v4; **1.2-1.3x slower** for time-based ids, a v7 against a v1 | the `uuid` package's `UUIDgenerate()` |
| [TypeScript](typescript/) | **1.2-6.4x slower** per call, except v7 on Node: 2.3x faster | `crypto.randomUUID()`, the `uuid` package's `v7()` |
| [Go](go/) | slower per call — [the control group](#the-control-group-go) | `google/uuid` |

The R and TypeScript rows were measured on an Apple M6 (Mac18,5) under macOS, every other row on linux-x64 on an Intel Core i9-11900H, so the two groups are not comparable with each other. The linux-x64 rows' v7 figures were measured before the per-millisecond re-seed (core change, 2026-10-27); the Apple M6 rows after it, and after the switch to `CCRandomGenerateBytes` on Apple platforms (2026-10-28).

**Regardless of where your language lands above:** if SQL Server is your RDBMS, [SQL Server ordering](#sql-server-ordering) below might be reason enough to reach for this on its own — the only practical way to mint a client-side ID on a frontier device and have it arrive already sorted for clustering, something `NEWSEQUENTIALID()` structurally can't do (server-side only) and something `IDENTITY(1,1)` can't do at all for a value — a many-to-many bridge table's composite key, most concretely — that needs to exist before the row does.

## The control group: Go

Go's per-call numbers lose to `google/uuid`, and the reason is worth stating precisely, because it's what makes the rest of the scoreboard credible.

Go's boundary cost isn't marshalling — it's `runtime.cgocall` defending Go's concurrency model. Goroutines run on tiny growable stacks the C ABI can't execute on, so every cgo call switches to the OS thread's system stack and does scheduler bookkeeping (so a blocking C call can't starve the scheduler), then unwinds it all on return: ~40ns on the box measured here, structural, and not diet-able. There is no PyO3-for-Go, because the thing being paid for isn't an interface layer that could be replaced — it's the runtime itself. The binding links the core straight into the binary and passes UUIDs to and from C by value, so a call allocates nothing — cgo with the toll and nothing else is already the best available door.

And on the far side of that boundary sits the only competition in the roster that plays by the same rules as the Rust core: `google/uuid` is pure compiled Go with no boundary at all, written by people who understand scale — a handful of bit shifts for `.Time()`, no culture machinery, no interpreter. When the native work costs tens of nanoseconds, a toll of the same size again can never amortize on a single call. Every other language's built-in lost to HyperUuid across a *smaller* boundary; Go's won across a *larger* one because its stdlib is genuinely that good. That's the control group: it demonstrates the benchmarks reward real speed, not story.

So the honest guidance is narrower for Go than for any other binding here: reach for it in exactly two situations. Either SQL Server is your RDBMS and you cluster on `uniqueidentifier` columns — the [SQL-order transforms](#sql-server-ordering) come from the same verified core as every other language's, which matters precisely when a Go service is minting IDs into the same tables a C# service reads — or you're bulk-generating, where the batch doors divide the toll by N and Go's numbers land right next to everyone else's (see [benchmarks](docs/benchmarks.md)). For everything else, use `google/uuid` — including its own `.Time()` for timestamp extraction, which wins outright even against this binding. The binding stays in the roster as a thought experiment and the control baseline the other nine languages are measured against; pretending it's more than that would cost this README its credibility.

## Platform support

Every language, on every platform, proven for real: `.github/workflows/ci.yml`'s `build-native` matrix builds the Rust core fresh on each of 5 real-hardware legs, then runs that language's actual test suite against that leg's freshly-built native library — not just that it compiles. A second job does the same for the two musl RIDs inside real Alpine containers, on the language's own official `*-alpine` image. Intel macOS has no leg of its own; see the osx-x64 note under the table.

| Language | linux-x64 | linux-arm64 | linux-musl-x64 | linux-musl-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [crates.io](https://crates.io/crates/hyperuuid) |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [NuGet](https://www.nuget.org/packages/HyperUuid) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `go get` (git tag) |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `.package(url:)` (git tag) |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | Fiddle, built | ✅ | ✅ | ✅ | [RubyGems](https://rubygems.org/gems/hyperuuid) |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | — | [Packagist](https://packagist.org/packages/skunkwerkx/hyperuuid) |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [PyPI](https://pypi.org/project/hyperuuid/) |
| [TypeScript](typescript/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [npm](https://www.npmjs.com/package/hyperuuid) |
| [R](r/) | source | source | source | source | source | source | source | experimental | [CRAN](https://cran.r-project.org/package=hyperuuid) |
| [C, C++](c/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | GitHub Release (one archive per RID) |

The cells that are not a plain ✅, and why each is deliberate:

- **osx-x64 (Intel macOS): built, and tested at the core, in TypeScript and in Java only.** There is no Intel macOS CI leg. The library is cross-compiled on the Apple silicon runner, attested, and shipped in every package, and the Rust core's own suite runs on it there under Rosetta 2, which is why that row keeps its ✅. TypeScript's and Java's are the two binding suites that run on Intel macOS, TypeScript's on an x64 Node under Rosetta 2 against an addon cross-built there, Java's on x64 Temurin 21 and 25 under Rosetta 2 (the JNI path and the FFM path), which is why their rows have a ✅; every other one still runs on Apple silicon against the same source. Ruby there installs the universal gem and runs on Fiddle, since no precompiled gem is built for it. The leg took 37 minutes against 8 on Apple silicon, for hardware Apple stopped selling in 2023 on runners GitHub retires by Fall 2027.
- **PHP on win-arm64.** PHP has never shipped a native Windows ARM64 build, so it runs as an x64 process there regardless of host CPU and loads the win-x64 library — already exercised for real by the win-x64 leg.
- **TypeScript on Bun.** The suite runs on Node on every leg and in both musl containers, and on Bun everywhere but win-arm64, where Bun is not yet verified, and osx-x64, whose cross-build leg runs Node alone.
- **C and C++: one archive per RID, plus Android, iOS, Mac Catalyst and WebAssembly.** CI packs every archive from the run's own libraries with the release's packer, then on each RID's runner builds a C consumer and a C++23 one against it through CMake, and the C one through pkg-config, shared and static (osx-x64 under Rosetta 2, musl in Alpine). The iOS, iOS simulator and Mac Catalyst archives are static only and built against on a Mac: linked for a device, run in the simulator, and run as Mac Catalyst on both architectures. The android-arm64, android-x64 and wasi-wasm32 archives are packed but not run: no runner executes them, and their libraries are the ones C#, Go and Swift already test there.
- **R: source.** R ships a source package: CRAN compiles it for each platform it serves and builds its own Windows and macOS binaries, and a source install needs Rust 1.89 or newer. CI runs `R CMD check --as-cran` on linux-x64, linux-arm64, osx-arm64 and win-x64, on the current R and, on linux-x64, the previous minor too; the musl and osx-x64 cells are source builds no job here checks. On win-arm64 the build picks `aarch64-pc-windows-gnullvm`; R on Windows ARM64 is experimental, and that build is untested.

The musl libraries are built so that they depend on musl's libc and nothing else — the unwinder is linked statically — which is what lets them load on a bare `alpine` or `python:alpine` image with no `libgcc` installed. The glibc libraries need glibc 2.34 or newer. The TypeScript addon links Rust's std, so that floor is not its floor; its own is not stated here yet.

**Runtime floors** follow upstream support: a version that has reached end of life is not a floor. Today that is Go 1.26, Python 3.11, Ruby 3.3, PHP 8.3, Node 22 and .NET 10, and the musl job runs the PHP, Ruby, Python and TypeScript suites on those oldest versions as well as the newest (for Node, on 22, 24 and 26). R follows CRAN's window instead: CI checks the current release and the previous minor, and the package declares R 4.2. C# covers Windows, Linux, macOS and Blazor WebAssembly on .NET 10; iOS, Mac Catalyst and Android need .NET 11. JDK 21 sits above that line, though 17 is still supported upstream (Temurin to 2027-10-31): Temurin builds no JDK 17 for Windows arm64 or musl arm64, two of the jar's eight platforms. Swift's floor is 6.2 for a different reason: it is the first release whose package manager can link a static library, which is how the binding reaches musl and WebAssembly at all. CI runs it on 6.2 as well as 6.4. The crate's own floor is Rust 1.89 (`rust-version` in its manifest, the floor of the current `uuid` 1.x), and CI checks the library, with and without std, on exactly that toolchain.

How each binding links the core (into the executable for Swift, Go and C#'s Native AOT), iOS, Mac Catalyst and Android, and how each one is published: [docs/platforms.md](docs/platforms.md).

## WebAssembly

Seven of these languages compile into a WebAssembly app with the core linked in: Rust on `wasm32-wasip1`,
C# in Blazor WebAssembly (.NET 10+), Python under Pyodide, Ruby through `rbwasm build`, Go
under TinyGo, Swift with swift.org's WebAssembly SDK, and C from the wasi-wasm32 archive. PHP's
is proven and unshipped, Java's is blocked on its compilers, and TypeScript and R are not built.
[docs/platforms.md](docs/platforms.md#webassembly) has the table, binding by binding, and why
each gap is where it is.

## Provenance

Every file a release ships verifies from the tag alone. One release attestation, signed by
this repository's own `release.yml` at the tag, names every registry package (the `.nupkg` as
packed and as nuget.org serves it, the gems, the wheels and the sdist, the crate, the Maven files, the nine
npm tarballs and the R source tarball), Swift's two zips and the fifteen C archives on the
GitHub Release, and every native library committed for Go and PHP, and its predicate is the release inventory: every file's path, size, SHA-256, the commit
it was built from and the repository that signed its build provenance. One command, any file:

```sh
gh attestation verify <file> --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Which identity signs each artifact's build provenance, and the gates on the way in: [docs/provenance.md](docs/provenance.md).

## Key features

- **RFC 9562 compliant** — correct version nibble and variant bits on every UUID, from every binding, because they all come from the same Rust core
- **One implementation, every call site** — no per-language reimplementation to drift out of sync; v5's SHA-1 hashing, v7's counter stream and the SQL Server permutation are each written exactly once
- **Faster than the platform's own call** — in every language measured on linux-x64 except Go, measured per binding with that ecosystem's own benchmark harness; TypeScript and R, measured so far only on an Apple M6, lose calls there ([their tables](docs/benchmarks.md#typescript-and-r-on-an-apple-m6))
- **Monotonically increasing v7** — a process-global stream (RFC 9562 §6.2 Method 1) guarantees strict ordering under concurrency, continued correctly across individual *and* batch calls, and a batch stays in order across a millisecond's end ([precisely](#v7-ordering-precisely)); when the stream cannot advance, the call fails with `CounterExhausted`
- **Batch generation** — `NewV7Batch`/`new_v7_batch` amortizes timestamp capture, counter reservation, and the random-bytes fetch across the whole batch
- **Fork- and snapshot-aware randomness** — the random bits come from the OS (`getrandom`, Windows' `ProcessPrng`, Apple's `CCRandomGenerateBytes`) or, on Linux and Android, a ChaCha20 keystream keyed from it. Linux buffers keystream in a mapping marked `MADV_WIPEONFORK`, so a forked child starts empty and keys its own (a kernel that cannot honour that flag skips the buffer), and no key outlives a second of wall-clock time, so a process restored from a snapshot (AWS Lambda SnapStart, CRaC, a cloned VM) throws away what it held, unless the snapshot was taken within a second of a key being drawn and restored onto a clock that has not moved on ([the details](rust/src/entropy.rs)). Tests on Linux and Apple fork a child and require it to draw different bytes
- **SQL Server byte ordering** — `V7ToSqlOrder`/`v7_to_sql_order`, as an RFC 9562 version 8, computed once in the Rust core and exported to every binding, verified against the real `System.Data.SqlTypes.SqlGuid` comparator, and validated and read in place without converting back
- **Inspection** — version, variant and a one-call RFC guard, in either layout, from the core
- **One conformance corpus** — [`corpus/*.json`](corpus/) pins v5 derivation, the v7 field layout, the SQL Server layout and its refusals, timestamp extraction and inspection. The Rust suite and every binding's suite replay it, and `corpus/oracle.py` re-derives every vector from code that shares nothing with the core
- **No runtime bridge** — direct FFI (`P/Invoke`, FFM, JNI, `cgo`, `Fiddle`, PHP `FFI`, R's `.Call`) or the Rust core linked directly into the VM as a native extension (PyO3, Magnus, napi-rs), never a serialization protocol — with Ruby's zero-compile `Fiddle` fallback kept fully supported and test-verified against the Magnus fast path
- **Genuinely allocation-free where it counts** — verified with a counting allocator in Rust and `[MemoryDiagnoser]` in C#, not just claimed
- **AOT-friendly** — C# publishes cleanly under `PublishAot`, with the core linked into the executable rather than sitting beside it; Java's JVM binding survives a real GraalVM Native Image build into a standalone native binary, no JVM required to run it
- **CI-proven, not CI-claimed** — 5 real-hardware platforms plus two musl RIDs in Alpine containers × 10 language/runtime targets (R is checked on four of the five hardware legs and not in Alpine; Intel macOS is cross-built and tested at the core, in TypeScript and in Java), each running that language's actual test suite against a freshly-built native library on every dispatch

## Layout

```
rust/       the core: twelve uuid_* exports plus hyperuuid_version, allocation-free and no_std; a cdylib (and two for Android), fifteen static archives (four for iOS and Mac Catalyst, two for Android) and the Blazor WebAssembly one
corpus/     the conformance vectors every suite replays, and the independent oracle that derives them
csharp/     the .NET 10 and 11 binding: UuidGenerator over LibraryImport, AOT smoke test, Blazor wasm
java/       the JDK 21+ binding: JNI on 21–24 and FFM on 25+ from one multi-release jar,
            Native Image smoke test
java/jni/   the JNI library, a no_std Rust cdylib linking the core in
python/     the 3.11+ binding: PyO3 native extension (abi3 and free-threaded wheels, Pyodide in the browser)
swift/      the SwiftPM binding: the core linked in as a static library on every platform
go/         the Go binding: the core linked in through cgo, and under TinyGo in the browser
ruby/       the 3.3+ binding: Magnus extension + Fiddle fallback
php/        the 8.3+ binding: ext-ffi (an ext-php-rs extension spike, proven in the browser, unshipped)
typescript/ the Node 22+ and Bun binding: a napi-rs addon with the core linked in, one npm platform package per RID
r/          the R 4.2+ binding: .Call into a C shim over the core's static library, built from vendored crates
include/    hyperuuid.h, the C header, generated by cbindgen from rust/src/ffi.rs, and
            hyperuuid.hpp, the hand-written C++23 header over it
c/          the C archives' CMake and pkg-config files, and C and C++ smoke consumers
docs/       benchmarks, platforms and provenance in depth
```

## Why "Hyper"

The SkunkWerkx Hyper* series — HyperUuid, [HyperCast](https://github.com/SkunkWerkx/HyperCast), [HyperTabular](https://github.com/SkunkWerkx/HyperTabular) — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## Contributing

Pull requests and issues are welcome. `.github/workflows/ci.yml` builds and tests every binding on every platform — a PR should stay green there before merging.

## License

[MIT](LICENSE)
