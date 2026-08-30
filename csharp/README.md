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

2. **A real monotonic counter.** `Guid.CreateVersion7()` implements no counter (RFC 9562 §6.2 Method 1): two BCL v7 GUIDs minted in the same millisecond sort randomly relative to each other, which is exactly the clustered-index fragmentation problem v7 adoption exists to solve. `UuidGenerator.NewV7()` reserves a slot in a process-global counter every call, guaranteeing strict creation order under concurrency — verified across interleaved individual *and* batch calls in this project's own test suite, not just in isolation.
3. **v6, which the BCL doesn't have at all.** A field-compatible reordering of v1 for the same sort/index locality as v7, useful when you're migrating off legacy v1 IDs. No `Guid.CreateVersion6` exists anywhere in the BCL.
4. **Batch generation.** `NewV7Batch(1000)` is ~3.9x faster than 1000 individual `NewV7()` calls (24 µs vs 93 µs) — one native call, one random-bytes fetch, one counter reservation for the whole batch, instead of paying per-item overhead a thousand times. `Guid.NewGuid()`/`CreateVersion7()` have no bulk API; you'd write that loop yourself.
5. **Cross-language consistency.** The exact same Rust core also mints v5 namespace UUIDs for Ruby, Python, Go, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't C#-only, that's not something the BCL can offer at all.
6. **SQL Server byte ordering, for free.** `UuidGenerator.V7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`V6ToSqlOrder` does the same for version 6), the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use. Verified directly against the real `SqlGuid` comparator in this package's own test suite, not a hand-rolled stand-in — and it's the same native function every other binding in this repo calls, not a C#-only reimplementation. Neither `Guid.NewGuid()` nor `Guid.CreateVersion7()` has any such concept.

The honest trade-off: this is a native dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID) instead of a BCL type that's always just there. If you only need plain v4 randomness in a C#-only codebase, `Guid.NewGuid()` is simpler and that's a completely reasonable choice.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime reflection anywhere in this assembly, and the project opts into (and fails the build on) the trim/Native-AOT analyzers via `IsAotCompatible`. See `HyperUuid.AotSmokeTest/` for a minimal AOT-published console app exercising it.

## WebAssembly (Blazor)

Works from a plain `<PackageReference Include="HyperUuid" />` — no `<NativeFileReference>`, no hand-written P/Invoke, no bridging in your own Rust build. Verified for real: a fresh Blazor WebAssembly app with nothing but that one package reference, run in an actual headless Chromium session, correctly generates v4s, matches the RFC 9562 v5 vector, and embeds a real-clock v7 timestamp that matches the actual wall clock exactly (zero drift) — not just "it builds."

**This is the real reason this package's floor is net10.0, not net8.0.** Not a preference — net8.0's WASM P/Invoke-table generator (`ManagedToNativeGenerator`, part of `dotnet/runtime`) scans the wrong copy of a multi-RID-asset package: the default `lib/{tfm}/` assembly, never the `runtimes/browser-wasm/lib/{tfm}/` one that actually ships and runs. Confirmed by inspecting the generated `pinvoke-table.h` directly: on net8.0 it registered this package's functions under module `"hyperuuid"`, never `"*"`, even though the deployed assembly was verified byte-for-byte identical to the correct browser build — so the app crashes with `DllNotFoundException("*")` at runtime, unconditionally, with no workaround found on the package side (redeclaring the P/Invoke directly in the consumer's own entry assembly doesn't avoid it either — same failure). Re-ran the identical repro against .NET 10 and it's fixed there: the generator correctly scans the browser assembly and the app runs end to end in a real headless-Chromium session.

**How:** this package ships two builds of the exact same `UuidGenerator` source. The default one (everywhere else) uses `[LibraryImport("hyperuuid")]`, resolved via `dlopen`. The `browser-wasm` one swaps to `[LibraryImport("*")]` — a statically-linked WASM native has no separate `"hyperuuid"` module to open, since its functions are already part of the same `dotnet.native.wasm` the app itself runs in; `"*"` resolves against the current module instead. Both land in the same `.nupkg`: the browser build under `runtimes/browser-wasm/lib/net10.0/`, the Rust core's `wasm32-unknown-emscripten` static library (`cargo rustc --crate-type staticlib` — the default `cdylib` produces an already-linked module `NativeFileReference` can't pull symbols from) under `runtimes/browser-wasm/nativeassets/net10.0/`. NuGet auto-selects the right assembly for any consumer building with `RuntimeIdentifier=browser-wasm`, falling back to the default build for every other RID — genuinely zero-effort either way, confirmed by inspecting a real self-contained `dotnet publish -r <rid>` output too: no WASM files leak into a non-WASM deployment.

The one piece that *doesn't* auto-wire — NuGet's `runtimes/{rid}/nativeassets/{tfm}/` convention is real, and the WASM SDK really does auto-promote a resolved `NativeLibrary` item into `NativeFileReference`, but restore doesn't actually populate `NativeLibrary` from a plain `PackageReference`'s `nativeassets` folder the way it does `native/` for ordinary P/Invoke (confirmed empirically, not assumed) — is supplied by this package's own `build/net10.0/HyperUuid.targets`, auto-imported into every consuming project via NuGet's standard convention, which adds both the `NativeFileReference` and (also confirmed necessary the hard way — linking the code in alone does *not* make it resolvable via `"*"` at runtime) an explicit `EmccExportedFunction` entry per native function this package P/Invokes. That's the actual mechanism making this "just works" for real, not a hopeful description of how NuGet packaging is supposed to behave.

**A real, currently-open upstream blocker you'll still hit, regardless of TFM:** the packaging above eliminates bringing your own Rust build and P/Invoke shim — it does *not* eliminate a genuine bug in the *consumer's own* .NET SDK. The moment any Blazor build does a native relink — which this package's own auto-injected `NativeFileReference` triggers just as surely as a hand-written one would — `Unknown option '--enable-bulk-memory-opt'` fires: a real Emscripten/rustc version-skew bug in the `wasm-tools` workload (rustc >= 1.87's WASM target-feature metadata vs. Binaryen `wasm-opt` builds older than Emscripten 3.1.74, which every `wasm-tools` SDK band bundles as of this writing — confirmed still true on .NET 10 and .NET 11 preview both, not just 8), confirmed identically on `linux-arm64` and `linux-x64`, and independently in an unrelated project ([PyO3/maturin#2549](https://github.com/PyO3/maturin/issues/2549)). Filed upstream with a minimal repro and a verified workaround: [dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858). Until that lands, every consumer building a Blazor app that references this package needs the same one-line fix — swap the SDK's bundled `wasm-opt` (`~/.dotnet/packs/Microsoft.NET.Runtime.Emscripten.<version>.Sdk.<rid>/<pack-version>/tools/bin/wasm-opt`) for a newer one (e.g. from a standalone `emsdk install latest`) — nothing this package can fix from the inside.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperUuid) — no extra package source needed:

```shell
dotnet add package HyperUuid
```

Targets net10.0 — see the WebAssembly section above for why that floor exists.

See [the repo root README](https://github.com/SkunkWerkx/HyperUuid/blob/master/README.md) for the full RFC 9562 coverage table and the state of every other language binding.
