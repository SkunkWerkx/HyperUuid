# Browser-wasm archive and .NET 10 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build each Hyper* core's `wasm32-unknown-emscripten` archive with no exception handling and no feature .NET 10 rejects, link it in Blazor on .NET 10 and .NET 11 with no .NET-side workaround, and bring HyperCast and HyperTabular to .NET 10 with `Verdict<T>.Match`/`Switch` for C# 14 consumers.

**Architecture:** A pinned nightly with `-Zbuild-std=core,compiler_builtins` and `-C target-cpu=mvp` builds the archive; a Forge guard script rejects any archive with EH instructions or any feature but `sign-ext` and `mutable-globals` (an allowlist since 2026-10-29; see the spec's Revision log, item 6). Every Forge change is keyed on a `wasm.toolchain` pin in the consumer's `.github/hyper.json`, so the Forge lands inert and each repo opts in with its own PR. HyperCast multi-targets `net10.0;net11.0` with an internal union polyfill below .NET 11.

**Tech Stack:** Rust nightly (`-Zbuild-std`), Emscripten, Binaryen, Python 3 (`unittest`), GitHub Actions, .NET SDK 11 (rc.3) and SDK 10.0.403, MSBuild targets, C# 15 / 14, headless Chromium.

**Spec:** `docs/superpowers/specs/2026-10-28-wasm-archive-net10-design.md`

## Global Constraints

- The owner drives git: no commit, push, tag, PR or workflow dispatch. Each task ends with its work verified and left in the working tree, and the report names what ran locally and what only CI can prove.
- Before editing a repo: `git fetch` and compare with origin.
- Nightly pin: `.github/hyper.json` → `"wasm": { "toolchain": "nightly-YYYY-MM-DD" }`; absent means `stable`. Pin date: the newest nightly that passes the guard and every browser leg (start from `nightly-2026-10-27`, proven in the spec).
- Archive rustflags: `-C panic=abort -C target-cpu=mvp -C target-feature=+sign-ext,+mutable-globals`.
- Alias: `rustc -Zbuild-std=core,compiler_builtins --release --target wasm32-unknown-emscripten --crate-type staticlib --no-default-features --features staticlib` (HyperTabular keeps its `--lib`).
- Forbidden instructions: `try`, `catch`, `catch_all`, `rethrow`, `delegate`, `throw`, `try_table`, `throw_ref`. Allowed features: `sign-ext`, `mutable-globals`; every other declared feature fails.
- The archive stays at `runtimes/browser-wasm/nativeassets/net11.0/`; `hyper-pack-nuget.yml` is not touched.
- No `WasmRunWasmOpt`, `-g` or `--translate-to-exnref` in any `.targets` file.
- `Match`/`Switch` ship on both frameworks; only `Polyfill.cs` is under `#if !NET11_0_OR_GREATER`.
- Diagnostics: `HYPERUUID003` keeps android/ios/maccatalyst and drops Blazor; new `HYPERCAST003`, `HYPERTABULAR003` for `net10.0-android`, `-ios`, `-maccatalyst`.
- Before trusting any .NET 10 wasm result: each Emscripten pack's `wasm-opt` matches the copy in its own nupkg.
- Workflow edits: run actionlint and the repo's `.github/scripts/tests`; Forge edits: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`.
- Formatters cover src, tests and smoke tests (`dotnet format --verify-no-changes` where the repo runs it; `shellcheck` on `check.sh`).
- Docs state current state only; history goes in the CHANGELOG under `## [Unreleased]`.
- **Packages under test are never served from a cache.** Every packed variant (positive or control) gets a version of its own (`<base>-local.<n>`), restored with a fresh `NUGET_PACKAGES` directory and a clean consumer `bin/`/`obj/`; after restore, the `sha256` of the restored `nativeassets/net11.0/lib*.a` (and of the managed DLL, where the variant changes it) must equal the bytes that were packed.
- **Review checkpoints:** after Task 3 (Forge), Task 7 (HyperUuid) and Task 11 (HyperCast), one fresh reviewer reads that repo's diff against this plan and the spec before the next repo starts; findings are fixed first.

## Review Focus

1. **One Blazor app linking two build-std archives** (HyperTabular's smoke links `libhypercast.a` and `libhypertabular.a`): each now carries its own `compiler_builtins` object; expected to link with no duplicate symbol. Pinned by Task 12's check.sh run.
2. **A Blazor app reaching the package through a class library** (buildTransitive) on .NET 10: expected to link the archive exactly like a direct reference. Pinned by Task 7's packaged-consumer step.
3. **`Match`/`Switch` on `default(Verdict<T>)`**: expected `InvalidOperationException`, never a silent call of either delegate. Pinned by Task 10's tests.
4. **A repo with no pin after the Forge merges**: expected byte-for-byte today's build and no guard. Pinned by Task 1's default test and Task 3's contract assertion.
5. **A pin naming a toolchain without `rust-src`, or a non-dated nightly**: expected a loud failure at config or install, not a stable build. Pinned by Task 1's validation test and Task 3's install step.

---

## Forge (`SkunkWerkx/.github`, checkout `../HyperForge`)

### Task 1: `wasm.toolchain` in `hyper_config.py`

**Files:**
- Modify: `.github/scripts/hyper_config.py` (module docstring keys list, `HyperConfig.__init__`, `shell()`, `main()` json)
- Test: `.github/scripts/test_hyper_config.py`

**Interfaces:**
- Produces: `HyperConfig.wasm_toolchain: str` (`'stable'` by default); shell variable `WASM_TOOLCHAIN`; json key `wasm_toolchain`.

- [ ] **Step 1: Write the failing tests**

```python
def test_wasm_toolchain_defaults_to_stable_and_reaches_shell_and_json(self):
    self.assertEqual(hyper_config.HyperConfig(MINIMAL).wasm_toolchain, 'stable')
    pinned = {**MINIMAL, 'wasm': {'toolchain': 'nightly-2026-10-27'}}
    self.assertEqual(hyper_config.HyperConfig(pinned).wasm_toolchain, 'nightly-2026-10-27')
    write_config(self.root, pinned)
    # shell: `echo $WASM_TOOLCHAIN` prints nightly-2026-10-27; json: data['wasm_toolchain'] == same

def test_wasm_toolchain_must_be_stable_or_a_dated_nightly(self):
    for bad in ('nightly', 'beta', 'nightly-2026-1-2', 1, None):
        with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, 'wasm.toolchain must be stable or nightly-YYYY-MM-DD'):
            hyper_config.HyperConfig({**MINIMAL, 'wasm': {'toolchain': bad}})
```

- [ ] **Step 2: Run them to verify they fail**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_hyper_config.py' -v` (from `../HyperForge`)
Expected: FAIL, `AttributeError: 'HyperConfig' object has no attribute 'wasm_toolchain'`

- [ ] **Step 3: Implement** `wasm_toolchain` in `HyperConfig.__init__` (validated with `^(stable|nightly-\d{4}-\d{2}-\d{2})$`), `WASM_TOOLCHAIN=` in `shell()`, `wasm_toolchain` in the json dump, and the key in the module docstring.

- [ ] **Step 4: Run the whole Forge suite**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`
Expected: OK

### Task 2: The archive guard

**Files:**
- Create: `.github/scripts/wasm_archive_guard.py`
- Test: `.github/scripts/test_wasm_archive_guard.py`

**Interfaces:**
- Produces: CLI `python3 wasm_archive_guard.py ARCHIVE --llvm-bin DIR`; exit 0 and `clean: N members, M functions` on stdout, or exit 1 with one `::error::` line per violation, naming member and instruction or feature. Functions `eh_instructions(archive, llvm_bin) -> dict[str, Counter]` and `forbidden_features(archive) -> dict[str, set[str]]`; any inspection failure raises `GuardError`, which `main` turns into exit 1.

- [ ] **Step 1: Write the failing tests**, one per fixture, each building its object in `setUp` with `clang --target=wasm32-unknown-unknown -c` (C, or `.s` for instructions C cannot reach) and `llvm-ar rc`. No skips: the tests fail if `clang`/`llvm-objdump`/`llvm-ar` are missing (validate.yml installs `llvm clang lld`).

| Test | Fixture | Expected |
| --- | --- | --- |
| `test_clean_object_passes` | `int f(int x){return x+1;}`, `-mcpu=mvp` | exit 0 |
| `test_throw_fails` | `.s` with `-mexception-handling`, a tag and `throw` | exit 1, message names `throw` |
| `test_legacy_try_catch_all_fails` | `.s` with `try` / `catch_all` / `end_try` | exit 1, names `try` and `catch_all` |
| `test_try_table_fails` | `.s` with `try_table` | exit 1, names `try_table` |
| `test_each_forbidden_feature_fails` | C built with `-mbulk-memory` (adds `bulk-memory-opt`), `-mreference-types` (adds `call-indirect-overlong`), `-mexception-handling` | exit 1, each names its feature; subTest per flag |
| `test_empty_archive_fails` | `llvm-ar rc` with no members | exit 1, `no members` |
| `test_non_wasm_member_fails` | a text file archived | exit 1, inspection error |
| `test_duplicate_member_names_are_each_inspected` | `llvm-ar q` two members both named `same.o`, the first clean, the second built with `-mbulk-memory` | exit 1, names `bulk-memory-opt` and the second occurrence (`same.o#2`) |
| `test_member_without_target_features_fails` | a wasm object with the section stripped (`llvm-objcopy --remove-section=target_features`) | exit 1, `no target_features` |

- [ ] **Step 2: Run them to verify they fail**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_wasm_archive_guard.py' -v`
Expected: FAIL, `ModuleNotFoundError: wasm_archive_guard`

- [ ] **Step 3: Implement** `wasm_archive_guard.py`: members read **by occurrence** with a small Python `ar` reader (global header, 60-byte member headers, the GNU `//` long-name table, symbol tables `/` and `/SYM64/` skipped), each written to its own temp file named `<name>#<occurrence>`, never `llvm-ar x` (it overwrites same-named members); instructions from `llvm-objdump -d` (mnemonic is the first token after the encoded bytes; a disassembly with zero `<name>:` function headers is a `GuardError`); features by parsing the `target_features` custom section (LEB128 count, then prefix byte, LEB128 length, name). Every `subprocess.run` uses `check=True`.

- [ ] **Step 4: Run the guard tests, then the Forge suite**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`
Expected: OK

- [ ] **Step 5: Prove it on the real archives** (local, emsdk's tools)

Run: `python3 .github/scripts/wasm_archive_guard.py <HyperUuid>/rust/target/wasm32-unknown-emscripten/release/libhyperuuid.a --llvm-bin ~/emsdk/upstream/bin`
Expected: exit 1, 40 `try` and 40 `catch_all` reported, plus `bulk-memory-opt`, `call-indirect-overlong`, `exception-handling`. The build-std archive from Task 4 exits 0.

### Task 3: `hyper-build-wasm.yml`, keyed on the pin

**Files:**
- Modify: `.github/workflows/hyper-build-wasm.yml` (header comment, `build-wasm` job, `test-blazor` job)
- Test: `.github/scripts/test_forge_producers.py` (or the file that already asserts this workflow's shape)

**Interfaces:**
- Consumes: `WASM_TOOLCHAIN` (Task 1), `wasm_archive_guard.py` (Task 2).
- Produces: in a pinned repo, a guarded build-std archive under the same artifact name and path as today; `test-blazor` with the .NET 10 runtime and `wasm-tools-net10`.

- [ ] **Step 1: Write the failing contract test**: `build-wasm` checks out the executing Forge revision as `.hyperforge` (the `hyper-build-wheels.yml` pattern: validate `job.workflow_sha`, then checkout); a step reads `WASM_TOOLCHAIN` into an output; the toolchain step installs `${{ steps.config.outputs.toolchain }}` with `components: rust-src` and `targets: wasm32-unknown-emscripten`; the build runs `cargo +"$WASM_TOOLCHAIN" wasm-staticlib`; the guard step has `if: steps.config.outputs.toolchain != 'stable'` and runs `wasm_archive_guard.py ... --llvm-bin "$EMSDK/upstream/bin"`; in `test-blazor`, the .NET 10 runtime (`dotnet-version: 10.0.x`) and `dotnet workload install wasm-tools-net10` steps carry the same `!= 'stable'` condition (via a `needs.build-wasm.outputs.toolchain` output).

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`
Expected: FAIL on the new assertions

- [ ] **Step 3: Implement** the workflow changes; rewrite the header paragraph on the consumer-side translation ("a repo that pins a nightly ships an archive with none, checked by the guard; one that does not keeps today's build"). `dtolnay/rust-toolchain` stays at the pinned commit, with its `toolchain:` input taking the pin.

- [ ] **Step 4: Verify**

Run: `python3 -m unittest discover -s .github/scripts -p 'test_*.py'` and `actionlint -shellcheck= -ignore 'property "workflow_(sha|repository)" is not defined'`
Expected: OK, no actionlint findings. CI-only: the workflow runs for real only when a consumer calls it (Task 6 onward).

---

## HyperUuid

### Task 4: The archive: pin, rustflags, alias

**Files:**
- Modify: `rust/.cargo/config.toml` (`[target.wasm32-unknown-emscripten]` and its comment; `wasm-staticlib` alias and its comment)
- Modify: `.github/hyper.json` (add `"wasm": { "toolchain": "nightly-2026-10-27" }`)
- Modify: `CLAUDE.md` ("Toolchains and floors": the nightly as the one commented exception; "Local loop": `rustup toolchain install <pin> --component rust-src`, then `cargo +<pin> wasm-staticlib`)

**Interfaces:**
- Produces: `cargo +<pin> wasm-staticlib` → `rust/target/wasm32-unknown-emscripten/release/libhyperuuid.a`, guard-clean.

- [ ] **Step 1: Verify the failing state**: the guard (Task 2) on today's stable archive exits 1.
- [ ] **Step 2: Implement** the config, alias and pin. Comments say why each flag is there (no unwinding sysroot, no legacy EH, no feature .NET 10's toolchain rejects) and that the section only applies to cargo runs from `rust/` (the Pyodide wheel builds from `python/`).
- [ ] **Step 3: Verify**

Run: `cd rust && source ~/emsdk/emsdk_env.sh && cargo +nightly-2026-10-27 wasm-staticlib && python3 ../../HyperForge/.github/scripts/wasm_archive_guard.py target/wasm32-unknown-emscripten/release/libhyperuuid.a --llvm-bin ~/emsdk/upstream/bin`
Expected: no rustc warnings; guard exit 0; archive about 350 KB. Control: `cargo wasm-staticlib` on stable stops with cargo's `-Z` nightly-only error.
- [ ] **Step 4: Check the target section reaches nothing else:** `cargo cdylib` and `cargo test` in `rust/` still pass on stable.

### Task 5: The targets file and its CI gate

**Files:**
- Modify: `csharp/HyperUuid/build/net10.0/HyperUuid.targets` (gate: Blazor branch and `_HyperUuidNet10Suffix` removed; exnref `ItemGroup` and its comment removed; folder comment gains "linked by .NET 10 and 11"; top comment's "refuses the platforms that need .NET 11" list loses Blazor)
- Modify: `.github/workflows/ci.yml:773-800` (step "Blazor WebAssembly needs .NET 11"), `ci.yml:130`, `ci.yml:649` comments
- Test: `.github/scripts/tests/test_workflow_contracts.py`

**Interfaces:**
- Produces: `_HyperUuidNet10Unsupported` empty for `browser-wasm` on net10.0 and net11.0.

- [ ] **Step 1: Write the failing contract test**: the step is named `The gate admits Blazor WebAssembly on .NET 10 and 11`; for each of `net10.0` and `net11.0` it asserts `_HyperUuidNet10Unsupported` evaluates empty with `-p:RuntimeIdentifier=browser-wasm`, and that a build with `-p:WasmNativeWorkloadAvailable=false` logs `error HYPERUUID002` and no `HYPERUUID003`. No step still asserts a net10 browser `HYPERUUID003`.
- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest discover -s .github/scripts/tests -v`
Expected: FAIL on the new step name

- [ ] **Step 3: Implement** the targets and `ci.yml` changes.
- [ ] **Step 4: Verify locally**

Run, for `GateFramework` in `net10.0 net11.0`: `dotnet msbuild csharp/HyperUuid.PlatformGateTest/HyperUuid.PlatformGateTest.csproj -getProperty:_HyperUuidNet10Unsupported -p:GateFramework=$f -p:RuntimeIdentifier=browser-wasm` → empty; then the `WasmNativeWorkloadAvailable=false` build → `HYPERUUID002` only. Control: `-p:GateFramework=net10.0-android` → `HYPERUUID003`. Then the contract tests and actionlint pass.

### Task 6: The Blazor smoke project and `check.sh`, four legs

**Files:**
- Modify: `csharp/HyperUuid.WasmSmokeTest/HyperUuid.WasmSmokeTest.csproj` (`TargetFrameworks` `net10.0;net11.0`; per-framework `PackageReference`s as in the spec; comment)
- Modify: `csharp/HyperUuid.WasmSmokeTest/check.sh`

**Interfaces:**
- Consumes: `.github/hyper.json` `wasm.toolchain` (Task 4).
- Produces: `check.sh` with legs `-c Release -f net11.0`, `-c Debug -f net11.0 -p:WasmNativeStrip=false`, `-c Release -f net10.0`, `-c Debug -f net10.0 -p:WasmNativeStrip=false`, each from clean, each requiring `<title>PASS</title>`; legs are selectable by name as arguments (`net11-release net11-debug net10-release net10-debug`, all by default); it reads the pin with `python3 -c` from `.github/hyper.json` (or `stable`) and runs `cargo +<pin> wasm-staticlib` unless `STATICLIB` is set.

- [ ] **Step 1: Verify the failing state**: with the stable archive staged (`STATICLIB=` the stable build), the new `check.sh` fails at the net11 Debug leg.
- [ ] **Step 2: Implement** the csproj and `check.sh` (one `run` function per leg, `${here:?}` in every `rm -rf`, the publish arguments echoed before each leg's results).
- [ ] **Step 3: Verify**

Run: `CHROMIUM=<playwright chromium> csharp/HyperUuid.WasmSmokeTest/check.sh`
Expected: four `ALL WASM CHECKS PASSED`, exit 0. Controls with `STATICLIB=<stable archive>`, **one invocation per leg**, each asserting its own failure:
  - `net10-release`: exit non-zero, publish output contains `Unknown option '--enable-bulk-memory-opt'`.
  - `net11-release` and `net11-debug`: publish succeeds, the page is not PASS, and `WebAssembly.compile` of that leg's published `dotnet.native*.wasm` in the same Chromium reports `mix of legacy and new exception handling instructions` (a small compile-probe page, scratchpad).
  - `net10-debug`: record the outcome; assert nothing. Try the net10 legs without `WebAssembly.DevServer`; keep it only if they fail without it, with the reason in a comment. `shellcheck` clean.

### Task 7: HyperUuid's SDK 10 packaged consumer, and docs

**Files:**
- Modify: `csharp/README.md` (Platform support: Blazor on .NET 10 and later; the translate-to-exnref paragraph replaced by the archive sentence; `HYPERUUID003` paragraph drops WebAssembly), `README.md` (platform support floor sentences), `CLAUDE.md` (.NET floor list: `HYPERUUID003` scope and the renamed gate step; Open follow-ups: drop the `HYPERUUID003`/#132858 item, add the nightly pin), `CHANGELOG.md` (`## [Unreleased]`: the archive and .NET 10 Blazor, closes #50)
- Local proof only (scratchpad): an SDK 10 consumer

- [ ] **Step 1: Pack** `dotnet pack csharp/HyperUuid/HyperUuid.csproj -c Release -o <feed> -p:Version=<base>-local.1` with the build-std archive staged; record its archive's `sha256`.
- [ ] **Step 2: SDK 10 consumer** in a scratch dir outside the repo's `global.json`, with its own `global.json` pinning `10.0.403`; log `dotnet --version`. A net10.0 Blazor app with `PackageReference` to the packed version (no project references, no manual imports), plus a variant reaching HyperUuid through a net10.0 class library (Review Focus 2). Publish and boot both in Release and in `-c Debug -p:WasmNativeStrip=false`.
Expected: PASS for all four, each restored into a fresh `NUGET_PACKAGES` with the restored archive's `sha256` matching Step 1. Control: a separate pack (`-local.2`) with the archive deleted (`zip -d`), restored fresh: the app fails at link.
- [ ] **Step 3: Docs** as listed; README examples verified by hand.
- [ ] **Step 4: Final HyperUuid sweep**: `.github/scripts/tests`, actionlint, `dotnet test` from `csharp/HyperUuid.Tests` (both TFMs), `check.sh`, and `git status` lists only this plan's files.

---

## HyperCast (`../HyperCast`)

### Task 8: HyperCast's archive, targets and `check.sh`

**Files:** the HyperCast counterparts of Tasks 4–6: `rust/.cargo/config.toml`, `.github/hyper.json`, `csharp/HyperCast/build/net11.0/HyperCast.targets` (exnref item at `:76` removed), `csharp/HyperCast.WasmSmokeTest/HyperCast.WasmSmokeTest.csproj`, `csharp/HyperCast.WasmSmokeTest/check.sh`, `CLAUDE.md` toolchain and local-loop lines.

- [ ] **Step 1: Verify the failing state**: the guard fails on HyperCast's stable archive.
- [ ] **Step 2: Implement** as in Tasks 4 and 6, net11.0 legs only for now (Task 9 adds net10.0).
- [ ] **Step 3: Verify**: guard exit 0 on `libhypercast.a`; `check.sh` net11 Release and Debug-unstripped PASS; stable-archive control fails the Debug leg.

### Task 9: HyperCast's .NET 10 floor

**Files:**
- Modify: `csharp/Directory.Build.props` (`TargetFrameworks` `net10.0;net11.0`; comment rewritten: unions compile for net10.0 through `Polyfill.cs`; CS8509 stays an error)
- Move: `csharp/HyperCast/build/net11.0/` → `build/net10.0/`, `buildTransitive/net11.0/` → `buildTransitive/net10.0/` (the buildTransitive import path updated); every smoke csproj's `Import`
- Modify: `HyperCast.targets` (add the `HYPERCAST003` gate, the same shape as `HYPERUUID003` without a Blazor branch)
- Modify: `csharp/HyperCast/HyperCast.csproj` (`EnablePackageValidation`, `EnableStrictModeForCompatibleFrameworksInPackage`)
- Modify: `.github/workflows/ci.yml` (`csharp_aot_frameworks: "net10.0 net11.0"`, `csharp_extra_runtime: "10.0"`; gate steps for `net10.0-android`, `-ios`, `-maccatalyst` as in HyperUuid's `ci.yml:442-460, 568-580`), `.github/scripts/tests/test_workflow_contracts.py`
- Create: `csharp/HyperCast/Polyfill.cs` (the spec's block, verbatim), so the unions compile for net10.0
- Modify: the Android and Apple smoke projects only if their framework lists come from `Directory.Build.props` (they pin `net11.0-*` and stay)

Follow HyperUuid's `docs/superpowers/plans/2026-10-28-csharp-net10.md` Tasks 1–3 and 5 for the targets move, the gate, the packed-package layout check and the CI probe steps, with HyperCast's names.

- [ ] **Step 1: Write the failing contract test** for the new gate steps and inputs.
- [ ] **Step 2: Implement** the move, gate, validation and CI inputs.
- [ ] **Step 3: Verify**: `dotnet build` both TFMs (control: without `Polyfill.cs`, net10.0 fails CS0246); `dotnet test` from `csharp/HyperCast.Tests` on both; `dotnet pack` passes strict package validation; nupkg has `lib/net10.0`, `lib/net11.0`, `build/net10.0`, `buildTransitive/net10.0`, no `build/net11.0`; a net9.0 restore fails NU1202; `net10.0-android` stops with `HYPERCAST003`; `check.sh` gains the two net10.0 legs and all four PASS.

### Task 10: `Verdict<T>` on .NET 10

**Files:**
- Modify: `csharp/HyperCast/Verdict{T}.cs` (`Match`, `Switch`, remarks on which path each consumer has)
- Test: `csharp/HyperCast.Tests/VerdictMatchTests.cs`

**Interfaces:**
- Produces: `public TResult Match<TResult>(Func<Success<T>, TResult> success, Func<Fault, TResult> fault)` and `public void Switch(Action<Success<T>> success, Action<Fault> fault)` on `Verdict<T>`, both frameworks.

- Consumes: Task 9's multi-targeted build and `Polyfill.cs`.

- [ ] **Step 1: Write the failing tests** (xunit and Shouldly, as the suite):

```csharp
[Fact] public void Match_calls_success_for_a_success() => Cast.Int32("42", NumFormat.Invariant).Match(s => s.Value, f => -1).ShouldBe(42);
[Fact] public void Match_calls_fault_for_a_fault() => Cast.Int32("abc", NumFormat.Invariant).Match(s => CastFailure.Unspecified, f => f.Reason).ShouldBe(CastFailure.Malformed);
[Fact] public void Switch_runs_exactly_one_action() { /* counters: success 1, fault 0 for "42"; reversed for "abc" */ }
[Fact] public void Match_and_Switch_throw_on_a_defaulted_verdict() { /* default(Verdict<int>): both throw InvalidOperationException; neither delegate ran */ }
[Fact] public void Match_and_Switch_reject_null_delegates() { /* ArgumentNullException, parameter names success / fault */ }
```

- [ ] **Step 2: Run them to verify they fail**: `dotnet test` from `csharp/HyperCast.Tests` → compile error, `Match` not defined.
- [ ] **Step 3: Implement** `Match` and `Switch` (switch on `_state`; `ArgumentNullException.ThrowIfNull` on each delegate; `InvalidOperationException` naming `default(Verdict<T>)` for `State.Default`), with XML docs.
- [ ] **Step 4: Run the suite on both TFMs**: PASS.
- [ ] **Step 5: Local proofs** (scratchpad):
  - **Repack** HyperCast now (`-local.<n>`), so every later test consumes `Match`/`Switch`; record hashes.
  - **Compiled dependency:** a net10.0 class library calling `Match`, `Switch` and a union switch (`LangVersion preview`), built against the packed HyperCast's net10.0 assembly; a net11.0 console app referencing that built DLL and the package; runs, no `MissingMethodException`.
  - **Its control, in two parts.** First, with `Match` removed from net11.0 only (temporary `#if`), `dotnet pack` must **fail strict package validation** (CP0002 naming `Match`). Then, for that isolated fixture alone, pack with `-p:EnablePackageValidation=false` under its own version, restore fresh, and the same app must throw `MissingMethodException`. The temporary `#if` is removed before anything else runs.
  - **SDK 10 consumer:** as Task 7 Step 2, with HyperCast: a net10.0 console app on SDK 10.0.403's default language version compiles and runs constructors, `TryGetValue`, `Match`, `Switch`, and gets `CS8652` ("The feature 'unions' is currently in Preview and *unsupported*") on a union switch (on SDK 11 with `LangVersion` 14 the same line is `CS9327`); then a net10.0 Blazor app boots in Release and Debug-unstripped.
  - **Reflection:** the net11.0 `HyperCast.Managed.dll` defines no `System.Runtime.CompilerServices.UnionAttribute` or `IUnion`.

### Task 11: HyperCast docs

**Files:** `csharp/README.md` (floor .NET 10 with the platform split; consumers without C# 15: `Match`, `Switch`, `TryGetValue`; name both diagnostics (SDK 10: `CS8652`, unions preview and unsupported; SDK 11 with an older `LangVersion`: `CS9327`) and that C# 15 needs SDK 11; the archive sentence in place of translate-to-exnref), `README.md` (floor sentences), `CLAUDE.md` (.NET floor list, as HyperUuid's), `CHANGELOG.md` (`## [Unreleased]`: .NET 10 floor, `Match`/`Switch`, the archive).

- [ ] **Step 1: Write** the docs; verify README examples by hand against the packed package.
- [ ] **Step 2: Sweep**: `.github/scripts/tests`, actionlint, `dotnet format --verify-no-changes`, `git status` lists only this plan's files.

---

## HyperTabular (`../HyperTabular`)

### Task 12: HyperTabular's archive, targets and `check.sh`

As Task 8, with `--lib` kept in the alias and the exnref item at `HyperTabular.targets:52` removed. Its Blazor smoke links both `libhypercast.a` (from the HyperCast package it references) and `libhypertabular.a`.

- [ ] **Step 1: Verify the failing state**: guard fails on HyperTabular's stable archive.
- [ ] **Step 2: Implement.**
- [ ] **Step 3: Verify**: guard exit 0; `check.sh` net11 legs PASS with HyperCast's build-std archive in the referenced package (the HyperCast repacked in Task 10, restored into a fresh `NUGET_PACKAGES`, its `libhypercast.a` hash checked): no duplicate-symbol error at link (Review Focus 1).

### Task 13: HyperTabular's .NET 10 floor and docs

As Tasks 9 and 11, with `HYPERTABULAR003`. `HyperTabular.csproj`'s `PackageReference Include="HyperCast"` moves to a HyperCast version carrying `lib/net10.0/`; locally, the version repacked in Task 10, from a local feed into a fresh `NUGET_PACKAGES`.

- [ ] **Step 1: Contract test** for the gate steps and inputs; FAIL.
- [ ] **Step 2: Implement** floor, gate, validation, CI inputs, docs, CHANGELOG.
- [ ] **Step 3: Verify**: both TFMs build and test; strict validation; nupkg layout; NU1202 on net9.0; `HYPERTABULAR003` on `net10.0-android`; `check.sh` four legs PASS; the SDK 10 Blazor consumer (Task 7 Step 2 shape) boots with HyperTabular and its transitive HyperCast.

### Task 14: Cross-repo sweep

- [ ] **Step 1:** In all four repos: tests, actionlint, formatters, and `git status` limited to this plan's files.
- [ ] **Step 2:** Each Emscripten pack's `wasm-opt` still matches its nupkg.
- [ ] **Step 3:** Report per repo: what ran locally, what only CI proves (the pinned `build-wasm` and `test-blazor` jobs, the gate steps on GitHub runners), and the pin date chosen.
