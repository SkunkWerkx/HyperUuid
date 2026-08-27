package io.github.buvinghausen.hyperuuid;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Instant;
import java.util.UUID;

/**
 * RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
 * into the native {@code libhyperuuid} shared library via the Java Foreign Function & Memory
 * API (stable since JDK 22 / JEP 454) — no runtime bridge, no reflection, no extra runtime
 * dependency (plain Java rather than Kotlin: {@code kotlin-stdlib} would otherwise be a real
 * transitive dependency for every consumer, unlike every other binding in this repo).
 *
 * <p>No allocation beyond the confined {@link Arena} scratch segments here — the underlying
 * Rust core never allocates for these calls either. This is JVM-only: {@code java.lang.foreign}
 * doesn't exist outside the JVM. This jar bundles a native build for every platform (see
 * {@link NativePlatform}) and picks the right one at runtime.
 */
public final class UuidGenerator {
    private UuidGenerator() {}

    /** Well-known namespace UUIDs defined in RFC 9562 Section 6.6. */
    public static final class Namespaces {
        public static final UUID DNS = UUID.fromString("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        public static final UUID URL = UUID.fromString("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
        public static final UUID OID = UUID.fromString("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
        public static final UUID X500 = UUID.fromString("6ba7b814-9dad-11d1-80b4-00c04fd430c8");

        private Namespaces() {}
    }

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = loadLibrary();

    private static final MethodHandle UUID_NEW_V4 = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v4").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V5 = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v5").orElseThrow(),
            FunctionDescriptor.of(
                    ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V6 = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v6").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_UNIX_MILLIS = LINKER.downcallHandle(
            LOOKUP.find("uuid_v6_unix_millis").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V6_BATCH = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v6_batch").orElseThrow(),
            FunctionDescriptor.of(
                    ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V7 = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v7").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_UNIX_MILLIS = LINKER.downcallHandle(
            LOOKUP.find("uuid_v7_unix_millis").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V7_BATCH = LINKER.downcallHandle(
            LOOKUP.find("uuid_new_v7_batch").orElseThrow(),
            FunctionDescriptor.of(
                    ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_TO_SQL_ORDER = LINKER.downcallHandle(
            LOOKUP.find("uuid_v7_to_sql_order").orElseThrow(), FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_TO_RFC_ORDER = LINKER.downcallHandle(
            LOOKUP.find("uuid_v7_to_rfc_order").orElseThrow(), FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_TO_SQL_ORDER = LINKER.downcallHandle(
            LOOKUP.find("uuid_v6_to_sql_order").orElseThrow(), FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_TO_RFC_ORDER = LINKER.downcallHandle(
            LOOKUP.find("uuid_v6_to_rfc_order").orElseThrow(), FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));

    /** The RFC 9562 §5.9 Nil UUID — all 128 bits zero. */
    public static final UUID NIL = new UUID(0L, 0L);

    /** The RFC 9562 §5.10 Max UUID — all 128 bits one. */
    public static final UUID MAX = new UUID(-1L, -1L);

    // The library must outlive every downcall made through it, so it's loaded into the
    // JDK-provided global arena that lives for the process's lifetime rather than one this
    // class would have to remember to keep a reference to.
    private static SymbolLookup loadLibrary() {
        String resourcePath = NativePlatform.resourcePath();
        try (InputStream resource = UuidGenerator.class.getResourceAsStream(resourcePath)) {
            if (resource == null) {
                throw new IllegalStateException(resourcePath
                        + " classpath resource not found (unsupported platform, or this jar was "
                        + "built without a native library for it)");
            }
            String libraryFileName = NativePlatform.current().libraryFileName();
            String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
            Path tmp = Files.createTempFile("hyperuuid", extension);
            tmp.toFile().deleteOnExit();
            Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
            return SymbolLookup.libraryLookup(tmp, Arena.global());
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    /** Creates a random UUID version 4 (RFC 9562 §5.4). */
    public static UUID newV4() {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment out = local.allocate(16);
            int rc;
            try {
                rc = (int) UUID_NEW_V4.invokeExact(out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v4 downcall failed unexpectedly", t);
            }
            if (rc != 0) {
                throw new IllegalStateException("uuid_new_v4 failed with code " + rc + " (random source failure)");
            }
            return readUuid(out);
        }
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8
     * name. The same (namespace, name) pair always produces the same UUID.
     */
    public static UUID newV5(UUID namespace, String name) {
        return newV5(namespace, name, StandardCharsets.UTF_8);
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name
     * encoded with {@code charset}. The same (namespace, name) pair always produces the same
     * UUID.
     */
    public static UUID newV5(UUID namespace, String name, Charset charset) {
        return newV5(namespace, name.getBytes(charset));
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
     * bytes. The same (namespace, name) pair always produces the same UUID.
     */
    public static UUID newV5(UUID namespace, byte[] name) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment nsSeg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(namespace), 0, nsSeg, ValueLayout.JAVA_BYTE, 0, 16);

            MemorySegment nameSeg;
            if (name.length == 0) {
                nameSeg = MemorySegment.NULL;
            } else {
                nameSeg = local.allocate(name.length);
                MemorySegment.copy(name, 0, nameSeg, ValueLayout.JAVA_BYTE, 0, name.length);
            }

            MemorySegment out = local.allocate(16);
            int rc;
            try {
                rc = (int) UUID_NEW_V5.invokeExact(nsSeg, nameSeg, name.length, out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v5 downcall failed unexpectedly", t);
            }
            if (rc != 0) {
                throw new IllegalStateException("uuid_new_v5 failed with code " + rc);
            }
            return readUuid(out);
        }
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
     * of version 1 for better sort/index locality, using the current time.
     */
    public static UUID newV6() {
        return newV6(System.currentTimeMillis());
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a Unix-epoch millisecond
     * timestamp. {@code clock_seq} and {@code node} are randomly generated on every call —
     * unlike version 7, there is no monotonic counter, so calls within the same millisecond
     * are not guaranteed to sort in creation order.
     */
    public static UUID newV6(long unixMillis) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment out = local.allocate(16);
            int rc;
            try {
                rc = (int) UUID_NEW_V6.invokeExact(unixMillis, out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v6 downcall failed unexpectedly", t);
            }
            if (rc == 2) {
                throw new IllegalArgumentException("unixMillis does not fit the 60-bit v6 timestamp field");
            }
            if (rc != 0) {
                throw new IllegalStateException("uuid_new_v6 failed with code " + rc + " (random source failure)");
            }
            return readUuid(out);
        }
    }

    /**
     * Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's timestamp
     * field. Only meaningful when {@code uuid}'s version nibble is 6 — the RFC 9562 bit
     * layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp",
     * so the caller is responsible for checking that first if it matters.
     */
    public static long v6UnixMillis(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                return (long) UUID_V6_UNIX_MILLIS.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v6_unix_millis downcall failed unexpectedly", t);
            }
        }
    }

    /**
     * Recovers the UTC timestamp embedded in a version 6 UUID as an {@link Instant}. Unlike
     * {@link #v7Timestamp}, this can never realistically overflow: v6's 60-bit tick count,
     * offset from the 1582 UUID epoch rather than 1970, tops out around the year 5236.
     */
    public static Instant v6Timestamp(UUID uuid) {
        return Instant.ofEpochMilli(v6UnixMillis(uuid));
    }

    /**
     * Creates {@code count} time-sortable version 6 UUIDs sharing one timestamp capture —
     * one downcall and one random-bytes fetch instead of {@code count} of each. {@code
     * clock_seq} and {@code node} are independently random per item.
     */
    public static UUID[] newV6Batch(int count, long unixMillis) {
        if (count == 0) {
            return new UUID[0];
        }
        try (Arena local = Arena.ofConfined()) {
            MemorySegment out = local.allocate((long) count * 16);
            int rc;
            try {
                rc = (int) UUID_NEW_V6_BATCH.invokeExact(unixMillis, count, out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v6_batch downcall failed unexpectedly", t);
            }
            if (rc == 2) {
                throw new IllegalArgumentException("unixMillis does not fit the 60-bit v6 timestamp field");
            }
            if (rc != 0) {
                throw new IllegalStateException(
                        "uuid_new_v6_batch failed with code " + rc + " (random source failure)");
            }
            return readUuidBatch(out, count);
        }
    }

    /** Creates {@code count} time-sortable version 6 UUIDs sharing the current time. */
    public static UUID[] newV6Batch(int count) {
        return newV6Batch(count, System.currentTimeMillis());
    }

    /** Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time. */
    public static UUID newV7() {
        return newV7(System.currentTimeMillis());
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond
     * timestamp.
     */
    public static UUID newV7(long unixMillis) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment out = local.allocate(16);
            int rc;
            try {
                rc = (int) UUID_NEW_V7.invokeExact(unixMillis, out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v7 downcall failed unexpectedly", t);
            }
            if (rc == 2) {
                throw new IllegalArgumentException("unixMillis must be non-negative and fit within 48 bits");
            }
            if (rc != 0) {
                throw new IllegalStateException("uuid_new_v7 failed with code " + rc + " (random source failure)");
            }
            return readUuid(out);
        }
    }

    /**
     * Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
     * {@code unix_ts_ms} field. Only meaningful when {@code uuid}'s version nibble is 7 — the
     * RFC 9562 bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a very early
     * timestamp", so the caller is responsible for checking that first if it matters.
     */
    public static long v7UnixMillis(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                return (long) UUID_V7_UNIX_MILLIS.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v7_unix_millis downcall failed unexpectedly", t);
            }
        }
    }

    /**
     * Recovers the UTC timestamp embedded in a version 7 UUID as an {@link Instant}. Throws
     * {@link java.time.DateTimeException} for a (spec-valid) embedded timestamp past year
     * 999,999,999 — far beyond the RFC's own 48-bit ceiling (year 10889), so this is
     * unreachable in practice for any genuine version 7 UUID, unlike the corresponding
     * Python/C# bindings.
     */
    public static Instant v7Timestamp(UUID uuid) {
        return Instant.ofEpochMilli(v7UnixMillis(uuid));
    }

    /**
     * Converts an RFC 9562-ordered version 7 {@code uuid} to the byte order SQL Server's
     * {@code uniqueidentifier} needs on the wire to sort by creation order.
     *
     * <p>{@code System.Data.SqlTypes.SqlGuid} comparison — and therefore T-SQL {@code ORDER BY}
     * on a {@code uniqueidentifier} column — doesn't compare a GUID's 16 bytes left to right;
     * it uses a fixed, non-sequential byte significance order. This moves the timestamp and
     * counter (the two fields that determine creation order) into that comparison's
     * most-significant bytes, and moves the trailing entropy, which carries no ordering
     * information, into the least-significant ones as one intact block. The permutation is
     * computed once in the native Rust core and verified there — and independently, against
     * the real {@code System.Data.SqlTypes.SqlGuid} comparator — in this project's C# test
     * suite; this binding calls the same native function rather than reimplementing the math.
     *
     * <p><b>Driver caveat:</b> this returns the raw 16 bytes SQL Server's wire format expects
     * for a {@code uniqueidentifier}, verified at that byte level — not against any specific
     * JDBC driver's own {@code UUID} parameter binding. ADO.NET's {@code Guid} binding applies
     * no further transform of its own (confirmed against the C# binding), so its equivalent
     * method can be passed straight through as an ordinary parameter; whether a given JDBC
     * driver's {@code setObject(UUID)} for a {@code uniqueidentifier} column reorders bytes
     * again on top of this hasn't been checked here — verify against your driver, or bind the
     * bytes directly as a fallback that sidesteps the question entirely.
     *
     * <p>Meaningful only for a genuine version 7 UUID; see {@link #v6ToSqlOrder} for v6.
     */
    public static UUID v7ToSqlOrder(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                UUID_V7_TO_SQL_ORDER.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v7_to_sql_order downcall failed unexpectedly", t);
            }
            return readUuid(seg);
        }
    }

    /**
     * Inverse of {@link #v7ToSqlOrder} — converts a SQL-Server-ordered version 7 {@code uuid}
     * back to RFC 9562 order.
     */
    public static UUID v7FromSqlOrder(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                UUID_V7_TO_RFC_ORDER.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v7_to_rfc_order downcall failed unexpectedly", t);
            }
            return readUuid(seg);
        }
    }

    /**
     * Converts an RFC 9562-ordered version 6 {@code uuid} to the byte order SQL Server's
     * {@code uniqueidentifier} needs on the wire to sort by creation order.
     *
     * <p>Same {@code SqlGuid} significance order as {@link #v7ToSqlOrder}, applied to v6's
     * very different field layout. v6 has no monotonic counter the way v7 does; the only
     * field that determines its creation order is the 60-bit timestamp itself, so this moves
     * that whole timestamp — most significant chunk first — into the comparison's most
     * significant bytes, and relocates {@code clock_seq}/{@code node} (no ordering value —
     * randomly generated per call, not a counter) into the remaining bytes. Version and
     * variant end up at different byte offsets than {@link #v7ToSqlOrder}'s result (octet 8's
     * top nibble and octet 6's top two bits here, not 7/8) — fine, since the two versions are
     * separate methods and a caller always knows which one it's calling.
     *
     * <p>Unlike v7, two version 6 UUIDs minted at the same millisecond have identical
     * timestamp bits — {@code clock_seq}/{@code node} are independently random, not a
     * counter — so this doesn't (and can't) make same-millisecond v6 UUIDs sort in creation
     * order any more than plain RFC order already does. Distinct timestamps sort correctly;
     * same-timestamp ties don't, by the RFC's own v6 design, not a limitation introduced here.
     *
     * <p>Meaningful only for a genuine version 6 UUID.
     */
    public static UUID v6ToSqlOrder(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                UUID_V6_TO_SQL_ORDER.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v6_to_sql_order downcall failed unexpectedly", t);
            }
            return readUuid(seg);
        }
    }

    /**
     * Inverse of {@link #v6ToSqlOrder} — converts a SQL-Server-ordered version 6 {@code uuid}
     * back to RFC 9562 order.
     */
    public static UUID v6FromSqlOrder(UUID uuid) {
        try (Arena local = Arena.ofConfined()) {
            MemorySegment seg = local.allocate(16);
            MemorySegment.copy(RfcBytes.toRfcBytes(uuid), 0, seg, ValueLayout.JAVA_BYTE, 0, 16);
            try {
                UUID_V6_TO_RFC_ORDER.invokeExact(seg);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_v6_to_rfc_order downcall failed unexpectedly", t);
            }
            return readUuid(seg);
        }
    }

    /**
     * Creates {@code count} time-sortable version 7 UUIDs sharing one timestamp capture and
     * one contiguous block of the monotonic counter — one downcall and one random-bytes
     * fetch instead of {@code count} of each.
     */
    public static UUID[] newV7Batch(int count, long unixMillis) {
        if (count == 0) {
            return new UUID[0];
        }
        try (Arena local = Arena.ofConfined()) {
            MemorySegment out = local.allocate((long) count * 16);
            int rc;
            try {
                rc = (int) UUID_NEW_V7_BATCH.invokeExact(unixMillis, count, out);
            } catch (Throwable t) {
                throw new AssertionError("hyperuuid: uuid_new_v7_batch downcall failed unexpectedly", t);
            }
            if (rc == 2) {
                throw new IllegalArgumentException("unixMillis must be non-negative and fit within 48 bits");
            }
            if (rc != 0) {
                throw new IllegalStateException(
                        "uuid_new_v7_batch failed with code " + rc + " (random source failure)");
            }
            return readUuidBatch(out, count);
        }
    }

    /** Creates {@code count} time-sortable version 7 UUIDs sharing the current time. */
    public static UUID[] newV7Batch(int count) {
        return newV7Batch(count, System.currentTimeMillis());
    }

    private static UUID readUuid(MemorySegment segment) {
        return RfcBytes.fromRfcBytes(segment.toArray(ValueLayout.JAVA_BYTE));
    }

    private static UUID[] readUuidBatch(MemorySegment segment, int count) {
        UUID[] result = new UUID[count];
        for (int i = 0; i < count; i++) {
            result[i] = RfcBytes.fromRfcBytes(segment.asSlice((long) i * 16, 16).toArray(ValueLayout.JAVA_BYTE));
        }
        return result;
    }
}
