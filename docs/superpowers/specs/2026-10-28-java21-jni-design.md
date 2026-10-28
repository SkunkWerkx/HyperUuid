# Java 21: a JNI base in a multi-release jar

2026-10-28 · ROADMAP Phase 1, Java row · **Draft for owner review.** Built from
`.superpowers/brainstorm/2026-10-28-phase1/java-brief.md` under the owner's decisions; the brief's
claims were re-checked against the tree (corrections in Open items). Ships in 1.1.0.

## Goal

One jar, `io.github.skunkwerkx:hyperuuid`, runs on JDK 21 and later. That reaches the JDK 21 LTS
shops in finance and insurance, which today cannot load a jar compiled for 25. On 21 to 24 the
jar reaches the core through JNI, from its base classes. On 25 and later it takes today's FFM
path, from `META-INF/versions/25`. Both paths have one public API, the same exceptions and
messages, and one Rust core built from one commit, so v5 and SQL-order bytes are identical. FFM
is only a preview API in 21 (it went final in 22, JEP 454), so no preview flag could ship it.

## Decisions (owner, 2026-10-28)

- **Floor 21, through a multi-release jar.** The base compiles for 21 and calls JNI through a
  package-private `NativeCore`; the FFM code moves unchanged under `META-INF/versions/25`. The
  public API, exceptions and messages are identical on both paths.
- **JNI layer:** a binding-owned no_std Rust cdylib in `java/jni/`, like `typescript/`. The C
  exports, `check_exports` and every other binding's bytes stay unchanged.
- **Two libraries per RID:** the attested core for FFM and GraalVM (unchanged), and the JNI
  library for 21 to 24.
- **Drop date:** Temurin's JDK 21 end, 2029-12-31, recorded the way Ruby 3.4's is.
- **Benchmarks:** the whole Java README re-measured on the Apple M6 (Mac18,5) and relabelled,
  with FFM on 25 and JNI on 21 side by side.
  Since 2026-10-28 this is part of the system-wide receipts pass, which moves every table in the
  repo to the M6. This spec still owns the Java harness changes (the FFM and JNI columns,
  `-PjmhJdk=21`, the jmhJar `Multi-Release` fix and the 5% FFM gate). The timing of the pass and
  the root README's table layout belong to that pass.
- **No JDK 17** (2026-10-28, after the cost was laid out). The README calls out the platform
  reason first: Temurin ships no JDK 17 for Windows arm64 or Alpine (musl) arm64 (Adoptium API,
  checked 2026-10-28; Temurin 21 ships both), so a 17 floor could not be tested on two of the
  jar's eight platforms without mixing vendors. Second, 17 leaves Temurin support on 2027-10-31.
- **osx-x64 under Rosetta:** yes (2026-10-28). Both suites run there, and the cell becomes ✅.
- **Siblings:** moot, since they are frozen. **Android:** out of scope.
- **Release vehicle:** 1.1.0. The release-mode board proves every new artifact shape before the
  tag.

## Design

### `java/jni/`

**Crate.** Package `hyperuuid-jni` 0.0.0, `publish = false`, edition 2024, with `[lib] name =
"hyperuuid_jni"` and `crate-type = ["cdylib"]`. It builds `libhyperuuid_jni.so`, `.dylib` or
`hyperuuid_jni.dll` in `java/jni/target/release/`. `Cargo.lock` is committed, as `typescript/`'s
is, and the existing `**/target/` rule covers the output.

**no_std.** The crate is `#![cfg_attr(not(test), no_std)]`, with no `alloc`. It has two normal
dependencies:
- `hyperuuid = { path = "../../rust", default-features = false, features = ["cdylib"] }`, which
  brings the core's panic handler, its `rust_eh_personality` stub and the libc link;
- `jni-sys = "0.4"`, which is no_std and types only, so no JDK headers are needed.

**C imports, not a `libc` dependency (amended 2026-10-28).** `malloc` and `free` for the
large-array path are declared in the shim's own `unsafe extern "C"` block, beside the 13 core
exports, the way the core declares `abort` (`rust/src/lib.rs`). Every target links the C library
already. The crate adds no dependency for them.

Both profiles set `panic = "abort"`: a no_std cdylib cannot unwind, and clippy builds the dev
profile. Release adds `lto`, `codegen-units = 1` and `strip`, as the core's does.

**Two configurations, kept apart (amended 2026-10-28).**
- **Production: no_std.** `cargo build --release --locked --lib` builds the cdylib with the core's
  `cdylib` feature and no `std`. The core's panic handler and its `rust_eh_personality` stub are
  linked in, and L2 checks the artifact depends on the C library alone. The no-panic proof is
  `cargo build --release --locked --example no_panic --features no-panic`. The example is built
  in the test configuration below, as the core's own no-panic example is built with `std`.
- **Tests: std.** `[dev-dependencies] hyperuuid = { path = "../../rust", default-features = false,
  features = ["cdylib", "std"] }`. The core's panic handler and personality stub are both
  `cfg(not(feature = "std"))` (lib.rs:56 and :88), so a test build links std's handler alone and
  never two. Edition 2024's resolver applies dev-dependency features only to test and example
  builds, so the production `--lib` build stays no_std. L2's libc-only check on that artifact is
  the guard. NC: a scratch copy with `resolver = "1"`, which unifies dev-dependency features into
  every build, brings std into `--lib` and must fail L2.
  The unit tests (L7c, and the table invariant below) run with `cargo test --locked` in
  `java/jni`.

**The registration table is unique by construction (amended 2026-10-28).** A `const` assertion
over the table fails the build if any two entries share a name. Equal counts prove completeness
only when the entries are distinct. `check_exports.py`'s "JNI table" site also rejects a repeated
name.

**Linking.** The core is linked statically, so each JNI library loads nothing else. `extern crate
hyperuuid;` forces the link, since a dependency that no path names is not linked. The shim
declares all 13 exports in an `unsafe extern "C"` block, so every return code is the C ABI's own.
`.cargo/config.toml` copies only the core's musl `-crt-static` sections.

**Registration (amended after review, 2026-10-28).** A JNI call resolves its native only when it
is first invoked, so loading a library proves nothing about whether the library is complete. The
shim therefore binds eagerly. It exports one JNI entry point, `JNI_OnLoad`, which calls
`RegisterNatives` on `NativeCore` with one static table of 14 entries. Each entry holds the method
name, its exact descriptor and the fn pointer. Thirteen entries are `<camel(export)>`, one per C
export from `uuidNewV4` to `hyperuuidVersion`. The fourteenth is `jniNatives`, which returns the
table's length, computed from the table and never written as a literal.
- `RegisterNatives` throws `NoSuchMethodError` for any entry with no matching Java `native`
  (name and descriptor). `JNI_OnLoad` then returns `JNI_ERR`, and `System.load` throws
  `UnsatisfiedLinkError`.
- After the load, init compares `jniNatives()` with a reflective count of `NativeCore`'s
  `native` methods. The entries are distinct, every one registered, and the counts are equal, so
  every Java native is bound before the first door runs. Any mismatch makes `isAvailable()`
  false, exactly as a missing export does on FFM.
- **UUIDs** cross in as two `jlong`s, rebuilt big-endian on the stack. They return through
  `SetLongArrayRegion` into a `long[2]`.
- **Arrays: critical only when bounded (amended after review, 2026-10-28).** JNI forbids other
  JNI calls inside a critical region and advises against long work there.
  - At or below `CRITICAL_MAX` (64 KiB: a v5 name, or a batch of up to 4,096 UUIDs), the array is
    pinned with `Get/ReleasePrimitiveArrayCritical`. Only the core call runs inside the region,
    which is microseconds. The region is released before any other JNI call, including the v5
    result's `SetLongArrayRegion`.
  - Above it, nothing is pinned. A v5 name is copied in with `GetByteArrayRegion` to a
    `libc::malloc` buffer. A batch is made by **one** core call into a `malloc` buffer, so one CAS
    still reserves the whole batch, and is then copied out with `SetByteArrayRegion` in 1 MiB
    chunks. The buffer is freed on every path.
  - The cost: a batch above 64 KiB briefly needs twice its size in memory, which the README
    says. A zero length passes null and pins nothing.
- **A failed array access (amended after review).** A null from a critical get or from `malloc`
  is never mapped as a core result. If `ExceptionCheck` shows a pending exception, the shim
  returns at once and Java rethrows that exception. Otherwise it raises `OutOfMemoryError` itself
  with `ThrowNew`. Table entries are matched, never unwrapped.

The library exports `JNI_OnLoad`, plus the 13 C exports that a cdylib re-exports from
its dependency. That second C ABI is not an offered entry point.

**No-panic.** Each registered native fn (an unexported `extern "system" fn`) and `JNI_OnLoad` carry `cfg_attr(feature = "no-panic", no_panic)`.
`examples/no_panic.rs` links every one of them, as the core's example does. A cdylib link alone
proves nothing, because a Linux shared library may keep undefined symbols.

### The `NativeCore` split

**Base** (`src/main/java`, `--release 21`). `UuidGenerator` keeps every public member, argument
check and rc mapping (`batchFailure`, `sqlOrderFailure`, `V7_COUNTER_EXHAUSTED`), and
`Availability`. It imports no `java.lang.foreign` type, and its Javadoc links none: javadoc runs
at 21 with `-Xwerror`, where those types are preview. The base `NativeCore` is the JNI path.

**Versioned** (`src/main/java25`, `--release 25`). Today's `Downcalls`, `Core`, `Scratch`,
`readUuid`/`writeUuid` and `export(...)` move here verbatim, as nested classes of `NativeCore`.

**One surface.** Both trees declare `final class NativeCore`, static members only, with identical
descriptors:
- `backend()`, which returns `"jni"` or `"ffm"` and loads nothing;
- `hyperuuidVersion()`;
- `scratch()`, this thread's scratch, read back with `result(s)` and `resultMillis(s)`;
- calls that write a scratch: `newV4`, `newV5(s, UUID, byte[])`, `newV7(s, long)`,
  `getTimestamp(s, UUID, int)`, `v7ToSqlOrder` and `v7ToRfcOrder`;
- direct calls: `newV7Batch(long, int, byte[])`, `readUuids(byte[], UUID[])`, `v7UnixMillis` and
  `v7UnixMillisIn`;
- `version`, `variant` and `isRfc`, each over `UUID` and over `byte[]`;
- the in-place `byte[]` reorders, which write only on rc 0.

Each call returns the C result raw, and only the base maps it. A single-UUID door still reads one
`ThreadLocal`.

**Load.** Each tree loads the library in a nested holder, so on both paths the first failing door
throws `ExceptionInInitializerError` and every later one `NoClassDefFoundError`. The
missing-platform and missing-resource messages come from one `NativePlatform` helper. Only two
texts differ by path:
- **The stale-library cause** names its own library. FFM keeps `libhyperuuid does not export <s>`;
  JNI gives either `libhyperuuid_jni implements <n> natives; this binding declares <m>`
  (`jniNatives()` against the reflective count) or the `UnsatisfiedLinkError` that
  `RegisterNatives` raised, as its cause.
- **FFM's `AssertionError`** around `invokeExact`, which nothing reaches.

`NativePlatform.Target` becomes `(rid, libraryFileName, jniLibraryFileName)`, with
`resourcePath()` and `jniResourcePath()`.

### Resources and loaders

| Platform | Core (FFM, GraalVM) | JNI library |
| --- | --- | --- |
| Linux, glibc and musl | `/native/{rid}/libhyperuuid.so` | `/native/{rid}/libhyperuuid_jni.so` |
| macOS | `/native/{rid}/libhyperuuid.dylib` | `/native/{rid}/libhyperuuid_jni.dylib` |
| Windows | `/native/{rid}/hyperuuid.dll` | `/native/{rid}/hyperuuid_jni.dll` |

Both paths resolve the RID through `NativePlatform`, which detects musl from `/proc/self/maps`.
Each extracts one file to `createTempFile("hyperuuid", ext)` with `deleteOnExit`, then loads it:
- **FFM** uses `libraryLookup(tmp, Arena.global())`, with all 13 lookups eager, as today.
- **JNI** uses `System.load(tmp)`, whose `JNI_OnLoad` registers every native, then runs the
  `jniNatives` check.

A JVM loads only one of the two.

### Gradle

**One JDK 25 toolchain.** Gradle and every compile run on 25.
- `val jdkFloor = 21` sets `--release` for `main`, `test`, `:benchmarks` and `:aot-smoke-test`.
- The `java25` source set compiles at 25, with `main`'s output on its classpath.
- The jar places `java25` under `META-INF/versions/25`, beside `Multi-Release: true`. The sources
  jar does the same.
- Module Metadata says `org.gradle.jvm.version: 21`. The POM says "JDK 21+: JNI on 21–24, FFM on
  25+".

**Suites, run against the jar.** `test` runs on Gradle's 25. `testFloor` runs on
`launcherFor(jdkFloor)`. Their classpath is the jar plus the tests, never `main`'s output. Each
task sets:
- `hyperuuid.expectBackend`, `ffm` or `jni`;
- `hyperuuid.expectFeature`, 25 or 21;
- `hyperuuid.noNativeJar`, the jar without `native/**`, which is never published;
- `--enable-native-access=ALL-UNNAMED`, which 21 accepts.

**Gates.** `checkSuiteParity` requires the same test names in both JUnit XML outputs, with none
skipped. `validateMultiReleaseJar`, which `test` depends on, runs `jar --validate` and checks:
- the manifest;
- class majors 65 for the base and 69 for the versioned tree;
- both libraries for every staged RID.

**Lazy toolchain.** Spotless, javadoc and publish never resolve JDK 21, so
`prepare-maven-package` keeps a single JDK. `java/gradle.properties` (new) turns auto-download
off. CI passes the floor's path, with auto-detect off.

**Staging.** `stageNativeLibrary` stages both `../rust/target/release` and `jni/target/release`,
unless the libraries were placed explicitly. It fails, naming the library, when only one of the
two exists, or when the JNI library is older than `jni/src`, `jni/Cargo.*`, `../rust/src` or
`../rust/Cargo.toml`.

**Harnesses.**
- `jmhJar` gains `Multi-Release: true`, which the JMH fat jar drops.
- `-PjmhJdk=21` points JMH's `jvm` at the floor.
- JMH `@Setup` and the smoke test's `run` fail unless the expected `NativeCore` loaded. Only the
  JNI one declares `native` methods and is class major 65; the FFM one is major 69.
- In an image, the smoke test asserts `isAvailable()` and that no JNI library is embedded. Only
  FFM could have run.

### GraalVM

**Images are 25+ and always FFM.** GraalVM for 25 reads `versions/25`. GraalVM CE for JDK 21
reached end of life on 2024-03-19 (endoflife.date). An image built on any GraalVM for 21 is
unsupported: it does not embed the JNI library, so `isAvailable()` is `false`.

**Metadata.** `native-image.properties` initializes
`io.github.skunkwerkx.hyperuuid.NativeCore$Downcalls` at build time. `reachability-metadata.json`
keeps the ten downcalls and replaces the `native/*/*` glob with `native/*/libhyperuuid.so`,
`native/*/libhyperuuid.dylib` and `native/*/hyperuuid.dll`.

**The README table** is re-measured on the M6. It gains a JNI row on Temurin 21, beside the FFM
JVM row and the Native Image row.

### Forge

All new inputs default off, so the siblings run exactly what they run today.

**`hyper-build-native.yml`** gains `java_jni` (bool) and `jdk_floor_version` (default `""`).

The desktop job runs these steps right after "Build this leg's static libraries", ahead of
TypeScript, to fail fast:
1. Install Temurin `jdk_floor_version` (`id: jdk_floor`) on every leg, then the matrix's 25.
2. Build `java/jni`. On Linux, add the GLIBC_2.34 check and the export check.
3. Place both libraries in `java_resources_dir/{rid}/`.
4. Run `./gradlew test testFloor checkSuiteParity javadoc
   -Porg.gradle.java.installations.paths=<floor path>`, with auto-detect off.
5. Upload and attest `jni-{rid}`, under the core's condition.

The old Java steps (:905-940) gain `!inputs.java_jni`, and the JDK note (:96-105) names the floor.

`build-native-osx-x64`, after the core's attestation:
1. Cross-build `java/jni`, and require `lipo -archs` to say `x86_64`.
2. Run both suites under Rosetta, on x64 Temurin 21 and 25.
3. Upload and attest `jni-osx-x64`.

`build-native-musl`:
- `musl-native` gains a `jni` input. It builds `java/jni` in the same container, under the core's
  RUSTFLAGS and its libc-only `readelf` gate, then uploads and attests `jni-{rid}`.
- Right after the static library, a new `musl-suite` binding, `java-jni`, runs `test testFloor
  checkSuiteParity :aot-smoke-test:run`. It uses `eclipse-temurin:<jdk_version>-jdk-alpine`, with
  the floor image's JDK copied in and mounted.
- The old Alpine Java step gains `!inputs.java_jni`.

**`hyper-check-core.yml`.** `lint_java_jni` adds a Linux job, `lint-java-jni`, in `java/jni`, that
runs:
- `cargo fmt --check`;
- `cargo clippy --release --locked --all-targets -- -D warnings`;
- `cargo test --locked`, the std test configuration (the L7c stub-`JNIEnv` tests and the table
  invariant);
- `cargo build --release --locked --lib`, then the libc-only check on the artifact (production,
  no_std);
- `cargo build --release --locked --example no_panic --features no-panic`;
- the export check.

**Pack.**
- `hyper_config.py` gains `jni_lib_name(rid)` and `JNI_LIB_NAMES`.
- `prepare-maven-package` gains a `jni` input. It downloads `jni-*`, runs `verify()` on each
  library and stages it beside the core, failing on a missing RID. After assembly it checks that
  the jar has both libraries per RID and `Multi-Release`.
- `hyper-pack-maven.yml` and `hyper-publish-maven.yml` pass `java_jni` through on both paths.

## Contract changes

⟨⟩ marks a figure from the receipt run (L12). The sentence shapes are fixed, and a loss is
written as a loss.

**`java/README.md`.**
- **Intro:** "… calling the core in-process: through JNI on JDK 21 to 24, and through
  `java.lang.foreign` (FFM) downcalls on JDK 25 and later, from one multi-release jar. JDK 21 is
  the floor, not 17: Temurin builds no JDK 17 for Windows arm64 or Alpine (musl) arm64, so a 17
  floor could not be tested on every platform this jar ships, and 17 leaves Temurin support on
  2027-10-31. JDK 22 to 24 take the JNI path; they are past end of life and not tested. Each
  `/native/{rid}/` holds the core and the JNI library built from it."
- **Gating:** "… or the bundled library lacks a function this binding was built against."
- **Native access:** "On JDK 21 to 23 nothing is printed without the grant. From 24, a JVM
  without it prints a four-line warning naming the method that loads native code:
  `Linker::downcallHandle` on 25 and later."
- **Shaded and fat jars (new):** "Keep `Multi-Release: true` (maven-shade's
  `ManifestResourceTransformer`, Shadow's `manifest`). Without it JDK 25 runs the JNI path, which
  is correct but slower, and its warning names `System::load`."
- **Fills:** "On JDK 21 to 24, a fill of up to 64 KiB holds the array for the core call alone.
  A larger one is made in native memory and copied in, so it never holds off garbage collection
  but briefly needs twice its size: a full `MAX_V7_BATCH` (1 GiB) needs 2 GiB."
- **Every table:**
  - is labelled "Apple M6 (Mac18,5), macOS ⟨ver⟩, Temurin ⟨25.0.x⟩ and ⟨21.0.x⟩, ⟨date⟩";
  - has an FFM column and a JNI column, each against its own JDK's `UUID.randomUUID()`;
  - adds one sentence: on macOS, the core calls `getentropy` for every id, which dominates single
    v4 and v7 calls;
  - drops every † note.
- **AOT:** "Native Image is built on GraalVM for JDK 25 and always takes the FFM path. GraalVM
  Community for JDK 21 is past end of life, so the JNI path has no image row."
- **Install:** "Requires JDK 21 or later."
- **Provenance:** "… the core and the JNI library for each platform, carry their own …".

**`README.md`, Java clauses only.**
- **Scoreboard:**
  - :20 names Java in the M6 sentence.
  - :61 becomes "⟨a–b⟩x ⟨faster/slower⟩ on JDK 25 (FFM), ⟨c–d⟩x on JDK 21 (JNI)".
  - :68 becomes "The Java, R and TypeScript rows …".
  - :71's bullet takes the new figures.
- **Crossings:** :86 becomes "Java's FFM, and JNI on JDK 21 to 24, cost …".
- **Platform support:**
  - :141's osx-x64 cell becomes ✅, and :152's note names Java beside TypeScript.
  - :165's JDK half becomes "JDK 21 sits above that line, though 17 is still supported upstream
    (Temurin to 2027-10-31): Temurin builds no JDK 17 for Windows arm64 or musl arm64, two of the
    jar's eight platforms." The C# item drops ".NET 11" from the same clause.
- **WebAssembly:**
  - :227 becomes "JNI on 21–24 and FFM downcalls on 25+". Its verdict: no Java-to-wasm compiler
    has FFM or loads a JNI library (L14).
  - :245 becomes "have no FFM and load no JNI library".
- **Benchmarks:**
  - Java leaves the linux-x64 tables: the :336 row goes, and :343's "3.6–4.9 µs" becomes
    "3.7–4.9 µs".
  - :345 moves into the M6 section, rewritten on Java's M6 figures.
  - :351 becomes "### Java, TypeScript and R, on an Apple M6", with :20's anchor updated.
- **Layout:**
  - :399 becomes "the JDK 21+ binding: JNI on 21–24 and FFM on 25+ from one multi-release jar,
    Native Image smoke test".
  - A new line follows: "java/jni/   the JNI library, a no_std Rust cdylib linking the core in".

**`ROADMAP.md:12`:** "JNI (21–24), FFM (25+) | Maven Central | JDK 21 | ⟨as :61⟩".

**`CHANGELOG.md`, Unreleased.**
- **Added:** "**Java runs on JDK 21.** The jar is multi-release: its base reaches the core through
  JNI, from a library built from the core for every platform, and JDK 25+ keeps FFM. The API,
  exceptions and messages are the same on both. The jar grows by ⟨n⟩ KB. *(Java)*"
- **Changed:** "**Java — the README's figures are from an Apple M6,** FFM on 25 beside JNI on 21.
  *(Java)*"

**`CLAUDE.md`.**
- **:38-47,** the C-export list adds: "Java's two `NativeCore` trees, `java/jni/src/lib.rs` (its
  extern block and its `RegisterNatives` table) and its `examples/no_panic.rs`".
- **After :58, a new entry:** "The JDK floor: `jdkFloor` (`21`) in `java/build.gradle.kts`,
  `jdk_floor_version` in `ci.yml`, and the floor sentences in `java/README.md` and README
  'Platform support'. JDK 21 reaches EOL at Temurin on 2029-12-31. Then the floor is 25: the JNI
  base, `java/jni/`, `jni-{rid}`, the input and the JNI sites in `check_exports.py` go, and the
  FFM `NativeCore` moves back into the base."
- **:94:** "(today Java's JNI path leads, then TypeScript and R)".
- **After :131-135:** "Java's JNI path needs its own library: `cargo build --release` in
  `java/jni/`, after `cargo cdylib`. Gradle stages both and refuses a stale one. Then, from
  `java/`, `./gradlew test testFloor checkSuiteParity javadoc`."
- **:184:** "except Java's, TypeScript's and R's".
- **:196** appends: "the first Maven Central publish of the multi-release jar, sixteen natives
  with their `jni-{rid}` attestations verified at pack".
- **:200:** "Java does not reach Android yet: ART has JNI, but it would need an AAR and an Android
  loader on top of the JNI library (Android builds of `java/jni/`, `System.loadLibrary` from the
  APK, API 26 for `java.time`). Python, Ruby and PHP don't reach Android either."

## Files

**[own]**
- `java/jni/{Cargo.toml,Cargo.lock,.cargo/config.toml,src/lib.rs,examples/no_panic.rs}`.
- `java/src/main/java/.../{UuidGenerator,NativeCore,NativePlatform}.java` and
  `java/src/main/java25/.../NativeCore.java`.
- Tests: `BackendTest` (new), `NativePlatformTest`, and `UuidGeneratorTest` (NoCore uses
  `noNativeJar`; `ExhaustionMain` forwards and asserts the backend).
- `java/{build.gradle.kts,gradle.properties}`, plus the builds and checks of
  `java/{benchmarks,aot-smoke-test}`.
- `native-image.properties`, `reachability-metadata.json` and `java/README.md`.

**[shared], this repo (10)**
- **`check_exports.py`,** in `SITES` :125-133, gets five Java sites:
  - FFM, which now reads `src/main/java25/.../NativeCore.java`;
  - "JNI natives", `static native \w+ (\w+)\(`;
  - "JNI table", the names in `lib.rs`'s `RegisterNatives` table (one `native!("<name>", "<descriptor>", <fn>)` per line);
  - "JNI no-panic";
  - "JNI imports", `fn ((?:uuid|hyperuuid)_\w+)\(`, against `all`.

  The natives, exports and no-panic sites map through `camel`, with `jniNatives` as their extra.
- **`test_check_exports.py`:** pins the five sites.
- **`ci.yml`:** `java_jni: true` and `jdk_floor_version: "21"` after :131, with a comment naming
  the older floor and its EOL; `lint_java_jni: true` after :204.
- **`release.yml`:** `java_jni: true` after :138 and after :445.
- **`test_workflow_contracts.py`:** asserts those four inputs.
- **`dependabot.yml:38`:** adds `"/java/jni"`, and the comment names it.
- **`README.md`, `ROADMAP.md:12`, `CHANGELOG.md` (2 bullets) and `CLAUDE.md`:** the lines above,
  clause by clause.

**[shared], Forge (9)**
- Workflows: `hyper-build-native.yml`, `hyper-check-core.yml`, `hyper-pack-maven.yml` and
  `hyper-publish-maven.yml`.
- Actions: `musl-native/*`, `musl-suite/*` and `prepare-maven-package/action.yml`.
- Scripts: `hyper_config.py` and `test_hyper_config.py`, plus the producer and action contract
  tests.

## Ordering constraints

1. **Forge first,** in one merge: its inputs default off, and a caller that passes an unknown
   input fails. Never while a release is in flight.
2. **This repo next,** in one pass, with clause-level edits to the shared files.
3. **Receipts last,** in the system-wide M6 pass. Two separate measurements (amended after
   review, 2026-10-28):
   - **The FFM regression gate** isolates the Java change. One core artifact is built once at
     the final commit, and its sha256 is recorded. The pre-split Java (the merge base's `java/`)
     and the post-split jar both run against that same file, on the same JDK, with the same JMH
     harness and in the same session. A faster core (Apple entropy, say) therefore cannot hide
     FFM overhead.
   - **The end-to-end tables** are the published receipts.

   The sha256 of the core, the JNI library and both jars are kept with the receipts.

## Verification

**Local.** The checks run on Temurin 21.0.13 and 25.0.5 and on GraalVM CE 25.4.4.1, all under
`~/.sdkman`. Each is paired with a negative control (NC).

- **L1 Exports.** `nm -gU` lists exactly `JNI_OnLoad` and the 13 C exports.
  NC: a stray `#[no_mangle]` fn fails it.
- **L2 no_std.** `cargo build --release --locked --lib`: the library depends on the C library
  alone, at about the core's size. Run separately from `cargo test`.
  NC: `default-features = true` on the core brings in std, and the check fails.
- **L3 Paths.** Run `./gradlew test testFloor checkSuiteParity javadoc`. `BackendTest` checks
  `backend()`, the native methods, the class major and `Runtime.version()`. NCs:
  - a flipped property fails;
  - `test` on `main`'s output reports `jni` on 25 and fails;
  - a `@DisabledOnJre(JAVA_21)` test fails `checkSuiteParity`.
- **L4 Same bytes, same messages.** `CorpusTest`, the exhaustion child's exact message
  (`UuidGeneratorTest.java:598-647`) and every refusal message all pass on both JDKs.
  NC: a shim that returns 1 for the core's 6 fails `testFloor` alone.
- **L5 Load failure.** NoCore (`UuidGeneratorTest.java:530`) passes on both JDKs against
  `noNativeJar`. NC: given the full jar, it sees `probe: true` and fails.
- **L6 Jar shape.** `validateMultiReleaseJar` passes, and `module.json` reads
  `org.gradle.jvm.version: 21`. NCs:
  - a `java.lang.foreign` import in the base fails `compileJava`;
  - a jar without `Multi-Release` fails both this check and L3.
- **L7 Staleness and completeness.** Touching `jni/src/lib.rs` fails staging. Each of these
  three scratch builds must make the availability probe report `false`, with its cause:
  - one table entry removed, so `jniNatives` follows the table and is one short;
  - one descriptor mistyped, so `RegisterNatives` throws;
  - a Java `native` added with no entry.
- **L7b Critical regions.** Both suites run with `-Xcheck:jni`, and any `WARNING in native
  method` on stderr fails the run. A test fills a `MAX_V7_BATCH` and a 1 MiB v5 name while a
  second thread allocates continuously. Both must finish with correct bytes.
  - NC: a scratch shim that makes a JNI call inside the critical region trips `-Xcheck:jni`.
- **L7d Unique table.** NC: a duplicated entry fails the build on the `const` assertion, and
  `check_exports.py` fails on its own.
- **L7c Null array access.** `cargo test --locked` in `java/jni` runs Rust unit tests against a stub `JNIEnv` table cover a null critical
  pointer in two cases: with a pending exception (nothing more is thrown, and no core result is
  mapped) and without one (`ThrowNew(OutOfMemoryError)`). The same tests cover a null `malloc`.
  - NC: the brief's "return -1" body fails the second case.
- **L8 Export sites.** `check_exports.py`. NC: removing one native, table entry or extern
  declaration fails exactly its site.
- **L9 No-panic.** The example links with `--features no-panic`. NC: an `.unwrap()` on a table
  entry fails the link.
- **L10 GraalVM.** Run `:aot-smoke-test:nativeRun` on GraalVM 25. NC: with the old `native/*/*`
  glob, the image embeds the JNI library and the smoke test fails.
- **L11 Format.** Run `spotlessCheck`, then `cargo fmt --check` and clippy in `java/jni`.
  NC: a misformatted `java25` file fails.
- **L12 Receipts.** Run JMH on 25 and with `-PjmhJdk=21`, plus the hand loops. Gate: FFM within
  5% of the pre-split baseline on every row, both against the same recorded core artifact
  (Ordering 3). NC: a `jmhJar` without `Multi-Release` aborts at
  `@Setup` on 25.
- **L13 Contracts.** Run `.github/scripts/tests` and the Forge's `validate.yml`. NC: dropping
  `java_jni` from `release.yml` fails.
- **L14 Docs.** Run the README Java snippets by hand on 21 and 25. Re-check the wasm verdict, with
  the date, against the docs for GraalVM Web Image, TeaVM and CheerpJ.

**CI only.**
- `build-native`: win-x64 and win-arm64 (the `ffi` shard, Temurin 21 beside Microsoft 25),
  linux-x64, linux-arm64 and osx-arm64.
- `build-native-musl`, on both RIDs.
- `build-native-osx-x64` and the eight `jni-{rid}` attestations, in release mode only.
- check-core's `lint-java-jni`.

**Release-time,** the first publish of a new artifact shape:
- **R1.** Before the tag, a release-mode master run produces eight attested `jni-{rid}`, with
  every Java step green, osx-x64 included.
- **R2.** A dry pack from that run: `gh run download`, place, `publishToMavenLocal`. The jar holds
  16 natives, and Gradle and Maven consumers on Temurin 21 and 25 report `jni` and `ffm`.
  NC: on 25, a copy with `Multi-Release` stripped reports `jni` and fails.
- **R3.** In the tag run, `prepare-maven-package` verifies 16 attestations, and the jar check
  passes.
- **R4.** At publish, Central accepts its first multi-release bundle. The jar from repo1 passes
  `jar --validate`, both README `gh attestation verify` commands, and R2's consumers.

## Nitpick audit

| Hole | Closed or labelled |
| --- | --- |
| JNI is slower | Measured and printed beside FFM (L12) |
| M6 calls may lose to `UUID.randomUUID()` | Printed as measured; :61 says "slower" if so. The Apple entropy item removes the per-id `getentropy` before the receipts |
| The split slows FFM | L12's 5% gate: old and new Java on one recorded core artifact, so a core speed-up cannot mask it |
| A shaded jar drops `Multi-Release` | The README section. The in-tree case, JMH's fat jar, is closed |
| Which path ran? | Four independent signals (L3), and `checkSuiteParity` |
| JDK 22–24 | Labelled untested and past EOL |
| Messages drift by path | The mapping lives only in the base. L4 runs on both, and the two path-specific texts are listed |
| Persisted bytes on JNI | The corpus runs on both paths (L4), over one core at one commit |
| A stale or incomplete JNI library | Eager `RegisterNatives` by name and descriptor, plus the table-derived count (L7), `check_exports`, the staging check and same-SHA attestation |
| Long critical regions on 21 | Bounded at 64 KiB. Larger fills go through native memory; `-Xcheck:jni` and a test under allocation pressure (L7b) |
| GraalVM for 21 | Labelled unsupported. Images run FFM (L10) |
| The JNI library re-exports the C ABI | Stated, and not offered as an entry point |
| JNI panics | Proved panic-free by example (L9), on Linux, as the core is |
| osx-x64, win-arm64, glibc | Rosetta suites (a new ✅); Temurin 21 on win-arm64; the GLIBC_2.34 check covers the JNI library |
| Central and multi-release jars | Central has no rule on them. R4 proves acceptance |
| Jar growth | Stated in the CHANGELOG |
| Why no JDK 17 | Stated in both READMEs: no Temurin 17 for win-arm64 or musl-arm64 (Adoptium API, 2026-10-28), and its 2027-10-31 EOL |

## Out of scope / Later

- **Android:** `java/jni` builds in `hyper-build-android.yml`, a `System.loadLibrary` loader, an
  AAR, API 26 and an emulator job.
- **Not planned:** JDK 17, linking the core into an image, and a public accessor for the active
  path.
- **Siblings:** HyperCast and HyperTabular get their own roadmaps after v1.1. Their APIs take
  `MemorySegment`, so this recipe does not carry over.

## Open items

Items 1 (no JDK 17) and 2 (Rosetta) were decided on 2026-10-28; see Decisions.

3. **Brief corrections:**
   - `.gitignore` needs no edit.
   - The README list missed :20, :68, :86, :165, :245 and :336-351.
   - Testing against the jar breaks NoCore.
   - JMH's fat jar drops `Multi-Release`.
   - The C exports are private (`mod ffi`), so the shim declares them and needs `extern crate`.
   - On Linux, a cdylib link proves nothing about panics.
