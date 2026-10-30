# C#: multi-target `net10.0;net11.0`

Written 2026-10-28 from ROADMAP Phase 1 (C#), the C# brief and the owner's decisions of the
same day (`docs/superpowers/brainstorm/2026-10-28-phase1/`). **Draft for owner review.** Ships in
1.1.0; `docs/superpowers/plans/` will hold the implementation plan.

## Goal

.NET 8 and 9 reach end of life on 2026-11-10; .NET 10 is the LTS until 2028-11-14
(endoflife.date, checked 2026-10-28). Today a net10.0 project cannot restore HyperUuid (NU1202:
every framework-specific asset is in a `net11.0` folder). After this change the shops on the only
supported LTS — servers, services, CLIs and desktop apps on Windows, Linux (glibc and musl) and
macOS, framework-dependent or Native AOT — get the same API, the same static link and the same CI
receipts as .NET 11, and the platforms .NET 10 can't serve fail at build with one clear error,
never at run time.

## Decisions (owner, 2026-10-28; decisions.md "C#" and "Release vehicle")

- **Approach A:** library, tests and AOT smoke test multi-target `net10.0;net11.0`.
- **net10 scope:** desktop and server only (Windows, Linux, macOS; framework-dependent and Native
  AOT). net10 WebAssembly, android, ios and maccatalyst are out: a WASM or MAUI shop runs current
  .NET, and it sidesteps .NET 10's Mono Android runtime.
- **One diagnostic, `HYPERUUID003`,** for a net10 browser, android, ios or maccatalyst consumer:
  it names the platform, points at net11, and cites dotnet/runtime#132858 for browser (checked
  2026-10-28: open, `untriaged`, no milestone).
- **Browser-wasm archive stays under `nativeassets/net11.0/`;** no Forge wasm change.
- **iOS and Mac Catalyst claims stay net11-only,** said explicitly in the README.
- **Benchmarks:** the harness stays net11 only; the tables are re-measured on .NET 11 on the Apple M6 in the system-wide receipts pass (Open item 1).
- **Must-haves:** a package-layout check that `build/net10.0/` exists, with a negative control
  for the silent fallback to the shared library; the Forge AOT publish gains `-f` and merges first.
- **Siblings frozen** until v1.1: no sibling work, no C# exception in CLAUDE.md.
- **Release vehicle:** everything in 1.1.0, "the ubiquity release".

## Design

### Package layout per TFM

| Path in the .nupkg | Selected by |
| --- | --- |
| `lib/net10.0/HyperUuid.Managed.dll` (+ `.xml`) | net10.0 and net10.0-* consumers |
| `lib/net11.0/HyperUuid.Managed.dll` (+ `.xml`) | net11.0 and later, every platform |
| `build/net10.0/HyperUuid.targets` | both: NuGet takes the nearest folder, so net11 gets it too |
| `buildTransitive/net10.0/HyperUuid.targets` | both; imports `../../build/net10.0/HyperUuid.targets` |
| `runtimes/{rid}/native/*`, `staticlibs/{rid}/*` | unchanged, framework-neutral |
| `runtimes/browser-wasm/nativeassets/net11.0/libhyperuuid.a` | unchanged; linked by literal path, net11 only |

No `build/net11.0/`: the wiring exists once. net9.0 and older still get NU1202. One source is
compiled twice, with no `#if NET11_0_OR_GREATER` and no `LangVersion`, so the code stays within
C# 14 (net10's default) and the net10 build rejects anything newer. `HyperUuid.csproj` sets
`EnablePackageValidation` and `EnableStrictModeForCompatibleFrameworksInPackage`, so pack fails
unless the two assemblies' public surfaces are *equal* (Microsoft Learn, package validation).

### `build/net10.0/HyperUuid.targets`

`git mv` from `build/net11.0/`; the wiring is unchanged except:

- **Gate.** `_HyperUuidNet10Unsupported` holds the platform name when `TargetFrameworkIdentifier`
  is `.NETCoreApp`, `TargetFrameworkVersion` is below 11.0, and: `RuntimeIdentifier ==
  browser-wasm` or `TargetPlatformIdentifier == browser` → "Blazor WebAssembly"; `android` →
  "Android"; `ios` (device and simulator) → "iOS"; `maccatalyst` → "Mac Catalyst".
- **Hook.** Target `_HyperUuidErrorUnsupportedNet10Platform`,
  `BeforeTargets="_CheckForInvalidConfigurationAndPlatform"`. Probed on this Mac on 2026-10-28
  with a stand-in targets file: hooked at `BeforeBuild` (as HYPERUUID002 is), the platform SDKs
  fire first ("requires Xcode 26.0" for net10 iOS and Mac Catalyst, XA0030 for net10 Android);
  hooked here, HYPERUUID003 came first on all four, and net10.0 desktop and the net11 twins were
  untouched.
- **Text.** `HyperUuid supports .NET 10 on Windows, Linux and macOS, framework-dependent or
  Native AOT. {platform} needs .NET 11 or later: target {tfm}.` where {tfm} is `net11.0`,
  `net11.0-android`, `net11.0-ios` or `net11.0-maccatalyst`. Browser appends: ` .NET 10's
  WebAssembly toolchain cannot link HyperUuid's native core:
  https://github.com/dotnet/runtime/issues/132858.`
- **Nothing else fires** while the gate applies: HYPERUUID002, the wasm `NativeFileReference`,
  `--translate-to-exnref`, the Apple `NativeReference` and the AOT `DirectPInvoke`/`NativeLibrary`
  blocks are conditioned off. On net11 every condition evaluates as today. No opt-out (Open item 2).
- **Header comment** (old lines 29-37, "One floor") rewritten: two frameworks, one file under
  `net10.0/` that NuGet selects for both, the gate and why, and why the archive stays `net11.0`.

### Projects

- `HyperUuid.Tests`: `TargetFrameworks` `net10.0;net11.0`; `dotnet test --project` runs both.
- `HyperUuid.AotSmokeTest`: `TargetFrameworks` `net10.0;net11.0`. Project mode (default) keeps
  the ProjectReference and hand import, now `../HyperUuid/build/net10.0/HyperUuid.targets`.
  **Package mode**: `-p:HyperUuidVersion=<v> -p:RestoreAdditionalProjectSources=<feed>` swaps
  both for `PackageReference HyperUuid` (the Android smoke test's convention), so the same smoke
  runs against the packed `.nupkg`; with no ProjectReference, `-p:TargetFrameworks=` is safe.
- `HyperUuid.WasmSmokeTest`, `HyperUuid.AppleSmokeTest`: stay `net11.0*`; import path only.
  `HyperUuid.AndroidSmokeTest` stays `net11.0-android` and, as a package consumer, now also
  proves net11 resolves `build/net10.0/`. `HyperUuid.Benchmarks` stays `net11.0`.
- **New `HyperUuid.PlatformGateTest`:** a source-less class library, `TargetFramework` =
  `$(GateFramework)`, no reference, importing `../HyperUuid/build/net10.0/HyperUuid.targets` by
  hand. Only for the HYPERUUID003 probes; not in `HyperUuid.slnx` (like the wasm/Apple/Android ones).

### CI legs: which jobs run each TFM

| Job | net10.0 | net11.0 |
| --- | --- | --- |
| Forge `build-native` desktop: win-x64, win-arm64, linux-x64, linux-arm64, osx-arm64 | tests; AOT publish + run | tests; AOT publish + run |
| Forge musl: linux-musl-x64, linux-musl-arm64 | tests; AOT publish + run | tests; AOT publish + run |
| `test-nuget-package` (new) | package AOT consumer, layout + NC, NU1202, browser HYPERUUID003 | package AOT consumer, browser gate control |
| `test-android` | `net10.0-android` HYPERUUID003 probe | the four APKs, unchanged |
| `test-apple-mobile` | `net10.0-ios`, `net10.0-maccatalyst` HYPERUUID003 probes | the three builds, unchanged |
| Forge `build-wasm` `test-blazor` | — (gated) | `check.sh`, unchanged |

osx-x64 stays cross-built with no leg, on both frameworks, as today.

**`test-nuget-package`** (ubuntu-26.04, `needs: [build-static, build-native]`, added to
`request-staging`'s `needs`):
1. Stage `native-linux-x64` into `runtimes/linux-x64/native/`, take `static-libs` over the
   checkout, set up the SDK from `global.json` only (as `hyper-pack-nuget.yml` does, so it also
   proves SDK 11 alone packs `net10.0`).
2. Pack `0.0.0-pkg.<run_id>` to a local feed; `unzip -l` must list `lib/net10.0/HyperUuid.Managed.dll`,
   `lib/net11.0/HyperUuid.Managed.dll`, `build/net10.0/HyperUuid.targets`,
   `buildTransitive/net10.0/HyperUuid.targets`, and nothing under `build/net11.0/`.
3. For each TFM, publish the AOT smoke test in package mode (`-f <tfm> -r linux-x64
   -p:PublishAot=true`): no `libhyperuuid.so` in the output, no `NEEDED libhyperuuid.so` in
   `readelf -d`, and the executable exits 0 printing `ALL NATIVE AOT CHECKS PASSED`.
4. **Negative control, the silent fallback:** pack again as `0.0.0-pkg.<run_id>.nobuild`, `zip
   -d` its `build/*` and `buildTransitive/*`; the net10.0 publish against it **must** put
   `libhyperuuid.so` beside the executable, or the job fails (step 3 could not see a loss).
5. `dotnet restore` of the smoke test in package mode with `-p:TargetFrameworks=net9.0` must
   fail with NU1202 (control: step 3's net10.0 restore).
6. `PlatformGateTest -p:GateFramework=net10.0 -p:RuntimeIdentifier=browser-wasm` must fail with
   `error HYPERUUID003` and `Blazor WebAssembly` in the log; the same with `net11.0` must succeed.

**Android and Apple probes** (one step each, after the existing C# builds): `PlatformGateTest`
with `GateFramework` `net10.0-android` (`test-android`), and `net10.0-ios` and
`net10.0-maccatalyst` (`test-apple-mobile`), each failing with `error HYPERUUID003` and the
platform name. No `ValidateXcodeVersion=false`: the gate fires before the Xcode check. The
negative control is the existing net11 builds in the same jobs, which import the same file.

### Forge (`SkunkWerkx/.github`), inert by default

- `hyper-build-native.yml`, two inputs:
  - `csharp_aot_frameworks` (string, default `""`, space-separated TFMs). Empty: one publish with
    no `-f`, as today. Otherwise one publish per TFM with `-f <tfm> -o aot-out/<tfm>`, and the
    trim-warning grep, the "no shared library beside it" check and the run per TFM, all appended
    to the one `aot-report-{rid}`. Without it a multi-targeted AOT project fails every leg with
    NETSDK1129 (brief probe P3).
  - `csharp_extra_runtime` (string, default `""`, a channel such as `10.0`). Desktop legs pass
    `<channel>.x` as setup-dotnet's `dotnet-version` beside `global-json-file` (both are
    installed; `global.json` still selects SDK 11). The musl suite copies
    `shared/Microsoft.NETCore.App/<channel>.*` from `mcr.microsoft.com/dotnet/runtime:<channel>-alpine`
    (platform-matched) into the SDK image's dotnet root before `dotnet test`: same registry and
    tag footing as the SDK image, no install script fetched at run time.
- `musl-suite/action.yml` and `suite-musl.sh` (`csharp)` case): the same two inputs and loop.
- Forge `README.md`: both inputs. Untouched: `hyper-pack-nuget.yml` (plain `dotnet pack` packs
  both groups; the wasm staging path stays `net11.0`), `publish-nuget`, `test_release_provenance.py`.

### `global.json` and the .NET 11 RTM switch

Unchanged by this item (`11.0.100-rc.2.26504.105`, `latestFeature`, `allowPrerelease`): SDK 11
builds and packs `net10.0` by downloading the targeting pack, which `test-nuget-package` proves
with no SDK 10 present. The RTM switch from the owner's memory note — `global.json` to
`11.0.100` with `latestFeature`, drop the `dotnet-sdk` Dependabot entry, revisit
`allowPrerelease` and the `11.*-*` floats, and drop `-p:ValidateXcodeVersion=false` from the
three Apple smoke builds — is a separate commit, in either order. Neither touches the other: the
extra `10.0.x` install can never satisfy an 11 floor before or after the switch, the musl image
still derives `sdk:11.0-alpine-aot` from `global.json`, and this item adds no new
`ValidateXcodeVersion` use, so the switch still removes exactly three.

## Contract changes (final wording)

**csharp/README.md**
- *Install* (line 261): "Targets net10.0 and net11.0. On .NET 10: Windows, Linux and macOS,
  framework-dependent or Native AOT. On .NET 11 and later: those, plus Blazor WebAssembly, iOS,
  Mac Catalyst and Android — see [Platform support](#platform-support)."
- *Target frameworks* (line 149): "**Target frameworks.** The package builds `net10.0` and
  `net11.0` from one source, and package validation holds their public APIs identical. Its
  targets file sits under `build/net10.0/` (and `buildTransitive/net10.0/`); NuGet hands that one
  file to .NET 11 projects too. A project older than .NET 10 is refused at restore with NU1202.
  WebAssembly needs .NET 11: a .NET 10 Blazor WebAssembly build stops with `HYPERUUID003` (see
  [Platform support](#platform-support))."
- *WebAssembly is .NET 11 and later only* (line 163), appended: "On .NET 10 the link itself
  fails upstream (dotnet/runtime#132858), so the package stops a .NET 10 browser-wasm build with
  `HYPERUUID003` instead."
- *Troubleshooting* (line 165) becomes: "**`Unknown option '--enable-bulk-memory-opt'`** is .NET
  10's WebAssembly toolchain meeting an archive from rustc 1.87 or later
  ([dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858)), and is why
  `HYPERUUID003` stops .NET 10 browser builds; .NET 11's toolchain relinks cleanly." The `wasm-opt`
  swap "verified workaround" is removed: on .NET 10 it is not enough (brief P4-P5).
  *Correction (Task 7 fix round 1):* the shipped text keeps "on a native relink" in the heading, dates the claim (as of 2026-10-28, issue open, reproduced on 10.0.14, `rust-version` 1.89) and adds that a package consumer is stopped by `HYPERUUID003` first, so the error is reachable only by importing the targets file by hand.
- *Platform support* intro: "Every row below is .NET 11 and later; with Windows, macOS, iOS, Mac
  Catalyst and Android, that is every platform .NET MAUI targets. On .NET 10 the package supports
  the Linux, macOS and Windows rows, framework-dependent or Native AOT." New paragraph: "**.NET 10
  on WebAssembly, iOS, Mac Catalyst or Android: `HYPERUUID003`.** These need .NET 11 or later. A
  `net10.0` Blazor WebAssembly project, or a `net10.0-android`, `net10.0-ios` or
  `net10.0-maccatalyst` one, stops at build with `HYPERUUID003`, which names the platform and the
  framework to target. With the platform's workload installed, it fires ahead of the Xcode and
  JDK checks. Without the workload, the SDK's own missing-workload error (NETSDK1147) can come first;
  that case is not probed. A project that reaches the
  package through a class library gets the same error."
- *iOS and Mac Catalyst* (line 186) opens: "A .NET 11 iOS, MAUI or Mac Catalyst app
  (`net11.0-ios`, `net11.0-maccatalyst`; .NET 10 is not supported on either) references…"
- *Android* (line 190), after "Mono is not supported there": "; a `net10.0-android` build, which
  would run on Mono, stops with `HYPERUUID003`".
- *AOT* (lines 123-130): the snippet gains `-f net10.0` and its path becomes
  `bin/Release/net10.0/linux-x64/publish/`; add "CI publishes and runs it for net10.0 and net11.0
  on every desktop and musl leg, and `test-nuget-package` repeats both against the packed
  `.nupkg`, where a package with its `build/` folders removed must publish the shared library
  beside the executable, so the check cannot pass blind."
- *Benchmarks* (lines 88 and 103): labelled with the framework actually measured (Open item 1).

**README.md** (clause-level, so the other Phase 1 items merge cleanly)
- Line 165: add ".NET 10" to the "Today that is …" list; delete ".NET 11" from "JDK 25 and .NET
  11 sit above that line" and its clause ("and .NET 11 is the one target framework…"); add "C#
  covers Windows, Linux and macOS on .NET 10; Blazor WebAssembly, iOS, Mac Catalyst and Android
  need .NET 11."
- Line 159: "where a .NET iOS, MAUI or Mac Catalyst app" → "where a .NET 11 iOS, MAUI or Mac
  Catalyst app".
- Line 226, appended to the C# cell: "A .NET 10 project stops with `HYPERUUID003`
  (dotnet/runtime#132858)."
- Line 398: "the .NET 11 binding" → "the .NET 10 and 11 binding".
- Line 68, appended: "The C# row was measured on .NET 10." (per Open item 1).

**CHANGELOG.md**, `## [Unreleased]` → `### Changed`:
> - **C# — .NET 10 is supported, on desktop and server.** The package builds `net10.0` and
>   `net11.0` from one source, and package validation holds their public APIs identical. On .NET
>   10 it covers Windows, Linux (glibc and musl) and macOS, framework-dependent and Native AOT;
>   Blazor WebAssembly, iOS, Mac Catalyst and Android stay .NET 11 and later, and a .NET 10
>   project for any of them stops at build with the new error `HYPERUUID003`, which names the
>   platform (for WebAssembly, dotnet/runtime#132858: .NET 10's toolchain cannot link the core).
>   `HyperUuid.targets` moves to `build/net10.0/` (and `buildTransitive/net10.0/`), one copy NuGet
>   selects for both frameworks. CI runs the suite and the Native AOT smoke test on both
>   frameworks on every desktop and musl leg, and a new `test-nuget-package` job proves from the
>   packed package that a .NET 10 Native AOT publish links the core in. *(C#)*

**ROADMAP.md**: the C# "Floor" cell becomes ".NET 10 (desktop, server); .NET 11 (all platforms)".

**CLAUDE.md**, "Lists that move together", new bullet:
> - The .NET floor: `TargetFrameworks` in `HyperUuid.csproj`, `HyperUuid.Tests.csproj` and
>   `HyperUuid.AotSmokeTest.csproj`, ci.yml's `csharp_aot_frameworks` and `csharp_extra_runtime`,
>   the `HYPERUUID003` gate and the `build/net10.0/` folder name, and the floor sentences in
>   `csharp/README.md` and `README.md`. .NET 10 reaches EOL on 2028-11-14: then drop it from all
>   of them, and the targets folder moves to the new lowest TFM.

"Open follow-ups", new bullet: "C#: revisit `HYPERUUID003` for browser-wasm when
dotnet/runtime#132858 moves; the brief's approach B (rewriting `target_features`) is the proven
fallback."

## Files

`[own]`
- `csharp/HyperUuid/HyperUuid.csproj`: `TargetFrameworks`, the two validation properties,
  comments at lines 83-90 and 129-131.
- `csharp/HyperUuid/build/net11.0/HyperUuid.targets` → `build/net10.0/HyperUuid.targets`.
- `csharp/HyperUuid/buildTransitive/net11.0/HyperUuid.targets` → `buildTransitive/net10.0/`, import path.
- `csharp/HyperUuid.Tests/HyperUuid.Tests.csproj:7`.
- `csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj`: TFMs, package mode, import (line 22).
- `csharp/HyperUuid.WasmSmokeTest/HyperUuid.WasmSmokeTest.csproj:25,33`,
  `csharp/HyperUuid.AppleSmokeTest/HyperUuid.AppleSmokeTest.csproj:46`: import paths.
- `csharp/HyperUuid.AndroidSmokeTest/HyperUuid.AndroidSmokeTest.csproj:12`: comment.
- `csharp/HyperUuid.PlatformGateTest/HyperUuid.PlatformGateTest.csproj`: new.
- `csharp/README.md`: the sections above.

`[shared]`
- `.github/workflows/ci.yml`: in `build-native`'s `with:`, `csharp_aot_frameworks: "net10.0
  net11.0"` after line 106, `csharp_extra_runtime: "10.0"` after line 126, and the comment at
  lines 121-125 rewritten; comments at lines 221 and 423 (`net11.0` → `net10.0`); one step in
  `test-apple-mobile` after line 416; one step in `test-android` after "Build the four APKs"
  (about line 525); new job `test-nuget-package` between `test-android` and `request-staging`;
  `request-staging`'s `needs` (lines 588-589) gains `test-nuget-package`.
- `.github/scripts/tests/test_workflow_contracts.py`: one new test that `test-nuget-package`
  needs `[build-static, build-native]` and is in `request-staging`'s `needs`.
- `README.md`: lines 68, 159, 165, 226, 398, clause-level as above.
- `CHANGELOG.md`: one bullet under Unreleased → Changed.
- `ROADMAP.md`: the C# row's Floor cell only.
- `CLAUDE.md`: one "Lists that move together" bullet, one "Open follow-ups" bullet.

`[Forge]` (separate repo and PR)
- `.github/workflows/hyper-build-native.yml`: inputs block after `csharp_aot_project`;
  setup-dotnet (lines 829-833); AOT steps (lines 860-897); musl call (lines 1617-1626).
- `.github/actions/musl-suite/action.yml` (inputs, env), `suite-musl.sh` (`csharp)` case, lines 64-90).
- `README.md`: the two inputs.

## Ordering constraints

1. The Forge PR merges first; its defaults are inert, so it is a no-op for every consumer. Not
   while a release is in flight (`gh run list -w release.yml` in all three repos).
2. This repo's change is one PR, and the AOT smoke test's `TargetFrameworks` and ci.yml's
   `csharp_aot_frameworks` land in the same commit: either alone fails every AOT leg (NETSDK1129).
3. The RTM `global.json` switch is independent, in its own commit, either order.
4. Before the 1.1.0 tag, a release-mode `ci.yml` run on master proves the new artifact shape (two
   `lib/` groups) through `test-nuget-package` and every leg.

## Verification

**Local** (this Mac: SDK 11.0.100-rc.3, runtime 10.0.x, workloads android, ios, maccatalyst,
wasm-tools, wasm-tools-net10; package checks use `-p:RestorePackagesPath=<scratch>`). Each check
has its negative control (NC).
1. `dotnet build csharp/HyperUuid/HyperUuid.csproj -c Release`: 0 warnings, both TFMs. NC: a call
   to an API that is in net11.0's reference assemblies and not net10.0's fails the net10.0 build.
2. `dotnet test --project csharp/HyperUuid.Tests/HyperUuid.Tests.csproj -c Release`: one result
   line per TFM, equal counts. NC: under a scratch `DOTNET_ROOT` symlinking everything but
   `shared/Microsoft.NETCore.App/10.*`, the net10.0 run fails to start.
3. Per TFM, `dotnet publish csharp/HyperUuid.AotSmokeTest/HyperUuid.AotSmokeTest.csproj -c Release
   -f <tfm> -r osx-arm64 -p:PublishAot=true -o aot-out/<tfm>` (the Forge's flags plus `-f`), then
   run it: no `warning (IL|AOT)`, no libhyperuuid in `otool -L`, `ALL NATIVE AOT CHECKS PASSED`.
   NC: without `-f`, NETSDK1129; with `-p:HyperUuidStaticLink=false`, the dylib appears.
4. `test-nuget-package` steps 2-5 with osx-arm64 natives (`otool -L` for `readelf -d`), including
   the `.nobuild` NC (`libhyperuuid.dylib` must reappear) and NU1202 for net9.0.
5. HYPERUUID003 naming the platform, from a packed package: a `Microsoft.NET.Sdk.WebAssembly`
   net10.0 app; `PlatformGateTest` for `net10.0`+`browser-wasm`, `net10.0-android`, `net10.0-ios`,
   `net10.0-maccatalyst`; and a net10.0 Blazor app reaching the package through a net10.0 class
   library (buildTransitive). NC: the net11 twins and a net10.0 console report no HYPERUUID003;
   with the gate target removed, `net10.0-ios` reaches the SDK's "requires Xcode 26.0" error.
6. `csharp/HyperUuid.WasmSmokeTest/check.sh`: PASS through the new import path (its existing
   NC: the archive unlinked fails the link).
7. The three `AppleSmokeTest` builds with ci.yml's flags (`ValidateXcodeVersion=false` included)
   build and run; NC: the device-symbol `nm` check.
8. `dotnet format whitespace csharp/HyperUuid.slnx --verify-no-changes`. NC: a one-space indent fails.
9. `dotnet pack` passes package validation. NC: a public member under `#if NET11_0_OR_GREATER` fails it.
10. `python3 -m unittest discover .github/scripts/tests` here and the Forge's
    `.github/scripts/test_*.py`; `actionlint` on every changed workflow. NC: drop
    `test-nuget-package` from `request-staging` and the new contract test fails.
11. The Forge's `suite-musl.sh csharp` for linux-musl-arm64 under Docker with both inputs set: both
    TFMs run. NC: `csharp_extra_runtime` empty, and the net10.0 test run fails to start.

**CI only** (the report says these were not run locally): the win-x64, win-arm64, linux-x64,
linux-arm64 and linux-musl-x64 legs on both TFMs; setup-dotnet's dual install on hosted images;
`test-nuget-package` on Linux (`readelf`); the `net10.0-android` probe and the Apple probes on
`xcode-27`.

**Release-time** (1.1.0, the first two-framework package): nuget.org's page lists `net10.0` and
`net11.0`; publish-nuget's `nuget-finalized` entry comparison passes (TFM-agnostic); local check
4 repeated against nuget.org's 1.1.0 (net10 AOT links the core, net9.0 gets NU1202); the
inventory and release attestation carry the one `.nupkg` and `.snupkg` as before.

## Nitpick audit

| Hole a reviewer could find | Closed or labelled |
| --- | --- |
| net10 AOT silently loads the dylib if `build/net10.0/` is missing | Closed: `test-nuget-package` 2-4 with the `.nobuild` NC |
| "Same API on both frameworks" is asserted | Closed: strict package validation fails pack on any difference |
| net10 is built but never run | Closed: tests on both TFMs on 5 desktop + 2 musl legs; local NC 2, 11 |
| SDK 11 can't really pack net10 in CI | Closed: `test-nuget-package` packs with `global.json`'s SDK only |
| HYPERUUID003 loses to Xcode/JDK/workload errors | Closed for Xcode and JDK with the workload installed (early hook, probed 2026-10-28; the Apple probes build an Exe, because the Xcode check runs only for one, and see HYPERUUID003 win over the Xcode error with the gate in and the Xcode error with it removed, on Xcode 27.1; CI runs the iOS, Mac Catalyst, Android and wasm probes, each proving HYPERUUID003 with the workload and toolchain present, not the missing-JDK or missing-Xcode case). Labelled, unprobed: with the workload missing, NETSDK1147 may come first, and the README says so (amended after review, 2026-10-28) |
| A net10 Blazor app is blocked by a server-side class library | Labelled: it got NU1202 in 1.0.0 already (Open item 2) |
| osx-x64 net10 is unrun | Labelled: cross-built, no leg on either framework, as the README says |
| `net10.0-macos`, `net10.0-tvos` | Not claimed on either framework; tvOS fails at native link as today |
| Benchmarks labelled with the wrong framework | Closed by Open item 1 (git shows they are .NET 10) |
| The `wasm-opt` swap was a "verified workaround" | Removed: it does not fix .NET 10 (brief P4-P5) |
| The musl 10 runtime is unpinned | Labelled: MCR by tag, like the existing SDK image; no script fetched |
| Probes ran on rc.3, CI pins rc.2 | Labelled: CI is the rc.2 receipt; same mechanism |
| ".NET 10 LTS until 2028-11-14" | Sourced: endoflife.date, 2026-10-28; recorded in CLAUDE.md |

## Out of scope / Later

- net10 Blazor WebAssembly, when dotnet/runtime#132858 moves (approach B is proven in Chrome).
- net10 Android (Mono), net10 iOS and Mac Catalyst (Xcode 26.0).
- No net10 benchmark column.
- Package baseline validation against 1.0.0 (`PackageValidationBaselineVersion`).
- HyperCast and HyperTabular get their own roadmaps after v1.1.

## Open items

1. **Benchmarks: decided 2026-10-28.** The tables are .NET 10 measurements today (`58cbdf6`,
   2026-10-06), so they are not relabelled. They are re-measured on .NET 11 on the Apple M6 in the
   system-wide receipts pass, which moves every table in the repo to the M6 and runs last in 1.1.0.
   This spec owns no benchmark wording: the README :68 and Benchmarks bullets above defer to that
   pass.
2. **An opt-out for HYPERUUID003?** Recommended: none in 1.1.0. The owner asked for a loud build
   failure, and the only case it blocks (a net10 browser or MAUI app reaching the package
   transitively) could not restore at all in 1.0.0. Add one only on request.
