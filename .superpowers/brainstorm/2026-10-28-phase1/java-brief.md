# Java: JDK 21 floor via a JNI base in a multi-release JAR (design brief)

2026-10-28 · ROADMAP Phase 1, Java row (`ROADMAP.md:55`) · read-only research, no repo edits

## 1. Goal

A team on JDK 21 LTS, the release most regulated shops run today, cannot use `io.github.skunkwerkx:hyperuuid`, because the jar is compiled for 25 and its native path is FFM. After the change, one jar runs on 21 and later. On 21–24 it reaches the core through JNI, from the jar's base classes. On 25+ it uses today's FFM path, from `META-INF/versions/25`. Both paths share one public API, one set of exception types and messages, and one Rust core, so v5 and SQL-order bytes stay identical. It has to happen now because 21 is the floor where buyers are, and it is supported upstream until at least 2028-09 (Oracle Premier) or 2029-12 (Temurin). FFM in 21 is preview-only, so no preview flag could make it shippable.

## 2. Current state

- **Build.** Gradle 9.8.2 (`java/gradle/wrapper/gradle-wrapper.properties`). `--release 25` is set at `java/build.gradle.kts:151-165`, and the same floor is in `benchmarks/build.gradle.kts:17-25` and `aot-smoke-test/build.gradle.kts:26-34`. The manifest carries only `Automatic-Module-Name` (`build.gradle.kts:128-130`). The POM description says "JDK 25+" (`build.gradle.kts:194`).
- **Native path.** All of it is in `UuidGenerator.java`, the only file that imports `java.lang.foreign` (verified by grep):
  - `Downcalls` (`:79-148`) holds 10 signature-only handles, all `critical(true)` except the version probe.
  - `Core` (`:161-221`) extracts `/native/{rid}/{lib}` to a temp file and calls `SymbolLookup.libraryLookup` (`:213-216`). It resolves all 13 exports eagerly (`:172-184`), so a stale core fails `isAvailable()` (`:235-262`).
  - Per-thread scratch is at `:302-308`.
  - The rc→exception mapping is Java-side: `newV7` at `:450-458`, and `batchFailure`/`V7_COUNTER_EXHAUSTED` at `:997-1013`.
- **RID resolution** is pure Java with no FFM (`NativePlatform.java:55-94`). The public API exposes no FFM type: 43 `public static` members, none taking or returning `MemorySegment`.
- **Tests.** There are 4 JUnit classes, none using FFM. Exhaustion is tested with the exact message at `UuidGeneratorTest.java:600-647`. Tests run on the classes directory with `--enable-native-access` (`build.gradle.kts:140-149`).
- **Local dev loop.** The `stageNativeLibrary` Sync stages `../rust/target/release/libhyperuuid.*` (`build.gradle.kts:84-101`).
- **GraalVM.** `native-image.properties:5` initializes `UuidGenerator$Downcalls` at build time. `reachability-metadata.json` registers the 10 downcall signatures and the `native/*/*` resource glob.
- **Forge CI.**
  - `hyper-build-native.yml` takes one `jdk_version` input, default "25" (`:190-194`). Its JDK note is at `:96-105`, the Java test step (`gradlew test javadoc`) at `:905-940`, and the Alpine step at `:1628-1635`, which uses `musl-suite/suite-musl.sh:130-139` on `eclipse-temurin:25-jdk-alpine`.
  - `hyper-check-core.yml:715-726` runs spotless on 25.
  - The TypeScript addon is the precedent for a binding-owned cdylib: "its own cdylib in typescript/, path-depending on rust/ … never touches the core library the other suites load" (`hyper-build-native.yml:569-590`).
- **Forge pack and publish.**
  - `release.yml:127-142` calls `hyper-pack-maven.yml`, whose `jdk_version` defaults to 25 (`:21-25`). That runs `actions/prepare-maven-package`, which verifies each `native-{rid}` attestation and copies it into `java/src/main/resources/native/{rid}/` (`action.yml:70-99`), then runs setup-java 25 (`:101`) and `publishMavenPublicationToReleaseBundleRepository`.
  - Publishing is `release.yml:432-456`, through `hyper-publish-maven.yml`.
- **Export checks the JNI choice runs into.**
  - `check_exports.py:125-133` reads the Java site from `export\("(\w+)"\)` in `UuidGenerator.java`.
  - `hyper.json:4-8` sets patterns to `pub extern "C" fn` and prefixes to `uuid_` and `hyperuuid_`.
  - Forge `hyper-check-core.yml:345-362` holds the cdylib to **exactly** the source's C exports ("and nothing else").
  - Forge `release_guard.py:240-256` filters symbols by prefix (`hyper_config.py:201-202`) and then requires equality with `core_exports`.
- **The shared cdylib is no_std.** `cargo cdylib` builds it with `--no-default-features --features cdylib` and `panic=abort` (`rust/.cargo/config.toml` alias, `rust/Cargo.toml` `[features]`). The panic handler is at `rust/src/lib.rs:56-68`. It is about 26 KB on linux-x64.

## 3. Upstream facts (all checked 2026-10-28)

| Fact | Source |
|---|---|
| FFM was final in **JDK 22** (JEP 454, "Closed / Delivered"). In 21 it was the third preview (JEP 442). | https://openjdk.org/jeps/454 |
| Probed: `javac --release 21` on today's `UuidGenerator.java` fails with "Linker/ValueLayout/MemorySegment is a preview API and is disabled by default". JDK 21's `Linker.Option` has no `critical(boolean)`, only `isTrivial()` (javap, Temurin 21.0.13 vs 25.0.5). | local probe |
| **Oracle JDK:** 21 Premier support ends **2028-09-30**, Extended **2031-09-30**. 25 Premier ends **2030-09-30**, Extended **2033-09-30**. | https://endoflife.date/api/v1/products/oracle-jdk/ |
| **Temurin:** 21 EOL **2029-12-31**, 25 EOL **2031-09-30**, 17 EOL **2027-10-31** (17 is still supported, so "no 17" is a choice, not policy). | https://endoflife.date/api/v1/products/eclipse-temurin/ |
| **Corretto:** 21 EOL **2030-10-31**, 25 EOL **2032-10-31**. | https://endoflife.date/api/v1/products/amazon-corretto/ |
| **GraalVM CE for JDK 21 reached EOL on 2024-03-19.** GraalVM for 25 is maintained (25.4.4.1). Oracle GraalVM for 21 dates: **UNVERIFIED**. | https://endoflife.date/api/v1/products/graalvm/ |
| How an MRJAR picks a class: a runtime of major N searches `META-INF/versions/N`, then lower versions down to 9, then the root. Higher N is never searched. `Multi-Release: true` is required. The public API "must be exactly the same across versions". A versioned class in a non-exported package, or a package-private one, is exempt. | https://docs.oracle.com/en/java/javase/21/docs/specs/jar/jar.html |
| **Probed: Gradle builds an MRJAR with one JDK 25 toolchain.** The setup is a second source set with `options.release = 25`, main with `release = 21`, and `jar { into("META-INF/versions/25") }`. The results: `jar --validate` exits 0, the class versions are 65 and 69, and Gradle Module Metadata says `org.gradle.jvm.version: 21`. On Temurin 21 the jar chose the base (JNI); on 25 it chose the versioned class (FFM). Gradle has no first-class MRJAR doc. The user guide only covers the pieces (`release`, custom source sets). | https://docs.gradle.org/current/userguide/building_java_projects.html, local probe |
| **JEP 472 (JDK 24): `System.load` and native-method binding warn without `--enable-native-access`. One flag covers JNI and FFM alike.** Probed: JNI on 25 printed the 4-line `System::load` warning, and on 21 it printed nothing. `--illegal-native-access=deny` blocked it (`ExceptionInInitializerError`). | https://openjdk.org/jeps/472, local probe |
| Maven Central requires sources and javadoc jars, `.md5`/`.sha1`, `.asc` on every file, and the POM fields. It has **no rule on native libraries, classifiers or MRJARs**, and Central does not evaluate jar contents. Today's jar already ships 8 natives. | https://central.sonatype.org/publish/requirements/ |
| GraalVM JNI: metadata is needed only for items native code looks up by name (FindClass, GetMethodID). Native-declared methods get linkage automatically. Coverage of `System.load` from a temp path is not stated (**UNVERIFIED**; it is moot, see §4). | https://www.graalvm.org/latest/reference-manual/native-image/dynamic-features/JNI/ |
| `jni` 0.22.4 is std-only (thiserror, combine, log, libloading). `jni-sys` 0.4.1 is `#![no_std]` (read from the crate sources). | crates.io API, downloaded sources |
| Adoptium ships JDK 21 for win-arm64 (21.0.12.1, one patch behind) and Alpine x64/arm64, so the Microsoft-JDK exception at `hyper-build-native.yml:96-105` is not needed for 21. | https://api.adoptium.net/v3/assets/latest/21/hotspot |

**Performance probe** (Apple M6 Mac18,5, hand loop of 5×5M calls, the same `libhyperuuid.dylib`, round 5). This is indicative only, not a README receipt:

| Crossing | JNI on 21 | JNI on 25 | FFM `critical(true)` on 25 |
|---|---:|---:|---:|
| `uuid_variant` (pure crossing, 2 longs in) | 4.9 ns | 4.7 ns | **2.5 ns** |
| `newV7(long)` (macOS `getentropy` per id dominates) | 630 ns | 657 ns | 598 ns |
| fill `byte[16000]` (critical pin on both) | 19.3 µs | 18.9 µs | 18.0 µs |

- **Per-call cost.** JNI costs about 2.2–2.4 ns more per call than critical FFM.
- **Linux.** On Linux, where `newV7` is 35.8 ns (`java/README.md`, JMH table), that predicts roughly a 6–10% loss on single calls on 21. This is not measured, and a JMH receipt is required.
- **Batches.** Batches and the `byte[]` fill should be flat.

## 4. Approaches

Common to all three:
- **MRJAR design.** The public `UuidGenerator` stays in the base, compiled `--release 21`. It keeps all argument checks and rc→exception mapping, so messages are identical on both paths by construction. It calls a package-private `NativeCore` with the same name in both trees:
  - base: the JNI `native` methods;
  - `src/main/java25/` → `META-INF/versions/25`: today's `Downcalls`/`Core`/`Scratch`, moved verbatim.
- **Why same-name shadowing.** It satisfies the MRJAR API rule, because only a package-private class is versioned. It also keeps the JNI class invisible on 25, so GraalVM (on 25) only ever sees FFM.
- **Rejected: JNI-only, dropping FFM.** It would be simpler, but it pays about 2.3 ns a call on 25 too, and it discards the proven GraalVM FFM work.

**(a) JNI symbols in the core cdylib under a feature.**
- **How it works.** `rust/src/jni.rs` uses `jni-sys`; the `jni` crate is out because it would put std into the no_std library. It adds `Java_io_github_skunkwerkx_hyperuuid_NativeCore_*` functions, enabled by `cdylib`.
- **Effect on "Adding or removing a C export":**
  - Every shipped `libhyperuuid` gains about 13 `Java_` symbols: C#'s NuGet payload, Ruby's Fiddle library, and PHP's committed `php/src/native/`.
  - Forge `check-cdylib` fails ("nothing else").
  - Adding a `pub extern "system" fn (Java_\w+)` pattern to `hyper.json` puts the symbols into `core_exports`. `release_guard.check_symbols` then fails on every committed static archive and PHP lib, unless the `Java_` prefix is added and the static libs carry the symbols too. Either way, `hyper_config.py` or `release_guard.py` needs a second symbol class.
  - `check_exports.py` would need new sites for the Rust `Java_` names and the Java `native` declarations. `cdylib_smoke.py` cannot call `Java_` functions without a JNIEnv, so it needs a carve-out.
  - The no-panic proof extends naturally: `#[no_panic]` on each fn, linked from `examples/no_panic.rs`.
- **Natives.** None new. The jar keeps one library per RID, and both paths load it: the JNI path through `System.load`, the FFM path through `libraryLookup`.
- **Cost.** The smallest jar and no new Forge artifact. But it makes the C ABI that nine bindings, and the Phase 3 "versioned stable C contract", depend on also carry Java's ABI. It also changes PHP's committed binaries.

**(b) A small C shim over the C ABI (`java/jni/hyperuuid_jni.c`).**
- **How it works.** About 15 `JNIEXPORT` functions call `uuid_*`, built per RID.
- **Linking.** Static against the `cargo staticlib` archive the Forge already builds per leg (`static_targets`), so there is one file per RID. Linked dynamically instead, the loader must extract both files side by side, set `$ORIGIN`/`@loader_path` (probed: the shim pinned an absolute install name until `install_name_tool`), and on Windows load the core first.
- **Effect on export lists.**
  - The C export set is untouched: no `hyper.json`, `check-cdylib`, `release_guard`, or `cdylib_smoke` change.
  - `check_exports.py` gains a site for the shim's prototypes, `C_PROTOTYPE`-style and "all". There is no Rust panic question.
- **Natives.** A new `jni-{rid}` artifact on each of the 8 legs, needing a C compiler plus JNI headers per OS. That means MSVC or clang-cl on Windows, the osx-x64 cross-build, and Alpine.
- **Cost.** The leanest semantics, but it introduces C toolchains into the Forge's 8 legs. It is the only C the Forge would compile for a binding.

**(c) A binding-owned no_std Rust cdylib (`java/jni/`, the TypeScript precedent). Recommended.**
- **How it works.**
  - `java/jni/Cargo.toml` path-depends on `../../rust` with `default-features = false, features = ["cdylib"]`, which brings the core's panic handler and `rust_eh_personality` stub, plus `jni-sys` 0.4.
  - It declares the 13 `uuid_*` exports in an `extern "C"` block, so the return codes are byte-for-byte the C ABI's. It wraps them in `extern "system" fn Java_…`, and builds with `panic = "abort"` into `java/jni/target`.
  - Output: `libhyperuuid_jni.so`, `.dylib`, or `hyperuuid_jni.dll`, statically containing the core, about 30 KB each (**estimate**).
- **Effect on export lists.**
  - The C export set is untouched: no change to `hyper.json` prefixes, `check-cdylib`, `release_guard`, or `cdylib_smoke.py`.
  - `check_exports.py` gains two sites: "Java (JNI shim)" over the extern block ("all"), and "Java (JNI natives)" over `static native` in `NativeCore.java` (camel spelling). The FFM site's path moves to `src/main/java25/`.
  - Panic and no-panic: `#[cfg_attr(feature="no-panic", no_panic)]` on each `Java_` fn. JNI function-table entries are `Option<fn>`, matched and never unwrapped. A null `GetPrimitiveArrayCritical` returns a code, and the JVM's pending OOM surfaces on return.
  - Whether linking the cdylib itself discharges the no-panic proof is **UNVERIFIED**. The fallback is an `examples/no_panic.rs` in the shim crate, mirroring the core's.
- **Natives.** The Forge builds it on every leg right after the core, as it does `typescript/`: osx-x64 cross-built, musl in Alpine. It uploads and attests it as `jni-{rid}`. `prepare-maven-package` verifies and stages it beside `native-{rid}` into `/native/{rid}/`.
- **Loaders.**
  - JNI: `NativeCore` extracts `/native/{rid}/libhyperuuid_jni.*` and calls `System.load`, wrapped in a static holder exactly like `Core`. A version check (`hyperuuid_version` through JNI, compared to the jar's) restores eager staleness detection, which lazy JNI binding otherwise loses.
  - FFM is unchanged.
- **Cost.** One new crate, plus about 20 Forge edits that mirror the `node` addon steps. No C toolchain is needed: `jni-sys` supplies the types, so no JDK headers are required at build time. Windows has no dependent-DLL issue because the core is linked in.

**Recommendation: (c).** It is the only option that leaves the C ABI, its five check sites, and every other binding's shipped bytes exactly as they are. That matters most here: persisted bytes and the export contract are what this repo guards. It also reuses a pattern the Forge already runs (the napi addon) instead of adding C compilers to 8 legs. (a) is cheaper in jar bytes, but it puts Java's ABI into every binding's library and needs a Forge symbol-class redesign. (b) avoids Rust-panic reasoning, but buys a C toolchain matrix.

## 5. Touch list (recommended approach)

**This repo**
- [own] `java/jni/Cargo.toml`, `java/jni/src/lib.rs`, `java/jni/.cargo/config.toml` (new crate; `Cargo.lock` follows the `typescript/` rule)
- [own] `java/src/main/java/io/github/skunkwerkx/hyperuuid/UuidGenerator.java`: FFM moves out, it delegates to `NativeCore`, and the Javadoc becomes path-neutral
- [own] `java/src/main/java/.../NativeCore.java` (new, JNI) and `java/src/main/java25/.../NativeCore.java` (new, FFM)
- [own] `java/src/main/java/.../NativePlatform.java`: `Target` gains the JNI library name
- [own] `java/build.gradle.kts`:
  - `release = 21` and the `java25` source set;
  - `Multi-Release: true`, with `java25` sources in the sources jar;
  - `test` (25→ffm) and `testJdk21` (21→jni), both run **against the jar** with an expected-backend property;
  - staging for `java/jni/target/release`;
  - the POM text.
- [own] `java/benchmarks/build.gradle.kts`: `release = 21`, plus `-PjmhJdk=21` to point JMH's `jvm` at a JDK 21
- [own] `java/aot-smoke-test/build.gradle.kts` (stays on 25; comment only)
- [own] `java/src/main/resources/META-INF/native-image/.../native-image.properties` (class name), and `reachability-metadata.json` (narrow the glob to `native/*/libhyperuuid.*`/`hyperuuid.dll`, so images do not embed 8 JNI libraries)
- [own] `java/src/test/java/.../BackendTest.java` (new) and `NativePlatformTest.java`
- [own] `java/README.md`: floor; both paths; the JEP 472 warning now applying to JNI on 25 too; JNI JMH rows printed next to FFM; the GraalVM section noting that images are 25-only (CE for 21 is EOL)
- [shared] `.github/scripts/check_exports.py` (Java FFM path; two new sites)
- [shared] `.github/scripts/tests/test_check_exports.py` (pin the JNI sites, as TS and R are pinned)
- [shared] `.github/workflows/ci.yml` (Forge inputs: `java_jni`, `jdk_floor_version: "21"`)
- [shared] `.github/workflows/release.yml` (maven-pack/maven-publish pass `java_jni`)
- [shared] `.github/scripts/tests/test_workflow_contracts.py`
- [shared] `.github/dependabot.yml` (cargo `directories` + `/java/jni`, `:37-38`)
- [shared] `.github/hyper.json` (only if the Forge reads the JNI crate dir from it rather than from an input)
- [shared] `.gitignore` (`java/jni/target`)
- [shared] `README.md` (`:61`, `:71`, `:141`, `:227`, `:399`), `ROADMAP.md` (`:12`, `:55`), `CHANGELOG.md` (Unreleased; also notes the JEP 472 warning on 25), `CLAUDE.md`:
  - new "JDK floor" list-that-moves-together entry: `options.release`, the Forge `jdk_floor_version`, and the README sentence, with the 21 EOL drop date;
  - `:200` Android wording;
  - the benchmark note.

**Forge (`../HyperForge`)**: all [shared], since every input defaults off so the siblings are unaffected:
- `.github/workflows/hyper-build-native.yml`:
  - build, upload and attest `jni-{rid}` on the desktop legs and the osx-x64 cross leg, placed right after the Rust compile (the fail-fast rule);
  - stage the JNI library for tests;
  - `setup-java` with both 21 and 25;
  - a `jdk_floor_version` run;
  - the JDK header note.
- `.github/actions/musl-native/` (build the JNI crate in Alpine) and `.github/actions/musl-suite/suite-musl.sh` (Java on a `21-jdk-alpine` image as well)
- `.github/actions/prepare-maven-package/action.yml` (download `jni-*`, verify, stage) and `hyper-pack-maven.yml`, `hyper-publish-maven.yml` (input pass-through)
- `.github/workflows/hyper-check-core.yml` (rustfmt/clippy over `java/jni` as a companion crate)
- `.github/scripts/hyper_config.py` (JNI library names per RID) plus `test_hyper_config.py`, and the Forge README/runbook if the asset set is listed

## 6. Sibling cascade

- **Both siblings have this binding, FFM on JDK 25:** HyperCast `java/build.gradle.kts:150-159`, HyperTabular `:140-148`.
- **Their public APIs expose `MemorySegment`: 59 public methods across the two** (HyperCast `Cast.java:484` and others; HyperTabular `Row.java:90`, `Workbook.java:131` and others).
- **MRJAR cannot carry that to 21.**
  - The base cannot name a preview type.
  - The MRJAR rule forbids a public API that differs by version.
  - So "floors match and move together" cannot be met by the same recipe without dropping or replacing those signatures. That is a breaking API redesign, and HyperTabular's batch views are built on segments.
- **Recommendation.** HyperUuid moves alone. Record an explicit, commented exception where the siblings pin 25. The cascade is then one comment per sibling, plus CLAUDE.md's floor sentence in all three repos (Q1).

## 7. Verification

**Local** (Temurin 21.0.13 is now under `~/.sdkman/candidates/java`; 25.0.5 and GraalVM CE 25.4 were already there). Each check, then its negative control:

1. **Build.** `cargo cdylib` in `rust/`, then `cargo build --release` in `java/jni/`. Check with `nm -gU` that only `Java_…NativeCore_*` and `uuid_*`/`hyperuuid_version` are exported.
   - Negative control: delete one wrapper, and the JNI suite fails with `UnsatisfiedLinkError`.
2. **Suites.** `./gradlew test testJdk21 javadoc` from `java/`. The same test classes run on both JDKs, against the built jar, and `BackendTest` asserts `NativeCore.backend()` equals `-Dhyperuuid.expectBackend`.
   - Negative controls: flip the property and it fails. Point `test` at the classes directory instead of the jar, and 25 reports `jni` and fails. That proves the MR wiring is what the suite exercises.
3. **Parity.** The exhaustion tests (`UuidGeneratorTest.java:600-647`), the batch-limit tests and the `assertRefused` message checks pass unchanged on both paths.
   - Negative control: change one rc mapping in the JNI path only, and only `testJdk21` fails.
4. **Jar shape.** `jar --validate`; `javap -v` reports major 65 for base classes and 69 for versioned ones; `module.json` says `org.gradle.jvm.version: 21`.
   - Negative control: an FFM import in the base fails `javac --release 21` (seen today).
5. **Export lists.** `python3 .github/scripts/check_exports.py`.
   - Negative control: drop one `extern "C"` declaration from the shim, and the check fails.
6. **No-panic proof.** Build the shim with `--features no-panic` in release.
   - Negative control: add an `.unwrap()` on a JNI table entry, and the link fails.
7. **GraalVM.** `./gradlew :aot-smoke-test:nativeRun` under GraalVM 25; it must still report FFM.
   - Negative control: point `native-image.properties` at the old class name, and the build warns or fails. Then re-measure the image table: the code moved, so the old numbers no longer count as receipts.
8. **Contract tests and formatting.** `.github/scripts/tests` and Forge `validate.yml`'s tests; `./gradlew spotlessCheck` (its glob `src/**` already covers `java25`); `cargo fmt --check` in `java/jni`.

**CI only**
- Windows x64 and arm64, Linux arm64, both musl RIDs, and the osx-x64 cross-build of the JNI crate.
- JDK 21 on win-arm64 from Temurin.
- Attestation of `jni-{rid}`.
- The full JMH receipt: JNI rows on 21 and the re-run FFM rows, on the same machine as the existing table (Q5).

**Release-time**
- The first Central publish of an MRJAR carrying 16 natives.
- `prepare-maven-package` verifying `jni-*` attestations.
- `gh attestation verify` on the jar from the tag.
- A consumer smoke test on JDK 21 resolving through Gradle Module Metadata (jvm.version 21).

## 8. Risks and unknowns (ranked)

1. **Sibling floors.** The "move together" rule cannot be honoured without a sibling API break (§6). This is the owner's call (Q1).
2. **Shaded or uber jars dropping `Multi-Release: true`.** maven-shade without its MR setting does this. On 25 such a jar silently runs JNI: slower, but correct. Its only visible symptom is the JEP 472 warning naming `System::load`. Document it in the README.
3. **Single-call loss on 21.** About 2.3 ns a crossing (probe), an estimated 6–10% on Linux `newV7`. Receipts are required, and the loss is printed next to the wins.
4. **No-panic proof mechanics for a second cdylib crate** (UNVERIFIED that a cdylib link alone triggers it). The fallback, an example binary, is known to work in the core.
5. **Lazy JNI binding.** A stale JNI library fails at first call, not at `isAvailable()`, unless the init-time version and ABI check is built. It is in the design, but it is new code.
6. **Gradle toolchain discovery of a setup-java JDK 21 in CI.** Likely needs `org.gradle.java.installations.fromEnv=JAVA_HOME_21_X64,…`, with auto-provisioning kept off (UNVERIFIED in CI).
7. **Jar growth.** About 8 × 30 KB, an estimate. Negligible, but measure it.
8. **Android stays shut.** ART has JNI, so the JNI library opens the door technically. The rest is not free:
   - Android RIDs for the shim, which means extending `hyper-build-android.yml`;
   - a loader branch using `System.loadLibrary` from the APK, because temp-file extraction does not apply there and `/proc/self/maps` would resolve `linux-arm64` glibc;
   - AAR packaging;
   - `java.time` needs API 26;
   - an emulator job.

   Out of scope; CLAUDE.md `:200` gets reworded to "would need an AAR and an Android loader on top of the JNI library".

## 9. Owner questions

1. **Siblings' floor.** (A, recommended) HyperUuid goes to 21 alone, with a commented exception in HyperCast and HyperTabular; (B) block until the siblings redesign their `MemorySegment` APIs; (C) keep HyperUuid at 25.
2. **JNI layer.** (A, recommended) (c), a binding-owned no_std Rust cdylib in `java/jni/`; (B) (b), a C shim on the static archive; (C) (a), `Java_` symbols in the shared core cdylib.
3. **Libraries per RID in the jar.** (A, recommended) two: the attested core for FFM (unchanged) and the JNI library for 21–24; (B) one: the JNI library also serves FFM through its re-exported `uuid_*` symbols, which saves about 200 KB but changes what the 25 path and GraalVM load.
4. **Which JDK 21 end date triggers the drop.** (A, recommended) Temurin's 2029-12-31, the vendor CI runs; (B) Oracle Premier's 2028-09-30; (C) Corretto's 2030-10-31.
5. **Benchmark machine for the JNI rows.** (A, recommended) the i9-11900H WSL2 box, beside the existing Java tables; (B) re-measure the whole Java README on the Apple M6 and relabel it.
