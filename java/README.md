# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)

**`java.util.UUID` can generate v4 (random) and v3 (MD5 name-based) — that's it. No v5, no v6, no v7, no batch API. This binding gives you the whole RFC.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling directly into the native `libhyperuuid` shared library via the Java Foreign Function & Memory API (stable since JDK 22 / JEP 454) — no runtime bridge, no reflection, no extra runtime dependency (plain Java, not Kotlin: see the root README for why that matters). This jar bundles a native build for every supported platform and picks the right one at runtime — and, alongside them, the same core as a `wasm32-wasip1` module that [GraalWasm](https://www.graalvm.org/webassembly/) can run inside the JVM with no native binary at all (see [WebAssembly](#webassembly-graalwasm)).

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

`./gradlew :benchmarks:jmh`, JMH average time, 1000 UUIDs per op:

| Benchmark | before | after | B/op after |
| --- | ---: | ---: | ---: |
| `newV7` x1000 individually | 121.5 µs | **74.5 µs** | 32,000 |
| `newV7Batch(1000)` | 33.2 µs | **26.1 µs** | 52,104 |
| `fillV7(UUID[])` into an existing array | 34.2 µs | **27.7 µs** | 48,088 |
| `fillV7(byte[])` into an existing buffer | 18.5 µs | **17.9 µs** | **0** |

The middle two rows are still the point: filling a `UUID[]` measures the same as allocating a fresh one, within overlapping error. The allocation was never the expensive part — rebuilding a thousand `UUID` objects from RFC bytes is. Only the `byte[]` form escapes that, and it now does so with **nothing allocated and nothing copied**: the caller's array is pinned and handed to the native side, which writes every UUID straight into it.

A `byte[]` whose length isn't a multiple of 16 throws `IllegalArgumentException`.

### Raw-byte SQL-order transforms

`v6/v7ToSqlOrder(byte[])` and `v6/v7FromSqlOrder(byte[])` apply the same native permutation in place on a caller's 16 bytes. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here now makes against the one native implementation.

## Benchmarks

Real numbers, [JMH](https://github.com/openjdk/jmh) (`./gradlew :benchmarks:jmh`), linux-arm64, JDK 25, 3 warmup + 5 measurement iterations, average time mode, `-prof gc` for the allocation column — before the carrier rewrite against after, same machine, same session:

| Method | before | after | B/op | vs. `UUID.randomUUID()` |
| --- | ---: | ---: | ---: | ---: |
| `UUID.randomUUID()` | 1137 ns | 1117 ns | 128 | baseline |
| `UuidGenerator.newV4()` | 155.0 ns | **101.8 ns** | 112 → **32** | **11.0x faster** |
| `UuidGenerator.newV5()` | 230.1 ns | **102.2 ns** | 272 → **64** | **10.9x faster** |
| `UuidGenerator.newV6()` | 127.8 ns | **67.1 ns** | 112 → **32** | **16.6x faster** |
| `UuidGenerator.newV7()` | 125.2 ns | **76.7 ns** | 112 → **32** | **14.6x faster** |

**What changed:** every door used to open an `Arena.ofConfined()` per call — a native allocation plus a scope teardown — and copy every input into it. Every downcall is now linked `Linker.Option.critical(true)`, so a caller's own `byte[]` (a v5 name, a batch destination, sixteen bytes to reorder in place) is pinned and handed to the native side directly, and the single-UUID doors use one per-thread 16-byte in/out scratch for the life of the thread, written and read as two big-endian longs with no `byte[]` in between. Sound because every export is a short, non-blocking computation over the bytes it was handed that never calls back into Java — the profile the option exists for — and `reachability-metadata.json` registers it, so the GraalVM Native Image smoke test proves it under AOT too. The 32 bytes left per call are the `UUID` object itself.

The FFM downcall doesn't lose to the JDK's own generator, and the reason is worth stating so the win isn't mistaken for a rigged comparison: `UUID.randomUUID()` is genuinely slow, largely because it goes through `java.security.SecureRandom` by default. Reported as measured, not adjusted to make the story better.

Batch generation vs. an equivalent loop:

| Method | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v7 | 74.5 µs | 26.1 µs | **2.9x** |
| v6 | 66.8 µs | 28.0 µs | **2.4x** |

The batch multiplier shrank from ~3.9x for the best reason available: the individual calls got faster, so there is less waste left to amortize.

Reproduce: `./gradlew :benchmarks:jmh`.

## AOT

Verified against a real GraalVM Native Image build, not just claimed compatible — see `aot-smoke-test/` (`./gradlew :aot-smoke-test:nativeRun`), which builds and runs a genuine standalone native binary exercising every function in this binding, including the SQL/RFC byte-order conversions, no JVM required to run it. Needed a bundled `META-INF/native-image/.../reachability-metadata.json` to register each distinct FFM downcall *signature* ahead of time (GraalVM's reachability analysis is per-signature, not per-function — four of this binding's methods share one signature `(ADDRESS)void`, and missing that one entry alone was enough to build clean and crash at runtime) — already shipped in this jar, verified by actually building and running the resulting executable with no JVM anywhere on `PATH`, so a consumer's own `native-image` build picks it up automatically with zero extra config.

## WebAssembly (GraalWasm)

The jar carries the Rust core a second time, as `native/wasm32-wasip1/hyperuuid.wasm` — the exact same twelve `uuid_*` C exports, compiled for WASI preview 1 instead of an OS. [GraalWasm](https://www.graalvm.org/webassembly/) runs that module inside the JVM, so `UuidGenerator` has a second interop path that needs no platform-specific binary and no `java.lang.foreign`: the polyglot API calls the exports, and the guest's own exported `malloc` supplies the buffers the core fills. Every public method, exception and message is identical between the two paths — the full test suite runs twice on every build (`./gradlew test testWasm`), once through each.

This is not the Java binding compiled *to* WebAssembly (the root README's WebAssembly table still says why that path is blocked). It is the opposite direction: the Rust core running *as* WebAssembly inside an ordinary JVM.

**Enabling it.** GraalWasm is deliberately not a dependency of this jar — its POM lists nothing, so the default FFM path pulls in nothing extra. Add the two artifacts yourself (`wasm` is a POM-type dependency that fans out into the Truffle runtime):

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hyperuuid:<version>")
    implementation("org.graalvm.polyglot:polyglot:25.3.4.1")
    runtimeOnly("org.graalvm.polyglot:wasm:25.3.4.1")
}
```

Then either set `-Dhyperuuid.backend=wasm` to force it, or do nothing: with the property unset, `UuidGenerator` takes the FFM path when the jar has a native build for the running OS/arch and falls back to the wasm module when it does not. `-Dhyperuuid.backend=native` forces FFM and fails loudly on a platform without a bundled library. `UuidGenerator.backend()` reports `"native"` or `"wasm"` for whichever won. Selecting wasm without GraalWasm on the classpath fails at class init with a message naming the two artifacts; the `org.graalvm.polyglot` classes are never loaded otherwise.

**What it costs**, measured through `UuidGenerator` itself on linux-arm64 with the module the jar ships: one million `newV7(long)` calls after warm-up, then three thousand fills of a 16,000-byte array and of a 1000-element `UUID[]`, in one loop with no harness between the caller and the class. The FFM row is the same loop on the same JVM, so the two are directly comparable (the JMH table above is the FFM path's own benchmark):

| Runtime | `newV7(long)` | `fillV7(byte[16000])` | `fillV7(UUID[1000])` |
| --- | ---: | ---: | ---: |
| FFM downcall, GraalVM CE 25 or Temurin 25 | 64 ns | 15.8 µs | 79 µs |
| GraalWasm on GraalVM CE 25 (JIT) | 420 ns | 15.9 µs | 26.1 µs |
| GraalWasm under GraalVM Native Image | 181 ns | 19.8 µs | — |
| GraalWasm on Temurin 25 (interpreter fallback) | 3.1 µs | 850 µs | 867 µs |

Three things those rows say plainly. On a stock OpenJDK, GraalWasm has no JIT: the engine prints a fallback-runtime warning at startup (`-Dpolyglot.engine.WarnInterpreterOnly=false` silences it) and runs the module interpreted, at roughly 50x the FFM cost per call and slower than `UUID.randomUUID()`. The JIT numbers need a GraalVM JDK or a Native Image build; nothing in this jar can change that. And the batch doors are where the two paths meet: one crossing per thousand UUIDs, and the byte fill lands at parity with FFM under the JIT. (The `UUID[]` fill reads faster than FFM here because it copies the bytes out in one crossing and builds the objects in plain Java; the FFM path's own number for that door is what it is.) The per-call gap is the polyglot crossing itself — each export is resolved once and called through its cached `Value`, and a UUID comes back in one 16-byte read, which together took the call from 675 ns to 420 ns; what remains is the engine's host-to-guest entry plus the lock.

One number from the same Native Image binary that is not about this backend: the FFM path itself measured 6.5 µs per `newV7(long)` under Native Image in this harness, a hundred times its JVM cost and far behind the wasm module in the same binary. That is a finding about `Linker.Option.critical` heap access under Native Image, recorded here because it was measured here, not something the wasm path changes.

**Threading.** A polyglot context does not allow concurrent access from multiple threads, so every call on the wasm path is serialized on one lock; one context and one module instance serve the whole process, which is also what keeps the core's process-wide v7 counter a single sequence. The FFM path has no lock. A hot, multi-threaded generator should expect that difference, not just the per-call one.

**Native Image.** The bundled `reachability-metadata.json` registers `WasmBackend`'s constructor for reflection and the `native/*/*` resource glob already covers the module, so a consumer's `native-image` build of the wasm path needs no extra configuration on this jar's account — verified by building the published jar plus the two GraalWasm artifacts into a native executable and running it with `-Dhyperuuid.backend=wasm` (the 181 ns row above); the same binary run without the property takes the FFM path.

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

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
