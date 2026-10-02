# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**`java.util.UUID` can generate v4 (random) and v3 (MD5 name-based) — that's it. No v5, no v6, no v7, no batch API. This binding gives you the whole RFC.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling directly into the native `libhyperuuid` shared library via `java.lang.foreign` (FFM) downcalls — no runtime bridge, no extra runtime dependency (plain Java, not Kotlin: see the root README for why that matters). JDK 25 is the floor: the first long-term-support release with the final FFM API (JEP 454 finalized it in JDK 22, and 22 through 24 are past end of life). The jar bundles a native build for every supported platform (Linux glibc, Linux musl, macOS, Windows × x64/arm64) under `/native/{rid}/` and picks the right one at runtime — and, alongside them, the same core as a `wasm32-wasip1` module that [GraalWasm](https://www.graalvm.org/webassembly/) can run inside the JVM with no native binary at all (see [WebAssembly](#webassembly-graalwasm)).

```java
import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import java.util.UUID;

UUID id = UuidGenerator.newV4();
UUID id2 = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "example.com");
UUID id3 = UuidGenerator.newV6();
UUID id4 = UuidGenerator.newV7();

Instant created = UuidGenerator.v7Timestamp(id4);

// Version-agnostic: Optional.empty() instead of assuming id4 is v6/v7:
Optional<Instant> maybeCreated = UuidGenerator.getTimestamp(id4);

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(id4);

// One downcall, one random-bytes fetch, one counter reservation for the whole batch:
UUID[] batch = UuidGenerator.newV7Batch(1000);
```

Returns plain `java.util.UUID` — no wrapper type, so it works everywhere a `UUID` already does (equality, hashing, `Comparable`, JPA/Hibernate entity IDs, `toString()`). `UuidGenerator.Namespaces.DNS`/`URL`/`OID`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.NIL`/`MAX` are the §5.9/§5.10 special values. `newV6`/`newV7` also accept an `Instant` directly (`newV6(Instant)`), not just a raw millisecond count; `getTimestamp` is the version-agnostic counterpart to `v6Timestamp`/`v7Timestamp` — it checks `uuid.version()` itself and returns `Optional.empty()` for anything but a genuine v6/v7 `UUID`, instead of assuming the caller already knows.

## Gating on the core

Nothing loads until the first call that needs the core — `UuidGenerator.NIL`/`MAX` and `Namespaces` never do. A consumer with a fallback of its own gates on `UuidGenerator.isAvailable()` first: probed once, cached, never throws — `false` when the platform library will not open, the jar carries no core for this OS/arch, GraalWasm is missing on the wasm path, or an older core lacks an export this binding was built against. The generating methods do not fall back. The first one to need a core that failed to load throws `ExceptionInInitializerError`, and every later one `NoClassDefFoundError`; either way the original failure is in its cause chain. `UuidGenerator.nativeVersion()` reports the loaded core's `major.minor.patch`, and succeeds exactly when `isAvailable()` is `true` — the pair that proves the library that resolved is the one this jar was built against. `UuidGenerator.backend()` says which path won.

```java
UUID id = UuidGenerator.isAvailable() ? UuidGenerator.newV7() : UUID.randomUUID();
```

A batch is bounded by its own output: `count * 16` bytes has to be an array, so `newV6Batch`/`newV7Batch` and the `UUID[]` fills take at most `Integer.MAX_VALUE / 16` (134,217,727) UUIDs per call and throw `IllegalArgumentException` for more, or for a negative count.

## Native access

FFM downcalls are a *restricted* operation: the JDK wants the application, not a library on its classpath, to say that native code may run. Without that the first call still works, and prints a four-line warning naming `java.lang.foreign.SymbolLookup::libraryLookup` and ending "Restricted methods will be blocked in a future release unless native access is enabled". Grant it where the JVM is launched:

```sh
java --enable-native-access=ALL-UNNAMED -cp app.jar:hyperuuid.jar com.example.Main        # on the classpath
java --enable-native-access=io.github.skunkwerkx.hyperuuid -p mods -m com.example.app     # on the module path
```

The jar's manifest carries `Automatic-Module-Name: io.github.skunkwerkx.hyperuuid`, so the module-path form has a stable name to grant rather than one derived from the jar's file name. An executable jar can make the same grant in its own manifest (`Enable-Native-Access: ALL-UNNAMED`). Under `--illegal-native-access=deny` with no grant the library cannot be loaded at all: `isAvailable()` is `false` and the generating methods throw. The wasm path wants the same flag, for Truffle's own native library rather than this one.

## Why not `java.util.UUID`?

The honest answer for versions v6/v7 is that there's no comparison to make — the JDK's own `UUID` class has never shipped them. Specifics, checked against the actual JDK source and OpenJDK's own issue tracker rather than assumed:

1. **No v5 at all.** `UUID.nameUUIDFromBytes()` is MD5-based — that's RFC 9562 v3, not v5. If you need deterministic namespace-based UUIDs that agree with every other RFC 9562 implementation (SHA-1, not MD5), the JDK has never had a built-in way to do it. This binding's v5 output is verified in CI to match RFC 9562's own Appendix A.4 test vector and Python's `uuid.uuid5` byte-for-byte.
2. **No v6/v7 in any released JDK.** A v7 factory (`UUID.ofEpochMillis`) is in progress upstream — [JDK-8357251](https://bugs.openjdk.org/browse/JDK-8357251) / [JDK-8334015](https://bugs.openjdk.org/browse/JDK-8334015) — but unshipped as of any JDK release. This binding gives you both today, including v6 (RFC 9562 §5.6's field-compatible reordering of v1), which isn't part of that upstream proposal at all.
3. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency, across both individual and batch calls.
4. **Batch generation.** `newV7Batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch, instead of paying per-item native-call overhead N times. `java.util.UUID` has no bulk generation API at all.
5. **Cross-language consistency.** The same Rust core mints v5/v6/v7 UUIDs for every other binding in this repo — a Java service and a Python or Go service produce byte-identical v5 UUIDs for the same `(namespace, name)`, which no per-language reimplementation can structurally guarantee.
6. **SQL Server byte ordering.** `UuidGenerator.v7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`v6ToSqlOrder` does the same for version 6, though same-millisecond v6 UUIDs aren't guaranteed to sort correctly since v6 has no counter), computed once in the native Rust core and verified there (and independently against the real `SqlGuid` comparator in the C# binding's own test suite) rather than reimplemented in Java. One caveat worth being direct about: this is verified at the raw-byte level against .NET's own `Guid` wire format, which ADO.NET passes through unchanged — it has *not* been checked against any specific JDBC driver's own `uniqueidentifier` parameter binding, which may or may not apply a further transform of its own. Verify against your driver, or bind the returned bytes directly, before relying on it in a JDBC-facing query.

The honest trade-off: this is a native library dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID inside the jar, or the wasm module below on a platform without one) instead of a type that's always sitting in `java.util`. If plain v4 randomness is all you need, `UUID.randomUUID()` is simpler and that's a completely reasonable choice.

## Destination-buffer fills

`fillV6`/`fillV7` write into an array you already own instead of allocating a fresh one per call, over either a `UUID[]` or a `byte[]`:

```java
UUID[] dst = new UUID[1000];
UuidGenerator.fillV7(dst);          // reuse dst across batches

byte[] raw = new byte[1000 * 16];
UuidGenerator.fillV7(raw);          // RFC-ordered bytes, no UUID objects at all
```

Java sits with C#, not with Go and Swift, on the cost question. `java.util.UUID` is two `long`s rather than 16 RFC-ordered bytes, so the `UUID[]` form still rebuilds every element from the native output — it removes the allocation, not the conversion. **The `byte[]` form is the one that removes real work**, since the native core already writes RFC-ordered bytes contiguously.

`./gradlew :benchmarks:jmh`, JMH average time, 1000 UUIDs per op (linux-x64, an Intel Core i9-11900H, JDK 25):

| Benchmark | Mean | B/op |
| --- | ---: | ---: |
| `newV7` x1000 individually | 43.2 µs | 32,000 |
| `newV7Batch(1000)` | 18.2 µs | 52,104 |
| `fillV7(UUID[])` into an existing array | 20.9 µs | 48,088 |
| `fillV7(byte[])` into an existing buffer | **9.4 µs** | **0** |

The middle two rows are still the point: filling a `UUID[]` measures the same as allocating a fresh one, within overlapping error. The allocation was never the expensive part — rebuilding a thousand `UUID` objects from RFC bytes is. Only the `byte[]` form escapes that, and it now does so with **nothing allocated and nothing copied**: the caller's array is pinned and handed to the native side, which writes every UUID straight into it.

A `byte[]` whose length isn't a multiple of 16 throws `IllegalArgumentException`.

### Raw-byte SQL-order transforms

`v6/v7ToSqlOrder(byte[])` and `v6/v7FromSqlOrder(byte[])` apply the same native permutation in place on a caller's 16 bytes. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here now makes against the one native implementation.

## Benchmarks

Real numbers, [JMH](https://github.com/openjdk/jmh) (`./gradlew :benchmarks:jmh`), linux-x64 (an Intel Core i9-11900H), JDK 25, 3 warmup + 5 measurement iterations, average time mode, `-prof gc` for the allocation column:

| Method | Mean | B/op | vs. `UUID.randomUUID()` |
| --- | ---: | ---: | ---: |
| `UUID.randomUUID()` | 172.9 ns | 128 | baseline |
| `UuidGenerator.newV4()` | 57.5 ns | 32 | **3.0x faster** |
| `UuidGenerator.newV5()` | 72.7 ns | 64 | **2.4x faster** |
| `UuidGenerator.newV6()` | 38.5 ns | 32 | **4.5x faster** |
| `UuidGenerator.newV7()` | 45.0 ns | 32 | **3.9x faster** |

**Why the doors are this cheap:** no door opens an arena or copies its input. Every downcall is linked `Linker.Option.critical(true)`, so a caller's own `byte[]` (a v5 name, a batch destination, sixteen bytes to reorder in place) is pinned and handed to the native side directly, and the single-UUID doors use one per-thread 16-byte in/out scratch for the life of the thread, written and read as two big-endian longs with no `byte[]` in between. Sound because every export is a short, non-blocking computation over the bytes it was handed that never calls back into Java — the profile the option exists for — and `reachability-metadata.json` registers it, so the GraalVM Native Image smoke test proves it under AOT too. The 32 bytes left per call are the `UUID` object itself.

The FFM downcall doesn't lose to the JDK's own generator, and the reason is worth stating so the win isn't mistaken for a rigged comparison: `UUID.randomUUID()` is genuinely slow, largely because it goes through `java.security.SecureRandom` by default. How slow is the machine's business — what its random source costs sets the width of the gap, and a box where that is expensive shows a far larger multiple than this one does. Reported as measured, not adjusted to make the story better.

Batch generation vs. an equivalent loop:

| Method | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v7 | 43.2 µs | 18.2 µs | **2.4x** |
| v6 | 42.1 µs | 21.5 µs | **2.0x** |

The batch multiplier is smaller than it was before the carrier rewrite, for the best reason available: the individual calls got faster, so there is less waste left to amortize. The `byte[]` fills are where the rest goes — 9.4 µs for v7 and 12.0 µs for v6, 4.6x and 3.5x over the loop.

Reproduce: `./gradlew :benchmarks:jmh`.

## AOT

Verified against a real GraalVM Native Image build, not just claimed compatible — see `aot-smoke-test/` (`./gradlew :aot-smoke-test:nativeRun`), which builds and runs a genuine standalone native binary that calls every public method of `UuidGenerator` — the `isAvailable()`/`nativeVersion()` probe, each generator in each of its overloads, the batch and fill forms over both `UUID[]` and `byte[]`, and the SQL/RFC byte-order conversions in their `UUID` and raw-byte forms — no JVM required to run it. Needed a bundled `META-INF/native-image/.../reachability-metadata.json` to register each distinct FFM downcall *signature* ahead of time (GraalVM's reachability analysis is per-signature, not per-function — four of this binding's methods share one signature `(ADDRESS)void`, and missing that one entry alone was enough to build clean and crash at runtime; the version probe's `()int`, the one downcall not linked critical, is a seventh entry of its own) and a `resources` glob covering `native/*/*` — both already shipped in this jar, verified by actually building and running the resulting executable with no JVM anywhere on `PATH`, so a consumer's own `native-image` build picks it up automatically with zero extra config. The jar's `native-image.properties` rides along the same way, and it is what keeps the downcalls compiled rather than interpreted in the image: about 80 ns per `newV7` there ([the numbers](#webassembly-graalwasm)).

## WebAssembly (GraalWasm)

The jar carries the Rust core a second time, as `native/wasm32-wasip1/hyperuuid.wasm` — the twelve `uuid_*` functions and `hyperuuid_version`, compiled for WASI preview 1 instead of an OS. [GraalWasm](https://www.graalvm.org/webassembly/) runs that module inside the JVM, so `UuidGenerator` has a second interop path that needs no platform-specific binary and no FFM downcall: the polyglot API calls the exports, and the guest's own exported `malloc` supplies the buffers the core fills. Every public method, exception and message is identical between the two paths — the full test suite runs twice on every build (`./gradlew test testWasm`), once through each.

This is not the Java binding compiled *to* WebAssembly (the root README's WebAssembly table still says why that path is blocked). It is the opposite direction: the Rust core running *as* WebAssembly inside an ordinary JVM.

**Enabling it.** GraalWasm is deliberately not a dependency of this jar — its POM lists nothing, so the default FFM path pulls in nothing extra. Add the two artifacts yourself (`wasm` is a POM-type dependency that fans out into the Truffle runtime):

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hyperuuid:<version>")
    implementation("org.graalvm.polyglot:polyglot:25.4.4.1.1")
    runtimeOnly("org.graalvm.polyglot:wasm:25.4.4.1.1")
}
```

Then either set `-Dhyperuuid.backend=wasm` to force it, or do nothing: with the property unset, `UuidGenerator` takes the FFM path when the jar has a native build for the running platform and that library loads, and falls back to the wasm module otherwise. "No native build" is decided exactly, not by nearest match: an architecture other than x64/arm64 (riscv64, ppc64le, s390x, 32-bit anything) or an OS other than Linux, macOS and Windows resolves to no library at all, and on Linux a musl process (Alpine) gets the musl build, never the glibc one. "Will not load" covers a bundled library the dynamic loader refuses — a temp directory mounted `noexec`, say; if the wasm path cannot start either, the failure thrown is the native one, with the wasm one attached as suppressed. `-Dhyperuuid.backend=native` forces FFM and fails loudly on a platform without a bundled library, or with one that will not load. `UuidGenerator.backend()` reports `"native"` or `"wasm"` for whichever won. Selecting wasm without GraalWasm on the classpath fails when the core is first needed — `isAvailable()` is `false`, and every other call throws — with a message naming the two artifacts; the `org.graalvm.polyglot` classes are never loaded otherwise.

**What it costs**, measured with the JMH suite this repo ships (`./gradlew :benchmarks:jmh` for the FFM rows, the same with `-Pwasm` for the others), on linux-x64 (an Intel Core i9-11900H), one session:

| Runtime | `newV7(long)` | `fillV7(byte[16000])` | `fillV7(UUID[1000])` |
| --- | ---: | ---: | ---: |
| FFM downcall, Temurin 25 | 45 ns | 9.4 µs | 20.9 µs |
| FFM downcall, GraalVM CE 25.4 | 40 ns | 9.4 µs | 17.8 µs |
| GraalWasm on GraalVM CE 25.4 (JIT) | 93 ns | 11.0 µs | 24.2 µs |
| GraalWasm on Temurin 25 (interpreter fallback) | 2.1 µs | 637 µs | 648 µs |

Three things those rows say plainly. Under the JIT the wasm path costs about twice the FFM downcall per call — and is still faster than `UUID.randomUUID()` — and the batch doors are where the two paths meet: one crossing per thousand UUIDs, and the byte fill lands within 20% of FFM. What remains per call is the polyglot crossing itself — each export is resolved once and called through its cached `Value`, and a UUID comes back in one 16-byte read — plus the lock.

On a stock OpenJDK, GraalWasm has no JIT: the engine prints a fallback-runtime warning at startup (`-Dpolyglot.engine.WarnInterpreterOnly=false` silences it) and runs the module interpreted, at roughly 50x the FFM cost per call and slower than `UUID.randomUUID()`. The JIT numbers need a GraalVM JDK or a Native Image build; nothing in this jar can change that. Keep `org.graalvm.polyglot:polyglot` and `:wasm` at the same release as the GraalVM JDK you run on (25.4.4.1.1 here): Truffle will not use a compiler from a different release, so a mismatched pair runs the interpreter on the JVM and fails a Native Image build.

**Under GraalVM Native Image** there is no JMH to run, so these rows are a hand loop — warm up, then one million `newV7(long)` calls and three thousand of each fill, best of five rounds — built into a native image with the metadata the jar ships. Same machine; the first two rows are the same loop on the JVM, and they land on the JMH figures above, which is what makes the loop trustworthy for the two rows JMH cannot produce:

| Runtime, hand loop | `newV7(long)` | `fillV7(byte[16000])` | `fillV7(UUID[1000])` |
| --- | ---: | ---: | ---: |
| FFM downcall, GraalVM CE 25.4 JVM | 36 ns | 9.3 µs | 14.1 µs |
| GraalWasm, GraalVM CE 25.4 JVM (JIT) | 102 ns | 12.1 µs | 24.2 µs |
| FFM downcall, Native Image | 78 ns | 12.8 µs | 31 µs |
| GraalWasm, Native Image | 163 ns | 15.7 µs | 32 µs |

Both paths cost about twice in a native image what they cost on the JVM, which is the ordinary price of an ahead-of-time compiler without a profile, and FFM stays the faster of the two.

FFM is compiled in the image, not interpreted, because the downcall handles are constants there: they are created from the C signature alone in a class that needs nothing from the library, take the export's address as their first argument, and `META-INF/native-image/.../native-image.properties` in the jar has that class initialized at image build time. A consumer's `native-image` build inherits it with no configuration.

**Threading.** A polyglot context does not allow concurrent access from multiple threads, so every call on the wasm path is serialized on one lock; one context and one module instance serve the whole process, which is also what keeps the core's process-wide v7 counter a single sequence. The FFM path has no lock. A hot, multi-threaded generator should expect that difference, not just the per-call one.

**Native Image.** The bundled `reachability-metadata.json` registers `WasmBackend`'s constructor for reflection and the `native/*/*` resource glob already covers the module, so a consumer's `native-image` build of the wasm path needs no extra configuration on this jar's account — verified by building the published jar plus the two GraalWasm artifacts into a native executable and running it with `-Dhyperuuid.backend=wasm` (the Native Image rows above); the same binary run without the property takes the FFM path. That proof is now a task rather than a hand build: `./gradlew :aot-smoke-test:nativeRun -Pwasm` puts GraalWasm on the smoke test's classpath, runs the binary with the property, and the binary prints `backend: wasm` before calling every public method; without the property it prints `backend: native` and links nothing extra.

**Reproducing the wasm numbers.** `./gradlew :benchmarks:jmh -Pwasm` runs the same JMH suite as the FFM table through the GraalWasm backend, with the longer warmup Truffle's runtime compilation needs (the FFM suite's one-second warmup gives error bars wider than the values on this path); `-PjmhInclude=<regex>` runs a subset. Run it under the JDK you want the row for — `JAVA_HOME` decides whether Truffle has a compiler to use. The rest of the JIT run, for the record: `newV4` 81 ns, `newV6` 80 ns, `newV5` 347 ns (the name crosses into guest memory), `newV7Batch(1000)` 24.2 µs.

## Verifying provenance

The published jar carries a GitHub build-provenance attestation, but not one signed by this
repo directly — `release.yml`'s `maven-publish` job hands off to a reusable workflow
(`hyper-publish-maven.yml`) that physically lives in `SkunkWerkx/.github`, and that's the
identity Fulcio records as the signer. `--repo` alone isn't enough; add `--signer-repo`,
or use `--owner` in place of both:

```sh
curl -LO https://repo1.maven.org/maven2/io/github/skunkwerkx/hyperuuid/X.Y.Z/hyperuuid-X.Y.Z.jar
gh attestation verify hyperuuid-X.Y.Z.jar \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-X.Y.Z.jar --owner SkunkWerkx
```

Get the signer-repo wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`,
which reads like a bad signature but is only an identity mismatch — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for the
full breakdown of which artifacts in this project are signed from which repo and why.

## Install

Published to [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid) — no extra repository configuration needed, `mavenCentral()` is virtually every Gradle/Maven project's default already:

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hyperuuid:<version>")
}
```

The current version is the one on the Maven Central badge above. Requires JDK 25 or later. The jar bundles a native build for all eight platforms (linux-x64, linux-arm64, linux-musl-x64, linux-musl-arm64, osx-x64, osx-arm64, win-x64, win-arm64) and picks the right one at runtime; see [Native access](#native-access) for the one flag the JVM wants.

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)
