package io.github.skunkwerkx.hyperuuid;

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
import java.nio.ByteOrder;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;

/**
 * RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7 time-sortable) calling directly
 * into the native {@code libhyperuuid} shared library via the Java Foreign Function &amp; Memory
 * API (stable since JDK 22 / JEP 454) — no runtime bridge, no reflection, no extra runtime
 * dependency (plain Java rather than Kotlin: {@code kotlin-stdlib} would otherwise be a real
 * transitive dependency for every consumer, unlike every other binding in this repo).
 *
 * <p>Nothing is copied on the way across. Every downcall is linked
 * {@link Linker.Option#critical(boolean) critical(true)}, so a caller's own {@code byte[]} — a
 * v5 name, a batch destination, sixteen bytes to reorder in place — is pinned and handed to the
 * native side directly, and the 16-byte in/out scratch the single-UUID doors need lives in one
 * per-thread segment for the life of the thread rather than a confined {@link Arena} opened
 * and torn down on every call. The underlying Rust core never allocates for these calls
 * either. This jar bundles a native build for every supported platform (see
 * {@link NativePlatform}) and picks the right one at runtime.
 *
 * <p>The same core also ships inside this jar as a {@code wasm32-wasip1} module, run by
 * <a href="https://www.graalvm.org/webassembly/">GraalWasm</a> when {@link #BACKEND_PROPERTY}
 * says so or when no native build exists for the running platform. That path needs
 * {@code org.graalvm.polyglot:polyglot} and {@code org.graalvm.polyglot:wasm} on the
 * classpath (optional dependencies, never pulled in transitively), serializes every call on
 * one lock, and costs several times a native downcall per operation; {@link #backend()}
 * reports which path is active. Everything else — every method, every exception, every
 * message — is identical between the two.
 */
public final class UuidGenerator {
    private UuidGenerator() {}

    /** Well-known namespace UUIDs defined in RFC 9562 Section 6.6. */
    public static final class Namespaces {
        /** The DNS namespace UUID. */
        public static final UUID DNS = UUID.fromString("6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        /** The URL namespace UUID. */
        public static final UUID URL = UUID.fromString("6ba7b811-9dad-11d1-80b4-00c04fd430c8");
        /** The ISO OID namespace UUID. */
        public static final UUID OID = UUID.fromString("6ba7b812-9dad-11d1-80b4-00c04fd430c8");
        /** The X.500 DN namespace UUID. */
        public static final UUID X500 = UUID.fromString("6ba7b814-9dad-11d1-80b4-00c04fd430c8");

        private Namespaces() {}
    }

    /**
     * Name of the system property that picks the interop path: {@code "native"} for the FFM
     * downcalls into the bundled platform library, {@code "wasm"} for the bundled
     * {@code wasm32-wasip1} module run by GraalWasm. Unset means native when this platform's
     * library is bundled, wasm otherwise.
     */
    public static final String BACKEND_PROPERTY = "hyperuuid.backend";

    /**
     * Non-null only when the wasm path was selected — see {@link #selectWasm()}. Every public
     * method checks this one {@code static final} against {@code null} before its FFM path;
     * the JIT folds that check away, so the native path costs exactly what it did before a
     * second backend existed.
     */
    private static final Backend WASM = selectWasm();

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = WASM == null ? loadLibrary() : null;

    // critical(true) is what lets a heap segment (MemorySegment.ofArray over the caller's
    // byte[]) cross without being copied into native memory first: the array is pinned for
    // the duration of the call instead. The contract in exchange — the callee must be short,
    // must not block, and must never upcall into Java — is exactly what every export here is:
    // a bounded computation over the bytes it was handed, with no callbacks.
    private static final Linker.Option CRITICAL = Linker.Option.critical(true);

    private static final MethodHandle UUID_NEW_V4 = downcall("uuid_new_v4", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V5 = downcall("uuid_new_v5", FunctionDescriptor.of(
                    ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V6 = downcall("uuid_new_v6", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_UNIX_MILLIS = downcall("uuid_v6_unix_millis", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V6_BATCH = downcall("uuid_new_v6_batch", FunctionDescriptor.of(
                    ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V7 = downcall("uuid_new_v7", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_UNIX_MILLIS = downcall("uuid_v7_unix_millis", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_NEW_V7_BATCH = downcall("uuid_new_v7_batch", FunctionDescriptor.of(
                    ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_TO_SQL_ORDER = downcall("uuid_v7_to_sql_order", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V7_TO_RFC_ORDER = downcall("uuid_v7_to_rfc_order", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_TO_SQL_ORDER = downcall("uuid_v6_to_sql_order", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    private static final MethodHandle UUID_V6_TO_RFC_ORDER = downcall("uuid_v6_to_rfc_order", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));

    // Null when the wasm backend is active — the static final MethodHandles above are then
    // never invoked, and there is no library to look symbols up in.
    private static MethodHandle downcall(String symbol, FunctionDescriptor descriptor) {
        if (LOOKUP == null) {
            return null;
        }
        return LINKER.downcallHandle(LOOKUP.find(symbol).orElseThrow(), descriptor, CRITICAL);
    }

    /**
     * Decides the interop path once, at class init, and never again. {@link #BACKEND_PROPERTY}
     * set to {@code "wasm"} forces the GraalWasm backend; {@code "native"} forces FFM (and fails
     * loudly if this platform has no bundled library); unset takes FFM when this platform's
     * native library is bundled and falls back to wasm when it is not — an OS/arch this jar
     * ships no native build for still works, just through the wasm module.
     *
     * <p>{@link WasmBackend} is instantiated by name so that {@code org.graalvm.polyglot} is
     * never loaded unless it is actually going to be used: it is a {@code compileOnly}
     * dependency of this jar, present at runtime only if the consumer added it.
     */
    private static Backend selectWasm() {
        String choice = System.getProperty(BACKEND_PROPERTY);
        boolean nativeAvailable;
        try {
            nativeAvailable = UuidGenerator.class.getResource(NativePlatform.resourcePath()) != null;
        } catch (RuntimeException | LinkageError unsupportedPlatform) {
            // NativePlatform refuses an OS/arch it has no RID for; that is exactly the case the
            // wasm module exists to cover.
            nativeAvailable = false;
        }
        if ("native".equals(choice) || (choice == null && nativeAvailable)) {
            return null;
        }
        if (choice != null && !"wasm".equals(choice)) {
            throw new IllegalStateException(
                    BACKEND_PROPERTY + " must be \"native\" or \"wasm\"; got \"" + choice + "\"");
        }
        if (UuidGenerator.class.getResource(WasmBackend.RESOURCE_PATH) == null) {
            throw new IllegalStateException(choice == null
                    ? NativePlatform.resourcePath() + " classpath resource not found (unsupported "
                            + "platform, or this jar was built without a native library for it), and "
                            + WasmBackend.RESOURCE_PATH + " is not bundled either"
                    : WasmBackend.RESOURCE_PATH + " classpath resource not found (this jar was built "
                            + "without the wasm module)");
        }
        try {
            return (Backend) Class.forName(UuidGenerator.class.getPackageName() + ".WasmBackend")
                    .getDeclaredConstructor()
                    .newInstance();
        } catch (ReflectiveOperationException e) {
            Throwable cause = e.getCause() != null ? e.getCause() : e;
            if (cause instanceof RuntimeException re) {
                throw re;
            }
            throw new IllegalStateException("hyperuuid: could not start the wasm backend", cause);
        } catch (NoClassDefFoundError e) {
            throw new IllegalStateException("hyperuuid: the wasm backend needs GraalWasm on the "
                    + "classpath — add org.graalvm.polyglot:polyglot and org.graalvm.polyglot:wasm "
                    + "(the latter is a POM-type dependency)", e);
        }
    }

    /**
     * Which interop path this process is using: {@code "native"} (FFM downcalls into the
     * bundled platform library) or {@code "wasm"} (the bundled {@code wasm32-wasip1} module run
     * by GraalWasm). Decided once at class init; see {@link #BACKEND_PROPERTY}.
     *
     * @return {@code "native"} or {@code "wasm"}
     */
    public static String backend() {
        return WASM == null ? "native" : WASM.name();
    }

    /** The RFC 9562 §5.9 Nil UUID — all 128 bits zero. */
    public static final UUID NIL = new UUID(0L, 0L);

    /** The RFC 9562 §5.10 Max UUID — all 128 bits one. */
    public static final UUID MAX = new UUID(-1L, -1L);

    // RFC 9562 order is exactly UUID's msb/lsb decomposition, so a UUID is two big-endian
    // longs in a segment — written and read as such, no byte[] in between. Unaligned,
    // because a heap segment over a caller's byte[] carries no alignment guarantee at all
    // (the aligned layout rejects it outright), and on every supported RID an unaligned
    // load of an aligned address costs the same as an aligned one.
    private static final ValueLayout.OfLong BIG_ENDIAN_LONG =
            ValueLayout.JAVA_LONG_UNALIGNED.withOrder(ByteOrder.BIG_ENDIAN);

    /**
     * Per-thread scratch for the single-UUID doors: sixteen bytes to hand a UUID in and
     * sixteen to receive one, allocated once per thread instead of a confined Arena opened
     * and closed on every call (a native allocation plus a scope teardown each time, which
     * was most of what those doors cost). Doors never nest, so no call can observe another's
     * scratch mid-flight; nothing is shared between threads, so no door needs locking.
     */
    private static final class Scratch {
        private final Arena arena = Arena.ofAuto();
        final MemorySegment in = arena.allocate(16, 8);
        final MemorySegment out = arena.allocate(16, 8);
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    private static UUID readUuid(MemorySegment segment, long offset) {
        return new UUID(segment.get(BIG_ENDIAN_LONG, offset), segment.get(BIG_ENDIAN_LONG, offset + 8));
    }

    private static MemorySegment writeUuid(MemorySegment segment, UUID uuid) {
        segment.set(BIG_ENDIAN_LONG, 0, uuid.getMostSignificantBits());
        segment.set(BIG_ENDIAN_LONG, 8, uuid.getLeastSignificantBits());
        return segment;
    }

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

    /**
     * Creates a random UUID version 4 (RFC 9562 §5.4).
     *
     * @return a new random version 4 UUID
     */
    public static UUID newV4() {
        if (WASM != null) {
            return WASM.newV4();
        }
        MemorySegment out = SCRATCH.get().out;
        int rc;
        try {
            rc = (int) UUID_NEW_V4.invokeExact(out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v4 downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v4 failed with code " + rc + " (random source failure)");
        }
        return readUuid(out, 0);
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a UTF-8
     * name. The same (namespace, name) pair always produces the same UUID.
     *
     * @param namespace the namespace UUID, e.g. one of {@link Namespaces}
     * @param name the UTF-8-encoded name
     * @return the deterministic version 5 UUID for this (namespace, name) pair
     */
    public static UUID newV5(UUID namespace, String name) {
        return newV5(namespace, name, StandardCharsets.UTF_8);
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a name
     * encoded with {@code charset}. The same (namespace, name) pair always produces the same
     * UUID.
     *
     * @param namespace the namespace UUID, e.g. one of {@link Namespaces}
     * @param name the name, encoded with {@code charset}
     * @param charset the charset {@code name} is encoded with
     * @return the deterministic version 5 UUID for this (namespace, name) pair
     */
    public static UUID newV5(UUID namespace, String name, Charset charset) {
        return newV5(namespace, name.getBytes(charset));
    }

    /**
     * Creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
     * bytes. The same (namespace, name) pair always produces the same UUID.
     *
     * @param namespace the namespace UUID, e.g. one of {@link Namespaces}
     * @param name the raw name bytes
     * @return the deterministic version 5 UUID for this (namespace, name) pair
     */
    public static UUID newV5(UUID namespace, byte[] name) {
        if (WASM != null) {
            return WASM.newV5(namespace, name);
        }
        Scratch scratch = SCRATCH.get();
        MemorySegment nsSeg = writeUuid(scratch.in, namespace);
        // The caller's own array crosses pinned; a zero-length name is the ABI's NULL.
        MemorySegment nameSeg = name.length == 0 ? MemorySegment.NULL : MemorySegment.ofArray(name);
        MemorySegment out = scratch.out;
        int rc;
        try {
            rc = (int) UUID_NEW_V5.invokeExact(nsSeg, nameSeg, name.length, out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v5 downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v5 failed with code " + rc);
        }
        return readUuid(out, 0);
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible reordering
     * of version 1 for better sort/index locality, using the current time.
     *
     * @return a new version 6 UUID timestamped at the current time
     */
    public static UUID newV6() {
        return newV6(System.currentTimeMillis());
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a Unix-epoch millisecond
     * timestamp. {@code clock_seq} and {@code node} are randomly generated on every call —
     * unlike version 7, there is no monotonic counter, so calls within the same millisecond
     * are not guaranteed to sort in creation order.
     *
     * @param unixMillis the Unix-epoch millisecond timestamp to embed
     * @return a new version 6 UUID timestamped at {@code unixMillis}
     * @throws IllegalArgumentException if {@code unixMillis} doesn't fit the 60-bit v6
     *     timestamp field
     */
    public static UUID newV6(long unixMillis) {
        if (WASM != null) {
            return WASM.newV6(unixMillis);
        }
        MemorySegment out = SCRATCH.get().out;
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
        return readUuid(out, 0);
    }

    /**
     * Creates a time-sortable UUID version 6 (RFC 9562 §5.6) from an {@link Instant} — pulls
     * the Unix-epoch milliseconds off {@code instant} and mints it through {@link #newV6(long)}.
     *
     * @param instant the timestamp to embed
     * @return a new version 6 UUID timestamped at {@code instant}
     * @throws IllegalArgumentException if {@code instant} doesn't fit the 60-bit v6 timestamp
     *     field
     */
    public static UUID newV6(Instant instant) {
        return newV6(instant.toEpochMilli());
    }

    /**
     * Recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's timestamp
     * field. Only meaningful when {@code uuid}'s version nibble is 6 — the RFC 9562 bit
     * layout doesn't distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp",
     * so the caller is responsible for checking that first if it matters.
     *
     * @param uuid a version 6 UUID
     * @return the embedded Unix-epoch millisecond timestamp
     */
    public static long v6UnixMillis(UUID uuid) {
        if (WASM != null) {
            return WASM.v6UnixMillis(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) UUID_V6_UNIX_MILLIS.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v6_unix_millis downcall failed unexpectedly", t);
        }
    }

    /**
     * Recovers the UTC timestamp embedded in a version 6 UUID as an {@link Instant}. Unlike
     * {@link #v7Timestamp}, this can never realistically overflow: v6's 60-bit tick count,
     * offset from the 1582 UUID epoch rather than 1970, tops out around the year 5236.
     *
     * @param uuid a version 6 UUID
     * @return the embedded UTC timestamp
     */
    public static Instant v6Timestamp(UUID uuid) {
        return Instant.ofEpochMilli(v6UnixMillis(uuid));
    }

    /**
     * Creates {@code count} time-sortable version 6 UUIDs sharing one timestamp capture —
     * one downcall and one random-bytes fetch instead of {@code count} of each. {@code
     * clock_seq} and {@code node} are independently random per item.
     *
     * @param count how many UUIDs to create
     * @param unixMillis the shared Unix-epoch millisecond timestamp to embed in each
     * @return {@code count} new version 6 UUIDs
     * @throws IllegalArgumentException if {@code unixMillis} doesn't fit the 60-bit v6
     *     timestamp field
     */
    public static UUID[] newV6Batch(int count, long unixMillis) {
        if (WASM != null) {
            return WASM.newV6Batch(count, unixMillis);
        }
        if (count == 0) {
            return new UUID[0];
        }
        // One heap array as the destination, crossing pinned, then one UUID per 16 bytes.
        MemorySegment out = MemorySegment.ofArray(new byte[count * 16]);
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
        UUID[] result = new UUID[count];
        for (int i = 0; i < count; i++) {
            result[i] = readUuid(out, (long) i * 16);
        }
        return result;
    }

    /**
     * Creates {@code count} time-sortable version 6 UUIDs sharing the current time.
     *
     * @param count how many UUIDs to create
     * @return {@code count} new version 6 UUIDs timestamped at the current time
     */
    public static UUID[] newV6Batch(int count) {
        return newV6Batch(count, System.currentTimeMillis());
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time.
     *
     * @return a new version 7 UUID timestamped at the current time
     */
    public static UUID newV7() {
        return newV7(System.currentTimeMillis());
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a Unix-epoch millisecond
     * timestamp.
     *
     * @param unixMillis the Unix-epoch millisecond timestamp to embed
     * @return a new version 7 UUID timestamped at {@code unixMillis}
     * @throws IllegalArgumentException if {@code unixMillis} is negative or doesn't fit
     *     within 48 bits
     */
    public static UUID newV7(long unixMillis) {
        if (WASM != null) {
            return WASM.newV7(unixMillis);
        }
        MemorySegment out = SCRATCH.get().out;
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
        return readUuid(out, 0);
    }

    /**
     * Creates a time-sortable UUID version 7 (RFC 9562 §6.2) from an {@link Instant} — pulls
     * the Unix-epoch milliseconds off {@code instant} and mints it through {@link #newV7(long)}.
     *
     * @param instant the timestamp to embed
     * @return a new version 7 UUID timestamped at {@code instant}
     * @throws IllegalArgumentException if {@code instant} is negative or doesn't fit within 48
     *     bits
     */
    public static UUID newV7(Instant instant) {
        return newV7(instant.toEpochMilli());
    }

    /**
     * Recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
     * {@code unix_ts_ms} field. Only meaningful when {@code uuid}'s version nibble is 7 — the
     * RFC 9562 bit layout doesn't distinguish "not a v7 UUID" from "v7 UUID with a very early
     * timestamp", so the caller is responsible for checking that first if it matters.
     *
     * @param uuid a version 7 UUID
     * @return the embedded Unix-epoch millisecond timestamp
     */
    public static long v7UnixMillis(UUID uuid) {
        if (WASM != null) {
            return WASM.v7UnixMillis(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) UUID_V7_UNIX_MILLIS.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_unix_millis downcall failed unexpectedly", t);
        }
    }

    /**
     * Recovers the UTC timestamp embedded in a version 7 UUID as an {@link Instant}. Throws
     * {@link java.time.DateTimeException} for a (spec-valid) embedded timestamp past year
     * 999,999,999 — far beyond the RFC's own 48-bit ceiling (year 10889), so this is
     * unreachable in practice for any genuine version 7 UUID, unlike the corresponding
     * Python/C# bindings.
     *
     * @param uuid a version 7 UUID
     * @return the embedded UTC timestamp
     */
    public static Instant v7Timestamp(UUID uuid) {
        return Instant.ofEpochMilli(v7UnixMillis(uuid));
    }

    /**
     * Recovers the UTC timestamp embedded in {@code uuid}, or {@link Optional#empty()} if it
     * isn't a version 6 or 7 UUID. Unlike {@link #v6Timestamp}/{@link #v7Timestamp}, this checks
     * {@code uuid.version()} itself first, so a caller doesn't need to already know (or
     * separately check) which version {@code uuid} is before asking — delegates straight to
     * whichever of those two methods applies, no bit-layout logic duplicated here.
     *
     * @param uuid any UUID
     * @return the embedded UTC timestamp, or empty if {@code uuid} isn't version 6 or 7
     */
    public static Optional<Instant> getTimestamp(UUID uuid) {
        return switch (uuid.version()) {
            case 6 -> Optional.of(v6Timestamp(uuid));
            case 7 -> Optional.of(v7Timestamp(uuid));
            default -> Optional.empty();
        };
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
     *
     * @param uuid an RFC 9562-ordered version 7 UUID
     * @return {@code uuid} reordered into SQL Server wire order
     */
    public static UUID v7ToSqlOrder(UUID uuid) {
        if (WASM != null) {
            return WASM.v7ToSqlOrder(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            UUID_V7_TO_SQL_ORDER.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_sql_order downcall failed unexpectedly", t);
        }
        return readUuid(seg, 0);
    }

    /**
     * Inverse of {@link #v7ToSqlOrder} — converts a SQL-Server-ordered version 7 {@code uuid}
     * back to RFC 9562 order.
     *
     * @param uuid a SQL-Server-ordered version 7 UUID
     * @return {@code uuid} reordered into RFC 9562 order
     */
    public static UUID v7FromSqlOrder(UUID uuid) {
        if (WASM != null) {
            return WASM.v7FromSqlOrder(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            UUID_V7_TO_RFC_ORDER.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_rfc_order downcall failed unexpectedly", t);
        }
        return readUuid(seg, 0);
    }

    /**
     * Converts an RFC 9562-ordered version 6 {@code uuid} to the byte order SQL Server's
     * {@code uniqueidentifier} needs on the wire to sort by creation order.
     *
     * <p>Same {@code SqlGuid} significance order as {@link #v7ToSqlOrder}, applied to v6's
     * very different field layout. v6 has no monotonic counter the way v7 does; the only
     * field that determines its creation order is the 60-bit timestamp itself, so this moves
     * that whole timestamp — most significant chunk first — into the comparison's most
     * significant bytes. Everything after it — {@code variant}, {@code clock_seq}, and
     * {@code node} (octets 8-15, already one contiguous run with no ordering value of its own —
     * {@code clock_seq}/{@code node} are randomly generated per call, not a counter, and
     * {@code variant} is a fixed constant either way) — moves as that single 8-byte span into
     * the remaining bytes, in the same relative order, not individually reshuffled. Version and
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
     *
     * @param uuid an RFC 9562-ordered version 6 UUID
     * @return {@code uuid} reordered into SQL Server wire order
     */
    public static UUID v6ToSqlOrder(UUID uuid) {
        if (WASM != null) {
            return WASM.v6ToSqlOrder(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            UUID_V6_TO_SQL_ORDER.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v6_to_sql_order downcall failed unexpectedly", t);
        }
        return readUuid(seg, 0);
    }

    /**
     * Inverse of {@link #v6ToSqlOrder} — converts a SQL-Server-ordered version 6 {@code uuid}
     * back to RFC 9562 order.
     *
     * @param uuid a SQL-Server-ordered version 6 UUID
     * @return {@code uuid} reordered into RFC 9562 order
     */
    public static UUID v6FromSqlOrder(UUID uuid) {
        if (WASM != null) {
            return WASM.v6FromSqlOrder(uuid);
        }
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            UUID_V6_TO_RFC_ORDER.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v6_to_rfc_order downcall failed unexpectedly", t);
        }
        return readUuid(seg, 0);
    }

    /**
     * Creates {@code count} time-sortable version 7 UUIDs sharing one timestamp capture and
     * one contiguous block of the monotonic counter — one downcall and one random-bytes
     * fetch instead of {@code count} of each.
     *
     * @param count how many UUIDs to create
     * @param unixMillis the shared Unix-epoch millisecond timestamp to embed in each
     * @return {@code count} new version 7 UUIDs
     * @throws IllegalArgumentException if {@code unixMillis} is negative or doesn't fit
     *     within 48 bits
     */
    public static UUID[] newV7Batch(int count, long unixMillis) {
        if (WASM != null) {
            return WASM.newV7Batch(count, unixMillis);
        }
        if (count == 0) {
            return new UUID[0];
        }
        // One heap array as the destination, crossing pinned, then one UUID per 16 bytes.
        MemorySegment out = MemorySegment.ofArray(new byte[count * 16]);
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
        UUID[] result = new UUID[count];
        for (int i = 0; i < count; i++) {
            result[i] = readUuid(out, (long) i * 16);
        }
        return result;
    }

    /**
     * Creates {@code count} time-sortable version 7 UUIDs sharing the current time.
     *
     * @param count how many UUIDs to create
     * @return {@code count} new version 7 UUIDs timestamped at the current time
     */
    public static UUID[] newV7Batch(int count) {
        return newV7Batch(count, System.currentTimeMillis());
    }

    // ---- Destination-buffer fills ----------------------------------------------------
    //
    // newV6Batch/newV7Batch allocate a fresh UUID[] and a scratch byte[] on every call.
    // These write into storage the caller already owns — the byte[] form pinned and handed to
    // the native side directly, nothing copied — so a hot path can reuse one buffer across
    // batches instead of handing the collector two objects per batch.
    //
    // Unlike the Go and Swift bindings, the UUID[] form still costs a per-element
    // conversion: java.util.UUID is two longs, not 16 RFC-ordered bytes, so every item has
    // to be rebuilt from the native output. That is exactly the C# binding's situation, and
    // it is why the byte[]/MemorySegment forms below are the ones that actually remove work
    // rather than just removing an allocation.

    private static void fillBytesNative(
            MemorySegment out, int count, long unixMillis, MethodHandle handle, String fn) {
        int rc;
        try {
            rc = (int) handle.invokeExact(unixMillis, count, out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: " + fn + " downcall failed unexpectedly", t);
        }
        if (rc == 2) {
            throw new IllegalArgumentException("unixMillis does not fit this version's timestamp field");
        }
        if (rc != 0) {
            throw new IllegalStateException(fn + " failed with code " + rc + " (random source failure)");
        }
    }

    private static void requireWholeUuids(int length) {
        if (length % 16 != 0) {
            throw new IllegalArgumentException(
                    "destination length must be a multiple of 16 (one whole UUID per 16 bytes); got " + length);
        }
    }

    /**
     * Fills {@code destination} with time-sortable version 7 UUIDs sharing one timestamp
     * capture and one contiguous block of the monotonic counter.
     *
     * <p>Writes into an array the caller already owns rather than allocating a new one. Each
     * element is still rebuilt from the native bytes, because {@link UUID} is two longs and
     * not RFC byte order — see {@link #fillV7(byte[], long)} for the form that skips that.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     */
    public static void fillV7(UUID[] destination, long unixMillis) {
        if (WASM != null) {
            WASM.fillV7(destination, unixMillis);
            return;
        }
        fillUuidArray(destination, unixMillis, UUID_NEW_V7_BATCH, "uuid_new_v7_batch");
    }

    /**
     * Fills {@code destination} with version 7 UUIDs sharing the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV7(UUID[] destination) {
        fillV7(destination, System.currentTimeMillis());
    }

    /**
     * Fills {@code destination} with time-sortable version 6 UUIDs sharing one timestamp
     * capture. {@code clock_seq} and {@code node} are independently random per item — unlike
     * version 7 there is no monotonic counter, so items are not guaranteed to sort in
     * creation order.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     */
    public static void fillV6(UUID[] destination, long unixMillis) {
        if (WASM != null) {
            WASM.fillV6(destination, unixMillis);
            return;
        }
        fillUuidArray(destination, unixMillis, UUID_NEW_V6_BATCH, "uuid_new_v6_batch");
    }

    /**
     * Fills {@code destination} with version 6 UUIDs sharing the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV6(UUID[] destination) {
        fillV6(destination, System.currentTimeMillis());
    }

    private static void fillUuidArray(
            UUID[] destination, long unixMillis, MethodHandle handle, String fn) {
        if (destination.length == 0) {
            return;
        }
        MemorySegment out = MemorySegment.ofArray(new byte[destination.length * 16]);
        fillBytesNative(out, destination.length, unixMillis, handle, fn);
        for (int i = 0; i < destination.length; i++) {
            destination[i] = readUuid(out, (long) i * 16);
        }
    }

    /**
     * Fills {@code destination} with raw RFC 9562-ordered version 7 UUID bytes, 16 per UUID.
     *
     * <p>This is the conversion-free form: the native core already writes RFC-ordered bytes
     * contiguously, so nothing is rebuilt on the way out. Prefer it when the destination is a
     * wire buffer or a database parameter that wants bytes anyway.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     *
     * @throws IllegalArgumentException if {@code destination.length} is not a multiple of 16
     */
    public static void fillV7(byte[] destination, long unixMillis) {
        if (WASM != null) {
            WASM.fillV7(destination, unixMillis);
            return;
        }
        fillByteArray(destination, unixMillis, UUID_NEW_V7_BATCH, "uuid_new_v7_batch");
    }

    /**
     * Fills {@code destination} with raw version 7 UUID bytes using the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV7(byte[] destination) {
        fillV7(destination, System.currentTimeMillis());
    }

    /**
     * Fills {@code destination} with raw RFC 9562-ordered version 6 UUID bytes, 16 per UUID.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     *
     * @throws IllegalArgumentException if {@code destination.length} is not a multiple of 16
     */
    public static void fillV6(byte[] destination, long unixMillis) {
        if (WASM != null) {
            WASM.fillV6(destination, unixMillis);
            return;
        }
        fillByteArray(destination, unixMillis, UUID_NEW_V6_BATCH, "uuid_new_v6_batch");
    }

    /**
     * Fills {@code destination} with raw version 6 UUID bytes using the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV6(byte[] destination) {
        fillV6(destination, System.currentTimeMillis());
    }

    private static void fillByteArray(
            byte[] destination, long unixMillis, MethodHandle handle, String fn) {
        requireWholeUuids(destination.length);
        if (destination.length == 0) {
            return;
        }
        // The caller's array is the destination: pinned for the call, written in place.
        fillBytesNative(MemorySegment.ofArray(destination), destination.length / 16, unixMillis, handle, fn);
    }

    // ---- Raw-byte SQL-order transforms -----------------------------------------------
    //
    // The same native permutations as the UUID-taking methods above, rewriting a caller's
    // own 16 bytes in place. Being pure byte-in/byte-out, these are the form a byte-level
    // correctness oracle can be pointed at directly — shared across every binding in this
    // repo rather than re-expressed against each language's own UUID type.

    private static void sqlOrderBytes(byte[] uuid, MethodHandle handle, String fn) {
        if (uuid.length != 16) {
            throw new IllegalArgumentException("a UUID is exactly 16 bytes; got " + uuid.length);
        }
        // In place, on the caller's own bytes — no staging copy in either direction.
        MemorySegment seg = MemorySegment.ofArray(uuid);
        try {
            handle.invokeExact(seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: " + fn + " downcall failed unexpectedly", t);
        }
    }

    /**
     * Rewrites the 16 RFC 9562-ordered version 7 bytes in {@code uuid} into SQL Server
     * {@code uniqueidentifier} sort order, in place. See {@link #v7ToSqlOrder(UUID)}.
     *
     * @param uuid the 16 RFC 9562-ordered bytes, rewritten in place
     */
    public static void v7ToSqlOrder(byte[] uuid) {
        if (WASM != null) {
            WASM.v7ToSqlOrder(uuid);
            return;
        }
        sqlOrderBytes(uuid, UUID_V7_TO_SQL_ORDER, "uuid_v7_to_sql_order");
    }

    /**
     * Inverse of {@link #v7ToSqlOrder(byte[])}, in place.
     *
     * @param uuid the 16 RFC 9562-ordered bytes, rewritten in place
     */
    public static void v7FromSqlOrder(byte[] uuid) {
        if (WASM != null) {
            WASM.v7FromSqlOrder(uuid);
            return;
        }
        sqlOrderBytes(uuid, UUID_V7_TO_RFC_ORDER, "uuid_v7_to_rfc_order");
    }

    /**
     * Rewrites the 16 RFC 9562-ordered version 6 bytes in {@code uuid} into SQL Server
     * {@code uniqueidentifier} sort order, in place. See {@link #v6ToSqlOrder(UUID)}.
     *
     * @param uuid the 16 RFC 9562-ordered bytes, rewritten in place
     */
    public static void v6ToSqlOrder(byte[] uuid) {
        if (WASM != null) {
            WASM.v6ToSqlOrder(uuid);
            return;
        }
        sqlOrderBytes(uuid, UUID_V6_TO_SQL_ORDER, "uuid_v6_to_sql_order");
    }

    /**
     * Inverse of {@link #v6ToSqlOrder(byte[])}, in place.
     *
     * @param uuid the 16 RFC 9562-ordered bytes, rewritten in place
     */
    public static void v6FromSqlOrder(byte[] uuid) {
        if (WASM != null) {
            WASM.v6FromSqlOrder(uuid);
            return;
        }
        sqlOrderBytes(uuid, UUID_V6_TO_RFC_ORDER, "uuid_v6_to_rfc_order");
    }
}
