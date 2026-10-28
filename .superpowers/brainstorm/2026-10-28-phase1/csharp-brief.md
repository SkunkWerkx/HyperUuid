# C# brief: target `net10.0` and `net11.0` (ROADMAP Phase 1)

Researched 2026-10-28, read-only. Probes ran in a scratch copy of `csharp/` on this Mac (SDK 11.0.100-rc.3.26522.148, runtimes 10.0.14 and 11.0.0-rc.3, Xcode 27.1). The only thing installed was the `wasm-tools-net10` workload, into `~/.dotnet`.

## 1. Goal

Today a .NET 10 project can't restore HyperUuid. It gets NU1202, because every framework-specific asset sits in a `net11.0` folder (`csharp/HyperUuid/HyperUuid.csproj:4`, `build/net11.0/HyperUuid.targets:29-37`). .NET 10 is the LTS until 2028-11-14. .NET 8 and 9 both reach EOL on 2026-11-10, and 11 is an STS release that isn't GA yet. So from 2026-11-10 the only supported LTS can't take the package. After this change a net10.0 project restores it and gets the same API, the same Native AOT static link, the same iOS/Mac Catalyst wiring and the same Android APK layout. Blazor WebAssembly is the one surface that doesn't come for free (section 3, F5–F7).

## 2. Current state

**Build.** There is one managed project, `HyperUuid.csproj`, with `<TargetFramework>net11.0</TargetFramework>` (:4). It sets `IsAotCompatible` (:46) and `TreatWarningsAsErrors` (:54), and has `InternalsVisibleTo HyperUuid.Tests` (:60). There is no `LangVersion`, so it uses the TFM default. The language features it uses are C# 14 at most: the `extension(Guid)` block (`GuidExtensions.cs:32`), and the doc there says so (:9). Every entry point is declared three times through `LibraryImport`, against `"hyperuuid"`, `"*"` and `"__Internal"` (`UuidGenerator.cs:43-187`). The APIs it uses are all .NET 8 or older (`new Guid(span, bigEndian)`, `TryWriteBytes(..., bigEndian, out _)`, `ArgumentOutOfRangeException.ThrowIf*`). `UuidGenerator.cs:41` documents "WebAssembly is .NET 11 and later only".

**Tests and benchmarks.** `HyperUuid.Tests.csproj:7` is net11.0 with MTP (`global.json` `test.runner`). It has `[ModuleInitializer]` (`TestStream.cs:13`, .NET 5+) and calls `Guid.CreateVersion7` and `Guid.Version` (`InspectionTests.cs:21,27`, `GuidExtensionsTests.cs:75`), which are .NET 9+. `HyperUuid.Benchmarks.csproj:5` is net11.0 and is in the slnx.

**Package layout.** `runtimes/**` (:91), `build/**` and `buildTransitive/**` (:127-128) and `staticlibs/**` (:133) are packed as-is. The targets file does four things:
- adds the wasm `NativeFileReference` by a literal path, `runtimes/browser-wasm/nativeassets/net11.0/libhyperuuid.a` (:38-40);
- runs the `HYPERUUID002` workload check (:52-57);
- appends `--translate-to-exnref` unconditionally for browser-wasm (:71-73);
- wires the iOS/Catalyst `NativeReference` (:108-115) and the Native AOT `DirectPInvoke`/`NativeLibrary` (:139-148).

`buildTransitive/net11.0/HyperUuid.targets:8` imports it.

**Smoke tests.**
- AOT: `HyperUuid.AotSmokeTest.csproj:5`, net11.0, imports the targets by hand (:22).
- Blazor: `WasmSmokeTest.csproj:4,12-13,33` (net11.0, `Components.WebAssembly` and `Components.Gateway` at `11.*-*`). `check.sh:22` stages into `nativeassets/net11.0`.
- Apple: `AppleSmokeTest.csproj:12` (`net11.0-ios;net11.0-maccatalyst`).
- Android: `AndroidSmokeTest.csproj:19` (`net11.0-android`, a PackageReference to a locally packed nupkg).

**CI (this repo).** `ci.yml` hands the Forge `csharp_aot_project` and `csharp_test_project` (:106,126) and `blazor_check` (:160). `test-apple-mobile` runs on `xcode-27` with `-f net11.0-*` and `ValidateXcodeVersion=false` (:240-253, :375-409). `test-android` packs (:494) and builds four APKs: CoreCLR and AOT, for x64 and arm64 (:512). Every `setup-dotnet` uses `global-json-file: global.json` only (:361-364, :475-477). `global.json` pins `11.0.100-rc.2.26504.105` with latestFeature and allowPrerelease. CodeQL's C# job is `build-mode: none` (`codeql.yml` matrix), so no SDK is involved there.

**Forge.**
- `hyper-build-native.yml:829-843`: setup-dotnet from global.json, then `dotnet test --project`.
- `hyper-build-native.yml:860-866`: `dotnet publish <aot> -r <rid> -p:PublishAot=true`, with no `-f`.
- `hyper-build-native.yml:1544-1554`: the musl image is `sdk:<global.json major.minor>-alpine-aot`.
- `musl-suite/suite-musl.sh:66-80`: `dotnet test` and the AOT publish run in that image.
- `hyper-build-wasm.yml:126-144`: stable rustc plus emsdk `latest`, then `cargo wasm-staticlib`.
- `hyper-build-wasm.yml:186-206`: `test-blazor`, which installs `wasm-tools` and runs `check.sh`.
- `hyper-pack-nuget.yml:175-177`: setup from global.json.
- `hyper-pack-nuget.yml:196-202`: stages the wasm archive into a hardcoded `nativeassets/net11.0`.
- `hyper-pack-nuget.yml:233-234`: plain `dotnet pack`.
- `publish-nuget/action.yml:60-62`: setup from global.json, then `dotnet nuget push`, which has no TFM sensitivity.
- `test_release_provenance.py:109`: a fixture that names `nativeassets/net11.0/libhyperuuid.a`.

HyperCast and HyperTabular share every one of these Forge files.

**History.** Up to 1.0.0 the package took net10.0 for native and gated Blazor below 11 with `HYPERUUID001`. 1.0.0 dropped both (CHANGELOG.md:186-194, :1214-1215).

## 3. Upstream facts (all checked 2026-10-28)

- **F1.** .NET 8 and 9 reach EOL on 2026-11-10. .NET 10 is the LTS until 2028-11-14 (latest 10.0.14 / SDK 10.0.403). .NET 11 is STS in "go-live" at rc.3; its EOL isn't listed. Sources: https://endoflife.date/api/dotnet.json and https://builds.dotnet.microsoft.com/dotnet/release-metadata/releases-index.json. The roadmap's claim that ".NET 8 ends Nov 10, 2026" is confirmed.
- **F2.** C# 14 is the default for net10.0 and C# 15 for net11.0. Language versions newer than the TFM's are unsupported. Extension members are C# 14. Source: https://learn.microsoft.com/dotnet/csharp/language-reference/language-versioning
- **F3.** When `setup-dotnet` gets both `dotnet-version` and `global-json-file`, it installs "versions from both inputs". A multi-line `dotnet-version` installs several. Source: https://raw.githubusercontent.com/actions/setup-dotnet/main/README.md
- **F4.** In .NET 10 the Android default runtime is Mono. CoreCLR on Android is experimental in 10 and the default in 11. Android NativeAOT is experimental (XA1040). Sources: https://learn.microsoft.com/dotnet/android/building-apps/build-properties#usemonoruntime and https://learn.microsoft.com/dotnet/android/messages/xa1040
- **F5.** Blazor's Emscripten differs by framework: net10 browser-wasm links with Emscripten **3.1.56** (Binaryen `wasm-opt` 117, clang-19), and net11 with Emscripten **6.0.2** (Binaryen 133). Measured from the installed workload manifests and packs: `~/.dotnet/sdk-manifests/11.0.100-rc.3/microsoft.net.workload.emscripten.net10` and `wasm-opt --version`. With SDK 11 a net10.0 project needs the `wasm-tools-net10` workload (from the manifest's `wasm-tools-net10` entry).
- **F6.** Binaryen 117 has no `--translate-to-exnref` pass; it has `--translate-to-new-eh`. Binaryen 133 has it (from `wasm-opt --help` on both). .NET 10's SDK post-link step fails with `Unknown option '--translate-to-exnref'` (probe P5).
- **F7.** rustc 1.87 and later write `bulk-memory-opt` and `call-indirect-overlong` into the archive's `target_features` section. Emscripten 3.1.56 turns each entry into an `--enable-…` flag that Binaryen 117 rejects. This is dotnet/runtime#132858, which is **open, untriaged and has no milestone** (https://github.com/dotnet/runtime/issues/132858). It still reproduces on 10.0.14 (probe P4). rustc flags can't remove those features: the LTO'd prebuilt sysroot carries them (probe P6, rustc 1.99.1 with LLVM 23.1.1).
- **F8.** Each .NET iOS/Catalyst workload requires one specific Xcode (https://learn.microsoft.com/dotnet/ios/troubleshooting/xcode-requirement). For .NET 10 that is Xcode 26.x; a Microsoft Q&A answer cites 26.6 and says Xcode 27 is not supported by the net10 iOS workload. **UNVERIFIED** against the dotnet/macios release notes. SDK 11's `ios` workload ships `Microsoft.iOS.Sdk.net10.0_26.0` (seen locally).
- **F9.** An SDK can build and pack an older TFM. SDK 11 alone produced `lib/net10.0` and ran net10 tests on the installed 10.0.14 runtime (P1–P2). Running net10 tests needs the 10 runtime on the machine. **UNVERIFIED** from a vendor doc; the evidence here is the probe only.

## 4. Approaches

**Probes behind them** (scratch copy, multi-targeted `net10.0;net11.0`):

| | Probe | Result |
|---|---|---|
| P1 | Library build | 0 warnings, 0 errors on both TFMs, with `TreatWarningsAsErrors` and `IsAotCompatible`. LibraryImport source generation is fine on net10. |
| P2 | Tests (`dotnet test --project`) | 86/86 passed on net10.0 and on net11.0. |
| P3 | AOT smoke multi-targeted | `dotnet publish` with no `-f` fails with **NETSDK1129**. With `-f net10.0 -r osx-arm64` it publishes with no IL/AOT warnings and prints `ALL NATIVE AOT CHECKS PASSED`, using ILCompiler 10.0.14. The executable has no libhyperuuid dependency and the dSYM has the `uuid_` symbols, so the core is linked in. `-p:TargetFramework=net10.0` on a single-TFM project fails with NETSDK1005. |
| P3b | Packed nupkg, net10 AOT consumer | With `build/net11.0` only, the consumer publishes `libhyperuuid.dylib` beside the exe: the static link is **silently lost**. With `build/net10.0` added, there is no dylib and the core is linked. A net9 consumer gets NU1202, so the floor still holds. |
| P4 | Blazor net10, shipped archive | Link fails: `Unknown option '--enable-bulk-memory-opt'`. |
| P4b | Blazor net10, `target_features` stripped | Fails: `Bulk memory operations require bulk memory`. The .NET 10 runtime objects don't declare it. |
| P5 | Blazor net10, `target_features` rewritten without the two unknown names | The relink passes, then the SDK's wasm-opt fails on `--translate-to-exnref`. |
| P5b | P5 plus the exnref item removed | Publish passes, and headless Chrome shows **`<title>PASS</title>`** with every check `ok`. |
| P5c | The same rewritten archive on net11, exnref kept | **PASS** in Chrome. One rewritten archive serves both frameworks. |
| P6 | Rebuild with `-C target-cpu=mvp` or `-C target-feature=-bulk-memory,...` | Both features are still in the section. |

### A. Multi-target; Blazor stays .NET 11+ (recommended)

**How it works.**
- `TargetFrameworks` becomes `net10.0;net11.0` in the library, the tests and the AOT smoke.
- `build/` and `buildTransitive/` move to `net10.0/` only: NuGet picks the nearest framework, so net11 gets them too. Alternatively keep both folders with one shared file.
- The browser-wasm blocks are gated on the consumer's TFM being 11 or later. A net10 browser-wasm consumer gets a new error, `HYPERUUID003`: "Blazor WebAssembly needs .NET 11+ (dotnet/runtime#132858)". It has the same `HyperUuidRequireWasmWorkload=false`-style opt-out to a managed fallback.
- In the Forge, the AOT step loops `-f` over a new `csharp_aot_frameworks` input (default: the project's single TFM), in both desktop and musl.
- Each setup-dotnet that runs tests also installs the .NET 10 runtime. That is a `dotnet-version` input, or `dotnet-install.sh --runtime dotnet --channel 10.0` inside the Alpine image.
- `test-android` adds a net10 Mono APK (x64, run in the emulator).
- Apple gets a net10 leg on an Xcode 26 runner, or is documented as unproven (Q3).

**Trade-offs.** It covers every native surface and stays honest about the browser. It adds no binary post-processing. It brings back a gate like 0.x had, but as an error rather than `HYPERUUID001`'s warning.

**Cost.** About 12 files here plus 3 Forge files, and roughly doubles the C# test time per leg.

### B. Full parity, Blazor included

**How it works.** A plus three more pieces:
- A Forge step in `hyper-build-wasm.yml` rewrites every member's `target_features` section to drop `bulk-memory-opt` and `call-indirect-overlong` (`llvm-objcopy --dump-section`/`--add-section` with a 20-line parser), and attests the rewritten archive.
- The exnref item is gated on TFM 11 or later.
- `test-blazor` runs `check.sh` twice, on net10 and net11.

**Trade-offs.** Proven locally in Chrome (P5b and P5c). But it changes the bytes of an attested, persisted artifact to work around an open upstream bug. It also tells Binaryen 117 less than rustc emitted; that's semantically sound, since bulk-memory and reference-types imply the two dropped names, but it's still a lie. It touches the shared Forge wasm build that HyperCast and HyperTabular ship. Firefox and Safari are unproven.

**Cost.** A plus about 2 Forge files, one CI leg, and a README section. It can be removed once .NET 10 servicing bumps Binaryen.

### C. Single `net10.0` TFM

**How it works.** `lib/net10.0` only, which net11 consumers also take. Targets gate as in A or B.

**Trade-offs.** It's the smallest package, but it doesn't meet the roadmap's wording. The net11 runtime path would be exercised only through roll-forward. The binding could never use a net11 API or C# 15 without re-adding the TFM.

**Recommendation: A.** It delivers what the LTS buyers need: the server, AOT and mobile surfaces. Blazor on .NET 10 is blocked by an open upstream bug that no supported rustc flag avoids. B is a proven fallback the owner can order now (Q1) or once #132858 moves.

## 5. Touch list (approach A)

This repo:
- `[own]` csharp/HyperUuid/HyperUuid.csproj: `TargetFrameworks`, and comments :86-90 and :131.
- `[own]` csharp/HyperUuid/build/net11.0/HyperUuid.targets: moves to `build/net10.0/`, gets the TFM gate and `HYPERUUID003`, and comments :29-37, :59-67.
- `[own]` csharp/HyperUuid/buildTransitive/net11.0/HyperUuid.targets: moves to `net10.0/`.
- `[own]` csharp/HyperUuid/UuidGenerator.cs:41: doc sentence.
- `[own]` csharp/HyperUuid.Tests/HyperUuid.Tests.csproj:7.
- `[own]` csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj:5,22: import path.
- `[own]` csharp/HyperUuid.WasmSmokeTest/HyperUuid.WasmSmokeTest.csproj:33: import path. It stays net11 under A.
- `[own]` csharp/HyperUuid.AppleSmokeTest/HyperUuid.AppleSmokeTest.csproj:12,46: add net10 TFMs if Q3=a.
- `[own]` csharp/HyperUuid.AndroidSmokeTest/HyperUuid.AndroidSmokeTest.csproj:12,19: add `net10.0-android` (Mono).
- `[own]` csharp/HyperUuid.Benchmarks/HyperUuid.Benchmarks.csproj:5: only if Q4=b.
- `[own]` csharp/README.md:70,130,147-165,177,190,261: floor, target frameworks, Blazor floor, Android on Mono for 10.
- `[shared]` .github/workflows/ci.yml:106,124-126 (comments, plus the new `csharp_aot_frameworks` input), 240-253 and 355-410 (Apple), 418-520 (Android legs and setup-dotnet), 221 (comment).
- `[shared]` README.md:165,226,398: floors sentence, wasm table, tree line.
- `[shared]` CHANGELOG.md: an Unreleased entry.
- `[shared]` ROADMAP.md: tick the Phase 1 C# row, and fix the table's "Floor .NET 11 only".
- `[shared]` CLAUDE.md: "Lists that move together" gets a .NET entry (TFMs, the Forge runtime list, README floor; .NET 10 EOL 2028-11-14).
- `[shared]` global.json: unchanged under A. Noted because the RTM switch (memory note) lands at the same time.
- `[shared]` .github/scripts/tests/test_workflow_contracts.py: only if the job shapes it asserts change.

Forge:
- `[shared]` .github/workflows/hyper-build-native.yml:829-843 (install the 10 runtime) and :860-911 (AOT `-f` loop and per-TFM exe path). New input. The fail-fast ordering is unaffected.
- `[shared]` .github/actions/musl-suite/action.yml and suite-musl.sh:66-80: runtime install in the container and the `-f` loop.
- `[shared]` .github/workflows/hyper-pack-nuget.yml:179-202: only if the archive folder is renamed (Q2).
- `[shared]` .github/scripts/test_release_provenance.py:109: only with Q2=b.
- `[shared]` (B only) hyper-build-wasm.yml:140-206: the feature rewrite and a second Blazor run.
- `[shared]` Forge README and docs, where they describe the C# inputs.

Siblings: see section 6.

## 6. Sibling cascade

Both siblings have the same C# binding shape: build and buildTransitive under `net11.0`, the exnref item (`HyperCast/csharp/HyperCast/build/net11.0/HyperCast.targets:76`, `HyperTabular/.../HyperTabular.targets:52`), and the same five smoke projects. But both pin net11.0 in `csharp/Directory.Build.props:9` / `:8` **deliberately**: `Verdict<T>` is a `[Union]`/`IUnion` type (`HyperCast/csharp/HyperCast/Verdict{T}.cs:33-34`) with `LangVersion preview`, which is C# 15 / .NET 11 (F2). They can't target net10 without dropping or polyfilling unions, and C# 15 on net10 is unsupported per F2. So CLAUDE.md's "floors match and move together" can't hold for C#. Either the rule gets a commented C# exception (HyperUuid net10, the siblings net11 because of unions), or HyperUuid stays net11 (Q5).

On the Forge side the AOT input defaults to one TFM and the runtime install is opt-in, so the siblings change nothing unless they opt in. Under B they would also inherit the rewritten wasm archive (proven for HyperUuid only).

## 7. Verification

**Local.** Every command copies the Forge's flags. Each has its negative control.

1. `dotnet build csharp/HyperUuid/HyperUuid.csproj -c Release`: 0 warnings for both TFMs. Negative control: add a net11-only API call (e.g. any API introduced in 11) and net10.0 must fail with CS0117/CS1061.
2. `dotnet test --project csharp/HyperUuid.Tests/HyperUuid.Tests.csproj -c Release`: two result lines, `(net10.0|arm64)` and `(net11.0|arm64)`, 86 each. Negative control: run with `DOTNET_ROOT` pointing at an install with no 10.x runtime and the net10 run must fail to start.
3. `dotnet publish <AotSmoke> -c Release -r osx-arm64 -f net10.0 -p:PublishAot=true`, then run it. `otool -L` must show no libhyperuuid, and the output must have no IL/AOT warnings. Negative control: without `-f` it must give NETSDK1129, and with `-p:HyperUuidStaticLink=false` the dylib must appear.
4. Pack, then a net10 PublishAot consumer from a local feed (probe P3b). The core must be linked. Negative control: delete `build/net10.0` from the nupkg and the dylib must reappear. A net9.0 consumer must get NU1202.
5. Blazor: `check.sh` on net11 must PASS. A net10.0 `-r browser-wasm` build of a consumer must stop with `HYPERUUID003`. Negative control: remove the gate and the net10 publish must fail with `--enable-bulk-memory-opt` (P4).
6. `dotnet format whitespace csharp/HyperUuid.slnx --verify-no-changes` (Forge `hyper-check-core.yml:713`). Negative control: one space indent must fail it.
7. `python3 -m unittest discover .github/scripts/tests` here, and the Forge's `.github/scripts/test_*.py`.

**Only CI can prove:**
- the Windows and Linux AOT legs on net10;
- the musl image with the 10 runtime installed;
- the net10 Android Mono APK in the emulator (no emulator was probed here);
- the Apple net10 builds, which need an Xcode 26 runner image;
- that `setup-dotnet` with both inputs really puts the 10 runtime beside the global.json SDK.

**At release:** the first nuget.org package with `lib/net10.0` and `lib/net11.0` groups. Check that the publish-nuget `nuget-finalized` verification and the inventory accept two `lib/` groups, and that the nuget.org page lists both frameworks.

## 8. Risks and unknowns (ranked)

1. **The static link is lost silently if the targets folder is wrong.** A net10 consumer still builds and runs off the dylib, so nothing fails (P3b). Check 4 and its negative control are the guard. Keep them in CI as an nupkg-layout assertion like test-android's (:494-498).
2. **Blazor on net10 is blocked upstream (F7).** Under A, net10 browser consumers get an error. Under B, the byte rewrite has to be maintained until #132858 is fixed, and only Chrome was proven.
3. **The Apple net10 leg needs an Xcode 26 runner (F8, UNVERIFIED).** `test-apple-mobile` is pinned to `xcode-27` (ci.yml:240-253), and a separate macOS job doubles its cost.
4. **Android on net10 runs Mono, not CoreCLR (F4).** CI proves only CoreCLR today. The loader path is the same (`lib/<abi>/libhyperuuid.so`) but unproven on Mono. Native AOT on Android is experimental on both frameworks.
5. **The Forge's AOT step and musl script assume one TFM.** NETSDK1129 would break every leg the moment the AOT project multi-targets, so the Forge has to merge before this repo does (CLAUDE.md: no Forge merges while a release is in flight).
6. **The floor rule breaks with the siblings (section 6).**
7. **Housekeeping.** `global.json` pins rc.2, but rc.3 is current and the only one installed here (latestFeature covers it). The RTM switch (memory note) should land separately to keep the diff readable.

## 9. Owner questions

1. **Blazor WebAssembly on .NET 10?**
   - (a) Not now: a `HYPERUUID003` error, revisited when dotnet/runtime#132858 moves. *(recommended)*
   - (b) Yes, via the Forge rewrite of `target_features` (approach B).
   - (c) Yes, but as a warning plus no link, like 0.x's `HYPERUUID001`.
2. **Where does the browser-wasm archive live?**
   - (a) Keep `nativeassets/net11.0/`, the only framework that links it under A, so there is no Forge change. *(recommended)*
   - (b) Rename it to `net10.0/`. Choose this if Q1=b.
3. **Apple net10 proof?**
   - (a) A net10 Catalyst and simulator build on an Xcode 26 runner. *(recommended if a runner image exists)*
   - (b) Document iOS and Catalyst on net10 as built from the same targets but untested in CI.
   - (c) Exclude net10 from the Apple claims.
4. **Benchmarks?**
   - (a) Stay net11 only, and the README says the tables are .NET 11. *(recommended: "claims are measured", and a net10 table needs a fresh harness run)*
   - (b) Add a net10 column on the next measuring pass.
5. **Floors with the siblings?**
   - (a) HyperUuid goes to net10 and CLAUDE.md records a commented C# exception: the siblings stay net11 for `[Union]`. *(recommended)*
   - (b) Hold HyperUuid at net11 until the siblings can follow.
   - (c) Polyfill unions in the siblings on net10 (unsupported per F2).
