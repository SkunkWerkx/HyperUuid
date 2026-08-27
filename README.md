# HyperUuid

[![Build native libraries and pack NuGet + Maven](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/build-packages.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/build-packages.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**One [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html)-compliant UUID engine, written once in Rust, called directly — not wrapped, not shimmed — from C#, Java, Go, Swift, Ruby, PHP, and Python.**

Every other polyglot ID library either reimplements the same generation logic per language (drift risk: seven codebases that can each get the bit-twiddling subtly wrong in different ways) or ships a server/sidecar process to generate IDs centrally (a network round-trip for something that should cost nanoseconds). HyperUuid does neither: a single Rust `cdylib` exports a plain C ABI, and every binding calls straight into it — `P/Invoke`, `FFM`, `purego`/`dlopen`, `Fiddle`, PHP's `FFI`, `ctypes` — sharing the *same* address space, the *same* generation logic, the *same* test vectors, on every platform. No runtime bridge, no serialization layer, no embedded interpreter.

And it's not just parity with your platform's built-in UUID call — for C#, it's measurably faster: **`UuidGenerator.NewV4()` beats `Guid.NewGuid()` by ~5.8x** with zero heap allocation (real BenchmarkDotNet numbers below, not a marketing claim).

## Quick start

```csharp
// C# — package "HyperUuid" on this repo's GitHub Packages feed (see "Published" below)
var id = UuidGenerator.NewV7();                     // time-sortable, RFC 9562 §6.2
var ts = UuidGenerator.V7Timestamp(id);              // recover the embedded timestamp
var batch = UuidGenerator.NewV7Batch(1000);          // 1000 IDs, one native call
```

```go
// Go — go get github.com/SkunkWerkx/HyperUuid/go
id, err := hyperuuid.NewV7()
ts, err := hyperuuid.V7Timestamp(id)
```

```ruby
# Ruby — gem "hyperuuid", git: "https://github.com/SkunkWerkx/HyperUuid", glob: "ruby/*.gemspec"
id = HyperUuid.new_v7
id.timestamp
```

Every binding follows the same shape — `new_v4`/`new_v5`/`new_v6`/`new_v7`, batch variants for v6/v7, timestamp extraction for v6/v7, and the RFC's `Nil`/`Max` constants. See each language's own README (linked in the table below) for its exact idiom and install instructions.

## RFC 9562 coverage

| Version | Purpose | RFC section |
| --- | --- | --- |
| **v4** | Cryptographically random | §5.4 |
| **v5** | Deterministic, namespace + name, SHA-1 (cross-language interoperable — the same `(namespace, name)` pair produces the same UUID everywhere, including against Python's own `uuid.uuid5`) | §5.5 |
| **v6** | Time-ordered, v1-field-compatible reordering for sort/index locality — no monotonic counter | §5.6 |
| **v7** | Time-ordered, 48-bit Unix-ms timestamp + 26-bit monotonic counter + random bits — strictly increasing even under concurrent generation | §6.2 |
| **Nil** / **Max** | The all-zero and all-one special values | §5.9 / §5.10 |

v1 (classic time-based, leaks a MAC-derived node ID) and v3 (MD5 name-based) are deliberately not implemented — RFC 9562 itself treats them as superseded by v6 and v5 respectively, so building them would just be completeness theater. v6 and v7 both embed a timestamp `*Timestamp`/`*_timestamp` can recover on every binding; v6's Gregorian-epoch tick count tops out around the year 5236, well short of any language's own datetime ceiling, so unlike v7 it can never realistically raise an overflow decoding it.

## State of the union

Every language, on every platform, proven for real: `.github/workflows/build-packages.yml`'s `build-native` matrix builds the Rust core fresh on each of 6 real-hardware legs, then runs that language's actual test suite against that leg's freshly-built native library — not just that it compiles.

| Language | linux-x64 | linux-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | proven, not yet published |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [NuGet](https://github.com/SkunkWerkx/HyperUuid/packages) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [Maven](https://github.com/SkunkWerkx/HyperUuid/packages) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | proven, not yet published |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | proven, not yet published |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | proven, not yet published |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | ✅ | — | proven, not yet published |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | proven, not yet published |

PHP skips win-arm64 deliberately: PHP has never shipped a native Windows ARM64 build, so it always runs under x64 emulation there regardless of host CPU — already exercised for real by the win-x64 leg.

**Published:** C# and Java, both to this repo's GitHub Packages feed. The JVM binding is plain Java, not Kotlin — `kotlin-stdlib` would otherwise be a real transitive dependency for every consumer, unlike every other binding here — and its AOT story is proven the same way C#'s is: a local GraalVM Native Image smoke test (`java/aot-smoke-test/`, `./gradlew :aot-smoke-test:nativeRun`) produces a genuine standalone native binary, no JVM required to run it.

**Proven, not yet published:** Go, Swift, Ruby, PHP, and Python are all CI-green on every platform above but don't have a registered `SkunkWerkx`/`buvinghausen` presence on their respective registries yet (pkg.go.dev, Swift Package Registry, RubyGems, Packagist, PyPI) — see each language's own README for how to consume it directly (a git dependency, VCS repository, etc.) in the meantime.

## Why not your platform's built-in UUID call?

Most languages *do* already have one — `Guid.NewGuid()`, `java.util.UUID.randomUUID()`, `uuid.uuid4()`, `SecureRandom.uuid`. HyperUuid isn't arguing you should never use those. It's for the specific, common situation where you need more than plain v4 randomness gives you:

1. **Time-sortable IDs with real ordering guarantees.** A v7 ID minted a microsecond after another one you generated on the same thread will sort after it — HyperUuid's monotonic counter (RFC 9562 §6.2 Method 1) guarantees that even under concurrent generation. Most stdlib v4 generators have no time-ordering story at all, and even a stdlib that *does* offer v7 (C#'s `Guid.CreateVersion7`, added in .NET 9) doesn't implement the counter, so two IDs minted in the same millisecond sort randomly relative to each other — exactly the index-fragmentation problem v7 adoption is meant to solve in the first place.
2. **One generation engine across a polyglot system.** If your API is C#, your batch jobs are Go, and your data pipeline is Python, plain per-language UUID libraries give you three independent implementations that all *should* agree bit-for-bit on RFC 9562 semantics but have no structural reason to. HyperUuid's v5 namespace UUIDs are verified in CI to match byte-for-byte with Python's own `uuid.uuid5` — because it's the literal same Rust code minting them everywhere, not three ports of the same spec.
3. **Batch throughput.** Need to backfill a million IDs? `NewV7Batch`/`new_v7_batch`/`NewV7BatchAt` (binding-dependent naming) shares one timestamp capture and one contiguous counter reservation across the whole batch instead of paying per-item overhead N times. Measured 3.5-19x faster than the equivalent loop of individual calls, depending on binding (numbers below) — most stdlib UUID facilities have no batch API at all.
4. **It's not slower for the trouble.** For C#, it's faster outright — see the benchmarks below.

The honest trade-off: this is one more native dependency to ship (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll`) versus a UUID call that's already sitting in your standard library. If you only need plain v4 randomness and don't care about cross-language consistency, the stdlib call is simpler and that's a completely reasonable choice.

## SQL Server ordering

`V7ToSqlOrder`/`v7ToSqlOrder`/`v7_to_sql_order` (C#/Java/Go/Swift/Python's version-explicit naming — Ruby's `#to_sql_order`/PHP's `->toSqlOrder()` stay polymorphic across both versions, matching their existing `#timestamp`/`->timestamp()` convention) converts an RFC 9562-ordered version 7 UUID to the byte order SQL Server's `uniqueidentifier` needs on the wire to sort by creation order, and the `FromSqlOrder`/`fromSqlOrder`/`from_sql_order` counterpart converts it back. `V6ToSqlOrder`/`v6ToSqlOrder`/`v6_to_sql_order` do the same for version 6. `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed, non-sequential byte significance order. For v7, this moves the timestamp and counter (the two fields that determine creation order) into that comparison's most-significant bytes, and moves the trailing entropy, which carries no ordering information, into the least-significant ones as one intact block — the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use for C#. v6 has no counter, so only its 60-bit timestamp is sort-relevant; `clock_seq`/`node` (random per call, not a counter) get relocated the same way v7's entropy does. Both are computed once in the Rust core and exported over FFI so every binding gets them from the same verified source instead of a seven-times-reimplemented one. Verified against the real `System.Data.SqlTypes.SqlGuid` comparator (not a hand-rolled stand-in) in the C# test suite; every other binding verifies the same sort behavior against a comparator replicating `SqlGuid`'s documented byte order.

**When this actually matters:** SQL Server already ships its own native answer to GUID-clustering fragmentation — [`NEWSEQUENTIALID()`](https://learn.microsoft.com/sql/t-sql/functions/newsequentialid-transact-sql), generated server-side. This feature is for the narrower case `NEWSEQUENTIALID()` can't cover: IDs generated *application-side* (offline-first clients, distributed systems, anything that needs the ID before the row is ever inserted) that should also behave well if a `uniqueidentifier` column holding them is the clustered index key. That's also the load-bearing condition — a random or naively-generated GUID only fragments a clustered index because SQL Server always maintains a clustered index's physical sort order on every insert; nothing has to be "turned on" for that to happen, but nothing here helps at all if the column isn't the clustered key in the first place. And plenty of real-world SQL Server schemas sidestep the whole problem by never clustering on the GUID at all — an `IDENTITY`/sequence integer as the clustered key, with the GUID kept as an ordinary non-clustered unique column — which remains a completely reasonable choice if application-side ID generation isn't a requirement.

Meaningful only for a genuine version 6 or 7 UUID, respectively. **v6 caveat:** two v6 UUIDs minted at the same millisecond have identical timestamp bits — with no counter to break the tie, their relative order after conversion isn't guaranteed to match creation order, the same limitation plain RFC order already has for v6, not something this transform introduces; every binding's v6 sort-correctness test therefore only exercises strictly increasing timestamps. **Java caveat:** this is verified at the raw-byte level against .NET's own `Guid` wire format (which ADO.NET passes through unchanged), not against any specific JDBC driver's `uniqueidentifier` parameter binding — check your driver, or bind the bytes directly, before relying on it there. **Ruby/PHP caveat:** converting *back* from SQL order can't tell v6 and v7 apart from the version nibble alone (it sits at a different byte offset per version) — both bindings resolve this deterministically by checking a byte position/field that's provably collision-free between the two versions, but PHP's `fromSqlOrder()` also accepts an explicit `$version` argument for when you already know it.

## Benchmarks

The "high-performance, allocation-free" claim is measured, not just asserted — each binding with a mature benchmarking ecosystem has its own harness, and the numbers agree with each other (all measured on linux-arm64; regenerate with the commands below on your own hardware).

### C# vs. `Guid.NewGuid()`

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*` (BenchmarkDotNet, `[MemoryDiagnoser]`):

| Method | Mean | Allocated |
| --- | ---: | ---: |
| `Guid.NewGuid()` | 630.26 ns | 0 B |
| `UuidGenerator.NewV4()` | 111.20 ns (**5.67x faster**) | 0 B |
| `UuidGenerator.NewV5()` | 131.31 ns (4.80x faster) | 0 B |
| `UuidGenerator.NewV6()` | 75.57 ns (**8.34x faster**) | 0 B |
| `UuidGenerator.NewV7()` | 82.10 ns (7.68x faster) | 0 B |

Every one of these is genuinely zero-allocation now — including `NewV5(Guid, string)`, which used to allocate 40 B encoding the name to UTF-8. Fixed by UTF-8-encoding into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique already used by the batch methods (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library).

### Batch generation vs. an equivalent loop

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*`, `cargo bench` (`rust/benches/`), `go test -bench=. -benchmem ./go/...`:

| Binding | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| Rust — v7 | 61.7 µs | 16.9 µs | **3.6x** |
| Rust — v6 | 52.5 µs | 20.7 µs | 2.5x |
| C# — v7 | 93 µs | 24 µs | **3.9x** |
| C# — v6 | 84 µs | 27 µs | 3.1x |
| Go — v7 | 514 µs | 27 µs | **19x** |
| Go — v6 | 512 µs | 33 µs | 15.6x |

Go's batch win is the largest of any binding, for a real architectural reason worth knowing about if you're choosing where to put your hot path: **every individual Go call does 4-7 heap allocations** (252-360 B/op via `go test -bench=. -benchmem`) — unlike Rust and C#, which are both genuinely zero-allocation per call. This is almost certainly `unsafe.Pointer` arguments crossing into `purego`'s dynamically-generated call trampolines defeating Go's escape analysis, a real cost of the "no cgo" approach every binding here shares. Batch generation collapses ~5000 of those allocations into 7, which is exactly why it wins bigger in Go than anywhere else.

Rust's own allocation-free claim isn't just asserted either — `rust/tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v6/v7 and asserts zero allocations, then asserts the batch functions' scratch buffer *does* allocate, confirming it's the one deliberate exception documented in `v6.rs`/`v7.rs`.

## Key features

- **RFC 9562 compliant** — correct version nibble and variant bits on every UUID, from every binding, because they all come from the same Rust core
- **One implementation, seven call sites** — no per-language reimplementation to drift out of sync; v5's SHA-1 hashing, v7's monotonic counter, and v6's Gregorian-epoch math are each written exactly once
- **Monotonically increasing v7** — a process-global counter (RFC 9562 §6.2 Method 1) guarantees strict ordering under concurrency, continued correctly across individual *and* batch calls
- **Batch generation** — `*Batch`/`*_batch` for v6/v7 amortizes timestamp capture, counter reservation, and the random-bytes fetch across the whole batch
- **SQL Server byte ordering** — `*ToSqlOrder`/`*_to_sql_order` for both v6 and v7, computed once in the Rust core and exported to every binding, verified against the real `System.Data.SqlTypes.SqlGuid` comparator
- **No runtime bridge** — direct FFI (`P/Invoke`, FFM, `purego`, `Fiddle`, PHP `FFI`, `ctypes`) into a shared-address-space native library, not a serialization protocol or an embedded interpreter
- **Genuinely allocation-free where it counts** — verified with a counting allocator in Rust and `[MemoryDiagnoser]` in C#, not just claimed
- **AOT-friendly** — C# publishes cleanly under `PublishAot`; Java's JVM binding survives a real GraalVM Native Image build into a standalone native binary, no JVM required to run it
- **CI-proven, not CI-claimed** — 6 real-hardware platforms × 8 language/runtime targets, each running that language's actual test suite against a freshly-built native library on every dispatch

## Contributing

Pull requests and issues are welcome. `.github/workflows/build-packages.yml` builds and tests every binding on every platform — a PR should stay green there before merging.

## License

[MIT](LICENSE)
