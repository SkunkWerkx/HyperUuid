# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**`UuidGenerator.NewV4()` beats `Guid.NewGuid()` by ~8x — with zero heap allocation, on every version including v5 — because it calls straight into a native Rust core instead of the BCL's own managed generator.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling directly into the native `libhyperuuid` shared library via source-generated [`LibraryImport`](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) P/Invoke — no runtime bridge, no reflection, AOT/trim-friendly. Ships as RID-specific native assets inside the package the standard NuGet way.

```csharp
using HyperUuid;

var id = UuidGenerator.NewV4();
var id2 = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "example.com");
var id3 = UuidGenerator.NewV6();
var id4 = UuidGenerator.NewV7();

// Time-sortable versions round-trip their embedded timestamp:
DateTimeOffset created = UuidGenerator.V7Timestamp(id4);

// Version-agnostic: null instead of assuming id4 is v6/v7:
DateTimeOffset? maybeCreated = UuidGenerator.GetTimestamp(id4);

// Same thing as an extension property on Guid itself — works on any Guid, whatever made it:
DateTimeOffset? created7 = id4.Timestamp;              // the creation time
DateTimeOffset? none = Guid.NewGuid().Timestamp;       // null — a v4 UUID carries no time

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
Guid sqlOrdered = UuidGenerator.V7ToSqlOrder(id4);

// Inspection: the one-call guard (RFC variant and version 7), in either layout, so a value
// read back from a uniqueidentifier column is validated and dated without converting it back:
bool isV7 = UuidGenerator.IsRfc(id4, 7);
bool stillV7 = UuidGenerator.IsRfc(sqlOrdered, 7, UuidLayout.SqlServer);
DateTimeOffset? fromSql = UuidGenerator.GetTimestamp(sqlOrdered, UuidLayout.SqlServer);

// Bulk generation shares one timestamp capture, one random-bytes fetch, and (v7) one
// contiguous counter reservation across the whole batch (at most MaxV7Batch per call):
Guid[] batch = UuidGenerator.NewV7Batch(1000);
```

Returns plain `System.Guid` — this binding does no byte-order conversion of its own beyond the `bigEndian: true` `Guid` constructor overload (.NET 8+), since that's already the correct, direct RFC 9562 mapping. `UuidGenerator.Namespaces.Dns`/`Url`/`Oid`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.Nil`/`Max` are the §5.9/§5.10 special values (`Nil` is literally `Guid.Empty`). `NewV6`/`NewV7` also accept a `DateTimeOffset` directly (`NewV6(DateTimeOffset)`), not just a raw millisecond count; `GetTimestamp` is the version-agnostic counterpart to `V6Timestamp`/`V7Timestamp` — it checks the version itself, the RFC variant included, and returns `null` for anything but a genuine RFC 9562 v6/v7 `Guid`, instead of assuming the caller already knows.

`Version`, `Variant` and `IsRfc` answer what a value is. `Version` returns the nibble (0 for `Nil`, 15 for `Max`). `Variant` returns a `UuidVariant` (`Ncs`, `Rfc9562`, `Microsoft` or `Future`). `IsRfc(id, n)` is the guard: true only for the RFC variant with version `n`. That is stricter than the BCL's `Guid.Version`, which reads the nibble alone. Each also has a `ReadOnlySpan<byte>` form over 16 raw bytes.

`Version`, `IsRfc` and the timestamp methods (`V6UnixMillis`, `V7UnixMillis`, `V6Timestamp`, `V7Timestamp` and `GetTimestamp`) each have an overload taking a `UuidLayout`. `UuidLayout.SqlServer` reads a `Guid` exactly as `V7ToSqlOrder`/`V6ToSqlOrder` return it and as ADO.NET hands it back from a `uniqueidentifier` column. The native core reads the permuted bytes in place, in one call: no `V7FromSqlOrder` first. In that layout only versions 6 and 7 exist: bytes that don't form a SQL-ordered v6 or v7 read as version 0 with no timestamp, and the core never confuses a SQL-ordered v6 for a v7, though their version nibbles sit where the other's random bits do. The layout is the caller's to know, not something a value reveals: carry it with the value, as a column type or a value converter already does. `UuidLayout.Unspecified`, the enum's `default`, throws `ArgumentOutOfRangeException` rather than guessing.

A version 7 batch (`FillV7`, `TryFillV7`, `NewV7Batch`) takes at most `UuidGenerator.MaxV7Batch` UUIDs (67,108,864, the 26-bit counter space). A larger one throws `ArgumentOutOfRangeException`, or makes `TryFillV7` return `false`, before any buffer is rented. Every batch is strictly increasing; one that crosses the counter's wrap stamps the UUIDs from there on a millisecond later ([v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely)).

`Guid.Timestamp` is that same call spelled as an extension property, via a C# 14 `extension` block (`GuidExtensions`). It exists because the classic `this Guid` extension form can only express *methods*, and a timestamp recovered from bits the value already holds is a projection, not an action — so it wants to read as a property, the way every comparable accessor on a .NET value type does. It is a re-spelling, not a second implementation: `GetTimestamp` holds the logic and a test pins the two to identical results across every version, so they cannot drift. Nothing about it is package-specific either — it reads a `Guid.CreateVersion7()` value from the BCL just as happily, since both write the same RFC 9562 layout. Extension members lower to ordinary static calls, so this adds no allocation and nothing for the trimmer or Native AOT to chase.

Before the first UUID, `UuidGenerator.IsAvailable` says whether the native library resolved and `UuidGenerator.NativeVersion` names the core it loaded — the probe a consumer with a managed fallback (`Guid.NewGuid()`, `Guid.CreateVersion7()`) gates on, instead of catching `DllNotFoundException` around its first real call. It is probed once and never throws; every other member lets a load failure propagate, the `Try*` forms included, which report the native layer's own return codes and nothing else. A library that loaded but predates the probe (0.3.0 and earlier) reads as unavailable too: a stale binary beside a newer binding is exactly the mismatch it exists to name.

## Why not `Guid.NewGuid()` / `Guid.CreateVersion7()`?

1. **It's measurably faster, not just different.** Real BenchmarkDotNet numbers, `[MemoryDiagnoser]`, linux-x64 on an Intel Core i9-11900H (`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*`, from the repo root):

   | Method | Mean | Allocated |
   | --- | ---: | ---: |
   | `Guid.NewGuid()` | 292.84 ns | 0 B |
   | `UuidGenerator.NewV4()` | 36.94 ns (**7.93x faster**) | 0 B |
   | `UuidGenerator.NewV5()` | 69.27 ns (4.23x faster) | 0 B |
   | `UuidGenerator.NewV6()` | 29.35 ns (**9.98x faster**) | 0 B |
   | `UuidGenerator.NewV7()` | 38.88 ns (7.53x faster) | 0 B |

   Including `NewV5(Guid, string)` — it used to allocate 40 B encoding the name to UTF-8 via `Encoding.UTF8.GetBytes(name)`; now it UTF-8-encodes into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, the same technique the batch methods already used (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library). `NewV5(Guid, ReadOnlySpan<char>)` is the same path for a name you hold as a slice rather than a `string` — one field of a parsed line, a pooled buffer — so nothing has to be materialized first, and `NewV5(Guid, ReadOnlySpan<byte>)` skips the encode step entirely: it hashes the bytes exactly as given, which also makes it the overload for a name that is not text at all.

2. **A real monotonic counter.** `Guid.CreateVersion7()` implements no counter (RFC 9562 §6.2 Method 1): two BCL v7 GUIDs minted in the same millisecond sort randomly relative to each other, which is exactly the clustered-index fragmentation problem v7 adoption exists to solve. `UuidGenerator.NewV7()` reserves a slot in a process-global counter every call, guaranteeing strict creation order under concurrency — verified across interleaved individual *and* batch calls in this project's own test suite, not just in isolation.
3. **v6, which the BCL doesn't have at all.** A field-compatible reordering of v1 for the same sort/index locality as v7, useful when you're migrating off legacy v1 IDs. No `Guid.CreateVersion6` exists anywhere in the BCL.
4. **Batch generation, in three shapes.** One native call, one random-bytes fetch, one counter reservation for the whole batch, instead of paying per-item overhead a thousand times. `Guid.NewGuid()`/`CreateVersion7()` have no bulk API; you'd write that loop yourself. Same machine and methodology as the table above (`--filter *Batch*`, 1000 UUIDs per operation):

   | Method | Mean | vs. individual | Allocated |
   | --- | ---: | ---: | ---: |
   | `NewV7()` x1000 individually | 37.51 µs | 1.00x | 0 B |
   | `NewV7Batch(1000)` → new `Guid[]` | 6.06 µs | 6.19x faster | 16,024 B |
   | `FillV7(Span<Guid>)` into an existing array | 5.13 µs | **7.31x faster** | **0 B** |
   | `FillV7(Span<byte>)` into an existing buffer | 3.78 µs | **9.92x faster** | **0 B** |

   The three rows amortize different things, which is why all three exist. `NewV7Batch` amortizes the FFI call but still allocates the result array. `FillV7(Span<Guid>)` drops the allocation entirely but still pays a `new Guid(chunk, bigEndian: true)` conversion per element, because `Guid`'s in-memory layout is mixed-endian and isn't RFC byte order. `FillV7(Span<byte>)` drops that conversion too — the native core already writes RFC-ordered bytes contiguously into your buffer — and that 1.4 µs gap between the last two rows *is* the per-element conversion cost, measured. `FillV6`/`NewV6Batch` behave the same way (6.71 / 5.80 / 4.54 µs respectively, against 26.68 µs for a thousand individual `NewV6()` calls).
5. **Cross-language consistency.** The exact same Rust core also mints v5 namespace UUIDs for Ruby, Python, Go, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte, and pinned by the [conformance corpus](https://github.com/SkunkWerkx/HyperUuid/tree/master/corpus) this package's tests replay alongside every other binding's. If your system isn't C#-only, that's not something the BCL can offer at all.
6. **SQL Server byte ordering, for free.** `UuidGenerator.V7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`V6ToSqlOrder` does the same for version 6), the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use. Verified directly against the real `SqlGuid` comparator in this package's own test suite, not a hand-rolled stand-in — and it's the same native function every other binding in this repo calls, not a C#-only reimplementation. Neither `Guid.NewGuid()` nor `Guid.CreateVersion7()` has any such concept.

7. **Non-throwing and zero-conversion call shapes, for callers that need them.** Every operation the native layer can fail — a random-source failure, or a timestamp its field cannot hold — has a `Try` twin that reports it as `false` rather than an exception: `TryNewV4`/`TryNewV6`/`TryNewV7` for single IDs, `TryFillV6`/`TryFillV7` for batches into a buffer you own. No exception ever crosses the P/Invoke boundary in either shape (the native layer signals with an `int` return code; see `rust/src/ffi.rs`), but the `Try` form lets a `Result`-style gateway branch on failure without wrapping every call in a `try`/`catch`. What has no twin, on purpose: `NewV5` has no failure mode to report (a hash of your own bytes, no random source and no timestamp); the allocating `NewV6Batch`/`NewV7Batch` are conveniences over the fills, so the non-throwing batch is `TryFill*` into your own array; and `V7Timestamp` throws only where `DateTimeOffset` itself cannot represent the year. Nor does `Try` cover a native library that never loaded — that is `UuidGenerator.IsAvailable`'s question, asked once up front. Separately, the SQL-order transforms and the batch fills both have raw-`Span<byte>` overloads that never construct a `Guid` at all — RFC-ordered bytes in, transformed bytes out, in place. That matters for two reasons: it's the form a byte-level correctness oracle can be pointed at directly (no need to model `Guid`'s mixed-endian field layout to compare results), and it's measurably the fastest batch path, since the native core already writes RFC bytes contiguously into your buffer and the `Guid` overload has to convert every element on the way out.

**The honest trade-off:** this is a native dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID) instead of a BCL type that's always just there. If you only need plain v4 randomness in a C#-only codebase, `Guid.NewGuid()` is simpler and that's a completely reasonable choice.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime reflection anywhere in this assembly, and the project opts into (and fails the build on) the trim/Native-AOT analyzers via `IsAotCompatible`.

That claim is reproducible rather than asserted. `HyperUuid.AotSmokeTest/` is a real AOT-published console app that crosses every native entry point the binding declares — the eighteen `uuid_*` functions and `hyperuuid_version` — through every call shape the public surface offers: the version probe; v4, v5, v6 and v7 generation, with v5 against the RFC 9562 Appendix A.4 vector on all three name overloads and for an empty name; the non-throwing `Try*` path (including that an out-of-range timestamp is *reported*, not thrown); both SQL-order directions for both versions, raw bytes against their `Guid` counterparts; the batch fills into `Guid` and raw-byte buffers and the allocating batches, and the v7 batch limit; inspection, and the SQL-layout reads on values still in SQL order; and the `Guid.Timestamp` extension property. It returns a nonzero exit code on any mismatch:

```shell
dotnet publish csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj \
  -c Release -r linux-x64 -p:PublishAot=true
./csharp/HyperUuid.AotSmokeTest/bin/Release/net10.0/linux-x64/publish/HyperUuid.AotSmokeTest
```

Last verified on `linux-x64` and, inside an Alpine container, `linux-musl-x64`: **zero `ILxxxx`/`AOTxxxx` trim or AOT diagnostics**, a 1.5 MB native binary with the core inside it and no shared library beside it, and `ALL NATIVE AOT CHECKS PASSED` with exit code 0. `TreatWarningsAsErrors` is on for the library project, so an analyzer warning is a build failure, not a line in a log nobody reads. CI re-proves it per platform on every PR (see [Native binary provenance](#native-binary-provenance)).

**The core is linked into the executable.** A Native AOT publish does not load
`libhyperuuid.so` (or the `.dylib`, or the `.dll`): the package carries the core as a static
library for each RID under `staticlibs/`, and its targets file hands the one for your RID to
the AOT linker and binds every P/Invoke as a direct call. The publish directory holds one
executable and no native library beside it, `UuidGenerator.IsAvailable` is always `true`, and
this package and HyperCast's can both be linked into the same executable. Nothing to configure;
`<HyperUuidStaticLink>false</HyperUuidStaticLink>` in the project puts it back to loading the shared
library, and so does publishing for a RID the package has no archive for. A JIT process is
unaffected: it cannot link an archive, and loads the shared library as before.

## WebAssembly (Blazor)

Works from a plain `<PackageReference Include="HyperUuid" />` on **.NET 11 and later** — no `<NativeFileReference>`, no hand-written P/Invoke, no bridging in your own Rust build. Verified for real, in an actual headless Chromium session: `HyperUuid.WasmSmokeTest` (below) answers the version probe, generates v4s, matches the RFC 9562 v5 vector, round-trips v6/v7 timestamps, fills 1000-id v6 and v7 batches in one native call each, round-trips the SQL-order transforms, and embeds a real-clock v7 timestamp within 2 seconds of the wall clock — not just "it builds."

**Target frameworks.** Two floors, different on purpose. The package targets net10.0, which is what the native platforms need. WebAssembly is .NET 11 and later only: the exception-handling translation described below is .NET 11 toolchain behavior, and the smoke test targets `net11.0` and nothing older. NuGet still imports the package's `.targets` into a net10.0 Blazor WebAssembly project (a `.targets` file directly under `build/` applies to every target framework), so the wiring is gated on the consuming project's own target framework: below .NET 11 the native core is not linked, and the build says so with warning `HYPERUUID001` rather than leaving it to be discovered in the browser. In that configuration `UuidGenerator.IsAvailable` is `false` — or, where the `wasm-tools` workload relinks the runtime, the link stops on undefined `uuid_*` symbols — so target net11.0, or gate on `IsAvailable` and keep a managed fallback.

**How:** one compiled assembly covers every platform, including `browser-wasm` — no separate build. Every native entry point is declared three times, unconditionally, sharing the same underlying C symbol: once against `"hyperuuid"` (resolved via `dlopen` on every real native platform), once against `"*"` (a statically-linked WASM native has no separate `"hyperuuid"` module to open, since its functions are already part of the same `dotnet.native.wasm` the app itself runs in; `"*"` resolves against the current module instead), and once against `"__Internal"` for iOS and Mac Catalyst (see [Platform support](#platform-support)). `OperatingSystem.IsBrowser()` and `OperatingSystem.IsIOS()` pick the right one at each call site — real runtime checks the .NET linker specifically knows how to constant-fold per publish target (the same mechanism the BCL itself uses for platform-conditional code), so a trimmed/published build still only ships the branch that platform can actually reach. Only the Rust core's `wasm32-unknown-emscripten` static library (`cargo rustc --crate-type staticlib` — the default `cdylib` produces an already-linked module `NativeFileReference` can't pull symbols from) is genuinely RID-specific, landing under `runtimes/browser-wasm/nativeassets/net10.0/` in the `.nupkg`; the managed assembly itself needs no RID-specific copy anymore, confirmed by inspecting a real self-contained `dotnet publish -r <rid>` output too: no WASM files leak into a non-WASM deployment.

Building this exact source once instead of twice also turned out to matter beyond simplicity: two independent, from-scratch `dotnet pack` runs produce a byte-identical managed assembly (verified with a real checksum comparison, not assumed) — the earlier two-builds-sharing-one-`obj/`-directory design never gave that guarantee, and was the leading suspect for this package's NuGet health-check failures on the releases that used it (0.0.5 and earlier).

The one piece that *doesn't* auto-wire — NuGet's `runtimes/{rid}/nativeassets/{tfm}/` convention is real, and the WASM SDK really does auto-promote a resolved `NativeLibrary` item into `NativeFileReference`, but restore doesn't actually populate `NativeLibrary` from a plain `PackageReference`'s `nativeassets` folder the way it does `native/` for ordinary P/Invoke (confirmed empirically, not assumed) — is supplied by this package's own `build/HyperUuid.targets`, auto-imported into every consuming project via NuGet's standard convention, which adds both the `NativeFileReference` and (also confirmed necessary the hard way — linking the code in alone does *not* make it resolvable via `"*"` at runtime) an explicit `EmccExportedFunction` entry per native function this package P/Invokes. That's the actual mechanism making this "just works" for real, not a hopeful description of how NuGet packaging is supposed to behave.

`HyperUuid.WasmSmokeTest` proves this chain in a real browser: a Blazor WebAssembly app (`net11.0`) that imports the shipped `build/HyperUuid.targets`, calls every native entry point — the eighteen `uuid_*` functions and `hyperuuid_version` — through the public `UuidGenerator` surface, and renders `PASS` or `FAIL`. Every one, because that is the only way the check means what it says: an entry point missing from the `EmccExportedFunction` list links fine and fails only when called. `./check.sh` in that directory stages the wasm static library, publishes the app, loads it in headless Chromium and requires `PASS`. CI runs the same script on every PR, in headless Chrome with the `wasm-tools` workload, against the static library that run just built (`STATICLIB` names it, so the script skips building its own); it is not part of the solution, so a plain `dotnet build` never needs the workload or a browser.

**WebAssembly is .NET 11 and later only.** .NET 11 links browser-wasm with the new (exnref) exception-handling encoding, while the precompiled Rust standard library inside the static library uses the legacy one, and the browser refuses a module that mixes them (`module uses a mix of legacy and new exception handling instructions`). The same `HyperUuid.targets` therefore appends Binaryen's translate-to-exnref pass to the SDK's post-link `wasm-opt`, with no action needed from a consumer.

**Previously reported SDK blocker, not reproduced on .NET 11.** Earlier `wasm-tools` SDK bands failed any native relink with `Unknown option '--enable-bulk-memory-opt'`, a rustc-versus-Binaryen skew filed as [dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858). On .NET SDK 11.0.100-rc.1 the smoke test relinks cleanly with the SDK's bundled `wasm-opt` and no workaround. If you hit it on another band, swapping the SDK's bundled `wasm-opt` (`~/.dotnet/packs/Microsoft.NET.Runtime.Emscripten.<version>.Sdk.<rid>/<pack-version>/tools/bin/wasm-opt`) for a newer one from a standalone `emsdk` is the verified workaround.

## Platform support

Native binaries ship inside the package for ten RIDs, plus static libraries for WebAssembly, iOS and Mac Catalyst. With Windows, macOS, iOS, Mac Catalyst and Android, that is every platform .NET MAUI targets:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux (glibc) | `linux-x64`, `linux-arm64` | `libhyperuuid.so` |
| Linux (musl — Alpine) | `linux-musl-x64`, `linux-musl-arm64` | `libhyperuuid.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhyperuuid.dylib` |
| Windows | `win-x64`, `win-arm64` | `hyperuuid.dll` |
| Blazor WebAssembly (.NET 11+) | `browser-wasm` | `libhyperuuid.a` (static — see above) |
| iOS | `ios-arm64`, `iossimulator-arm64` | `libhyperuuid.a` (static — see below) |
| Mac Catalyst | `maccatalyst-arm64`, `maccatalyst-x64` | `libhyperuuid.a` (static — see below) |
| Android (API 21+) | `android-arm64`, `android-x64` | `libhyperuuid.so`; `libhyperuuid.a` for Native AOT (see below) |

**musl is its own build, not the glibc one relabeled.** A glibc `libhyperuuid.so` does not load under musl's dynamic loader, and NuGet's RID graph falls back from `linux-musl-x64` to `linux-x64` when nothing more specific is in the package — which is what 0.3.0 and earlier did on Alpine: the glibc library was selected, failed to load, and the first call threw. The musl libraries are built inside Alpine itself and depend on nothing but musl libc. Proven the way a consumer meets it, in an `mcr.microsoft.com/dotnet/sdk` Alpine container: this binding's whole test suite and the Native AOT smoke test (`-r linux-musl-x64`) against the musl library, then a throwaway console app consuming the packed `.nupkg` through a plain `PackageReference` with the glibc *and* musl libraries both inside it, which maps `runtimes/linux-musl-x64/native/libhyperuuid.so` and nothing else. The same app against a glibc-only package is the control: `UuidGenerator.IsAvailable` is `false` there, and nothing throws until something ignores it.

On any platform outside that table the package still restores and compiles — the managed assembly is platform-neutral — and `UuidGenerator.IsAvailable` is how an app finds out at run time that no native library came with it.

**iOS and Mac Catalyst: the core is linked into the app.** A .NET iOS, MAUI or Mac Catalyst app references the package like any other and writes nothing else. Those platforms have no `runtimes/{rid}/native/` to load a library from: the .NET SDK for them links native code into the app's own executable, and a P/Invoke reaches it under the library name `__Internal`. So the package carries the core as a static library for each of the four RIDs above, `build/HyperUuid.targets` hands the one for the RID being built to the SDK as a [`NativeReference` with `Kind=Static`](https://learn.microsoft.com/dotnet/maui/migration/ios-binding-projects), and `UuidGenerator` declares every entry point a third time against `__Internal`, picked by `OperatingSystem.IsIOS()` (which is true on Mac Catalyst too). It is the SDK's own native link that takes the archive, so the same wiring serves an app compiled by Mono's AOT compiler, one that runs interpreted, and one published with [Native AOT](https://learn.microsoft.com/dotnet/core/deploying/native-aot/ios-like-platforms/). A universal Mac Catalyst app is built once per RID and merged, and each half links its own archive.

`HyperUuid.AppleSmokeTest` is the Native AOT smoke test (`SmokeTest.cs`) as an app, and CI's `test-apple-mobile` job builds it three ways on a Mac from that run's archives: as a Mac Catalyst app, run as a process; for the iOS simulator, installed and launched; and for an iOS device with signing off, where the check is that the app's executable defines the core's symbols, since no runner has a device to run it on.

**Android: the shared library, out of the APK.** A .NET for Android or MAUI app (`net11.0-android`, API 24 and later, .NET 11's floor) references the package and writes nothing else. On CoreCLR, .NET 11's Android runtime (Mono is no longer supported there), the SDK takes `runtimes/android-arm64/native/libhyperuuid.so` and its x64 twin out of the package and stores each in the APK under `lib/arm64-v8a/` and `lib/x86_64/`, and the ordinary `hyperuuid` import opens it like any Linux library. The libraries are built with the NDK for API level 21, below any app that can reference them, and their segments are aligned to 16 KB: Android 15 devices may use 16 KB pages, a library aligned for 4 KB does not load on one, and Google Play requires the alignment of every new app. A Native AOT publish (`PublishAot`, `-r android-arm64`) takes the [AOT](#aot) wiring instead, linking `staticlibs/android-{rid}/libhyperuuid.a` into the app's own native library, and the shared one is left out of the APK. `android-arm64` covers effectively every Android device in use and `android-x64` the emulator; these are the two RIDs .NET for Android builds by default. The 32-bit `android-arm` and `android-x86` are not in the package, so an app that adds them gets `UuidGenerator.IsAvailable == false` on those ABIs.

`HyperUuid.AndroidSmokeTest` is the Native AOT smoke test's `SmokeTest.Run()` again, started from an Activity, and unlike the other smoke tests it takes HyperUuid as a package, from a local folder CI packs it into, because the package's layout is what Android needs proven. CI's `test-android` job builds it four ways from that run's libraries: CoreCLR and Native AOT, for each RID. The x64 pair runs in an x86_64 emulator whose image uses 16 KB pages, so a library aligned for 4 KB would fail to load there. Nothing hosted runs arm64 Android, so the arm64 pair is inspected instead: each CoreCLR APK must carry `libhyperuuid.so` for its ABI and pass `zipalign -P 16`, the check Google Play applies, and each Native AOT APK must not carry it at all, because the core is linked into the app.

Two .NET 11 RC1 workload behaviors shape that job, and an app on RC1 may meet them too. The android workload's build tasks require JDK 21 (`XA0030` on newer). And the workload RC1 resolves was built against a runtime newer than RC1 on nuget.org, so a Native AOT publish fails to restore `Microsoft.NETCore.App.Runtime.NativeAOT.android-*` by exact version until the .NET 11 daily feed (`https://pkgs.dev.azure.com/dnceng/public/_packaging/dotnet11/nuget/v3/index.json`) is added as a restore source; both should go away by .NET 11 GA.

**Not supported: tvOS and the iOS simulator on Intel Macs.** `iossimulator-x64` and tvOS have no archive, so an app for them fails at its native link on the undefined `uuid_*` symbols, at build time and not at run time.

## Native binary provenance

The `.nupkg` carries compiled native code, which is a real thing to ask questions about before adopting it inside a trust boundary. What is and isn't currently guaranteed:

**Where the binaries come from.** Nothing under `csharp/HyperUuid/runtimes/` is committed — it's `.gitignore`d. The native libraries are built from `rust/` by CI and staged into the package at pack time, so what ships is produced by the same workflow run that built and tested the source. (The Go, PHP, and Swift bindings are different: those *do* carry committed binaries, staged by `stage-native-binaries.yml`, whose commit message records the exact source SHA and CI run ID they came from — e.g. `chore: stage native binaries from ci.yml run 33438784689`.)

**Building it yourself.** The core is a normal Rust crate with no build-time codegen, so you never have to take the shipped binary at all:

```shell
cd rust && cargo cdylib
# -> target/release/libhyperuuid.so  (.dylib on macOS, hyperuuid.dll on Windows)
```

Drop the result into `csharp/HyperUuid/runtimes/<rid>/native/` and the package's own MSBuild globs will pick it up, or point `dlopen` at it however you prefer — the C ABI in `rust/src/ffi.rs` is the entire contract: the eighteen `uuid_*` functions and `hyperuuid_version`, taking plain pointers into your own buffers. For local development nothing needs dropping anywhere: when no library has been staged under `runtimes/` for your machine's RID, the project copies `rust/target/release/` straight to the output, so `dotnet test` after a `cargo cdylib` just runs.

**Reproducibility, stated honestly.** The build is deterministic *locally*: `cargo clean -p hyperuuid` followed by `cargo cdylib` reproduces a byte-identical `libhyperuuid.so` (verified by SHA-256). It is **not** currently bit-reproducible *across machines* — a local `rustc 1.98.0` build on WSL and the CI-built `linux-arm64` artifact differ in both hash and size (458,712 vs 458,176 bytes), as you'd expect from differing toolchain versions and embedded build paths. So "rebuild it and compare hashes" is not a verification path a consumer can currently rely on.

**Signed provenance.** Because rebuild-and-compare doesn't work across machines, the mechanism that does is a cryptographic attestation binding each artifact to the workflow run and commit that produced it. CI emits [SLSA build provenance](https://github.com/actions/attest-build-provenance) at three points, because the package is not the same bytes at every stage of its life:

| Attested artifact | Where | How to verify |
| --- | --- | --- |
| Each native library, as built | `hyper-build-native.yml` | `gh attestation verify libhyperuuid.so --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github` |
| The `.nupkg` as packed, pre-push | `hyper-pack-nuget.yml` | strip the repo signature first (below) |
| The `.nupkg` as published | `release.yml`, after the push | verify the downloaded file directly |

The reason for the last two rows: **nuget.org adds its repository signature as a `.signature.p7s` entry inside the `.nupkg` zip during validation**, which changes the file's SHA-256. So the package you download is not the package that was built, and one attestation cannot cover both. Rather than pick, the pipeline takes both — and because the mutation is exactly one added zip entry, the pre-push attestation stays recoverable:

```shell
# verify the published bytes directly — nothing to undo.
# Signed by release.yml, which lives in this repo, so no --signer-repo is needed.
gh attestation verify HyperUuid.X.Y.Z.nupkg --repo SkunkWerkx/HyperUuid

# or recover the as-built artifact and verify that instead.
# Signed by hyper-pack-nuget.yml over in the forge repo, so this half needs --signer-repo.
zip -d HyperUuid.X.Y.Z.nupkg .signature.p7s
gh attestation verify HyperUuid.X.Y.Z.nupkg \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

**Why `--signer-repo` appears on some of these and not others.** `--repo X` asserts two
separate things: that the artifact came from repo X, and that the workflow which signed it
also lives in X. Everything CI builds here comes from this repo, so the first half always
holds — but the signing step's location varies. Anything signed inside a reusable workflow
(`hyper-build-native.yml`, `hyper-pack-nuget.yml`) is signed by a file that physically lives
in `SkunkWerkx/.github`, and that is what Fulcio records as the build signer; anything signed
directly by this repo's own `release.yml` is signed by this repo. Get it wrong and `gh`
reports `verifying with issuer "sigstore.dev"` with no further detail, which reads like a bad
signature but is only an identity mismatch. `--owner SkunkWerkx` works for every row above if
you would rather not track which is which.

The release run's job summary prints all three digests — as packed, as published, and as published-with-the-signature-removed — and asserts that the third equals the first. That claim is checked on every release rather than asserted here, so if nuget.org ever changes how it finalizes packages, the run says so instead of the README quietly going stale.

Attestations are produced by every release-mode build — the release's own run and the weekly one. Pull requests build in `pr` mode, which ships nothing and so signs nothing. The post-publish half is non-blocking: the push is irreversible, so a slow nuget.org validation is never allowed to turn a successful publish into a failed release.

**Not currently done: NuGet author signing.** The package carries nuget.org's repository signature but no author signature of our own, which would need an X.509 code-signing certificate registered to the account. It's complementary to the above rather than a substitute, and the difference is who does the checking: an author signature is verified automatically by every consumer's SDK at restore time, whereas an attestation is only checked by someone who deliberately runs `gh attestation verify`. Attestation ties an artifact to a commit and a build; an author signature ties it to an identity. If you want the automatic restore-time check, this is the gap.

**Per-platform AOT receipts.** The same CI run publishes `HyperUuid.AotSmokeTest` under Native AOT on five desktop RIDs (every one but `osx-x64`, which is cross-built and has no CI leg) and, in Alpine containers, on the two musl ones, fails the build on any `ILxxxx`/`AOTxxxx` trim diagnostic, executes the resulting binary, and requires exit 0. Each leg's log uploads as an `aot-report-{rid}` artifact.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperUuid) — no extra package source needed:

```shell
dotnet add package HyperUuid
```

Per-RID native libraries ship inside the package under `runtimes/`, so a consumer adds one reference and nothing else — no build step, no manual native staging. See [Platform support](#platform-support) for the list.

Targets net10.0 for the native platforms; Blazor WebAssembly needs .NET 11 or later — see [WebAssembly (Blazor)](#webassembly-blazor).

See [the repo root README](https://github.com/SkunkWerkx/HyperUuid/blob/master/README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
