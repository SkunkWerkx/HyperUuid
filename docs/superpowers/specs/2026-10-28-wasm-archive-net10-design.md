# Browser-wasm: an archive with no exception handling, and .NET 10 everywhere

Written 2026-10-28 from issue #50 and the owner's decisions of the same day; revised the same
day after a review (five findings, all verified against the code and folded in; see Revision
log). **Draft for owner review.** Spans HyperUuid, HyperCast, HyperTabular and the Forge
(`SkunkWerkx/.github`), as a scoped exception to the sibling freeze. Ships in 1.1.0, and closes
#50. `docs/superpowers/plans/` will hold the implementation plan.

## Goal

Every Hyper* package's browser-wasm archive is built by Rust in the form .NET links, with no
.NET-side workaround: no translate pass, no `WasmRunWasmOpt` override, nothing that depends on
which configuration the consumer builds. The same archive serves .NET 10 and .NET 11, so
Blazor WebAssembly joins the platforms .NET 10 is supported on: in HyperUuid now, and in
HyperCast and HyperTabular, which move their floor from .NET 11 to .NET 10 as well.

## Decisions (owner, 2026-10-28)

- **A correct archive over a stable-only toolchain.** The archive is built on a pinned, dated
  nightly with `-Zbuild-std`. The owner prefers no nightly step, but prefers an archive with no
  workaround far more, and is "all in on the nightly" for the smaller package and .NET 10 as a
  WebAssembly target. A stable trim was considered (Evidence) and declined.
- **Native binaries stay on stable.** build-std leaves the cdylib byte-identical (50,608 B on
  macOS arm64, `lto = true` already folds `core` in), and the Forge already trims the native
  archives. watchOS devices (`arm64_32`), the one native target that would need it, are out.
- **The smaller package is the point.** The browser-wasm asset drops from about 3 MB to about
  350 KB. The linked app barely changes (below); the size claim is about the `.nupkg`.
- **Cascade** to HyperCast and HyperTabular: "it needs to hit all the .NET wasm runtimes".
  The freeze otherwise stands (memory `prove-in-hyperuuid-then-cascade`).
- **In 1.1.0, with no stopgap.** "There is no debug fix: we compile it right and fold this
  into the 1.1 release."
- **.NET 10 WebAssembly is in scope.** "I'd rather support WASM in .NET 10 than not."
- **HyperCast gets .NET 10 through a union polyfill** behind conditional compilation, so a
  .NET 11 consumer carries none of it. The owner also asked for the .NET 10 helper API
  (`Match`, `Switch`) to be conditional; review finding 1 shows that breaks compiled
  dependencies, so those two ship on both frameworks (below); the owner confirmed.
- **HyperTabular follows HyperCast.** It is net11-only because it depends on HyperCast.
- **The nightly pin lives in `.github/hyper.json`,** one place for the Forge and `check.sh`
  (owner, accepting the recommendation).

## Evidence (local, 2026-10-28)

macOS arm64, .NET SDK 11.0.100-rc.3.26522.148 with `wasm-tools` and `wasm-tools-net10`,
Chromium 156.0.8078.4 (Playwright's), rustc 1.99.1 stable and `nightly-2026-10-27` (rustc
1.101.0-nightly 41089d07a).

### Where the legacy exception handling comes from

Counted with `llvm-objdump -d` over `try`, `catch`, `catch_all`, `rethrow`, `delegate`,
`throw`, `try_table` and `throw_ref`:

| Archive | Linked by | EH instructions |
| --- | --- | --- |
| `wasm32-unknown-emscripten`, stable (`cargo wasm-staticlib`) | C# Blazor | 80: 40 `try`, 40 `catch_all` |
| `go/staticlib/wasm` (`wasm32-wasip1`) | TinyGo | 0 (15 functions disassembled) |
| Swift bundle, `wasm32-unknown-wasip1` | Swift wasm, and the Ruby wasm gem's archive type | 0 (15 functions disassembled) |

All 80 are in Rust's prebuilt `compiler_builtins` (`__addtf3`, `__divdi3`, `__udivti3`, ...):
the emscripten target's sysroot is built to unwind, and our `panic=abort` does not rebuild it.
The Pyodide wheel is a separate PyO3 build (`pyodide build` in `python/`, Pyodide's own
Emscripten and flags) and links none of these archives. The wasip1 archives, and so TinyGo,
Swift and Ruby, are untouched by this design.

### The archive

`-Zbuild-std=core,compiler_builtins` with `-C target-cpu=mvp -C
target-feature=+sign-ext,+mutable-globals` (revised in implementation; see the Revision log):

- 350,762 bytes and 2 objects (stable: 3,280,182 bytes, 271 objects).
- 0 of all eight EH instructions, `throw` included.
- `target_features` (as first measured, with the four features since cut to two):
  `+multivalue +mutable-globals +nontrapping-fptoint +sign-ext -shared-mem`. The stable archive also declares `bulk-memory`, `bulk-memory-opt`,
  `call-indirect-overlong`, `exception-handling` and `reference-types`. The two that .NET 10's
  Binaryen 117 rejects (dotnet/runtime#132858) are gone.
- No rustc warnings. Naming `-bulk-memory-opt`/`-call-indirect-overlong` directly warns
  "unknown and unstable feature ... will become a hard error"; `target-cpu=mvp` with an
  explicit list does not. P6 of the Phase 1 C# brief found `target-cpu=mvp` ineffective: that
  was the prebuilt sysroot, which build-std rebuilds.

### The stable alternative, considered and declined

The Forge's `static-libs` trim, which keeps only the crate's own object (the native archives
ship at 7 to 34 KB), applied to today's stable wasm archive: that object holds 0 legacy EH
instructions, since all 80 are in the `compiler_builtins` objects the trim drops. It still
declares `bulk-memory-opt` and `call-indirect-overlong`. So a trimmed stable archive would
likely serve .NET 11 (not linked or booted; it may need Emscripten's own `compiler-rt` for
builtins), but never .NET 10. The owner chose the nightly for both of its gains: the smaller
package and .NET 10 WebAssembly.

### Booted in Chromium 156 (HyperUuid's Blazor smoke app, all checks)

| Case | Stable archive | New archive |
| --- | --- | --- |
| .NET 11, Release | PASS only with the translate pass | PASS, no translate pass |
| .NET 11, Debug, unstripped | refused: "mix of legacy and new exception handling instructions" (#50) | PASS, `wasm-opt` off entirely |
| .NET 10, Release, targets without the exnref item | fails at link: `Unknown option '--enable-bulk-memory-opt'` | **PASS** |
| .NET 10, Debug, unstripped | not run | not run (Verification adds it) |

The .NET 10 row is from the stock 10.0.14 Emscripten pack. This machine's pack had had its
`wasm-opt` replaced by the rc.3 pack's Binaryen 133 (byte-identical, modified 03:18 the same
day), under which the stable archive falsely passed. It was restored from the pack's own
nupkg (version 117) before these runs, and the replaced binary kept in the session scratchpad.
The .NET 10 probe app referenced `Microsoft.AspNetCore.Components.WebAssembly` 10.* and
`Microsoft.AspNetCore.Components.WebAssembly.DevServer` 10.* in place of the Gateway package.
Firefox and Safari are not tested.

### Linked size (Release publish, .NET 11)

| Archive | `dotnet.native.wasm` | Brotli |
| --- | --- | --- |
| stable | 3,053,408 B | 957,624 B |
| build-std | 3,051,948 B | 957,386 B |

The linker already drops the stable archive's dead weight; the gain is in the package.

### The union on .NET 10 (HyperCast's real `Verdict{T}.cs`, `Success{T}.cs`, `Fault.cs`, `CastFailure.cs`)

| Case | Result |
| --- | --- |
| Library multi-targeted `net10.0;net11.0`, `LangVersion preview`, internal polyfill of `UnionAttribute` and `IUnion` on net10.0 | Identical on both: the exhaustive switch and the case-type conversions compile, and a partial switch fails CS8509 on both |
| Same, polyfill removed (control) | CS0246: `UnionAttribute`, `IUnion` not found |
| net10.0 consumer, `LangVersion preview` (SDK 11) | Union syntax and exhaustiveness work through the internal polyfill; runs |
| net10.0 consumer, C# 14 (SDK 10's only choice) | `CS9327: Feature 'unions' is not available in C# 14.0` on the case-type switch and on `Verdict<int> w = new Fault(...)`. `TryGetValue` and the constructors compile |
| net10.0 consumer on SDK 10.0.403, default language version (review probe, confirmed) | `CS8652: The feature 'unions' is currently in Preview and *unsupported*` on the same lines |

.NET 11's own types, by reflection: `UnionAttribute` (`AttributeTargets.Class | Struct`,
`Inherited = false`, no members) and `IUnion` (`object? Value { get; }`), both in
`System.Runtime.CompilerServices` in CoreLib.

## Design

### Rust: the `wasm-staticlib` build (all three repos)

`rust/.cargo/config.toml`:

```toml
[target.wasm32-unknown-emscripten]
rustflags = ["-C", "panic=abort", "-C", "target-cpu=mvp",
             "-C", "target-feature=+sign-ext,+mutable-globals"]

[alias]
wasm-staticlib = "rustc -Zbuild-std=core,compiler_builtins --release --target wasm32-unknown-emscripten --crate-type staticlib --no-default-features --features staticlib"
```

(HyperTabular keeps its `--lib`.) The comment above the target section says why each flag is
there: no unwinding sysroot, no legacy EH, no feature .NET 10's toolchain rejects. The section
applies only to cargo runs from `rust/` for this target; the Pyodide build runs from
`python/`. The alias needs nightly; on stable it stops with cargo's "-Z flag is only accepted
on the nightly channel", which is the intended failure.

The nightly is pinned per repo in `.github/hyper.json`:

```json
"wasm": { "toolchain": "nightly-2026-10-27" }
```

Refreshed by hand, like the other branch-head toolchains Dependabot ignores, and commented
where it is read as the one nightly exception to "each language's primary toolchain is its
current release".

### Forge: `hyper-build-wasm.yml`, inert until a repo sets a pin

Every Forge change is keyed on `wasm.toolchain`. A repo without it (each repo until its own
PR lands) builds, packs and tests exactly as today. Nothing in `hyper-pack-nuget.yml` changes:
the archive keeps its staging path, `runtimes/browser-wasm/nativeassets/net11.0/` (below).

- `hyper_config.py` reads `wasm.toolchain`; absent means `stable` and today's build.
- With a pin: install it with `rust-src` (`dtolnay/rust-toolchain` at the pinned name), run
  `cargo +<pin> wasm-staticlib`.
- With a pin, the **archive guard**, a Forge script (`wasm_archive_guard.py`) run after the
  `rust_eh_personality` check. Two independent checks, each failing on its own:
  - **Instructions:** fails if any function holds `try`, `catch`, `catch_all`, `rethrow`,
    `delegate`, `throw`, `try_table` or `throw_ref`.
  - **Features:** fails if any object's `target_features` names `exception-handling`,
    `bulk-memory-opt` or `call-indirect-overlong`.
  - **Inspection is checked, not trusted:** a non-zero exit from `llvm-ar` or
    `llvm-objdump`, an archive with no members, a member with no `target_features` section,
    or a disassembly with no functions fails the guard. An empty result is never read as
    clean.
- The guard's tests live with the Forge's other script tests, with **isolated negative
  fixtures**: tiny objects built by the test from a few lines of C or `.s` with emsdk's clang,
  one per forbidden instruction class (a `throw`, a legacy `try`/`catch_all`, a `try_table`),
  one per forbidden feature, one empty archive and one non-wasm member. Each must fail with
  the reason it was built for, and one clean fixture must pass.
- `test-blazor`, with a pin: install the .NET 10 runtime beside global.json's SDK, and
  `wasm-tools-net10` beside `wasm-tools`. Without a pin, keep today's installation unchanged.
- The header comment's paragraph on the consumer-side translation is rewritten: there is none
  once a repo pins.

### Each package's `.targets`

Removed: the `--translate-to-exnref` item. No configuration-specific fix replaces it (#50 was
prototyped with a `WasmRunWasmOpt` default and dropped). With nothing to translate, the SDK's
own defaults are right in every configuration. The comment that explained them is replaced by
one sentence pointing at the archive guard.

The archive **stays** at `runtimes/browser-wasm/nativeassets/net11.0/`. The targets file links
it by literal path, so the folder name is not something NuGet selects by. A net10.0 consumer
gets no copy-local item from a `net11.0` folder (nothing to drop), and a net11.0 one still has
it dropped by `_HyperUuidDropWasmArchiveFromCopyLocal`. The targets comment that explains the
folder name gains one clause: .NET 10 links it too. The earlier draft's rename is withdrawn
(review finding 2): it coupled a shared Forge path to three repos' targets files.

HyperUuid only: `HYPERUUID003` no longer refuses Blazor WebAssembly. It still refuses
`net10.0-android`, `-ios` and `-maccatalyst`, and loses the #132858 suffix.

### CI gate checks (HyperUuid `ci.yml`, "Blazor WebAssembly needs .NET 11")

Today's step requires a net10.0 browser build to fail with `HYPERUUID003`; it becomes
**"The gate admits Blazor WebAssembly on .NET 10 and 11"**:

- For `GateFramework` net10.0 and net11.0 with `RuntimeIdentifier=browser-wasm`:
  `_HyperUuidNet10Unsupported` evaluates empty.
- For both, a build with `-p:WasmNativeWorkloadAvailable=false` fails with `HYPERUUID002` and
  without `HYPERUUID003` (the missing-workload rejection is kept on both frameworks).
- Kept: the mobile rejections (`net10.0-android`, `-ios`, `-maccatalyst` stop with
  `HYPERUUID003`) wherever they are checked today, unchanged.

HyperCast and HyperTabular gain the same gate step for their `003` codes.

### Blazor smoke projects and `check.sh` (all three repos)

The smoke project multi-targets `net10.0;net11.0`, with framework-appropriate references:

```xml
<ItemGroup Condition="'$(TargetFramework)' == 'net10.0'">
  <PackageReference Include="Microsoft.AspNetCore.Components.WebAssembly" Version="10.*" />
</ItemGroup>
<ItemGroup Condition="'$(TargetFramework)' == 'net11.0'">
  <PackageReference Include="Microsoft.AspNetCore.Components.WebAssembly" Version="11.*-*" />
  <PackageReference Include="Microsoft.AspNetCore.Components.Gateway" Version="11.*-*" PrivateAssets="all" />
</ItemGroup>
```

`check.sh` serves the publish output statically, so the net10 leg needs no dev server package;
the probe carried `WebAssembly.DevServer` 10.*, and implementation confirms the leg passes
without it (or keeps it, with the reason). `check.sh`:

- Builds the archive as CI does: reads the pin from `.github/hyper.json` and runs `cargo
  +<pin> wasm-staticlib` (unless `STATICLIB` is given).
- Passes `-f` on every leg, four in all, each from clean, each requiring PASS:

  | Leg | Arguments |
  | --- | --- |
  | net11 Release | `-c Release -f net11.0` |
  | net11 Debug unstripped | `-c Debug -f net11.0 -p:WasmNativeStrip=false` |
  | net10 Release | `-c Release -f net10.0` |
  | net10 Debug unstripped | `-c Debug -f net10.0 -p:WasmNativeStrip=false` |

### HyperCast and HyperTabular: the .NET 10 floor

Mirrors HyperUuid's Phase 1 net10 spec (`2026-10-28-csharp-net10-design.md`), with Blazor in:

- `csharp/Directory.Build.props`: `TargetFrameworks` `net10.0;net11.0` for the library, tests
  and AOT and wasm smoke projects. `LangVersion preview` stays: the C# 15 compiler in SDK 11
  builds both, and the probe shows the unions compile identically for net10.0. The comment
  that names .NET 11 as the floor *because of* unions is rewritten.
- `build/net11.0/` → `build/net10.0/` (and `buildTransitive/`), with a gate like
  `HYPERUUID003`: `HYPERCAST003` and `HYPERTABULAR003`, for `net10.0-android`, `-ios` and
  `-maccatalyst`.
- Package validation as HyperUuid has it, strict mode included: with the API identical on both
  frameworks (below), the public surfaces are equal and strict mode can pass.
- HyperTabular references HyperCast as a package (`Version="1.0.0"`), so its .NET 10 floor
  needs a HyperCast release with `lib/net10.0/` first.

#### `Verdict<T>` on .NET 10

`Polyfill.cs`, compiled only below .NET 11:

```csharp
#if !NET11_0_OR_GREATER
namespace System.Runtime.CompilerServices
{
	[AttributeUsage(AttributeTargets.Class | AttributeTargets.Struct, Inherited = false)]
	sealed class UnionAttribute : Attribute;
	interface IUnion { object? Value { get; } }
}
#endif
```

Internal, so it is not public surface, never collides with a consumer's types, and is absent
from the net11.0 assembly.

On `Verdict<T>`, on **both** frameworks, the path a C# 14 consumer has instead of union
syntax:

```csharp
public TResult Match<TResult>(Func<Success<T>, TResult> success, Func<Fault, TResult> fault) => ...
public void Switch(Action<Success<T>> success, Action<Fault> fault) { ... }
```

Two delegates, both required, so a missing case is a compile error without unions. Both throw
`InvalidOperationException` on a defaulted verdict, as the switch throws
`SwitchExpressionException`.

Why both frameworks (review finding 1): a net10.0 library compiled against HyperCast's
net10.0 assembly can be loaded, unchanged, by a net11.0 application, which resolves HyperCast's
net11.0 assembly. If `Match` existed only on net10.0, that library would throw
`MissingMethodException` with nobody having retargeted anything. Microsoft's package
compatibility rules require the higher framework's surface to cover the lower one's
([NuGet package compatibility rules](https://learn.microsoft.com/dotnet/standard/library-guidance/nuget-package-compatibility-rules)).
The cost on net11.0 is two small generic methods.

The remarks on `Verdict<T>` say which path each consumer has: `switch` over the case types on
C# 15 (any .NET 11 project, or .NET 10 with `LangVersion` 15), and `Match`, `Switch` or
`TryGetValue` on C# 14. HyperTabular's own code uses the union, which its net10.0 build
compiles through HyperCast's polyfill (proven by the consumer-preview row).

## Contract changes

- C# README (all three), Platform support: Blazor WebAssembly on .NET 10 and later. The
  translate-to-exnref paragraph goes; in its place, one sentence: the archive carries no
  exception handling and no feature newer than .NET 10's toolchain, checked when it is built.
- HyperCast and HyperTabular README floors: .NET 10, with the same platform split as HyperUuid
  (Android, iOS, Mac Catalyst on .NET 11).
- HyperCast README: the section for consumers without C# 15 (`Match`, `Switch`,
  `TryGetValue`), naming both diagnostics: `CS8652` on SDK 10, `CS9327` on SDK 11 with an older
  `LangVersion`.
- CLAUDE.md (all three): the nightly as a commented toolchain exception; the local loop
  (`rustup toolchain install <pin> --component rust-src`); the .NET floor list (HyperUuid: the
  `HYPERUUID003` scope and the gate step in `ci.yml`); the open follow-up on `HYPERUUID003`
  and #132858 goes, replaced by one on the nightly pin. HyperCast and HyperTabular gain the
  .NET floor list.
- CHANGELOG, Unreleased: HyperUuid's entry describes the archive and closes #50; HyperCast and
  HyperTabular each get their floor and wasm entries, and HyperCast's names `Match` and
  `Switch`.

## Ordering

1. Forge PR. Inert: every change is keyed on a pin no repo has yet, and the pack staging path
   does not change. Merge.
2. HyperUuid: pin, alias, targets, gate step, smoke project, check.sh, docs. CI proves the
   guard and all four browser legs.
3. HyperCast: the same, plus the .NET 10 floor and `Verdict<T>`. Released before step 4 ships.
4. HyperTabular: the same, against the released HyperCast.

Each repo's producer (its pin and alias) and consumer (its targets and `check.sh`) change in
one PR, so no package is ever built half-migrated. No Forge merge while a release is in flight.

## Verification (each with its negative control)

- **Archive guard:** passes on the build-std archive. Every isolated fixture fails for its own
  reason (Forge tests), and the stable archive fails too.
- **`check.sh`:** all four legs PASS. Controls: the stable archive fails net11 Debug and
  net10 Release. Record the stable archive's net10 Debug result without prescribing failure:
  that case is not yet measured, and an unstripped Debug build can skip the `wasm-opt` step
  whose rejected feature flags explain the Release failure. The archive guard must reject
  the stable archive independently of whether an individual browser configuration accepts it.
- **Gate (HyperUuid):** net10.0 and net11.0 browser-wasm pass the gate and stop at `HYPERUUID002`
  without the workload. Control: `net10.0-android` still stops with `HYPERUUID003`.
- **HyperCast net10.0:** the full test suite on both frameworks; a partial switch inside the
  library fails CS8509 on both. Control: removing the polyfill fails the net10.0 build.
- **`Match`/`Switch`:** a net10.0 C# 14 consumer compiles and runs with them. The polyfill
  types are absent from the net11.0 assembly (reflection), and package validation passes in
  strict mode.
- **SDK 10 packaged consumer:** outside the repository's SDK 11 `global.json`, select an
  installed SDK 10 explicitly and log `dotnet --version`. Restore the locally packed NuGet
  packages using `PackageReference`, with no project references or manual targets imports.
  Exercise constructors, `TryGetValue`, `Match` and `Switch`, then publish and boot a net10
  Blazor consumer in Release and unstripped Debug with SDK 10's stock wasm-tools workload.
  This checks the actual compiler and package asset/targets selection, including the archive
  retained under `nativeassets/net11.0/`; SDK 11 with `LangVersion=14` alone is not this test.
  Include HyperTabular's transitive HyperCast package. Control: removing the packaged wasm
  archive makes the consumer build fail.
- **Compiled-dependency compatibility (finding 1):** a net10.0 class library that calls
  `Match`, `Switch` and the union paths, built against HyperCast's net10.0 assembly, referenced
  as a built DLL by a net11.0 application that resolves HyperCast's net11.0 assembly: it runs
  with no `MissingMethodException`. Control: the same test with `Match` removed from the
  net11.0 build throws `MissingMethodException`.
- **Local toolchain sanity:** the `wasm-opt` in each Emscripten pack matches the pack's nupkg
  before any .NET 10 result is trusted.

## Nitpick audit

- **A nightly builds an attested artifact.** Mitigated by the dated pin, the guard and the
  browser legs; it is still the one place the release depends on an unstable compiler.
- **`-Zbuild-std` can change under us.** Moving the pin is a deliberate PR, re-proven by CI.
- **Chromium only.** Firefox and Safari are not tested.
- **`target-cpu=mvp` drops `bulk-memory` and `reference-types` from the archive.** The core
  then copies memory without `memory.copy`; the effect on its speed in the browser is not
  measured. The rest of the module is still built with both by the SDK.
- **The archive folder says `net11.0` and .NET 10 links it.** Kept on purpose (finding 2);
  the targets comment says so.

## Out of scope / Later

- Firefox and Safari legs.
- Moving off nightly: when the emscripten target's sysroot stops carrying legacy EH, or
  build-std stabilizes, the pin goes and the alias returns to stable.
- .NET 10 on Android, iOS and Mac Catalyst.
- **Removing `Match` and `Switch`.** When the floor moves to .NET 11 (the owner expects that
  when .NET 12 ships), every consumer has union syntax and the two methods can go, with the
  polyfill. Removing public API is breaking, so it waits for HyperCast 2.0.

## Open items

1. **Pin date** at implementation: the newest nightly that passes the guard and every browser
   leg.

## Revision log

Review of 2026-10-28, each finding checked against the code before folding in:

1. Conditional `Match`/`Switch` breaks compiled net10 dependencies in net11 apps. Confirmed by
   the resolution rule; now on both frameworks, with a compiled-dependency test. The owner confirmed.
2. A Forge-first rename of the staging path would break unconverted repos: all three targets
   files and `hyper-pack-nuget.yml:200` name `nativeassets/net11.0`. The rename is withdrawn,
   so the Forge change is inert by construction.
3. `ci.yml`'s "Blazor WebAssembly needs .NET 11" step (`ci.yml:773`) requires the net10
   browser build to fail with `HYPERUUID003`. Replaced by acceptance checks on both frameworks,
   keeping the missing-workload and mobile rejections.
4. All three smoke projects reference `Microsoft.AspNetCore.Components.WebAssembly` and
   `.Gateway` at `11.*-*` unconditionally, and `check.sh` publishes without `-f`. Now
   per-framework references, `-f` on every leg, and a net10 Debug unstripped leg.
5. The guard's list omitted `throw`. Added (the build-std archive has 0 `throw`; the stable
   one has 40 `try` and 40 `catch_all`, no `throw`), with checked inspection and isolated
   negative fixtures.

Implementation, 2026-10-29 (HyperTabular's net10 Release leg):

6. `+nontrapping-fptoint` and `+multivalue` were wrong for .NET 10. A .NET 10 Release publish
   runs the SDK's own `wasm-opt` (`WasmApp.Common.targets:912`) after Emscripten's, which has
   by then stripped the target_features section; it enables only simd, bulk memory and
   exception handling past Binaryen 117's defaults (sign-ext, mutable-globals), so
   HyperTabular's six `i64.trunc_sat_f64_s` failed validation. HyperUuid's and HyperCast's
   archives held none, so their legs had passed. The rustflags are now
   `+sign-ext,+mutable-globals`, and the guard's three-name denylist became an allowlist of
   those two, which rejects all three earlier archives. Sizes: HyperUuid 349,970 B, HyperCast
   498,640 B, HyperTabular 663,172 B; all twelve browser legs pass.
