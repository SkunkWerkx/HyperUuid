# Phase 1 integration note (controller, 2026-10-28)

Where two or more specs touch the same lines. Every one of them is a clause-level edit, so they
merge by hand without conflict. Give one plan, or one integrator, the job of editing each file
below in a single pass.

## This repo

| File | Spots | Items |
| --- | --- | --- |
| `README.md` | :165 floors sentence | Rust (1.85→1.89), PHP (8.2→8.3), C# (add .NET 10, drop it from "sit above"), Java (JDK 21 clause) |
| `README.md` | :68 benchmark-machine sentence | C# (measured on .NET 10), Java (Java joins the M6 rows) |
| `README.md` | :398–:399 project tree | C# (:398), Java (:399 and new `java/jni/` line) |
| `README.md` | :155, :88, :173, :190, :195, :229, :404 | Rust :155; Python :88/:173/:190/:195/:229; PHP :404 (one owner each) |
| `ci.yml` `check-core` `with:` | | Rust `extra_features`, `no_panic_command`; Python `big_endian`; Java `lint_java_jni` |
| `ci.yml` `build-native` `with:` | | C# `csharp_aot_frameworks`, `csharp_extra_runtime`; Java `java_jni`, `jdk_floor_version`; PHP `php_floor_version: "8.3"` |
| `ci.yml` jobs | | C#: new `test-nuget-package`, steps in `test-android`/`test-apple-mobile`, `request-staging` needs |
| `test_workflow_contracts.py` | | C# (new job), Java (four inputs) |
| `CHANGELOG.md` Unreleased | | Rust ×2, Python ×1, PHP ×1, C# ×1, Java ×2 |
| `ROADMAP.md` | one row each | Rust :59, PHP :15, C# row, Java :12 |
| `CLAUDE.md` "Lists that move together" | new bullets | Python sdist, PHP floor, .NET floor, JDK floor; Java also extends the C-export bullet |
| `CLAUDE.md` other | | C# open follow-up; Java :94 fail-fast, :131 local loop, :184, :196, :200 |
| `dependabot.yml` | | Java only (`/java/jni`) |

## The Forge (one PR, merged first, every input off by default)

| File | Items |
| --- | --- |
| `hyper-build-native.yml` | C# (AOT `-f` loop, extra runtime, musl call), Java (JNI steps ahead of TypeScript, osx-x64, musl). PHP no longer touches the Forge: its default stays 8.2 for the siblings |
| `musl-suite/*` | C# (`csharp)` case), Java (new `java-jni` binding) |
| `hyper-check-core.yml` | Python (`test-big-endian`), Java (`lint-java-jni`); both are new jobs appended |
| `hyper_config.py` | Python (`sdist`), Java (`JNI_LIB_NAMES`) |
| pack and publish | Python (verify-wheels, inventory, metadata, notes), Java (maven pack and publish, prepare-maven-package) |

## Order

1. The Forge PR.
2. The Rust MSRV bump, with the `as_chunks` rewrites. The Python sdist's floor proof needs 1.89.
3. Then the rest, in any order.
4. Apple entropy, then the system-wide M6 receipts pass last: every table, one machine, one window.
5. A release-mode board on master before the tag proves:
   - the sdist (18 PyPI files);
   - the two-TFM nupkg;
   - the eight `jni-{rid}` attestations;
   - osx-x64.

## Fifth item: Apple entropy (added 2026-10-28)
`docs/superpowers/specs/2026-10-28-apple-entropy-design.md`. It touches `rust/src/entropy.rs` only, plus
one bullet each in the CHANGELOG and CLAUDE.md's open follow-ups. It lands after the v7 re-seed and
before the system-wide M6 receipts pass. The integrator strikes the Java spec's "macOS getentropy
dominates" sentence and its audit row.

## External review, 2026-10-28
Eight findings, each verified against the code and folded in:
1. **Java:** eager `RegisterNatives` with a count taken from the table.
2. **Java:** critical regions bounded at 64 KiB, larger fills through `malloc`, and `-Xcheck:jni`.
3. **Java:** a null array access raises `OutOfMemoryError` explicitly.
4. **Java:** the FFM gate runs old and new Java on one recorded core artifact.
5. **PHP:** the Forge default stays 8.2; ci.yml passes 8.3 in the same commit.
6. **Python:** promotion needs the full suite and the GIL off after import.
7. **Apple:** the fork test advances state before forking, with an inherited-state negative control.
8. **Rust:** separate controls for the no_std compile proof and for dependency isolation.

Also: **C#'s `HYPERUUID003` ordering claim** is narrowed to the case with the workload installed.

## Plans (written 2026-10-28)
- 1 Rust: `docs/superpowers/plans/2026-10-28-rust-uuid1-msrv.md`. Done, in the tree, uncommitted.
- 2 Python/PHP: `…/2026-10-28-python-sdist-php83.md`. Forge work is in Tasks 3-5.
- 3 C#: `…/2026-10-28-csharp-net10.md`. Forge work is in Task 4. Tasks 2 and 6 ship together after the Forge merges, because either alone hits NETSDK1129.
- 4 Java: `…/2026-10-28-java21-jni.md`. Forge work is in Task 8.
- 5 Apple entropy: `…/2026-10-28-apple-entropy.md`. It needs no Forge change.

The Forge tasks of plans 2, 3 and 4, together with plan 1's `forge-gate.patch`, make up the one Forge PR, merged first. Each plan's repo-only tasks run before it. Each plan's wiring task (`ci.yml`, `hyper.json`) runs after the merge, because `ForgeInterfaceTests` reads `../HyperForge`.
