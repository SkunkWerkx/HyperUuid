# hyperuuid

**`java.util.UUID` can generate v4 (random) and v3 (MD5 name-based) — that's it. No v5, no v6, no v7, no batch API. This binding gives you the whole RFC.**

RFC 9562 UUID v4 (random), v5 (deterministic), v6 and v7 (time-sortable) generation, calling directly into the native `libhyperuuid` shared library via the Java Foreign Function & Memory API (stable since JDK 22 / JEP 454) — no runtime bridge, no reflection, no extra runtime dependency (plain Java, not Kotlin: see the root README for why that matters). This jar bundles a native build for every supported platform and picks the right one at runtime.

```java
import io.github.buvinghausen.hyperuuid.UuidGenerator;
import java.util.UUID;

UUID id = UuidGenerator.newV4();
UUID id2 = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "example.com");
UUID id3 = UuidGenerator.newV6();
UUID id4 = UuidGenerator.newV7();

Instant created = UuidGenerator.v7Timestamp(id4);

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(id4);

// One downcall, one random-bytes fetch, one counter reservation for the whole batch:
UUID[] batch = UuidGenerator.newV7Batch(1000);
```

Returns plain `java.util.UUID` — no wrapper type, so it works everywhere a `UUID` already does (equality, hashing, `Comparable`, JPA/Hibernate entity IDs, `toString()`). `UuidGenerator.Namespaces.DNS`/`URL`/`OID`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.NIL`/`MAX` are the §5.9/§5.10 special values.

## Why not `java.util.UUID`?

The honest answer for versions v6/v7 is that there's no comparison to make — the JDK's own `UUID` class has never shipped them. Specifics, checked against the actual JDK source and OpenJDK's own issue tracker rather than assumed:

1. **No v5 at all.** `UUID.nameUUIDFromBytes()` is MD5-based — that's RFC 9562 v3, not v5. If you need deterministic namespace-based UUIDs that agree with every other RFC 9562 implementation (SHA-1, not MD5), the JDK has never had a built-in way to do it. This binding's v5 output is verified in CI to match RFC 9562's own Appendix A.4 test vector and Python's `uuid.uuid5` byte-for-byte.
2. **No v6/v7 in any released JDK.** A v7 factory (`UUID.ofEpochMillis`) is in progress upstream — [JDK-8357251](https://bugs.openjdk.org/browse/JDK-8357251) / [JDK-8334015](https://bugs.openjdk.org/browse/JDK-8334015) — but unshipped as of any JDK release. This binding gives you both today, including v6 (RFC 9562 §5.6's field-compatible reordering of v1), which isn't part of that upstream proposal at all.
3. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency, across both individual and batch calls.
4. **Batch generation.** `newV7Batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch, instead of paying per-item native-call overhead N times. `java.util.UUID` has no bulk generation API at all.
5. **Cross-language consistency.** The same Rust core mints v5/v6/v7 UUIDs for every other binding in this repo — a Java service and a Python or Go service produce byte-identical v5 UUIDs for the same `(namespace, name)`, which no per-language reimplementation can structurally guarantee.
6. **SQL Server byte ordering.** `UuidGenerator.v7ToSqlOrder(id4)` converts a version 7 UUID to the byte order `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — needs to sort by creation order (`v6ToSqlOrder` does the same for version 6, though same-millisecond v6 UUIDs aren't guaranteed to sort correctly since v6 has no counter), computed once in the native Rust core and verified there (and independently against the real `SqlGuid` comparator in the C# binding's own test suite) rather than reimplemented in Java. One caveat worth being direct about: this is verified at the raw-byte level against .NET's own `Guid` wire format, which ADO.NET passes through unchanged — it has *not* been checked against any specific JDBC driver's own `uniqueidentifier` parameter binding, which may or may not apply a further transform of its own. Verify against your driver, or bind the returned bytes directly, before relying on it in a JDBC-facing query.

The honest trade-off: this is a native library dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID inside the jar) instead of a type that's always sitting in `java.util`. If plain v4 randomness is all you need, `UUID.randomUUID()` is simpler and that's a completely reasonable choice.

## Benchmarks

Real numbers, [JMH](https://github.com/openjdk/jmh) (`./gradlew :benchmarks:jmh`), linux-arm64, JDK 25, 3 warmup + 5 measurement iterations, average time mode:

| Method | Mean | vs. `UUID.randomUUID()` |
| --- | ---: | ---: |
| `UUID.randomUUID()` | 1111.64 ns | baseline |
| `UuidGenerator.newV4()` | 152.82 ns | **7.27x faster** |
| `UuidGenerator.newV5()` | 221.03 ns | **5.03x faster** |
| `UuidGenerator.newV6()` | 118.49 ns | **9.38x faster** |
| `UuidGenerator.newV7()` | 136.37 ns | **8.15x faster** |

Unlike the Ruby/PHP bindings in this repo, the FFM downcall here doesn't lose to the JDK's own generator — `UUID.randomUUID()` is genuinely slow, largely because it goes through `java.security.SecureRandom` by default, not because the comparison is unfair to it. Reported as measured, not adjusted to make the story better.

Batch generation vs. an equivalent loop:

| Method | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v7 | 124.39 µs | 31.80 µs | **3.91x** |
| v6 | 107.47 µs | 36.95 µs | 2.91x |

Reproduce: `./gradlew :benchmarks:jmh`.

## AOT

Verified against a real GraalVM Native Image build, not just claimed compatible — see `aot-smoke-test/` (`./gradlew :aot-smoke-test:nativeRun`), which builds and runs a genuine standalone native binary exercising v4/v5/v6/v7 generation and batch, no JVM required to run it. Needed a bundled `META-INF/native-image/.../reachability-metadata.json` to register the FFM downcall shapes ahead of time — already shipped in this jar, so a consumer's own `native-image` build picks it up automatically.

## Install

Published to this repo's GitHub Packages Maven registry:

```kotlin
repositories {
    maven { url = uri("https://maven.pkg.github.com/SkunkWerkx/HyperUuid") }
}
dependencies {
    implementation("io.github.buvinghausen:hyperuuid:<version>")
}
```

See [the repo root README](../README.md) for the full RFC 9562 coverage table and the state of every other language binding.
