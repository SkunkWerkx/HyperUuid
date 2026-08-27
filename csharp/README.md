# HyperUuid

**`UuidGenerator.NewV4()` beats `Guid.NewGuid()` by ~5.67x — with zero heap allocation, on every version including v5 — because it calls straight into a native Rust core instead of the BCL's own managed generator.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling directly into the native `libhyperuuid` shared library via source-generated [`LibraryImport`](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) P/Invoke — no runtime bridge, no reflection, AOT/trim-friendly. Ships as RID-specific native assets inside the package the standard NuGet way.

```csharp
using HyperUuid;

var id = UuidGenerator.NewV4();
var id2 = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "example.com");
var id3 = UuidGenerator.NewV6();
var id4 = UuidGenerator.NewV7();

// Time-sortable versions round-trip their embedded timestamp:
DateTimeOffset created = UuidGenerator.V7Timestamp(id4);

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
Guid sqlOrdered = UuidGenerator.V7ToSqlOrder(id4);

// Bulk generation shares one timestamp capture, one random-bytes fetch, and (v7) one
// contiguous counter reservation across the whole batch:
Guid[] batch = UuidGenerator.NewV7Batch(1000);
```

Returns plain `System.Guid` — this binding does no byte-order conversion of its own beyond the `bigEndian: true` `Guid` constructor overload (.NET 8+), since that's already the correct, direct RFC 9562 mapping. `UuidGenerator.Namespaces.Dns`/`Url`/`Oid`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.Nil`/`Max` are the §5.9/§5.10 special values (`Nil` is literally `Guid.Empty`).

## Why not `Guid.NewGuid()` / `Guid.CreateVersion7()`?

1. **It's measurably faster, not just different.** Real BenchmarkDotNet numbers, `[MemoryDiagnoser]`, linux-arm64 (`dotnet run -c Release --project ../csharp/HyperUuid.Benchmarks -- --filter *Generation*`):

   | Method | Mean | Allocated |
   | --- | ---: | ---: |
   | `Guid.NewGuid()` | 630.26 ns | 0 B |
   | `UuidGenerator.NewV4()` | 111.20 ns (**5.67x faster**) | 0 B |
   | `UuidGenerator.NewV5()` | 131.31 ns (4.80x faster) | 0 B |
   | `UuidGenerator.NewV6()` | 75.57 ns (**8.34x faster**) | 0 B |
   | `UuidGenerator.NewV7()` | 82.10 ns (7.68x faster) | 0 B |

   Including `NewV5(Guid, string)` — it used to allocate 40 B encoding the name to UTF-8 via `Encoding.UTF8.GetBytes(name)`; now it UTF-8-encodes into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique the batch methods already used (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library). `NewV5(Guid, ReadOnlySpan<byte>)` remains available if you already have bytes and want to skip the encode step entirely.

2. **A real monotonic counter, on every target framework this package supports.** `Guid.CreateVersion7()` only exists from .NET 9 onward — this package targets net8.0, so it's the only way to get RFC 9562 v7 there at all. Even where `CreateVersion7` *is* available, it implements no counter (RFC 9562 §6.2 Method 1): two BCL v7 GUIDs minted in the same millisecond sort randomly relative to each other, which is exactly the clustered-index fragmentation problem v7 adoption exists to solve. `UuidGenerator.NewV7()` reserves a slot in a process-global counter every call, guaranteeing strict creation order under concurrency — verified across interleaved individual *and* batch calls in this project's own test suite, not just in isolation.
3. **v6, which the BCL doesn't have at all.** A field-compatible reordering of v1 for the same sort/index locality as v7, useful when you're migrating off legacy v1 IDs. No `Guid.CreateVersion6` exists anywhere in the BCL.
4. **Batch generation.** `NewV7Batch(1000)` is ~3.9x faster than 1000 individual `NewV7()` calls (24 µs vs 93 µs) — one native call, one random-bytes fetch, one counter reservation for the whole batch, instead of paying per-item overhead a thousand times. `Guid.NewGuid()`/`CreateVersion7()` have no bulk API; you'd write that loop yourself.
5. **Cross-language consistency.** The exact same Rust core also mints v5 namespace UUIDs for Ruby, Python, Go, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't C#-only, that's not something the BCL can offer at all.
6. **SQL Server byte ordering, for free.** `UuidGenerator.V7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`V6ToSqlOrder` does the same for version 6), the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use. Verified directly against the real `SqlGuid` comparator in this package's own test suite, not a hand-rolled stand-in — and it's the same native function every other binding in this repo calls, not a C#-only reimplementation. Neither `Guid.NewGuid()` nor `Guid.CreateVersion7()` has any such concept.

The honest trade-off: this is a native dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID) instead of a BCL type that's always just there. If you only need plain v4 randomness in a C#-only codebase, `Guid.NewGuid()` is simpler and that's a completely reasonable choice.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime reflection anywhere in this assembly, and the project opts into (and fails the build on) the trim/Native-AOT analyzers via `IsAotCompatible`. See `HyperUuid.AotSmokeTest/` for a minimal AOT-published console app exercising it.

## WebAssembly (Blazor)

Proven working end-to-end — real headless-browser run, not just "it compiles" — but not yet a turnkey NuGet experience; see `HyperUuid.WasmSmokeTest/` for the exact working shape.

**What works today:** the Rust core builds cleanly as a `wasm32-unknown-emscripten` static library (`cargo rustc --release --target wasm32-unknown-emscripten --crate-type staticlib` — the default `cdylib` crate type produces an already-linked module `<NativeFileReference>` can't pull symbols from; `staticlib` is required). Reference the resulting `.a` via `<NativeFileReference>` in a Blazor WebAssembly app, same as any native library. Verified with a real smoke test in a real headless Chromium session: v4 randomness, the RFC 9562 v5 vector, a fixed-timestamp v7, and a real-clock v7 (embedded timestamp matched the actual wall clock exactly, zero drift) all pass.

**One code change needed on the C# side:** this assembly's own `UuidGenerator` uses `[LibraryImport("hyperuuid")]`, correct for every real dlopen-based platform — but a statically-linked WASM native has no separate `"hyperuuid"` module to open; its functions are already part of the same `dotnet.native.wasm` the app itself runs in. `[LibraryImport("*")]` resolves against the current module instead. `HyperUuid.WasmSmokeTest/NativeWasm.cs` declares its own minimal WASM-specific P/Invoke surface with `"*"` rather than reusing this project's `UuidGenerator` directly — there's currently no single build of this assembly that works correctly on both native and WASM targets at once (see "Not yet" below).

**A real, currently-open upstream blocker:** `<NativeFileReference>`-ing a `wasm32-unknown-emscripten` staticlib built by rustc >= 1.87 fails .NET's native relink with `Unknown option '--enable-bulk-memory-opt'` — a genuine Emscripten/rustc version-skew bug in the `wasm-tools` workload, confirmed identically on `linux-arm64` and `linux-x64`, and independently in an unrelated project ([PyO3/maturin#2549](https://github.com/PyO3/maturin/issues/2549)). Filed upstream with a minimal repro and a verified workaround: [dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858). Until that lands, building this yourself needs the same workaround — swap the SDK's bundled `wasm-opt` (`~/.dotnet/packs/Microsoft.NET.Runtime.Emscripten.<version>.Sdk.<rid>/<pack-version>/tools/bin/wasm-opt`) for a newer one (e.g. from a standalone `emsdk install latest`).

**Not yet:** a turnkey NuGet experience — shipping the `.a` as a `runtimes/browser-wasm/nativeassets/{tfm}/` package asset and a second `LibraryImport("*")` build of this assembly under `runtimes/browser-wasm/lib/{tfm}/`, so a consumer's own Blazor project needs no `NativeFileReference`/P/Invoke work at all. That's real, additional engineering, deliberately not done yet — today, using this from WASM means bridging in your own Rust build the way `HyperUuid.WasmSmokeTest/` does.

## Install

Published to this repo's GitHub Packages NuGet feed — add it as a package source, then:

```shell
dotnet add package HyperUuid
```

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
