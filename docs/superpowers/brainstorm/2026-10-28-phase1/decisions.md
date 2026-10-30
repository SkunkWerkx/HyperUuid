# Phase 1 owner decisions (2026-10-28)

## Rust uuid interop
- **MSRV bump.** The MSRV may rise to follow the widely adopted `uuid` floor. `uuid` 1.29.0
  declares `rust-version = "1.89.0"` (checked in its Cargo.toml on 2026-10-28), so ours goes
  from 1.85 to 1.89 and the `uuid1` feature targets current `uuid` 1.x. The 1.26.1 lock pin
  is not kept.
  - **The single source.** `rust/Cargo.toml` holds the MSRV (`rust-version`). The Forge's
    MSRV job reads it from there (hyper-check-core.yml:481-487).
  - **Siblings.** HyperCast and HyperTabular are both at 1.88 today. "Floors move together"
    → proposed: raise both siblings to 1.89 in the cascade, which also closes the existing
    1.85/1.88 gap. To confirm with the owner when the spec is reviewed.

## Small floors (owner, 2026-10-28)
- **Ruby 3.3: skip.** It stays on Fiddle. The Magnus grid is unchanged (3.4 and 4.0). No Ruby work in Phase 1.
- **Python sdist: ship it.** Use the brief's recommended shape: built in the Forge's hyper-build-wheels.yml as one more inventory file, opt-in through hyper.json. The brief's open questions still need answers in the spec: the free-threaded 3.13/3.14 and PyPy fall-through, s390x correctness, and the first-publish risk.
- **PHP: floor 8.2 → 8.3 now.** The owner's reasoning: real adoption is 2027, past 8.2's EOL on 2026-12-31.
- **sdist cascade: Uuid first.** HyperCast and HyperTabular stay wheels-only until HyperUuid's 1.1.0 has shipped an sdist through a real release, then they follow in their next release.
- **sdist wording.** Keep "no compiler needed" for platforms with a wheel, and add: platforms without a wheel build from the sdist and need Rust >= 1.89.
- **PHP timing.** The floor moves in 1.1.0.

## Sibling repos (owner, 2026-10-28), which overrides the cascade notes above
- **No cascade until v1.1 ships.** Nothing goes to HyperCast or HyperTabular until HyperUuid v1.1 is released and its roadmap is delivered. They are archives for now and get their own roadmaps afterwards.
- **What this cancels.** It drops the sibling MSRV bump to 1.89 and the "sdist cascade after 1.1.0" note, and the Phase 1 specs list no sibling work. CLAUDE.md's "floors move together" is suspended until then.

## Guiding criterion for every Phase 1 spec (owner, 2026-10-28)
Ubiquity and availability, and being nitpick-proof under scrutiny. Each spec says what reach it buys and where a hostile reviewer could still poke a hole, and it closes or labels each one.

## C# (owner, 2026-10-28)
- **Approach A: multi-target `net10.0;net11.0`.** This covers the library, the tests and the AOT smoke test.
- **Blazor on net10: not now.** It gets a clear build error, `HYPERUUID003`, that names dotnet/runtime#132858. Blazor stays net11 and later, so the browser-wasm archive stays under `nativeassets/net11.0/` (Q2 follows from this: no Forge wasm change). Revisit when #132858 moves.
- **Apple: exclude net10.** The iOS and Mac Catalyst claims stay net11-only, so the README must say so explicitly.
- **Benchmarks: net11 only,** with the tables labelled .NET 11.
- **Siblings (Q5): moot,** since they are frozen until v1.1. CLAUDE.md gets no C# exception.
- **Must-haves from the brief:** a package-layout check that `build/net10.0/` exists, with a negative control, so AOT never silently falls back to the shared library; and the Forge's AOT publish gains `-f` and merges before the repo change.
- **net10 scope, refined by the owner.** net10 covers desktop and server only: Windows, Linux and macOS, framework-dependent and Native AOT. net10 WebAssembly and every MAUI platform (android, ios, maccatalyst) are out. Rationale: a WASM or MAUI shop is cutting-edge and runs current .NET. That also sidesteps .NET 10's Mono-based Android runtime; .NET 11's Android support is CoreCLR.
  - **Proposed for the spec:** the package's build targets raise HYPERUUID003 for a net10.0-browser, -android, -ios or -maccatalyst consumer. It is one diagnostic naming the platform and pointing at net11, so the gap fails loudly at build, never at runtime.

## North star (owner, 2026-10-28)
Twelve languages: Rust, C, C++, C#, Go, Java, PHP, Python, R, Ruby, Swift and TypeScript. C and C++ are ROADMAP Phase 3 (C first: a cbindgen header, pkg-config/CMake config and a stable versioned C ABI).

## Java (owner, 2026-10-28)
- **Floor 21 through a multi-release JAR.** The base compiles for 21 and uses JNI through a package-private `NativeCore`. The FFM code moves unchanged under META-INF/versions/25. Public API, exceptions and messages are identical on both paths.
- **JNI layer: a binding-owned no_std Rust cdylib in `java/jni/`,** like typescript/. The C exports, check_exports and other bindings' bytes stay unchanged.
- **Two libraries per RID in the JAR:** the attested core for FFM and GraalVM (unchanged), and the JNI library for 21–24.
- **Drop date: Temurin's JDK 21 end, 2029-12-31.** It is recorded the way Ruby 3.4's is.
- **Benchmarks: re-measure the whole Java README on the Apple M6 (Mac18,5)** and relabel it, with FFM (25) and JNI (21) side by side.
- **Siblings (Q1): moot,** since they are frozen until v1.1.
- **Android: out of scope.** ART has JNI, but it needs an AAR and an Android loader. That goes in the "later" notes.

## Release vehicle (owner, 2026-10-28)
- **Everything in 1.1.0, "the ubiquity release".** All of Phase 1 ships in 1.1.0 with the v7 re-seed and the first npm and CRAN publish: C# net10, Java 21, the Python sdist, PHP 8.3, and Rust uuid1 with MSRV 1.89. The release-mode board must prove every new artifact shape before the tag.

## Rust uuid1 (owner, 2026-10-28)
- **Scope:** `From` both ways, `From<&Uuid>` and cross-type `PartialEq`. No `unsafe`, and no dependency beyond `uuid`.
- **No-panic:** yes. `#[no_panic]` goes on the `From` bodies, the call sites go into examples/no_panic.rs, and ci.yml gets the `no_panic_command` override.
- **docs.rs:** the brief's recommendation stands by default: `features = ["uuid1"]`, `--cfg docsrs` and `doc(cfg)` banners.
- **Feature name: `uuid1`.** The dependency is declared `dep:uuid`, so `uuid1` is the only public name, and `uuid2` sits beside it when uuid 2.0 ships.

## Fact check (controller, 2026-10-28): R and CRAN under MSRV 1.89
CRAN machines' rustc, from eitsupi/cran-rust-version's 2026-10-25 snapshot
(https://github.com/eitsupi/cran-rust-version/releases/tag/2026-10-25), a community tracker:
| Machines | rustc |
| --- | --- |
| Debian | 1.97.1 |
| Fedora | 1.99.0 |
| macOS release | 1.93.0 |
| macOS oldrel arm64 | 1.91.1 (the lowest) |
| Windows | 1.92.0 |
Every flavour is ≥ 1.89, so the bump does not threaten the first CRAN submission. The Rust spec's "CRAN rustc unchecked" becomes "checked against the tracker, re-check before submitting".

## Answers on the specs (owner, 2026-10-28)
- **Python:** a real s390x QEMU leg (`test-big-endian`), not a label.
- **Java:** the floor stays at JDK 21. The READMEs give the platform reason first: Temurin ships no JDK 17
  for Windows arm64 or Alpine (musl) arm64 (Adoptium API, 2026-10-28), then 17's EOL on 2027-10-31.
  The osx-x64 Rosetta suites go in.
- **C#:** the tables are re-measured on .NET 11, not relabelled as .NET 10.
- **Benchmarks, system-wide: every table moves to the Apple M6 mini (Mac18,5).** Owner: "it is the
  always on dev powerhouse that drives this entire flow". One receipts pass, run last in 1.1.0 after all
  code lands (the v7 re-seed, the Java split, C# net10), covers every harness in every binding plus the
  root README, on one machine and in one window. It replaces the i9-11900H tables and the per-spec
  re-measures. CLAUDE.md's benchmark follow-up is rewritten to match. The existing rules stand: losses
  beside wins, Go stays the control group, nothing is softened.
