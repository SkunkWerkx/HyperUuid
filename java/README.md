# hyperuuid

[![CI](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperUuid/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hyperuuid.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hyperuuid)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperUuid/blob/master/LICENSE)

**`java.util.UUID` can generate v4 (random) and v3 (MD5 name-based) — that's it. No v5, no v7, no batch API. This binding gives you the whole RFC.**

RFC 9562 UUID v4 (random), v5 (deterministic) and v7 (time-sortable) generation, with v7's version 8 SQL Server order, calling directly into the native `libhyperuuid` shared library via `java.lang.foreign` (FFM) downcalls — no runtime bridge, no extra runtime dependency (plain Java, not Kotlin: see the root README for why that matters). JDK 25 is the floor: the first long-term-support release with the final FFM API (JEP 454 finalized it in JDK 22, and 22 through 24 are past end of life). The jar bundles a native build for every supported platform (Linux glibc, Linux musl, macOS, Windows × x64/arm64) under `/native/{rid}/` and picks the right one at runtime.

```java
import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;

UUID id = UuidGenerator.newV4();
UUID id2 = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "example.com");
UUID id3 = UuidGenerator.newV7();

Instant created = UuidGenerator.v7Timestamp(id3);

// Checked first: Optional.empty() for anything that isn't an RFC 9562 v7:
Optional<Instant> maybeCreated = UuidGenerator.getTimestamp(id3);

// Byte order SQL Server's uniqueidentifier needs on the wire to sort by creation order:
UUID sqlOrdered = UuidGenerator.v7ToSqlOrder(id3);

// One downcall, one random-bytes fetch, one counter reservation for the whole batch:
UUID[] batch = UuidGenerator.newV7Batch(1000);
```

Returns plain `java.util.UUID` — no wrapper type, so it works everywhere a `UUID` already does (equality, hashing, `Comparable`, JPA/Hibernate entity IDs, `toString()`). `UuidGenerator.Namespaces.DNS`/`URL`/`OID`/`X500` are RFC 9562 §6.6's well-known namespaces; `UuidGenerator.NIL`/`MAX` are the §5.9/§5.10 special values. `newV7` also accepts an `Instant` directly (`newV7(Instant)`), not just a raw millisecond count; `getTimestamp` is the checked counterpart to `v7Timestamp`: it checks first, in one call into the core, and returns `Optional.empty()` for anything that isn't an RFC 9562 version 7 UUID, instead of assuming the caller already knows. The variant is checked as well as the version, so a 7 nibble under the NCS, Microsoft or future variant has no timestamp, and only version 7 timestamps are read: a version 1 or 6 from another generator has none. It also reads SQL-ordered values in place; see [Inspection and layouts](#inspection-and-layouts).

## Gating on the core

Nothing loads until the first call that needs the core — `UuidGenerator.NIL`/`MAX` and `Namespaces` never do. A consumer with a fallback of its own gates on `UuidGenerator.isAvailable()` first: probed once, cached, never throws — `false` when the platform library will not open, the jar carries no core for this OS/arch, or an older core lacks an export this binding was built against. The generating methods do not fall back. The first one to need a core that failed to load throws `ExceptionInInitializerError`, and every later one `NoClassDefFoundError`; either way the original failure is in its cause chain. `UuidGenerator.nativeVersion()` reports the loaded core's `major.minor.patch`, and succeeds exactly when `isAvailable()` is `true` — the pair that proves the library that resolved is the one this jar was built against.

```java
UUID id = UuidGenerator.isAvailable() ? UuidGenerator.newV7() : UUID.randomUUID();
```

A batch is bounded by its own output: `count * 16` bytes has to be an array, so no batch can exceed `Integer.MAX_VALUE / 16` (134,217,727) UUIDs, and a negative count throws `IllegalArgumentException`. A version 7 batch is bounded tighter still, by its counter: `UuidGenerator.MAX_V7_BATCH` (`1 << 26`, 67,108,864) per call across `newV7Batch` and both `fillV7` forms. Every v7 batch up to that size is in strictly increasing order. The counter is one process-wide sequence, so a batch can straddle the point where it wraps back to 0, and the UUIDs from there on carry a timestamp 1 ms later than the one supplied rather than sorting before the ones ahead of them. A larger batch would have to reuse counter values within one millisecond, so it throws `IllegalArgumentException` naming the limit, before anything is allocated or written. The roll-forward orders one batch, not the stream: the next batch or `newV7` call in the same real millisecond starts its counter just past the wrap and carries the supplied timestamp, so it sorts before the previous batch's tail, which was stamped a millisecond later. Two single calls either side of the wrap in one millisecond sort in reverse the same way. It happens at most once per `MAX_V7_BATCH` UUIDs the process mints. Every batch failure that is the caller's (a timestamp out of range, a batch too large) is an `IllegalArgumentException`. Only a random-source failure is an `IllegalStateException`.

## Native access

FFM downcalls are a *restricted* operation: the JDK wants the application, not a library on its classpath, to say that native code may run. Without that the first call still works, and prints a four-line warning naming `java.lang.foreign.Linker::downcallHandle` and ending "Restricted methods will be blocked in a future release unless native access is enabled". Grant it where the JVM is launched:

```sh
java --enable-native-access=ALL-UNNAMED -cp app.jar:hyperuuid.jar com.example.Main        # on the classpath
java --enable-native-access=io.github.skunkwerkx.hyperuuid -p mods -m com.example.app     # on the module path
```

The jar's manifest carries `Automatic-Module-Name: io.github.skunkwerkx.hyperuuid`, so the module-path form has a stable name to grant rather than one derived from the jar's file name. An executable jar can make the same grant in its own manifest (`Enable-Native-Access: ALL-UNNAMED`). Under `--illegal-native-access=deny` with no grant the library cannot be loaded at all: `isAvailable()` is `false` and the generating methods throw.

## Why not `java.util.UUID`?

The honest answer for v5 and v7 is that there's no comparison to make — the JDK's own `UUID` class has never shipped them. Specifics, checked against the actual JDK source and OpenJDK's own issue tracker rather than assumed:

1. **No v5 at all.** `UUID.nameUUIDFromBytes()` is MD5-based — that's RFC 9562 v3, not v5. If you need deterministic namespace-based UUIDs that agree with every other RFC 9562 implementation (SHA-1, not MD5), the JDK has never had a built-in way to do it. This binding's v5 output is verified in CI to match RFC 9562's own Appendix A.4 test vector and Python's `uuid.uuid5` byte-for-byte.
2. **No v7 in any released JDK.** A v7 factory (`UUID.ofEpochMillis`) is in progress upstream — [JDK-8357251](https://bugs.openjdk.org/browse/JDK-8357251) / [JDK-8334015](https://bugs.openjdk.org/browse/JDK-8334015) — but unshipped as of any JDK release. This binding gives you one today.
3. **A real monotonic counter for v7.** A process-global counter (RFC 9562 §6.2 Method 1) guarantees strict creation order under concurrency, across both individual and batch calls.
4. **Batch generation.** `newV7Batch(count)` shares one timestamp capture, one random-bytes fetch, and one counter reservation across the whole batch, instead of paying per-item native-call overhead N times. `java.util.UUID` has no bulk generation API at all.
5. **Cross-language consistency.** The same Rust core mints v5/v7 UUIDs for every other binding in this repo — a Java service and a Python or Go service produce byte-identical v5 UUIDs for the same `(namespace, name)`, which no per-language reimplementation can structurally guarantee.
6. **SQL Server byte ordering.** `UuidGenerator.v7ToSqlOrder(id3)` converts a version 7 UUID to an RFC 9562 version 8 (§5.8, the version for custom layouts) that `System.Data.SqlTypes.SqlGuid` comparison — and therefore T-SQL `ORDER BY` on a `uniqueidentifier` column — sorts by creation order. The 8 and the RFC variant sit where SQL Server's text form shows them, and the other 122 bits hold all of the version 7's, so every version 7 converts and `v7FromSqlOrder` gives back the exact original. Both directions throw `IllegalArgumentException` for a value of the wrong version: anything but an RFC 9562 version 7 going in, anything without the layout's 8 and variant coming back. The layout is computed in the native Rust core and verified there (and independently against the real `SqlGuid` comparator in the C# binding's own test suite) rather than reimplemented in Java. Bind the result as bytes, never with `setObject(UUID)`: [Binding to SQL Server](#binding-to-sql-server) has why and how.

The honest trade-off: this is a native library dependency (a platform-specific `libhyperuuid.so`/`.dylib`/`.dll` bundled per-RID inside the jar) instead of a type that's always sitting in `java.util`. If plain v4 randomness is all you need, `UUID.randomUUID()` is simpler and that's a completely reasonable choice.

## Destination-buffer fills

`fillV7` writes into an array you already own instead of allocating a fresh one per call, over either a `UUID[]` or a `byte[]`:

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
| `newV7` x1000 individually | 34.9 µs | 32,000 |
| `newV7Batch(1000)` | 9.3 µs | 52,104 |
| `fillV7(UUID[])` into an existing array | 12.3 µs | 48,088 |
| `fillV7(byte[])` into an existing buffer | **3.6 µs** | **0** |

The middle two rows are the point: filling a `UUID[]` is no cheaper than allocating a fresh one, and measures slower here. The allocation is not the expensive part — rebuilding a thousand `UUID` objects from RFC bytes is. Only the `byte[]` form escapes that, and it does so with **nothing allocated and nothing copied**: the caller's array is pinned and handed to the native side, which writes every UUID straight into it.

A `byte[]` whose length isn't a multiple of 16 throws `IllegalArgumentException`.

### Raw-byte SQL-order transforms

`v7ToSqlOrder(byte[])` and `v7FromSqlOrder(byte[])` apply the same native conversion in place on a caller's 16 bytes, and refuse the same values with the same `IllegalArgumentException`, leaving the bytes untouched. Bytes alone don't say which order they are in, so one case slips through: bytes already in SQL order pass for an RFC version 7 about one time in 16, and `v7ToSqlOrder(byte[])` then converts them again instead of refusing. Track which order a buffer holds. Being pure byte-in/byte-out, they're the form a byte-level correctness oracle can be pointed at directly — the same cross-check every binding here makes against the one native implementation.

## Inspection and layouts

The core answers what a UUID is, so nothing here reads bits in Java:

```java
UuidGenerator.version(id);                 // the version nibble, 0-15 (agrees with id.version())
UuidGenerator.variant(id);                 // UuidVariant.NCS / RFC_9562 / MICROSOFT / FUTURE
UuidGenerator.isRfc(id, 7);                // the RFC variant and version 7, in one call

UUID sql = UuidGenerator.v7ToSqlOrder(id);
UuidGenerator.version(sql, UuidLayout.SQL_SERVER);        // 7
UuidGenerator.isRfc(sql, 7, UuidLayout.SQL_SERVER);       // true
UuidGenerator.getTimestamp(sql, UuidLayout.SQL_SERVER);   // read in place, no conversion back; variant checked
```

`isRfc` is the guard to run before trusting a value's version-specific fields, such as `v7UnixMillis`. A version outside 0-15 (or negative) is simply `false`. `UuidVariant` covers the four RFC 9562 §4.1 variants. `variant` never returns its `UNSPECIFIED`, which is there only so each constant's `code()` matches the core's. `variant` reads RFC 9562 order only, and has no layout form. Don't feed it a SQL-ordered value or a `uniqueidentifier` read-back. To validate one of those, use `isRfc` with `UuidLayout.SQL_SERVER`, which checks the variant where that layout puts it.

`UuidLayout` says which byte order a value is held in: `RFC_9562`, what every other method takes and returns, or `SQL_SERVER`, what `v7ToSqlOrder` returns. A SQL-ordered value is still a `java.util.UUID` whose two longs are the SQL Server wire bytes, most significant first. `version`, `isRfc`, `v7UnixMillis`, `v7Timestamp` and `getTimestamp` each take a layout. Without one they mean RFC 9562. Every SQL-ordered value is a version 8, but its 8 sits at octet 7, where SQL Server's mixed-endian text form shows it; a `UUID` holding the wire bytes keeps key bits where `UUID.version()` looks, so neither that nor `version(sqlOrdered)` reads 8. In `SQL_SERVER`, `version` answers 7, the version a value was converted from, once the core has checked the 8 and the variant, and 0 for bytes that don't carry them. RFC-ordered bytes can still happen to carry the 8 and the variant there (about one random v4 in 16 does), so the layout is the caller's to track; it can't be inferred from the value. Carry it with the value, and pass it at every call site that can see a SQL-ordered value. The forms without a layout read RFC order, so `isRfc(sqlOrdered, 7)` reads the wrong bytes; there is deliberately no layout-agnostic check. `UuidLayout.UNSPECIFIED` throws `IllegalArgumentException` and a `null` layout throws `NullPointerException`. Neither guesses.

`version`, `variant` and `isRfc` also take 16 raw bytes (`byte[]`), already in the layout's order: for `SQL_SERVER`, the bytes SQL Server stores, as `v7ToSqlOrder(byte[])` writes them. Any other length throws `IllegalArgumentException`.

## Binding to SQL Server

Bind `v7ToSqlOrder`'s result with `setBytes`, never `setObject(UUID)`. mssql-jdbc 13.6 sends a
`UUID`, with or without `microsoft.sql.Types.GUID`, and a `String`, in SQL Server's mixed-endian
text order, which stores the first three groups byte-reversed and loses the creation order
([details](https://github.com/SkunkWerkx/HyperUuid#sql-server-ordering)). It has no connection property that changes that. Bytes go in as binary and
land byte for byte, and `getBytes` on the column reads them back untouched:

```java
UUID stored = UuidGenerator.v7ToSqlOrder(id3);
byte[] bytes = ByteBuffer.allocate(16)
        .putLong(stored.getMostSignificantBits())
        .putLong(stored.getLeastSignificantBits())
        .array();
insert.setBytes(1, bytes);

byte[] raw = rows.getBytes("id");                          // the uniqueidentifier's own bytes
UuidGenerator.version(raw, UuidLayout.SQL_SERVER);         // 7
```

`getObject(…, UUID.class)` and `getString` both return the value with those groups reversed.

## Benchmarks

Real numbers, [JMH](https://github.com/openjdk/jmh) (`./gradlew :benchmarks:jmh`), linux-x64 (an Intel Core i9-11900H), JDK 25, 3 warmup + 5 measurement iterations, average time mode, `-prof gc` for the allocation column:

| Method | Mean | B/op | vs. `UUID.randomUUID()` |
| --- | ---: | ---: | ---: |
| `UUID.randomUUID()` | 160.6 ns | 128 | baseline |
| `UuidGenerator.newV4()` | 31.5 ns | 32 | **5.1x faster** |
| `UuidGenerator.newV5()` | 57.3 ns | 64 | **2.8x faster** |
| `UuidGenerator.newV7()` | 35.8 ns | 32 | **4.5x faster** |

**Why the doors are this cheap:** no door opens an arena or copies its input. Every downcall but the version probe's is linked `Linker.Option.critical(true)`, so a caller's own `byte[]` (a v5 name, a batch destination, sixteen bytes to reorder in place) is pinned and handed to the native side directly, and the single-UUID doors use one per-thread 16-byte in/out scratch for the life of the thread, written and read as two big-endian longs with no `byte[]` in between. Sound because every export is a short, non-blocking computation over the bytes it was handed that never calls back into Java — the profile the option exists for — and `reachability-metadata.json` registers it, so the GraalVM Native Image smoke test proves it under AOT too. The 32 bytes left per call are the `UUID` object itself.

The FFM downcall doesn't lose to the JDK's own generator, and the reason is worth stating so the win isn't mistaken for a rigged comparison: `UUID.randomUUID()` is genuinely slow, largely because it goes through `java.security.SecureRandom` by default. How slow is the machine's business — what its random source costs sets the width of the gap, and a box where that is expensive shows a far larger multiple than this one does. Reported as measured, not adjusted to make the story better.

Batch generation vs. an equivalent loop:

| Method | 1000 individual calls | `*Batch(1000)` | Speedup |
| --- | ---: | ---: | ---: |
| v7 | 34.9 µs | 9.3 µs | **3.7x** |

The batch multiplier is modest for the best reason available: the individual calls are cheap, so there is little waste left to amortize. The `byte[]` fill is where the rest goes: 3.6 µs, 9.7x over the loop.

Reproduce: `./gradlew :benchmarks:jmh`.

## AOT

Verified against a real GraalVM Native Image build, not just claimed compatible — see `aot-smoke-test/` (`./gradlew :aot-smoke-test:nativeRun`), which builds and runs a genuine standalone native binary that calls every public method of `UuidGenerator` — the `isAvailable()`/`nativeVersion()` probe, each generator in each of its overloads, the batch and fill forms over both `UUID[]` and `byte[]`, the SQL/RFC byte-order conversions in their `UUID` and raw-byte forms, and the version/variant/layout inspection — no JVM required to run it. Needed a bundled `META-INF/native-image/.../reachability-metadata.json` to register each distinct FFM downcall *signature* ahead of time (GraalVM's reachability analysis is per-signature, not per-function — the two SQL-order conversions share `(ADDRESS)int` with `uuid_new_v4` and `uuid_variant`, and a signature missing from the file is enough to build clean and crash at runtime; the generators, the inspection and the timestamp exports fill eight more, and the version probe's `()int`, the one downcall not linked critical, is a tenth entry of its own) and a `resources` glob covering `native/*/*` — both already shipped in this jar, verified by actually building and running the resulting executable with no JVM anywhere on `PATH`, so a consumer's own `native-image` build picks it up automatically with zero extra config. The jar's `native-image.properties` rides along the same way, and it is what keeps the downcalls compiled rather than interpreted in the image: 57 ns per `newV7(long)` there (the table below).

A native image still loads the core's shared library at startup, extracted from the jar as on the JVM, rather than linking the static archive into the executable the way C# Native AOT and Go do. Linking it in works on Linux, but only through GraalVM's internal builder API, with separate linker handling for each OS, so it is deliberately not done; the forge's [levers not pulled](https://github.com/SkunkWerkx/.github#levers-deliberately-not-pulled) table has the full reasoning, the proven recipe, and what would change the answer.

**What it costs.** There is no JMH to run in an image, so these rows are a hand loop — warm up, then one million `newV7(long)` calls and three thousand of each fill, best of five rounds — built into a native image with the metadata the jar ships, on linux-x64 (an Intel Core i9-11900H), one session. The first row is the same loop on the JVM. Its `newV7(long)` and byte-fill columns land on the JMH figures above, which is what makes the loop trustworthy for the row JMH cannot produce; its `UUID[]` fill does not (7.9 µs against JMH's 12.3 µs), so that column compares the image with this loop on the JVM, not with JMH:

| Runtime, hand loop | `newV7(long)` | `fillV7(byte[16000])` | `fillV7(UUID[1000])` |
| --- | ---: | ---: | ---: |
| FFM downcall, GraalVM CE 25.4 JVM | 38 ns | 3.5 µs | 7.9 µs |
| FFM downcall, Native Image | 57 ns | 3.7 µs | 19.5 µs |

A native image costs more than the JVM, up to about two and a half times, which is the ordinary price of an ahead-of-time compiler without a profile. The byte fill is the exception, level in both, because nearly all of it is native work.

FFM is compiled in the image, not interpreted, because the downcall handles are constants there: they are created from the C signature alone in a class that needs nothing from the library, take the export's address as their first argument, and `META-INF/native-image/.../native-image.properties` in the jar has that class initialized at image build time. A consumer's `native-image` build inherits it with no configuration.

## Verifying provenance

The published jar verifies from the release tag alone, and so do the sources and javadoc jars,
the `.pom` and their signatures: the release attestation, signed by this repository's
`release.yml` at the tag, names every file of the Central bundle among everything the release
ships, with the release inventory as its predicate.

```sh
curl -LO https://repo1.maven.org/maven2/io/github/skunkwerkx/hyperuuid/<version>/hyperuuid-<version>.jar
gh attestation verify hyperuuid-<version>.jar --repo SkunkWerkx/HyperUuid \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Its build provenance is the second check. The bundle is assembled by `hyper-pack-maven.yml`,
a reusable workflow that lives in `SkunkWerkx/.github`, and that is the identity Fulcio
records as the signer, so `--repo` alone is not enough there: add `--signer-repo`, or use
`--owner` in place of both. The native libraries inside the jar carry their own, from the
Forge's build workflows at the preparation commit on master, and the pack job verifies each of
them before it goes into the jar.

```sh
gh attestation verify hyperuuid-<version>.jar \
  --repo SkunkWerkx/HyperUuid --signer-repo SkunkWerkx/.github
# or: gh attestation verify hyperuuid-<version>.jar --owner SkunkWerkx
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

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
