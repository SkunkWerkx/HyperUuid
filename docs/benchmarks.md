# Benchmarks

The "high-performance, allocation-free" claim is measured, not just asserted — each binding with a mature benchmarking ecosystem has its own harness (BenchmarkDotNet, JMH, criterion, `testing.B`, package-benchmark, benchmark-ips, phpbench, pyperf, mitata, bench), the numbers agree with each other, and the losses print next to the wins (all measured on linux-x64, an Intel Core i9-11900H, except [TypeScript's and R's](#typescript-and-r-on-an-apple-m6); regenerate with the commands in each binding's README on your own hardware). Three measurement rules are baked into these numbers. PHP benchmarks run with `XDEBUG_MODE=off` (a loaded Xdebug inflates everything ~14x uniformly). Every case is warmed before it is timed, so a first call's library load is never in a figure. And **the operating system prices part of every one of these calls** — the wall-clock read and the random source — so a ratio with either on one side of it belongs to the machine as much as to the code: where a clock read costs a microsecond instead of tens of nanoseconds, every comparison against a platform call that reads the clock internally looks several times better than it does here. The time-based benchmarks carry explicit-timestamp variants for that reason, and the figures printed are from a machine where those calls are cheap.

## By language

The summary table is the [scoreboard](../README.md#the-scoreboard) in the main README; what each row stands on:

The R and TypeScript rows were measured on an Apple M6 (Mac18,5) under macOS, every other row on linux-x64 on an Intel Core i9-11900H, so the two groups are not comparable with each other. The linux-x64 rows' v7 figures were measured before the per-millisecond re-seed (core change, 2026-10-27); the Apple M6 rows after it, and after the switch to `CCRandomGenerateBytes` on Apple platforms (2026-10-28).

- **[C#](../csharp/)** — 7.5-7.9x faster than `Guid.NewGuid()` for v4 and v7, zero allocation on every call, and the only way to get a v7 with a real counter, strictly increasing within the process's stream, before .NET 9 — even on .NET 9+, `Guid.CreateVersion7()` still has no counter at all. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[Java](../java/)** — 2.8-5.1x faster than `UUID.randomUUID()`, against no real competition: `java.util.UUID` has never shipped v5 or v7. Proven under GraalVM Native Image too, not just the JVM. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[Rust](../rust/)** — this *is* the engine. 1.7-2.4x faster than the `uuid` crate on v4, v5 and v7, and 9x at reading a timestamp back out, allocation-free, asserted by a real counting-allocator test, not just claimed. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[Swift](../swift/)** — every call beats `Foundation.UUID()` by 11-16x, while also being the only way to get v5/v7 in Swift at all — Foundation only ever does v4. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[Python](../python/)** — every call ahead of stdlib's own C-accelerated ones through the PyO3 native extension: 3.4x faster than `uuid.uuid4()`, 4.2x faster than `uuid.uuid5()`, 3.8x faster than 3.14's own `uuid.uuid7()`, and timestamp extraction 1.6x faster than `UUID.time` as a `datetime`, 2.4x as a plain integer. On 3.11-3.13, where stdlib has no v7 at all, it's not even a comparison. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[Ruby](../ruby/)** — the same mechanism swap as Python, same result: **5.5x faster than `SecureRandom.uuid`** for v4, 3.8-4.0x for v7, 2.1x for v5 — and `SecureRandom.uuid` only ever does random v4 anyway. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[PHP](../php/)** — a v4 costs less than a *naive inline pure-PHP v4* (three lines of `random_bytes` + bit twiddling, no RFC validation), by 1.4x, and a v7 by 1.2x — the whole native round trip for less than the price of PHP-level byte fiddling — and timestamp extraction beats `ramsey/uuid` by 45x. Still the only zero-Composer-dependency way to generate v4, v5 and v7 in PHP at all. (v7 figures measured before the per-millisecond re-seed, core change, 2026-10-27.)
- **[R](../r/)** — on the Apple M6, `uuid_v4()` is 12.7x faster than the `uuid` package's `UUIDgenerate()` for one id and 29x for 1000, and `uuid_v7()` is 1.3x slower than its `UUIDgenerate(use.time = TRUE)` for one id and 1.2x for 1000. That comparison is different work: `uuid` makes a version 1 (a clock sequence and a fixed node, no random bits) and has no version 7, but it is the time-based call an R user has today, and it is faster. Every function is vectorised and propagates `NA` element-wise.
- **[TypeScript](../typescript/)** — on the Apple M6, `newV4()` is 4.1x slower than `crypto.randomUUID()` on Node and 6.4x on Bun, and `newV7()` is 2.3x faster than the `uuid` package's `v7()` on Node and 1.2x slower on Bun. The losses are the random source: on Apple platforms the core asks the operating system's generator for every id, where `crypto.randomUUID()` draws from a buffer it refills in bulk. A batch pays for it once, so `newV7Batch(1000)` is 3.5x faster than 1000 `newV7()` calls on Node and 6.2x on Bun. What the binding gives is the same engine as every other binding, under Node and Bun, with the SQL Server order.

## Where the wins come from — two crossing strategies

The scoreboard above wasn't free, and the mechanism behind it is the actual finding of this project. There is no single trick; there are two, chosen per language by measuring where each one's boundary cost actually lives:

**Direct FFI, where the crossing floor is already nanoseconds.** C#'s `P/Invoke` and Java's FFM cost on the order of ten nanoseconds per call; PHP's built-in `ext-ffi` measured ~105ns. At those floors the engine's own speed dominates, so those bindings call the C ABI directly — and any remaining slowness is *wrapper*, which gets dieted, not excused. PHP is the proof: with no mechanism change at all, a wrapper that reuses static scratch across calls and passes inputs as zero-copy `const char *` strings is what puts it level with the naive inline v4.

**A native extension, where the FFI mechanism itself is the cost.** CPython's `ctypes` prices every call at ~1µs of interpreted marshalling; Ruby's `Fiddle` charges over a microsecond. No diet fixes that — the mechanism is the bill. So those two bindings link the Rust core *directly into the language VM* as an ordinary native extension (PyO3, Magnus), turning the crossing into a plain C function call. The two bindings part ways from there: PyO3 ships one `abi3` wheel per platform that covers every CPython 3.11+ on that platform, beside PEP 803's `abi3t` wheel for free-threaded CPython 3.15+, where the extension runs with the GIL off, so `pip` resolves a native wheel on every platform that has one, falls back to building the same extension from the sdist elsewhere (CI proves that build on glibc 2.17 and musl), and there is no `ctypes` fallback — nothing left for it to buy. Magnus has no stable-ABI story across Ruby versions the way `abi3` gives PyO3 (a precompiled platform gem is tied to one Ruby minor version), so Ruby's gems are *fat* — one compiled extension per supported Ruby minor inside each platform gem — and Ruby keeps a real `Fiddle` fallback for whatever falls outside that grid: auto-selected on any platform/Ruby combination without a prebuilt Magnus gem, which today means Ruby 3.3, a Ruby newer than the release, Intel macOS, and any platform outside the eight RIDs — shipped only in the universal gem, the last resort — with the same test suite running green against both backends and cross-backend agreement pinned by tests, so the fallback is never a second implementation that can drift.

TypeScript and R were decided from the start rather than by measurement. TypeScript took the native-extension door: a Node-API addon built with napi-rs, the core linked into the `.node` file, which Node and Bun both load. R calls the C ABI from a hand-written shim reached through `.Call`, linked against the core's static library, which a source install builds from the Rust crates vendored into the package. Neither has a fallback.

Which leaves exactly one language where neither strategy applies — and that's not an accident.

## C# vs. `Guid.NewGuid()`

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*` (BenchmarkDotNet, `[MemoryDiagnoser]`):

| Method | Mean | Allocated |
| --- | ---: | ---: |
| `Guid.NewGuid()` | 292.84 ns | 0 B |
| `UuidGenerator.NewV4()` | 36.94 ns (**7.93x faster**) | 0 B |
| `UuidGenerator.NewV5()` | 69.27 ns (4.23x faster) | 0 B |
| `UuidGenerator.NewV7()` † | 38.88 ns (7.53x faster) | 0 B |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

Every one of these is genuinely zero-allocation — including `NewV5(Guid, string)`, which UTF-8-encodes the name into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique the batch methods use (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library).

## The interpreted tier

The headline single-call numbers from the [Ruby](../ruby/) and [PHP](../php/) READMEs, worth restating here:

| Call | Time | The platform comparison |
| --- | ---: | --- |
| Ruby `HyperUuid.new_v4` (Magnus) | 191 ns | `SecureRandom.uuid` 1.06 µs — **5.5x faster** |
| Ruby `HyperUuid.new_v7` (Magnus, explicit ms) † | 264 ns | — **4.0x faster** |
| PHP `HyperUuid::newV4()` | 224 ns | naive inline `random_bytes` v4 308 ns — **1.4x faster** |
| PHP `->timestamp()` (v7) | 293 ns | `ramsey/uuid` `getDateTime()` 13.2 µs — **45x faster** |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

The zero-compile `Fiddle` fallback (`HYPERUUID_PURE=1`, and automatic on any platform without a prebuilt Magnus gem) keeps its own honest numbers in the [Ruby README](../ruby/) — slower, mechanism-bound, and still fully supported. Python has no `ctypes` fallback at all; PyO3's `abi3` wheels leave nothing for one to cover.

## Batch generation vs. an equivalent loop

`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Batch*`, `cargo bench` (from `rust/`, one v7 case per process, as [its README](../rust/README.md#benchmarks) says), `go test -bench=. -benchmem ./...` (from `go/`):

| Binding | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| Rust — v7 † | 25.2 µs | 4.40 µs | **5.7x** |
| C# — v7 † | 37.5 µs | 6.06 µs | **6.2x** |
| Go — v7 † | 69.8 µs | 6.97 µs | **10.0x** |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

Go's batch win is the largest of the three because its per-call toll is the largest: nearly all of a single call is `runtime.cgocall` (see [the control group](../README.md#the-control-group-go)), and a batch pays it once for 1000 UUIDs. The individual calls allocate nothing (v5 aside, which pays for Go's own `[]byte(name)`), so the win is time alone — and batch is exactly where the control group stops being the exception.

Rust's own allocation-free claim isn't just asserted either — `rust/tests/allocation_free.rs` wraps a counting `#[global_allocator]` around 1000 calls to each of v4/v5/v7 and around the batch function, and asserts zero allocations for all of them. Batches draw their entropy through the caller's own buffer, which is what lets the crate build without `alloc` at all.

## Skipping object construction entirely

The batch doors above still hand back a collection of the language's own UUID type. Every binding also offers a form that hands back **raw RFC 9562-ordered bytes** — a destination buffer to fill, or one contiguous byte string — constructing no UUID objects at all. For the interpreted tier that turns out to matter far more than the batching did:

| Binding | batch → objects | raw bytes | speedup | API |
| --- | ---: | ---: | ---: | --- |
| Ruby † | 202 µs | **4.9 µs** | **41x** | `new_v7_batch_bytes` |
| Python † | 132 µs | **3.7 µs** | **36x** | `fill_v7(bytearray)` |
| PHP † | 67.1 µs | **4.2 µs** | **16x** | `newV7BatchBytes` |
| Java † | 9.3 µs | **3.6 µs** | 2.6x | `fillV7(byte[])` |
| Go † | 7.0 µs | **3.7 µs** | 1.9x, and 0 allocs | `FillV7BytesAt` |
| C# † | 6.1 µs | **3.8 µs** | 1.6x, and 0 allocs | `FillV7(Span<byte>)` |
| Swift † | 3.7 µs | **3.8 µs** | level | `fillV7(into: raw bytes)` |

† Measured before the per-millisecond re-seed (core change, 2026-10-27).

Read the right-hand column, not the speedup column: **every binding converges on roughly 3.6–4.9 µs per 1000 UUIDs**, because that is what the work actually costs. The native call is not the bottleneck in any of them; what varies is the price each language charges to wrap those 16000 bytes in a thousand objects — 200 µs of it in Ruby, 130 µs in Python, essentially none in Swift. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

Java is the instructive middle. It is not an interpreted language, yet it gains 2.6x where C# gains 1.6x and Swift nothing, and the reason is visible in its own numbers: filling a `UUID[]` measures 12.3 µs against `newV7Batch`'s 9.3 µs, no cheaper for skipping the array allocation, because `java.util.UUID` is two `long`s and every element has to be rebuilt regardless of who allocated the array. Only its `byte[]` form escapes that. The dividing line is not compiled-versus-interpreted; it is whether the language's UUID type is already RFC-ordered bytes. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

That also explains why Go, C# and Swift barely move: they are already at or near the floor. Their win is allocation, not time — `FillV7` writes into a buffer you already own, so a hot loop allocates nothing at all. In Go and Swift it needs no per-element conversion either, since `uuid.UUID` is `[16]byte` and Foundation's `UUID` wraps `uuid_t`, both already in RFC order; C# and Java must rebuild each element because `System.Guid` is mixed-endian and `java.util.UUID` is two longs.

**One caveat, and it inverts the advice** — documented on every method in the three dynamic bindings, because getting it wrong is a pessimization: this is only faster if bytes are the *destination*. In Python, filling a buffer and then building `uuid.UUID` objects from it measures ~890 µs, more than six times as slow as `new_v7_batch`, because the extension constructs them through a faster path internally than anything callable from Python. Use the byte forms for a bind parameter, a wire format, or a bulk `COPY` — not as a step on the way to objects. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

## TypeScript and R, on an Apple M6

These two were measured on an Apple M6 (Mac18,5) under macOS on 2026-10-30, a different machine and OS from every table above, so they are not comparable to the linux-x64 numbers.

TypeScript, Node 26.13.0 and Bun 1.4.4, with [mitata](https://github.com/evanwashere/mitata) (`npm run bench` and `npm run bench:bun`, from `typescript/`), averages per call; Bun's `uuid` `v7()` is the noisiest baseline here, so read its ratio with that spread:

| Call | Node 26 | Bun 1.4 | vs. |
| --- | ---: | ---: | --- |
| `newV4()` | 239 ns | 258 ns | `crypto.randomUUID()`: 58.0 ns / 40.5 ns: **4.1x / 6.4x slower** |
| `newV7()` | 155 ns | 166 ns | `uuid`'s `v7()`: 350 ns / 135 ns: 2.3x faster / **1.2x slower** |
| `newV7Batch(1000)` | 44.2 µs | 27.3 µs | 1000x `newV7()`: 155 µs / 169 µs: 3.5x / 6.2x faster |
| `newV7BatchBytes(1000)` | 3.62 µs | 2.93 µs | `newV7Batch(1000)`: 12x / 9.3x faster |
| `v7UnixMillis(id)` | 115 ns | 156 ns | |

The losses are the random source: on Apple platforms the core asks `CCRandomGenerateBytes` for every id (the user-space ChaCha20 path in `rust/src/entropy.rs` is Linux and Android only), which costs more from 12 bytes up, and a v4 draws 16 where a v7 draws 11. A round trip into the addon that touches no randomness (`v7UnixMillis`) is 115 ns on Node. `crypto.randomUUID()` draws from a buffer of random bytes it refills in bulk, and a batch pays once in the same way.

R 4.6.2 (Homebrew build), against the [uuid](https://cran.r-project.org/package=uuid) package 1.2.2 with bench 1.1.4 (`R_LIBS=<lib> Rscript bench/bench.R` from `r/`, against an installed copy), medians:

| Comparison (median) | hyperuuid | uuid | Result |
| --- | --- | --- | --- |
| v4, one id | 779 ns | 9.92 µs | hyperuuid 12.7x faster |
| v4, 1000 ids | 256 µs | 7.41 ms | hyperuuid 29x faster |
| time-based, one id | 2.46 µs | 1.89 µs | **hyperuuid 1.3x slower** |
| time-based, 1000 ids | 119.35 µs | 101.02 µs | **hyperuuid 1.2x slower** |

The time-based rows compare different work: `UUIDgenerate(use.time = TRUE)` makes version 1 ids (a clock sequence and a fixed node, no random bits), while `uuid_v7()` makes version 7 ids from a counter stream re-seeded every millisecond, with fresh random bits, and uuid has no version 7. `uuid_v4(1000)` costs more than `uuid_v7(1000)` for the same reason as TypeScript's single ids: each v4 is one random-source call, while a v7 batch fetches its random bits once.
