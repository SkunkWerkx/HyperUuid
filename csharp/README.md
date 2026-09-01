# HyperUuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperUuid.svg)](https://www.nuget.org/packages/HyperUuid)

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

// Version-agnostic: null instead of assuming id4 is v6/v7:
DateTimeOffset? maybeCreated = UuidGenerator.GetTimestamp(id4);

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
Guid sqlOrdered = UuidGenerator.V7ToSqlOrder(id4);

// Bulk generation shares one timestamp capture, one random-bytes fetch, and (v7) one
// contiguous counter reservation across the whole batch:
Guid[] batch = UuidGenerator.NewV7Batch(1000);
```

Returns plain `System.Guid` — this binding does no byte-order conversion of its own beyond the `bigEndian: true` `Guid` constructor overload (.NET 8+), since that's already the correct, direct RFC 9562 mapping. `UuidGenerator.Namespaces.Dns`/`Url`/`Oid`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.Nil`/`Max` are the §5.9/§5.10 special values (`Nil` is literally `Guid.Empty`). `NewV6`/`NewV7` also accept a `DateTimeOffset` directly (`NewV6(DateTimeOffset)`), not just a raw millisecond count; `GetTimestamp` is the version-agnostic counterpart to `V6Timestamp`/`V7Timestamp` — it checks the version nibble itself and returns `null` for anything but a genuine v6/v7 `Guid`, instead of assuming the caller already knows.

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
4. **Batch generation, in three shapes.** One native call, one random-bytes fetch, one counter reservation for the whole batch, instead of paying per-item overhead a thousand times. `Guid.NewGuid()`/`CreateVersion7()` have no bulk API; you'd write that loop yourself. Same machine and methodology as the table above (`--filter *Batch*`, 1000 UUIDs per operation):

   | Method | Mean | vs. individual | Allocated |
   | --- | ---: | ---: | ---: |
   | `NewV7()` x1000 individually | 87.10 µs | 1.00x | 0 B |
   | `NewV7Batch(1000)` → new `Guid[]` | 21.95 µs | 3.97x faster | 16,024 B |
   | `FillV7(Span<Guid>)` into an existing array | 20.74 µs | **4.20x faster** | **0 B** |
   | `FillV7(Span<byte>)` into an existing buffer | 18.20 µs | **4.79x faster** | **0 B** |

   The three rows amortize different things, which is why all three exist. `NewV7Batch` amortizes the FFI call but still allocates the result array. `FillV7(Span<Guid>)` drops the allocation entirely but still pays a `new Guid(chunk, bigEndian: true)` conversion per element, because `Guid`'s in-memory layout is mixed-endian and isn't RFC byte order. `FillV7(Span<byte>)` drops that conversion too — the native core already writes RFC-ordered bytes contiguously into your buffer — and that 2.5 µs gap between the last two rows *is* the per-element conversion cost, measured. `FillV6`/`NewV6Batch` behave the same way (24.96 / 24.41 / 21.61 µs respectively).
5. **Cross-language consistency.** The exact same Rust core also mints v5 namespace UUIDs for Ruby, Python, Go, and every other binding in this repo — verified in CI to match Python's own `uuid.uuid5` byte-for-byte. If your system isn't C#-only, that's not something the BCL can offer at all.
6. **SQL Server byte ordering, for free.** `UuidGenerator.V7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`V6ToSqlOrder` does the same for version 6), the same permutation this project's own [SequentialGuid](https://github.com/buvinghausen/SequentialGuid)/[Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) already use. Verified directly against the real `SqlGuid` comparator in this package's own test suite, not a hand-rolled stand-in — and it's the same native function every other binding in this repo calls, not a C#-only reimplementation. Neither `Guid.NewGuid()` nor `Guid.CreateVersion7()` has any such concept.

7. **Non-throwing and zero-conversion call shapes, for callers that need them.** Every fallible operation has a `Try` twin — `TryNewV4`/`TryNewV6`/`TryNewV7`/`TryFillV6`/`TryFillV7` — that reports failure as `false` rather than an exception. No exception ever crosses the P/Invoke boundary in either shape (the native layer signals with an `int` return code; see `rust/src/ffi.rs`), but the `Try` form lets a `Result`-style gateway branch on failure without wrapping every call in a `try`/`catch`. Separately, the SQL-order transforms and the batch fills both have raw-`Span<byte>` overloads that never construct a `Guid` at all — RFC-ordered bytes in, transformed bytes out, in place. That matters for two reasons: it's the form a byte-level correctness oracle can be pointed at directly (no need to model `Guid`'s mixed-endian field layout to compare results), and it's measurably the fastest batch path, since the native core already writes RFC bytes contiguously into your buffer and the `Guid` overload has to convert every element on the way out.

The honest trade-off: this is a native dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID) instead of a BCL type that's always just there. If you only need plain v4 randomness in a C#-only codebase, `Guid.NewGuid()` is simpler and that's a completely reasonable choice.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime reflection anywhere in this assembly, and the project opts into (and fails the build on) the trim/Native-AOT analyzers via `IsAotCompatible`.

That claim is reproducible rather than asserted. `HyperUuid.AotSmokeTest/` is a real AOT-published console app that exercises the full public surface — v4/v5/v7 generation against the RFC 9562 Appendix A.4 vector, the non-throwing `Try*` path (including that an out-of-range timestamp is *reported*, not thrown), the raw-byte SQL-order transform against its `Guid` counterpart, and a destination-buffer batch fill — and returns a nonzero exit code on any mismatch:

```shell
dotnet publish csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj \
  -c Release -r linux-arm64 /p:PublishAot=true
./csharp/HyperUuid.AotSmokeTest/bin/Release/net10.0/linux-arm64/publish/HyperUuid.AotSmokeTest
```

Last verified on `linux-arm64`: **zero `ILxxxx`/`AOTxxxx` trim or AOT diagnostics**, a 1.2 MB self-contained native binary, and `ALL NATIVE AOT CHECKS PASSED` with exit code 0. `TreatWarningsAsErrors` is on for the library project, so an analyzer warning is a build failure, not a line in a log nobody reads.

## WebAssembly (Blazor)

Works from a plain `<PackageReference Include="HyperUuid" />` — no `<NativeFileReference>`, no hand-written P/Invoke, no bridging in your own Rust build. Verified for real: a fresh Blazor WebAssembly app with nothing but that one package reference, run in an actual headless Chromium session, correctly generates v4s, matches the RFC 9562 v5 vector, and embeds a real-clock v7 timestamp that matches the actual wall clock exactly (zero drift) — not just "it builds."

**This is the real reason this package's floor is net10.0, not net8.0.** Not a preference — net8.0's WASM P/Invoke-table generator (`ManagedToNativeGenerator`, part of `dotnet/runtime`) scans the wrong copy of a multi-RID-asset package: the default `lib/{tfm}/` assembly, never the `runtimes/browser-wasm/lib/{tfm}/` one that actually ships and runs. Confirmed by inspecting the generated `pinvoke-table.h` directly: on net8.0 it registered this package's functions under module `"hyperuuid"`, never `"*"`, even though the deployed assembly was verified byte-for-byte identical to the correct browser build — so the app crashes with `DllNotFoundException("*")` at runtime, unconditionally, with no workaround found on the package side (redeclaring the P/Invoke directly in the consumer's own entry assembly doesn't avoid it either — same failure). Re-ran the identical repro against .NET 10 and it's fixed there: the generator correctly scans the browser assembly and the app runs end to end in a real headless-Chromium session.

**How:** one compiled assembly covers every platform, including `browser-wasm` — no separate build. Every native entry point is declared twice, unconditionally, sharing the same underlying C symbol: once against `"hyperuuid"` (resolved via `dlopen` on every real native platform), once against `"*"` (a statically-linked WASM native has no separate `"hyperuuid"` module to open, since its functions are already part of the same `dotnet.native.wasm` the app itself runs in; `"*"` resolves against the current module instead). `OperatingSystem.IsBrowser()` picks the right one at each call site — a real runtime check the .NET linker specifically knows how to constant-fold per publish target (the same mechanism the BCL itself uses for platform-conditional code), so a trimmed/published build still only ships the branch that platform can actually reach. Only the Rust core's `wasm32-unknown-emscripten` static library (`cargo rustc --crate-type staticlib` — the default `cdylib` produces an already-linked module `NativeFileReference` can't pull symbols from) is genuinely RID-specific, landing under `runtimes/browser-wasm/nativeassets/net10.0/` in the `.nupkg`; the managed assembly itself needs no RID-specific copy anymore, confirmed by inspecting a real self-contained `dotnet publish -r <rid>` output too: no WASM files leak into a non-WASM deployment.

Building this exact source once instead of twice also turned out to matter beyond simplicity: two independent, from-scratch `dotnet pack` runs now produce a byte-identical managed assembly (verified with a real checksum comparison, not assumed) — the earlier two-builds-sharing-one-`obj/`-directory design never gave that guarantee, and is the leading suspect for this package's NuGet health-check failures on every release before this one.

The one piece that *doesn't* auto-wire — NuGet's `runtimes/{rid}/nativeassets/{tfm}/` convention is real, and the WASM SDK really does auto-promote a resolved `NativeLibrary` item into `NativeFileReference`, but restore doesn't actually populate `NativeLibrary` from a plain `PackageReference`'s `nativeassets` folder the way it does `native/` for ordinary P/Invoke (confirmed empirically, not assumed) — is supplied by this package's own `build/net10.0/HyperUuid.targets`, auto-imported into every consuming project via NuGet's standard convention, which adds both the `NativeFileReference` and (also confirmed necessary the hard way — linking the code in alone does *not* make it resolvable via `"*"` at runtime) an explicit `EmccExportedFunction` entry per native function this package P/Invokes. That's the actual mechanism making this "just works" for real, not a hopeful description of how NuGet packaging is supposed to behave.

**A real, currently-open upstream blocker you'll still hit, regardless of TFM:** the packaging above eliminates bringing your own Rust build and P/Invoke shim — it does *not* eliminate a genuine bug in the *consumer's own* .NET SDK. The moment any Blazor build does a native relink — which this package's own auto-injected `NativeFileReference` triggers just as surely as a hand-written one would — `Unknown option '--enable-bulk-memory-opt'` fires: a real Emscripten/rustc version-skew bug in the `wasm-tools` workload (rustc >= 1.87's WASM target-feature metadata vs. Binaryen `wasm-opt` builds older than Emscripten 3.1.74, which every `wasm-tools` SDK band bundles as of this writing — confirmed still true on .NET 10 and .NET 11 preview both, not just 8), confirmed identically on `linux-arm64` and `linux-x64`, and independently in an unrelated project ([PyO3/maturin#2549](https://github.com/PyO3/maturin/issues/2549)). Filed upstream with a minimal repro and a verified workaround: [dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858). Until that lands, every consumer building a Blazor app that references this package needs the same one-line fix — swap the SDK's bundled `wasm-opt` (`~/.dotnet/packs/Microsoft.NET.Runtime.Emscripten.<version>.Sdk.<rid>/<pack-version>/tools/bin/wasm-opt`) for a newer one (e.g. from a standalone `emsdk install latest`) — nothing this package can fix from the inside.

## Platform support

Native binaries ship inside the package for six RIDs, plus a WebAssembly static library:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux | `linux-x64`, `linux-arm64` | `libhyperuuid.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhyperuuid.dylib` |
| Windows | `win-x64`, `win-arm64` | `hyperuuid.dll` |
| Blazor WebAssembly | `browser-wasm` | `libhyperuuid.a` (static — see above) |

**Known gap: iOS, Mac Catalyst, and Android are not supported.** A .NET MAUI app can reference
this package for its Windows and macOS heads, which the RIDs above cover, but not for its mobile
heads — the package neither ships those native assets nor declares those target frameworks. Stated
here as an explicit gap rather than left for a consumer to discover at link time.

Android is the smaller half: it needs an NDK cross-build added to the release matrix, but
resolution is then ordinary `dlopen` of a `.so` out of `runtimes/{rid}/native/`, exactly like the
Linux RIDs already do.

Apple mobile is a packaging change, not a matrix row.
[Native AOT for iOS-like platforms](https://learn.microsoft.com/dotnet/core/deploying/native-aot/ios-like-platforms/)
(.NET 9+) does cover `ios-arm64`, `iossimulator-arm64`/`-x64` and `maccatalyst-arm64`/`-x64` — but a
native dependency on those targets is linked statically into the app, via
[`NativeReference` with `Kind=Static`](https://learn.microsoft.com/dotnet/maui/migration/ios-binding-projects)
or Native AOT's
[`NativeLibrary`/`DirectPInvoke`](https://learn.microsoft.com/dotnet/core/deploying/native-aot/interop),
rather than resolved at runtime from `runtimes/{rid}/native/`. That is structurally the same problem
the WebAssembly support above already solves: build the Rust core as a `.a` rather than a shared
library, and let this package's own auto-imported `build/net10.0/HyperUuid.targets` inject the
reference so a consumer still writes nothing but a `PackageReference`. The packaging mechanism is
therefore already proven in this repo; what is *not* yet established is how the managed
`LibraryImport` declaration should resolve against a statically-linked core on iOS, which is the
first thing to settle whenever this is picked up.

## Native binary provenance

The `.nupkg` carries compiled native code, which is a real thing to ask questions about before adopting it inside a trust boundary. What is and isn't currently guaranteed:

**Where the binaries come from.** Nothing under `csharp/HyperUuid/runtimes/` is committed — it's `.gitignore`d. The native libraries are built from `rust/` by CI and staged into the package at pack time, so what ships is produced by the same workflow run that built and tested the source. (The Go, PHP, and Swift bindings are different: those *do* carry committed binaries, staged by `stage-native-binaries.yml`, whose commit message records the exact source SHA and CI run ID they came from — e.g. `chore: stage native binaries from ci.yml run 33438784689`.)

**Building it yourself.** The core is a normal Rust crate with no build-time codegen, so you never have to take the shipped binary at all:

```shell
cd rust && cargo build --release
# -> target/release/libhyperuuid.so  (.dylib on macOS, hyperuuid.dll on Windows)
```

Drop the result into `csharp/HyperUuid/runtimes/<rid>/native/` and the package's own MSBuild globs will pick it up, or point `dlopen` at it however you prefer — the C ABI in `rust/src/ffi.rs` is the entire contract, and it's twelve exported functions that take plain pointers.

**Reproducibility, stated honestly.** The build is deterministic *locally*: `cargo clean -p hyperuuid` followed by `cargo build --release` reproduces a byte-identical `libhyperuuid.so` (verified by SHA-256). It is **not** currently bit-reproducible *across machines* — a local `rustc 1.98.0` build on WSL and the CI-built `linux-arm64` artifact differ in both hash and size (458,712 vs 458,176 bytes), as you'd expect from differing toolchain versions and embedded build paths. So "rebuild it and compare hashes" is not a verification path a consumer can currently rely on.

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
gh attestation verify HyperUuid.0.1.1.nupkg --repo SkunkWerkx/HyperUuid

# or recover the as-built artifact and verify that instead.
# Signed by hyper-pack-nuget.yml over in the forge repo, so this half needs --signer-repo.
zip -d HyperUuid.0.1.1.nupkg .signature.p7s
gh attestation verify HyperUuid.0.1.1.nupkg \
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

Attestations are produced on pushes, releases, and same-repo pull requests. Only pull requests *from forks* go unattested, because a fork's token can't sign — so a fork PR legitimately has none, while a branch PR in this repo does. The post-publish half is non-blocking: the push is irreversible, so a slow nuget.org validation is never allowed to turn a successful publish into a failed release.

**Not currently done: NuGet author signing.** The package carries nuget.org's repository signature but no author signature of our own, which would need an X.509 code-signing certificate registered to the account. It's complementary to the above rather than a substitute, and the difference is who does the checking: an author signature is verified automatically by every consumer's SDK at restore time, whereas an attestation is only checked by someone who deliberately runs `gh attestation verify`. Attestation ties an artifact to a commit and a build; an author signature ties it to an identity. If you want the automatic restore-time check, this is the gap.

**Per-platform AOT receipts.** The same CI run publishes `HyperUuid.AotSmokeTest` under Native AOT on all six RIDs, fails the build on any `ILxxxx`/`AOTxxxx` trim diagnostic, executes the resulting binary, and requires exit 0. Each leg's log uploads as an `aot-report-{rid}` artifact.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperUuid) — no extra package source needed:

```shell
dotnet add package HyperUuid
```

Targets net10.0 — see the WebAssembly section above for why that floor exists.

See [the repo root README](https://github.com/SkunkWerkx/HyperUuid/blob/master/README.md) for the full RFC 9562 coverage table and the state of every other language binding.
