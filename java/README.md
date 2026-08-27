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

The honest trade-off: this is a native library dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID inside the jar) instead of a type that's always sitting in `java.util`. If plain v4 randomness is all you need, `UUID.randomUUID()` is simpler and that's a completely reasonable choice.

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
