# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**`UuidGenerator.NewV4()` beats `Guid.NewGuid()` by ~8x — with zero heap allocation, on every version including v5 — because it calls straight into a native Rust core instead of the BCL's own managed generator.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, plus v7's SQL Server order as an RFC 9562 version 8, calling directly into the native `libhyperuuid` shared library via source-generated [`LibraryImport`](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) P/Invoke — no runtime bridge, no reflection, AOT/trim-friendly. Ships as RID-specific native assets inside the package the standard NuGet way.

```sh
dotnet add package HyperUuid
```

```csharp
using HyperUuid;

var id = UuidGenerator.NewV4();
var id2 = UuidGenerator.NewV5(UuidGenerator.Namespaces.Dns, "example.com");
var id4 = UuidGenerator.NewV7();
Console.WriteLine($"v4: {id}");
Console.WriteLine($"v5: {id2}");
Console.WriteLine($"v7: {id4}");

// A v7 round-trips its embedded timestamp:
DateTimeOffset created = UuidGenerator.V7Timestamp(id4);
Console.WriteLine($"timestamp: {created}");
long unixMillis = UuidGenerator.V7UnixMillis(id4); // the same instant as Unix-epoch milliseconds
Console.WriteLine($"unix millis: {unixMillis}");

// Version-agnostic: null instead of assuming id4 is a v7:
DateTimeOffset? maybeCreated = UuidGenerator.GetTimestamp(id4);
Console.WriteLine($"checked timestamp: {maybeCreated}");

// Same thing as an extension property on Guid itself — works on any Guid, whatever made it:
DateTimeOffset? created7 = id4.Timestamp;              // the creation time
DateTimeOffset? none = Guid.NewGuid().Timestamp;       // null — a v4 UUID carries no time
Console.WriteLine($"extension timestamp: {created7}");
Console.WriteLine($"v4 timestamp: {none?.ToString() ?? "null"}");

// Inspection: the one-call guard (RFC variant and version 7):
bool isV7 = UuidGenerator.IsRfc(id4, 7);
Console.WriteLine($"is v7: {isV7}");

// The layout SQL Server's uniqueidentifier sorts by creation order, an RFC 9562 version 8
// (throws ArgumentException for anything but a v7):
Guid sqlOrdered = UuidGenerator.V7ToSqlOrder(id4);
Console.WriteLine($"sql order: {sqlOrdered}");

// The wire bytes are ToByteArray(), not the string form: this is the hex the corpus's
// sql_order.json vectors hold. sqlOrdered.ToString() prints .NET's mixed-endian reading of them.
string sqlHex = Convert.ToHexStringLower(sqlOrdered.ToByteArray());
Console.WriteLine($"sql hex: {sqlHex}");

// The same guard in either layout, so a value read back from a uniqueidentifier column is
// validated and dated without converting it back:
bool fromV7 = UuidGenerator.IsRfc(sqlOrdered, 7, UuidLayout.SqlServer); // true: converted from a v7
bool isV8 = UuidGenerator.IsRfc(sqlOrdered, 8);                         // true: what it is now
DateTimeOffset? fromSql = UuidGenerator.GetTimestamp(sqlOrdered, UuidLayout.SqlServer);
Console.WriteLine($"sql is v7: {fromV7}");
Console.WriteLine($"is v8: {isV8}");
Console.WriteLine($"sql timestamp: {fromSql}");

// Bulk generation shares one timestamp capture, one random-bytes fetch, and (v7) one
// contiguous counter reservation across the whole batch (at most MaxV7Batch per call):
Guid[] batch = UuidGenerator.NewV7Batch(1000);
Console.WriteLine($"batch: {batch.Length}");
```

Returns plain `System.Guid` — this binding does no byte-order conversion of its own beyond the `bigEndian: true` `Guid` constructor overload (.NET 8+), since that's already the correct, direct RFC 9562 mapping. `UuidGenerator.Namespaces.Dns`/`Url`/`Oid`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.Nil`/`Max` are the §5.9/§5.10 special values (`Nil` is literally `Guid.Empty`). `NewV7` also accepts a `DateTimeOffset` directly (`NewV7(DateTimeOffset)`), not just a raw millisecond count; `GetTimestamp` is the version-agnostic counterpart to `V7Timestamp` — it checks the version itself, the RFC variant included, and returns `null` for anything but a genuine RFC 9562 v7 `Guid`, instead of assuming the caller already knows. A v1 or v6 minted elsewhere reports its version and has no timestamp here.

`Version`, `Variant` and `IsRfc` answer what a value is. `Version` returns the nibble (0 for `Nil`, 15 for `Max`). `Variant` returns a `UuidVariant` (`Ncs`, `Rfc9562`, `Microsoft` or `Future`). `IsRfc(id, n)` is the guard: true only for the RFC variant with version `n`. That is stricter than the BCL's `Guid.Version`, which reads the nibble alone. Each also has a `ReadOnlySpan<byte>` form over 16 raw bytes.

`Version`, `IsRfc` and the timestamp methods (`V7UnixMillis`, `V7Timestamp` and `GetTimestamp`) each have an overload taking a `UuidLayout`. `UuidLayout.SqlServer` reads a `Guid` exactly as `V7ToSqlOrder` returns it and as ADO.NET hands it back from a `uniqueidentifier` column. The native core reads the converted bytes in place, in one call: no `V7FromSqlOrder` first. The conversion writes an RFC 9562 version 8 (§5.8, the version for custom layouts), so in that layout `Version` answers the version a value was converted from, 7, and 0 for anything else, and the timestamp methods read its timestamp. The layout is the caller's to know, not something a value reveals: carry it with the value, as a column type or a value converter already does, and pass it at every call site that can see a SQL-ordered value. (An RFC-ordered version 8 `Guid` minted elsewhere has its 8 and variant exactly where SQL order puts them, and reads as a converted value in that layout.) The overloads without a layout read RFC order, and to them, as to `Guid.Version` and SQL Server's own text form, a SQL-ordered value is what it is, a version 8: `Version(sqlOrdered)` is 8, `IsRfc(sqlOrdered, 8)` is true, `IsRfc(sqlOrdered, 7)` is false, and `GetTimestamp(sqlOrdered)` is `null`. Pass `UuidLayout.SqlServer` to learn the source version and its timestamp. There is deliberately no layout-agnostic check, and `Variant` is RFC order only (`IsRfc` with the layout is the guard for a SQL-ordered value). `UuidLayout.Unspecified`, the enum's `default`, throws `ArgumentOutOfRangeException` rather than guessing.

The SQL-order conversions are exact inverses: a v7 and a v8 both have 122 free bits, so every RFC 9562 version 7 converts, whatever its timestamp, and comes back exactly. `V7ToSqlOrder` throws `ArgumentException` for anything that isn't a version 7, and `V7FromSqlOrder` for anything that doesn't carry the SQL layout's 8 and variant; the raw-byte overloads leave the buffer as it was.

A version 7 batch (`FillV7`, `TryFillV7`, `NewV7Batch`) takes at most `UuidGenerator.MaxV7Batch` UUIDs (67,108,864, the 26-bit counter space). A larger one throws `ArgumentOutOfRangeException`, or makes `TryFillV7` return `false`, before any buffer is rented. Every batch is strictly increasing; one that outruns a millisecond's steps splits into the next and stamps the UUIDs from there on a millisecond later ([v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely)).

Every v7 call reserves the next step of one process-wide stream. A call whose timestamp is within 10 s of the newest the stream has reserved at sorts after every UUID from a call that completed before it began, on any thread. A timestamp more than 11 s behind the stream, or past 2^44 − 2, is minted at itself, outside the stream, and is ordered against nothing: backfill and fixed test timestamps work, but one call far in the future leaves real-time calls out of stream until the clock catches up. A split that would take the stream more than 1000 ms ahead of the supplied timestamp, or past 2^44 − 2, throws `InvalidOperationException` (`version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond`) and reserves nothing, so it is a state error and not an argument error. A call whose timestamp is past the stream's millisecond succeeds. `TryNewV7` and `TryFillV7` return `false` for it, as for any other failure.

`Guid.Timestamp` is that same call spelled as an extension property, via a C# 14 `extension` block (`GuidExtensions`). It exists because the classic `this Guid` extension form can only express *methods*, and a timestamp recovered from bits the value already holds is a projection, not an action — so it wants to read as a property, the way every comparable accessor on a .NET value type does. It is a re-spelling, not a second implementation: `GetTimestamp` holds the logic and a test pins the two to identical results across every version, so they cannot drift. Nothing about it is package-specific either — it reads a `Guid.CreateVersion7()` value from the BCL just as happily, since both write the same RFC 9562 layout. Extension members lower to ordinary static calls, so this adds no allocation and nothing for the trimmer or Native AOT to chase.

Before the first UUID, `UuidGenerator.IsAvailable` says whether the native library resolved and `UuidGenerator.NativeVersion` names the core it loaded, as a `System.Version` (`null` when nothing loaded) — the probe a consumer with a managed fallback (`Guid.NewGuid()`, `Guid.CreateVersion7()`) gates on, instead of catching `DllNotFoundException` around its first real call. It is probed once and never throws; every other member lets a load failure propagate, the `Try*` forms included, which report the native layer's own return codes and nothing else. A library that loaded but predates the probe (0.3.0 and earlier) reads as unavailable too: a stale binary beside a newer binding is exactly the mismatch it exists to name.

## Why not `Guid.NewGuid()` / `Guid.CreateVersion7()`?

1. **It's measurably faster, not just different.** Real BenchmarkDotNet numbers, `[MemoryDiagnoser]`, linux-x64 on an Intel Core i9-11900H (`dotnet run -c Release --project csharp/HyperUuid.Benchmarks -- --filter *Generation*`, from the repo root):

   | Method | Mean | Allocated |
   | --- | ---: | ---: |
   | `Guid.NewGuid()` | 292.84 ns | 0 B |
   | `UuidGenerator.NewV4()` | 36.94 ns (**7.93x faster**) | 0 B |
   | `UuidGenerator.NewV5()` | 69.27 ns (4.23x faster) | 0 B |
   | `UuidGenerator.NewV7()` † | 38.88 ns (7.53x faster) | 0 B |

   † Measured before the per-millisecond re-seed (core change, 2026-10-27).

   Including `NewV5(Guid, string)` — it UTF-8-encodes the name into a 256-byte stack buffer with an `ArrayPool` fallback for longer names, rather than allocating through `Encoding.UTF8.GetBytes(name)`, the same technique the batch methods use (and, before that, proven in this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) library). `NewV5(Guid, ReadOnlySpan<char>)` is the same path for a name you hold as a slice rather than a `string` — one field of a parsed line, a pooled buffer — so nothing has to be materialized first, and `NewV5(Guid, ReadOnlySpan<byte>)` skips the encode step entirely: it hashes the bytes exactly as given, which also makes it the overload for a name that is not text at all.

2. **A real counter stream.** `Guid.CreateVersion7()` implements no counter (RFC 9562 §6.2 Method 1): two BCL v7 GUIDs minted in the same millisecond sort randomly relative to each other, which is exactly the clustered-index fragmentation problem v7 adoption exists to solve. `UuidGenerator.NewV7()` reserves the next step of one process-global stream every call, guaranteeing strict creation order under concurrency for calls within 10 s of the stream's newest timestamp ([v7 ordering, precisely](https://github.com/SkunkWerkx/HyperUuid#v7-ordering-precisely)) — verified across interleaved individual *and* batch calls in this project's own test suite, not just in isolation.
3. **Batch generation, in three shapes.** One native call, one random-bytes fetch, one counter reservation for the whole batch, instead of paying per-item overhead a thousand times. `Guid.NewGuid()`/`CreateVersion7()` have no bulk API; you'd write that loop yourself. Same machine and methodology as the table above (`--filter *Batch*`, 1000 UUIDs per operation):

   | Method | Mean | vs. individual | Allocated |
   | --- | ---: | ---: | ---: |
   | `NewV7()` x1000 individually † | 37.51 µs | 1.00x | 0 B |
   | `NewV7Batch(1000)` → new `Guid[]` † | 6.06 µs | 6.19x faster | 16,024 B |
   | `FillV7(Span<Guid>)` into an existing array † | 5.13 µs | **7.31x faster** | **0 B** |
   | `FillV7(Span<byte>)` into an existing buffer † | 3.78 µs | **9.92x faster** | **0 B** |

   † Measured before the per-millisecond re-seed (core change, 2026-10-27).

   The three rows amortize different things, which is why all three exist. `NewV7Batch` amortizes the FFI call but still allocates the result array. `FillV7(Span<Guid>)` drops the allocation entirely but still pays a `new Guid(chunk, bigEndian: true)` conversion per element, because `Guid`'s in-memory layout is mixed-endian and isn't RFC byte order. `FillV7(Span<byte>)` drops that conversion too — the native core already writes RFC-ordered bytes contiguously into your buffer — and that 1.4 µs gap between the last two rows *is* the per-element conversion cost, measured.
4. **Cross-language consistency.** The exact same Rust core also mints v5 namespace UUIDs for Ruby, Python, Go, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte, and pinned by the [conformance corpus](https://github.com/SkunkWerkx/HyperUuid/tree/master/corpus) this package's tests replay alongside every other binding's. If your system isn't C#-only, that's not something the BCL can offer at all.
5. **SQL Server byte ordering, for free.** `UuidGenerator.V7ToSqlOrder(id4)` converts a version 7 UUID to a layout that `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — sorts by creation order. The result is an RFC 9562 version 8, so it remains a valid UUID that says what it is; it is the permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid) uses, with a version 8 in place of its 7. Verified directly against the real `SqlGuid` comparator in this package's own test suite, not a hand-rolled stand-in — and it's the same native function every other binding in this repo calls, not a C#-only reimplementation. Neither `Guid.NewGuid()` nor `Guid.CreateVersion7()` has any such concept. (Measured before the per-millisecond re-seed, core change, 2026-10-27.)

6. **Non-throwing and zero-conversion call shapes, for callers that need them.** Every generation the native layer can fail — a random-source failure, a timestamp its field cannot hold, or an exhausted v7 stream — has a `Try` twin that reports it as `false` rather than an exception: `TryNewV4`/`TryNewV7` for single IDs, `TryFillV7` for batches into a buffer you own. No exception ever crosses the P/Invoke boundary in either shape (the native layer signals with an `int` return code; see `rust/src/ffi.rs`), but the `Try` form lets a `Result`-style gateway branch on failure without wrapping every call in a `try`/`catch`. What has no twin, on purpose: `NewV5` has no failure mode to report (a hash of your own bytes, no random source and no timestamp); the allocating `NewV7Batch` is a convenience over the fills, so the non-throwing batch is `TryFillV7` into your own array; `V7Timestamp` throws only where `DateTimeOffset` itself cannot represent the year; and the SQL-order conversions throw only for a value of the wrong version, which `IsRfc` checks without throwing (`IsRfc(value, 7)` on the way in, `IsRfc(value, 7, UuidLayout.SqlServer)` on the way back). Nor does `Try` cover a native library that never loaded — that is `UuidGenerator.IsAvailable`'s question, asked once up front. Separately, the SQL-order transforms and the batch fills both have raw-`Span<byte>` overloads that never construct a `Guid` at all — RFC-ordered bytes in, transformed bytes out, in place. That matters for two reasons: it's the form a byte-level correctness oracle can be pointed at directly (no need to model `Guid`'s mixed-endian field layout to compare results), and it's measurably the fastest batch path, since the native core already writes RFC bytes contiguously into your buffer and the `Guid` overload has to convert every element on the way out.

**The honest trade-off:** this is a native dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID) instead of a BCL type that's always just there. If you only need plain v4 randomness in a C#-only codebase, `Guid.NewGuid()` is simpler and that's a completely reasonable choice.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime reflection anywhere in this assembly, and the project opts into (and fails the build on) the trim/Native-AOT analyzers via `IsAotCompatible`.

That claim is reproducible rather than asserted. `HyperUuid.AotSmokeTest/` is a real AOT-published console app that crosses every native entry point the binding declares — the twelve `uuid_*` functions and `hyperuuid_version` — through every call shape the public surface offers: the version probe; v4, v5 and v7 generation, with v5 against the RFC 9562 Appendix A.4 vector on all three name overloads and for an empty name; the non-throwing `Try*` path (including that an out-of-range timestamp is *reported*, not thrown); a v6 minted elsewhere reporting its version and no timestamp; both SQL-order directions, raw bytes against their `Guid` counterparts, the version 8 they write, the last v7 millisecond round-tripping, and their refusals (a wrong version either way, the bytes left untouched); the batch fills into `Guid` and raw-byte buffers and the allocating batches, and the v7 batch limit; inspection, and the SQL-layout reads on values still in SQL order; and the `Guid.Timestamp` extension property. It returns a nonzero exit code on any mismatch:

```shell
dotnet publish csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj \
  -c Release -r linux-x64 -f net10.0 -p:PublishAot=true
./csharp/HyperUuid.AotSmokeTest/bin/Release/net10.0/linux-x64/publish/HyperUuid.AotSmokeTest
```

Last verified on `linux-x64` and, inside an Alpine container, `linux-musl-x64`: **zero `ILxxxx`/`AOTxxxx` trim or AOT diagnostics**, a 1.5 MB native binary (measured on net11.0, the only target then) with the core inside it and no shared library beside it, and `ALL NATIVE AOT CHECKS PASSED` with exit code 0. `TreatWarningsAsErrors` is on for the library project, so an analyzer warning is a build failure, not a line in a log nobody reads. CI re-proves it per platform on every PR (see [Native binary provenance](#native-binary-provenance)): it publishes and runs it for net10.0 and net11.0 on every desktop and musl leg, and `test-nuget-package` repeats both against the packed `.nupkg`, where a package with its `build/` folders removed must publish the shared library beside the executable, so the check cannot pass blind.

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

Works from a plain `<PackageReference Include="HyperUuid" />` on **.NET 10 and later**, with the `wasm-tools` workload installed — no `<NativeFileReference>`, no hand-written P/Invoke, no bridging in your own Rust build. Verified for real, in an actual headless Chromium session: `HyperUuid.WasmSmokeTest` (below) answers the version probe, generates v4s, matches the RFC 9562 v5 vector, round-trips v7 timestamps, reads no timestamp from a v6, fills a 1000-id v7 batch in one native call, round-trips the SQL-order transforms and checks their version 8 marker and refusals, and embeds a real-clock v7 timestamp within 2 seconds of the wall clock — not just "it builds."

**Target frameworks.** The package builds `net10.0` and `net11.0` from one source, and package validation holds their public APIs identical. Its targets file sits under `build/net10.0/` (and `buildTransitive/net10.0/`); NuGet hands that one file to .NET 11 projects too. A project older than .NET 10 is refused at restore with NU1202. Blazor WebAssembly works on both frameworks (see [Platform support](#platform-support)).

**Needs the `wasm-tools` workload.** The native core is linked into the app's own `dotnet.native.wasm`, and only the workload's toolchain relinks that runtime. Without the workload the SDK skips the link with nothing louder than its own warning and the publish succeeds, shipping a runtime with no `uuid_*` symbols in it; `NETSDK1147` does not fire for this. So the package's targets stop the build with error `HYPERUUID002` instead (`dotnet workload install wasm-tools`; a `net10.0` app built with the .NET 11 SDK takes `wasm-tools-net10`). An app that gates on `UuidGenerator.IsAvailable` and keeps a managed fallback can set `<HyperUuidRequireWasmWorkload>false</HyperUuidRequireWasmWorkload>`.

**How:** one compiled assembly covers every platform, including `browser-wasm` — no separate build. Every native entry point is declared three times, unconditionally, sharing the same underlying C symbol: once against `"hyperuuid"` (resolved via `dlopen` on every real native platform), once against `"*"` (a statically-linked WASM native has no separate `"hyperuuid"` module to open, since its functions are already part of the same `dotnet.native.wasm` the app itself runs in; `"*"` resolves against the current module instead), and once against `"__Internal"` for iOS and Mac Catalyst (see [Platform support](#platform-support)). `OperatingSystem.IsBrowser()` and `OperatingSystem.IsIOS()` pick the right one at each call site — real runtime checks the .NET linker specifically knows how to constant-fold per publish target (the same mechanism the BCL itself uses for platform-conditional code), so a trimmed/published build still only ships the branch that platform can actually reach. Only the Rust core's `wasm32-unknown-emscripten` static library (`cargo +<pin> wasm-staticlib`, a `--crate-type staticlib` build — the default `cdylib` produces an already-linked module `NativeFileReference` can't pull symbols from) is genuinely RID-specific, landing under `runtimes/browser-wasm/nativeassets/net11.0/` in the `.nupkg`; the managed assembly itself needs no RID-specific copy, confirmed by inspecting a real self-contained `dotnet publish -r <rid>` output too: no WASM files leak into a non-WASM deployment.

Building this exact source once also matters beyond simplicity: two independent, from-scratch `dotnet pack` runs produce a byte-identical managed assembly (verified with a real checksum comparison, not assumed), which two builds sharing one `obj/` directory cannot guarantee.

The one piece that *doesn't* auto-wire — NuGet's `runtimes/{rid}/nativeassets/{tfm}/` convention is real, and the WASM SDK really does auto-promote a resolved `NativeLibrary` item into `NativeFileReference`, but restore doesn't actually populate `NativeLibrary` from a plain `PackageReference`'s `nativeassets` folder the way it does `native/` for ordinary P/Invoke (confirmed empirically, not assumed) — is supplied by this package's own `build/net10.0/HyperUuid.targets`, auto-imported into every consuming project via NuGet's standard convention, which adds the `NativeFileReference`. That is the whole wiring: the SDK scans every assembly's P/Invokes and generates a pinvoke table that calls each native function by symbol, so the link resolves them against the archive (with it missing, `wasm-ld` reports an undefined symbol per entry point) and `LibraryImport("*")` reaches each one through that table. No `EmccExportedFunction` list is needed: `"*"` resolution goes through the pinvoke table, not an Emscripten export, which the smoke test below proves on .NET SDK 11.0.100-rc.2 by passing without one and failing at the link with the archive unlinked. Exporting nothing also keeps `uuid_variant` clear of Emscripten's libuuid JavaScript library, which defines a stub of that name and would take over an export of it.

The link happens whether or not the app ends up calling HyperUuid. The targets file is evaluated before the trimmer runs and cannot see what it removes, so a client bundle whose trimmer drops `HyperUuid.Managed` entirely still links the archive and its exports into `dotnet.native.wasm`. One Blazor consumer measured the cost of that, for this package and HyperCast together, at 66 KB raw and 2 KB compressed.

`HyperUuid.WasmSmokeTest` proves this chain in a real browser: a Blazor WebAssembly app (`net10.0` and `net11.0`) that imports the shipped `build/net10.0/HyperUuid.targets`, calls every native entry point — the twelve `uuid_*` functions and `hyperuuid_version` — through the public `UuidGenerator` surface, and renders `PASS` or `FAIL`. Every one: the link proves each symbol is in the archive, and only a call proves it works through the pinvoke table. `./check.sh` in that directory stages the wasm static library, publishes the app four ways (each framework, in Release and in an unstripped Debug build, where the SDK runs no `wasm-opt` after the link), loads each in headless Chromium and requires `PASS`; name legs (`net11-release net11-debug net10-release net10-debug`) to run fewer. CI runs the same script on every PR, in headless Chrome with the `wasm-tools` and `wasm-tools-net10` workloads, against the static library that run just built (`STATICLIB` names it, so the script skips building its own); it is not part of the solution, so a plain `dotnet build` never needs the workload or a browser.

**WebAssembly on .NET 10 and later, as linked.** The static library is built with `-Zbuild-std` (on stable Rust, with `RUSTC_BOOTSTRAP=1` for that one build), for plain wasm (MVP plus `sign-ext` and `mutable-globals`), instead of on Rust's prebuilt sysroot. So it holds no exception-handling instructions, and a browser never sees .NET 11's encoding mixed with the sysroot's legacy one (`module uses a mix of legacy and new exception handling instructions`), whether or not the SDK runs `wasm-opt` after the link. And it uses no feature but those two, Binaryen's defaults, which every `wasm-opt` run in a .NET 10 publish accepts with no flag: Emscripten's does not know `bulk-memory-opt` or `call-indirect-overlong` (dotnet/runtime#132858), and the SDK's own, after the link, rejects a saturating float-to-int conversion. The archive is checked for both before it is packed.

**`Unknown option '--enable-bulk-memory-opt'` on a native relink** is .NET 10's WebAssembly toolchain meeting a wasm object that declares `bulk-memory-opt` or `call-indirect-overlong`, as everything rustc 1.87 or later builds on its prebuilt sysroot does ([dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858), open as of 2026-10-28, reproduced on .NET 10.0.14). This package's archive declares neither; another native library linked into the same app can still bring them in. .NET 11's toolchain relinks either.

## Platform support

Native binaries ship inside the package for ten RIDs, plus static libraries for WebAssembly, iOS and Mac Catalyst. Every row below is .NET 11 and later; with Windows, macOS, iOS, Mac Catalyst and Android, that is every platform .NET MAUI targets. On .NET 10 the package supports the Linux, macOS and Windows rows, framework-dependent or Native AOT, and Blazor WebAssembly:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux (glibc) | `linux-x64`, `linux-arm64` | `libhyperuuid.so` |
| Linux (musl — Alpine) | `linux-musl-x64`, `linux-musl-arm64` | `libhyperuuid.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhyperuuid.dylib` |
| Windows | `win-x64`, `win-arm64` | `hyperuuid.dll` |
| Blazor WebAssembly (.NET 10+) | `browser-wasm` | `libhyperuuid.a` (static — see above) |
| iOS | `ios-arm64`, `iossimulator-arm64` | `libhyperuuid.a` (static — see below) |
| Mac Catalyst | `maccatalyst-arm64`, `maccatalyst-x64` | `libhyperuuid.a` (static — see below) |
| Android (API 21+) | `android-arm64`, `android-x64` | `libhyperuuid.so`; `libhyperuuid.a` for Native AOT (see below) |

**musl is its own build, not the glibc one relabeled.** A glibc `libhyperuuid.so` does not load under musl's dynamic loader, and NuGet's RID graph falls back from `linux-musl-x64` to `linux-x64` when nothing more specific is in the package, so without a musl build Alpine would select the glibc library, fail to load it, and throw on the first call. The musl libraries are built inside Alpine itself and depend on nothing but musl libc. Proven the way a consumer meets it, in an `mcr.microsoft.com/dotnet/sdk` Alpine container: this binding's whole test suite and the Native AOT smoke test (`-r linux-musl-x64`) against the musl library, then a throwaway console app consuming the packed `.nupkg` through a plain `PackageReference` with the glibc *and* musl libraries both inside it, which maps `runtimes/linux-musl-x64/native/libhyperuuid.so` and nothing else. The same app against a glibc-only package is the control: `UuidGenerator.IsAvailable` is `false` there, and nothing throws until something ignores it.

**.NET 10 on iOS, Mac Catalyst or Android: `HYPERUUID003`.** These need .NET 11 or later. A `net10.0-android`, `net10.0-ios` or `net10.0-maccatalyst` project stops at build with `HYPERUUID003`, which names the platform and the framework to target. With the platform's workload installed, it fires ahead of the Xcode and JDK checks. Without the workload, the SDK's own missing-workload error (NETSDK1147) can come first; that case is not probed. A project that reaches the package through a class library gets the same error.

On any platform outside that table the package still restores and compiles — the managed assembly is platform-neutral — and `UuidGenerator.IsAvailable` is how an app finds out at run time that no native library came with it.

**iOS and Mac Catalyst: the core is linked into the app.** A .NET 11 iOS, MAUI or Mac Catalyst app (`net11.0-ios`, `net11.0-maccatalyst`; .NET 10 is not supported on either) references the package like any other and writes nothing else. Those platforms have no `runtimes/{rid}/native/` to load a library from: the .NET SDK for them links native code into the app's own executable, and a P/Invoke reaches it under the library name `__Internal`. So the package carries the core as a static library for each of the four RIDs above, `build/net10.0/HyperUuid.targets` hands the one for the RID being built to the SDK as a [`NativeReference` with `Kind=Static`](https://learn.microsoft.com/dotnet/maui/migration/ios-binding-projects), and `UuidGenerator` declares every entry point a third time against `__Internal`, picked by `OperatingSystem.IsIOS()` (which is true on Mac Catalyst too). It is the SDK's own native link that takes the archive, so the same wiring serves an app compiled by Mono's AOT compiler, one that runs interpreted, and one published with [Native AOT](https://learn.microsoft.com/dotnet/core/deploying/native-aot/ios-like-platforms/). A universal Mac Catalyst app is built once per RID and merged, and each half links its own archive.

`HyperUuid.AppleSmokeTest` is the Native AOT smoke test (`SmokeTest.cs`) as an app, and CI's `test-apple-mobile` job builds it three ways on a Mac from that run's archives: as a Mac Catalyst app, run as a process; for the iOS simulator, installed and launched; and for an iOS device with signing off, where the check is that the app's executable defines the core's symbols, since no runner has a device to run it on.

**Android: the shared library, out of the APK.** A .NET for Android or MAUI app (`net11.0-android`, API 24 and later, .NET 11's floor) references the package and writes nothing else. On CoreCLR, .NET 11's Android runtime (Mono is not supported there; a `net10.0-android` build, which would run on Mono, stops with `HYPERUUID003`), the SDK takes `runtimes/android-arm64/native/libhyperuuid.so` and its x64 twin out of the package and stores each in the APK under `lib/arm64-v8a/` and `lib/x86_64/`, and the ordinary `hyperuuid` import opens it like any Linux library. The libraries are built with the NDK for API level 21, below any app that can reference them, and their segments are aligned to 16 KB: Android 15 devices may use 16 KB pages, a library aligned for 4 KB does not load on one, and Google Play requires the alignment of every new app. A Native AOT publish (`PublishAot`, `-r android-arm64`) takes the [AOT](#aot) wiring instead, linking `staticlibs/android-{rid}/libhyperuuid.a` into the app's own native library, and the shared one is left out of the APK. `android-arm64` covers effectively every Android device in use and `android-x64` the emulator; these are the two RIDs .NET for Android builds by default. The 32-bit `android-arm` and `android-x86` are not in the package, so an app that adds them gets `UuidGenerator.IsAvailable == false` on those ABIs.

`HyperUuid.AndroidSmokeTest` is the Native AOT smoke test's `SmokeTest.Run()` again, started from an Activity, and unlike the other smoke tests it takes HyperUuid as a package, from a local folder CI packs it into, because the package's layout is what Android needs proven. CI's `test-android` job builds it four ways from that run's libraries: CoreCLR and Native AOT, for each RID. The x64 pair runs in an x86_64 emulator whose image uses 16 KB pages, so a library aligned for 4 KB would fail to load there. Nothing hosted runs arm64 Android, so the arm64 pair is inspected instead: each CoreCLR APK must carry `libhyperuuid.so` for its ABI and pass `zipalign -P 16`, the check Google Play applies, and each Native AOT APK must not carry it at all, because the core is linked into the app.

**Not supported: tvOS and the iOS simulator on Intel Macs.** `iossimulator-x64` and tvOS have no archive, so an app for them fails at its native link on the undefined `uuid_*` symbols, at build time and not at run time.

## Native binary provenance

The `.nupkg` carries compiled native code, which is a real thing to ask questions about before adopting it inside a trust boundary. What is and isn't currently guaranteed:

**Where the binaries come from.** Nothing under `csharp/HyperUuid/runtimes/` is committed — it's `.gitignore`d. The native libraries are built from `rust/` by CI and staged into the package at pack time, so what ships is produced by the same workflow run that built and tested the source. (The Go, PHP, and Swift bindings are different: those *do* carry committed binaries, staged by `stage-native-binaries.yml`, whose commit message records the exact source SHA and CI run ID they came from — e.g. `chore: stage native binaries from ci.yml run 33438784689`.)

**Building it yourself.** The core is a normal Rust crate with no build-time codegen, so you never have to take the shipped binary at all:

```shell
cd rust && cargo cdylib
# -> target/release/libhyperuuid.so  (.dylib on macOS, hyperuuid.dll on Windows)
```

Drop the result into `csharp/HyperUuid/runtimes/<rid>/native/` and the package's own MSBuild globs will pick it up, or point `dlopen` at it however you prefer — the C ABI in `rust/src/ffi.rs` is the entire contract: the twelve `uuid_*` functions and `hyperuuid_version`, taking plain pointers into your own buffers. For local development nothing needs dropping anywhere: when no library has been staged under `runtimes/` for your machine's RID, the project copies `rust/target/release/` straight to the output, so `dotnet test` after a `cargo cdylib` just runs. The test project multi-targets net10.0 and net11.0, so both runtimes must be installed, and `--project` takes a path relative to the directory you run from: run it from `csharp/HyperUuid.Tests` (`dotnet test -c Release`), or pass an absolute path, since `--project csharp/HyperUuid.Tests/HyperUuid.Tests.csproj` from the repo root doubles the path.

**Reproducibility, stated honestly.** A rebuild is byte-identical only on the same toolchain *and* the same build image. Measured on 2026-10-25 for linux-x64, building commit `052b374` (the core CI staged into `php/src/native/` in `2043fb9`) with `cargo cdylib` in the `rust:latest` Docker image: a clean rebuild, and a build from a different checkout path, reproduce one SHA-256 exactly. CI's library from the same commit, though, is 25,968 bytes against that image's 25,944, with a different hash, although both were built by rustc 1.99.1 and linked by the same LLD 23.1.1, with an identically sized `.text`. The difference is the C toolchain's startup objects, GCC 15.2 on CI's Ubuntu 26.04 runner against 14.2 on the image's Debian, which are linked into every shared library and shift the code after them. So "rebuild it and compare hashes" verifies a binary only for someone who reproduces CI's runner image as well as its toolchain.

**Signed provenance.** Because rebuild-and-compare doesn't work across machines, the mechanism that does is a cryptographic attestation binding each artifact to the workflow run and commit that produced it. Two kinds cover the package.

The *release attestation* is one statement, signed by this repository's `release.yml` at the release tag, whose subjects are every file the release ships — the `.nupkg` as packed and as nuget.org serves it, every other registry package, and every native library committed for Go, Swift and PHP — and whose predicate is the release inventory (`inventory.json`: every file's size, SHA-256 and the commit it was built from). One command verifies any of them from the tag alone:

```shell
gh attestation verify HyperUuid.<version>.nupkg --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

*Build provenance* ([SLSA](https://github.com/actions/attest-build-provenance)) is the second layer, emitted where the bytes are made, which for this package is four places, because it is not the same bytes at every stage of its life:

| Attested artifact | Where | Signer |
| --- | --- | --- |
| Each native library, as built | `hyper-build-native.yml` | `SkunkWerkx/.github` |
| The browser-wasm archive, as built | `hyper-build-wasm.yml` | `SkunkWerkx/.github` |
| The `.nupkg` as packed, pre-push | `hyper-pack-nuget.yml` | `SkunkWerkx/.github` |
| The `.nupkg` as published | `release.yml`, after the push | `SkunkWerkx/HyperUuid` |

The reason for the last two rows: **nuget.org adds its repository signature as a `.signature.p7s` entry inside the `.nupkg` zip during validation**, which changes the file's SHA-256. So the package you download is not the package that was built, and one attestation cannot cover both. The pipeline takes both, and the release attestation is made for both: once in the publication gate over the packed bytes, once after the push over the served bytes, with the same inventory as predicate. The signer column is the flag that changes: an artifact signed by a reusable workflow in `SkunkWerkx/.github` needs `--signer-repo SkunkWerkx/.github` beside `--repo` (or `--owner SkunkWerkx` in place of both); one signed by this repository's `release.yml` verifies with `--repo` alone. Get it wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which reads like a bad signature but is only an identity mismatch.

```shell
# the published bytes, signed by release.yml
gh attestation verify HyperUuid.<version>.nupkg --repo SkunkWerkx/HyperUuid

# the as-built package, recovered by removing nuget.org's one added entry; signed by the Forge
zip -d HyperUuid.<version>.nupkg .signature.p7s
gh attestation verify HyperUuid.<version>.nupkg \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
```

The release run downloads the package nuget.org serves, runs `dotnet nuget verify` on it, and requires every entry but `.signature.p7s` to match the packed package before attesting it. That claim is checked on every release rather than asserted here, so if nuget.org ever changes how it finalizes packages, the run says so instead of the README quietly going stale.

Attestations are produced by every release-mode build — the release's own run and the weekly one. Pull requests build in `pr` mode, which ships nothing and so signs nothing. The release itself is two acts at one tag: the tag push packs, attests and preserves everything without uploading, and a dispatch of `release.yml` publishes from those preserved bytes, waiting up to 20 minutes for nuget.org to serve the package before attesting the served bytes; the Go tag and GitHub Release wait on it. A slower validation is recovered by dispatching again with `publication_run_id`, which pushes nothing twice (see [RELEASING.md](../.github/RELEASING.md)). The root README's [Provenance](../README.md#provenance) section has the whole picture across every binding.

**Not currently done: NuGet author signing.** The package carries nuget.org's repository signature but no author signature of our own, which would need an X.509 code-signing certificate registered to the account. It's complementary to the above rather than a substitute, and the difference is who does the checking: an author signature is verified automatically by every consumer's SDK at restore time, whereas an attestation is only checked by someone who deliberately runs `gh attestation verify`. Attestation ties an artifact to a commit and a build; an author signature ties it to an identity. If you want the automatic restore-time check, this is the gap.

**Per-platform AOT receipts.** The same CI run publishes `HyperUuid.AotSmokeTest` under Native AOT on five desktop RIDs (every one but `osx-x64`, which is cross-built and has no CI leg) and, in Alpine containers, on the two musl ones, fails the build on any `ILxxxx`/`AOTxxxx` trim diagnostic, executes the resulting binary, and requires exit 0. Each leg's log uploads as an `aot-report-{rid}` artifact.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperUuid) — no extra package source needed:

```shell
dotnet add package HyperUuid
```

Per-RID native libraries ship inside the package under `runtimes/`, so a consumer adds one reference and nothing else — no build step, no manual native staging. See [Platform support](#platform-support) for the list.

Targets net10.0 and net11.0. On .NET 10: Windows, Linux and macOS, framework-dependent or Native AOT, and Blazor WebAssembly. On .NET 11 and later: those, plus iOS, Mac Catalyst and Android — see [Platform support](#platform-support).

See [the repo root README](https://github.com/SkunkWerkx/HyperUuid/blob/master/README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
