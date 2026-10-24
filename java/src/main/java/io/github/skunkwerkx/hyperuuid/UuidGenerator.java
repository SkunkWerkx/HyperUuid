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
import java.util.Objects;
import java.util.Optional;
import java.util.UUID;

/**
 * RFC 9562 UUID generation (v4 random, v5 deterministic, v7 time-sortable, with a version 8
 * SQL Server order for v7) calling directly
 * into the native {@code libhyperuuid} shared library via the Java Foreign Function &amp; Memory
 * API (JEP 454; this binding's floor is JDK 25) — no runtime bridge, no extra runtime
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
 * <p>Nothing loads until the first call that needs the core. A consumer with a fallback of
 * its own gates on {@link #isAvailable()} first — the one probe that never throws — because
 * the generating methods do not fall back: a core that failed to load is thrown from every
 * one of them as the failure it was. {@link #nativeVersion()} names the core that loaded.
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
     * The downcall handles: one per C signature the core exports, none of them bound to an
     * address. Each takes the export's address as its leading argument, which {@link Core}
     * looks up once the library is loaded.
     *
     * <p>They live apart from {@link Core} for GraalVM Native Image. An image can only compile
     * a call through a {@code MethodHandle} that is already a constant when the image is
     * built; a handle created at run time — which is what binding one to a symbol's address
     * forces, since the address does not exist until the library is loaded — is invoked
     * through the image's method-handle interpreter instead, at microseconds a call (measured:
     * 6.4 µs for {@code newV7}, against 36 ns on the JVM). Nothing in this class needs the
     * library, so {@code META-INF/native-image/.../native-image.properties} has it initialized
     * at image build time, and the handles are constants in the image. On the JVM the split
     * changes nothing: the class initializes on the first native-path call, and the JIT folds
     * a {@code static final} handle either way.
     */
    private static final class Downcalls {
        private Downcalls() {}

        private static final Linker LINKER = Linker.nativeLinker();

        // critical(true) is what lets a heap segment (MemorySegment.ofArray over the caller's
        // byte[]) cross without being copied into native memory first: the array is pinned for
        // the duration of the call instead. The contract in exchange — the callee must be short,
        // must not block, and must never upcall into Java — is exactly what every export here is:
        // a bounded computation over the bytes it was handed, with no callbacks.
        private static final Linker.Option CRITICAL = Linker.Option.critical(true);

        // (out) -> rc — uuid_new_v4
        private static final MethodHandle NEW =
                LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS), CRITICAL);
        // (namespace, name, name_len, out) -> rc — uuid_new_v5
        private static final MethodHandle NEW_NAMED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (unix_millis, out) -> rc — uuid_new_v7
        private static final MethodHandle NEW_AT = LINKER.downcallHandle(
                FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS), CRITICAL);
        // (unix_millis, count, out) -> rc — uuid_new_v7_batch
        private static final MethodHandle NEW_BATCH = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS),
                CRITICAL);
        // (uuid) -> unix_millis — uuid_v7_unix_millis
        private static final MethodHandle UNIX_MILLIS =
                LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS), CRITICAL);
        // (uuid) -> rc, rewritten in place on success — uuid_v7_to_sql_order, uuid_v7_to_rfc_order
        private static final MethodHandle REORDER =
                LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS), CRITICAL);
        // (uuid, layout) -> version — uuid_version
        private static final MethodHandle VERSION_IN = LINKER.downcallHandle(
                FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_INT), CRITICAL);
        // (uuid) -> variant code — uuid_variant
        private static final MethodHandle VARIANT =
                LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS), CRITICAL);
        // (uuid, version, layout) -> 1/0 — uuid_is_rfc
        private static final MethodHandle IS_RFC = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.JAVA_INT),
                CRITICAL);
        // (uuid, layout, millis_out) -> 7/0 — uuid_get_timestamp
        private static final MethodHandle GET_TIMESTAMP = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS),
                CRITICAL);
        // (uuid, layout) -> unix_millis — uuid_v7_unix_millis_in
        private static final MethodHandle UNIX_MILLIS_IN = LINKER.downcallHandle(
                FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_INT), CRITICAL);
        // () -> packed version — the probe. Nothing crosses, so it is not linked critical.
        private static final MethodHandle VERSION = LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT));

        // RFC 9562 order is exactly UUID's msb/lsb decomposition, so a UUID is two big-endian
        // longs in a segment — written and read as such, no byte[] in between. Unaligned,
        // because a heap segment over a caller's byte[] carries no alignment guarantee at all
        // (the aligned layout rejects it outright), and on every supported RID an unaligned
        // load of an aligned address costs the same as an aligned one. Here for the same
        // reason the handles are: a layout is read through a VarHandle, and one that is not a
        // constant in the image costs ~70 ns a load there instead of one instruction.
        private static final ValueLayout.OfLong BIG_ENDIAN_LONG =
                ValueLayout.JAVA_LONG_UNALIGNED.withOrder(ByteOrder.BIG_ENDIAN);
    }

    /**
     * The loaded core: which path won, the library it resolved to, and the address of every
     * export in it — all {@code static final}, all resolved in this holder's own class init.
     * A holder rather than fields on {@code UuidGenerator} itself so that nothing loads
     * until the first call that needs the core ({@link UuidGenerator#NIL},
     * {@link UuidGenerator#MAX} and {@link Namespaces} never do), and so that
     * {@link UuidGenerator#isAvailable()} can
     * observe a load failure without {@code UuidGenerator} having failed to initialize: a
     * call after a failed load throws the {@link NoClassDefFoundError} for this class, and
     * the constants and the probe go on working.
     */
    private static final class Core {
        private Core() {}

        // The bundled library for this platform, loaded once at class init. A platform this
        // jar ships no build for, or a library that will not open (a temp directory mounted
        // noexec, say), fails this class's init with the reason, and isAvailable() says so.
        private static final SymbolLookup LOOKUP = loadLibrary(NativePlatform.current());

        // Where each export lives in the loaded library — the leading argument of the
        // Downcalls handle with its signature. Looked up here, once, so an export missing from
        // an older core fails this class's init (and isAvailable() says so) rather than a call.
        private static final MemorySegment UUID_NEW_V4 = export("uuid_new_v4");
        private static final MemorySegment UUID_NEW_V5 = export("uuid_new_v5");
        private static final MemorySegment UUID_NEW_V7 = export("uuid_new_v7");
        private static final MemorySegment UUID_V7_UNIX_MILLIS = export("uuid_v7_unix_millis");
        private static final MemorySegment UUID_NEW_V7_BATCH = export("uuid_new_v7_batch");
        private static final MemorySegment UUID_V7_TO_SQL_ORDER = export("uuid_v7_to_sql_order");
        private static final MemorySegment UUID_V7_TO_RFC_ORDER = export("uuid_v7_to_rfc_order");
        private static final MemorySegment UUID_VERSION = export("uuid_version");
        private static final MemorySegment UUID_VARIANT = export("uuid_variant");
        private static final MemorySegment UUID_IS_RFC = export("uuid_is_rfc");
        private static final MemorySegment UUID_GET_TIMESTAMP = export("uuid_get_timestamp");
        private static final MemorySegment UUID_V7_UNIX_MILLIS_IN = export("uuid_v7_unix_millis_in");
        private static final MemorySegment HYPERUUID_VERSION = export("hyperuuid_version");

        private static MemorySegment export(String symbol) {
            return LOOKUP.find(symbol)
                    .orElseThrow(() -> new IllegalStateException("libhyperuuid does not export " + symbol));
        }

        // Why there is no native library to load, for the messages below: no build exists for
        // this platform at all, or one should and this jar was packed without it.
        private static String nativeMissing(NativePlatform.Target target) {
            return target == null
                    ? "hyperuuid: this jar carries no native library for " + NativePlatform.describe()
                    : target.resourcePath() + " classpath resource not found (this jar was built "
                            + "without a native library for this platform)";
        }

        // The library must outlive every downcall made through it, so it's loaded into the
        // JDK-provided global arena that lives for the process's lifetime rather than one this
        // class would have to remember to keep a reference to.
        private static SymbolLookup loadLibrary(NativePlatform.Target target) {
            if (target == null) {
                throw new IllegalStateException(nativeMissing(null));
            }
            try (InputStream resource = UuidGenerator.class.getResourceAsStream(target.resourcePath())) {
                if (resource == null) {
                    throw new IllegalStateException(nativeMissing(target));
                }
                String libraryFileName = target.libraryFileName();
                String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
                Path tmp = Files.createTempFile("hyperuuid", extension);
                tmp.toFile().deleteOnExit();
                Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
                return SymbolLookup.libraryLookup(tmp, Arena.global());
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
    }

    /**
     * Whether the core resolved: the bundled platform library loaded, and every export this
     * binding was built against was found in it. Probed once, on first call, and cached;
     * never throws. A library that will not open, a jar built without a core for this
     * platform, an export missing from an older core — all come back {@code false}. This is
     * what a consumer with a fallback of its own ({@link UUID#randomUUID()}, say) gates on
     * before the first call: the generating methods themselves throw the load failure they
     * hit rather than quietly mint from somewhere else. {@code true} exactly when
     * {@link #nativeVersion()} succeeds.
     *
     * @return {@code true} when the core is loaded and callable
     */
    public static boolean isAvailable() {
        return Availability.AVAILABLE;
    }

    // Its own holder so the answer is computed once and cached without UuidGenerator's own
    // init depending on the load. The probe is the version export: the cheapest crossing
    // there is, and reaching it proves Core initialized — every export resolved.
    private static final class Availability {
        static final boolean AVAILABLE = probe();

        private Availability() {}

        private static boolean probe() {
            try {
                nativeVersion();
                return true;
            } catch (RuntimeException | LinkageError unavailable) {
                // Core failing to initialize surfaces as ExceptionInInitializerError (and
                // NoClassDefFoundError on every later touch) — LinkageErrors, whatever the
                // loader's own exception underneath was. Nothing else is expected, and
                // nothing else is swallowed.
                return false;
            }
        }
    }

    /**
     * The version of the core this process actually loaded — the bundled platform library —
     * as {@code major.minor.patch}, decoded from the core's own
     * {@code hyperuuid_version} export. The probe a host uses to prove the library it
     * resolved is the one this binding was built against, before minting the first UUID.
     * Takes nothing and touches nothing; the only way it fails is the core not having
     * loaded, which it reports as the load failure itself.
     *
     * @return the loaded core's version as {@code "major.minor.patch"}
     */
    public static String nativeVersion() {
        int packed;
        try {
            packed = (int) Downcalls.VERSION.invokeExact(Core.HYPERUUID_VERSION);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: hyperuuid_version downcall failed unexpectedly", t);
        }
        // major << 16 | minor << 8 | patch, per ffi.rs.
        return (packed >>> 16) + "." + ((packed >>> 8) & 0xFF) + "." + (packed & 0xFF);
    }

    /** The RFC 9562 §5.9 Nil UUID — all 128 bits zero. */
    public static final UUID NIL = new UUID(0L, 0L);

    /** The RFC 9562 §5.10 Max UUID — all 128 bits one. */
    public static final UUID MAX = new UUID(-1L, -1L);

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
        return new UUID(
                segment.get(Downcalls.BIG_ENDIAN_LONG, offset), segment.get(Downcalls.BIG_ENDIAN_LONG, offset + 8));
    }

    private static MemorySegment writeUuid(MemorySegment segment, UUID uuid) {
        segment.set(Downcalls.BIG_ENDIAN_LONG, 0, uuid.getMostSignificantBits());
        segment.set(Downcalls.BIG_ENDIAN_LONG, 8, uuid.getLeastSignificantBits());
        return segment;
    }

    /**
     * Creates a random UUID version 4 (RFC 9562 §5.4).
     *
     * @return a new random version 4 UUID
     */
    public static UUID newV4() {
        MemorySegment out = SCRATCH.get().out;
        int rc;
        try {
            rc = (int) Downcalls.NEW.invokeExact(Core.UUID_NEW_V4, out);
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
        Scratch scratch = SCRATCH.get();
        MemorySegment nsSeg = writeUuid(scratch.in, namespace);
        // The caller's own array crosses pinned; a zero-length name is the ABI's NULL.
        MemorySegment nameSeg = name.length == 0 ? MemorySegment.NULL : MemorySegment.ofArray(name);
        MemorySegment out = scratch.out;
        int rc;
        try {
            rc = (int) Downcalls.NEW_NAMED.invokeExact(Core.UUID_NEW_V5, nsSeg, nameSeg, name.length, out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v5 downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw new IllegalStateException("uuid_new_v5 failed with code " + rc);
        }
        return readUuid(out, 0);
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
        MemorySegment out = SCRATCH.get().out;
        int rc;
        try {
            rc = (int) Downcalls.NEW_AT.invokeExact(Core.UUID_NEW_V7, unixMillis, out);
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
     * timestamp", so the caller is responsible for checking that first if it matters
     * ({@link #isRfc(UUID, int)} is the check).
     *
     * @param uuid a version 7 UUID
     * @return the embedded Unix-epoch millisecond timestamp
     */
    public static long v7UnixMillis(UUID uuid) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) Downcalls.UNIX_MILLIS.invokeExact(Core.UUID_V7_UNIX_MILLIS, seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_unix_millis downcall failed unexpectedly", t);
        }
    }

    /**
     * Recovers the UTC timestamp embedded in a version 7 UUID as an {@link Instant}. Cannot
     * overflow: the 48-bit field tops out in the year 10889, far inside what an
     * {@code Instant} holds — unlike the corresponding Python/C# bindings, whose date types
     * stop at the year 9999.
     *
     * @param uuid a version 7 UUID
     * @return the embedded UTC timestamp
     */
    public static Instant v7Timestamp(UUID uuid) {
        return Instant.ofEpochMilli(v7UnixMillis(uuid));
    }

    /**
     * Recovers the UTC timestamp embedded in {@code uuid}, or {@link Optional#empty()} if it
     * isn't an RFC 9562 version 7 UUID. Unlike {@link #v7Timestamp}, this checks first, so a
     * caller doesn't need to already know (or separately check) whether {@code uuid} is a
     * version 7 before asking. The variant is checked as well as the version: a 7 nibble under
     * the NCS, Microsoft or future variant is not an RFC version and has no timestamp. Only
     * version 7 timestamps are read: a version 1 or 6 from another generator comes back
     * empty. One call into the core, with no bit-layout logic here.
     *
     * @param uuid any RFC 9562-ordered UUID
     * @return the embedded UTC timestamp, or empty if {@code uuid} isn't an RFC 9562 version 7
     *     UUID
     */
    public static Optional<Instant> getTimestamp(UUID uuid) {
        return getTimestamp(uuid, UuidLayout.RFC_9562);
    }

    /**
     * {@link #getTimestamp(UUID)} for a {@code uuid} held in {@code layout}'s byte order — in
     * {@link UuidLayout#SQL_SERVER}, the timestamp of a value straight from
     * {@link #v7ToSqlOrder(UUID)} (or a {@code uniqueidentifier} column), read from its bytes
     * with no conversion back first, or empty for anything that isn't a SQL-ordered value
     * converted from an RFC 9562 version 7 UUID (see {@link #version(UUID, UuidLayout)}). The
     * variant is checked in either layout.
     *
     * @param uuid any UUID, held in {@code layout}'s byte order
     * @param layout the byte order {@code uuid} is held in
     * @return the embedded UTC timestamp, or empty if {@code uuid} isn't an RFC 9562 version 7
     *     UUID in {@code layout}
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static Optional<Instant> getTimestamp(UUID uuid, UuidLayout layout) {
        int code = layoutCode(layout);
        // One call: the core checks the variant and the version and reads the timestamp,
        // writing the millis into the out scratch only when it answers 7.
        Scratch scratch = SCRATCH.get();
        MemorySegment seg = writeUuid(scratch.in, uuid);
        int version;
        try {
            version = (int) Downcalls.GET_TIMESTAMP.invokeExact(Core.UUID_GET_TIMESTAMP, seg, code, scratch.out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_get_timestamp downcall failed unexpectedly", t);
        }
        if (version == 0) {
            return Optional.empty();
        }
        return Optional.of(Instant.ofEpochMilli(scratch.out.get(ValueLayout.JAVA_LONG, 0)));
    }

    /**
     * {@link #v7UnixMillis(UUID)} for a {@code uuid} held in {@code layout}'s byte order,
     * reading a SQL-ordered value's bytes directly. Meaningful only for a genuine
     * version 7 UUID in that layout; {@link #isRfc(UUID, int, UuidLayout)} is the check.
     *
     * @param uuid a version 7 UUID, held in {@code layout}'s byte order
     * @param layout the byte order {@code uuid} is held in
     * @return the embedded Unix-epoch millisecond timestamp
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static long v7UnixMillis(UUID uuid, UuidLayout layout) {
        return v7UnixMillisIn(uuid, layoutCode(layout));
    }

    /**
     * {@link #v7Timestamp(UUID)} for a {@code uuid} held in {@code layout}'s byte order; see
     * {@link #v7UnixMillis(UUID, UuidLayout)}. Cannot overflow, for the same reason.
     *
     * @param uuid a version 7 UUID, held in {@code layout}'s byte order
     * @param layout the byte order {@code uuid} is held in
     * @return the embedded UTC timestamp
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static Instant v7Timestamp(UUID uuid, UuidLayout layout) {
        return Instant.ofEpochMilli(v7UnixMillis(uuid, layout));
    }

    // ---- Inspection ------------------------------------------------------------------
    //
    // The layout knowledge lives in the core: these only hand it a UUID's sixteen bytes. In
    // either layout those are the UUID's two longs, most significant first — exactly how
    // v7ToSqlOrder writes its input and reads its result back — so a
    // SQL-ordered UUID's bytes are the SQL Server wire bytes the core expects, with no
    // layout-dependent conversion on this side.

    private static int layoutCode(UuidLayout layout) {
        Objects.requireNonNull(layout, "layout");
        if (layout == UuidLayout.UNSPECIFIED) {
            throw new IllegalArgumentException(
                    "layout must be UuidLayout.RFC_9562 or UuidLayout.SQL_SERVER; got UNSPECIFIED");
        }
        return layout.code();
    }

    private static void requireSingleUuid(byte[] uuid) {
        if (uuid.length != 16) {
            throw new IllegalArgumentException("a UUID is exactly 16 bytes; got " + uuid.length);
        }
    }

    private static int versionIn(UUID uuid, int layout) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        return versionNative(seg, layout);
    }

    private static int versionNative(MemorySegment seg, int layout) {
        try {
            return (int) Downcalls.VERSION_IN.invokeExact(Core.UUID_VERSION, seg, layout);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_version downcall failed unexpectedly", t);
        }
    }

    private static int variantNative(MemorySegment seg) {
        try {
            return (int) Downcalls.VARIANT.invokeExact(Core.UUID_VARIANT, seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_variant downcall failed unexpectedly", t);
        }
    }

    private static boolean isRfcNative(MemorySegment seg, int version, int layout) {
        try {
            return (int) Downcalls.IS_RFC.invokeExact(Core.UUID_IS_RFC, seg, version, layout) != 0;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_is_rfc downcall failed unexpectedly", t);
        }
    }

    private static long v7UnixMillisIn(UUID uuid, int layout) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) Downcalls.UNIX_MILLIS_IN.invokeExact(Core.UUID_V7_UNIX_MILLIS_IN, seg, layout);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_unix_millis_in downcall failed unexpectedly", t);
        }
    }

    /**
     * The RFC 9562 version nibble of {@code uuid}, 0 through 15 — 0 for {@link #NIL}, 15 for
     * {@link #MAX}. Agrees with {@link UUID#version()}, read by the core. Says nothing about
     * the variant: use {@link #isRfc(UUID, int)} when the question is "an RFC 9562 UUID of
     * version N".
     *
     * <p>Reads {@code uuid} as RFC 9562 order. For a value from {@link #v7ToSqlOrder(UUID)}, or
     * read back from a {@code uniqueidentifier} column, pass
     * {@link UuidLayout#SQL_SERVER} to {@link #version(UUID, UuidLayout)}: this overload reads
     * the wrong bytes there.
     *
     * @param uuid any RFC 9562-ordered UUID
     * @return its version nibble
     */
    public static int version(UUID uuid) {
        return versionIn(uuid, UuidLayout.RFC_9562.code());
    }

    /**
     * The version of a {@code uuid} held in {@code layout}'s byte order.
     *
     * <p>In {@link UuidLayout#RFC_9562} this is {@link #version(UUID)}. In
     * {@link UuidLayout#SQL_SERVER} it is the version a SQL-ordered value was converted from,
     * 7, and 0 for anything else. Every SQL-ordered value is a version 8 (see
     * {@link #v7ToSqlOrder(UUID)}): the core checks for the 8 and the variant where that layout
     * puts them. What it can't know is whether the value is in SQL Server order at all — that
     * is the caller's to track: RFC-ordered bytes can happen to carry the 8 and the variant
     * there (a random v4 does one time in 16), and then this answers 7.
     *
     * @param uuid any UUID, held in {@code layout}'s byte order
     * @param layout the byte order {@code uuid} is held in
     * @return its version in that layout
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static int version(UUID uuid, UuidLayout layout) {
        return versionIn(uuid, layoutCode(layout));
    }

    /**
     * {@link #version(UUID)} over 16 raw RFC 9562-ordered bytes. The bytes SQL Server stores
     * for a SQL-ordered value need {@link #version(byte[], UuidLayout)} with
     * {@link UuidLayout#SQL_SERVER}: this overload reads the wrong bytes there.
     *
     * @param uuid the 16 bytes
     * @return their version nibble
     * @throws IllegalArgumentException if {@code uuid} is not exactly 16 bytes
     */
    public static int version(byte[] uuid) {
        return version(uuid, UuidLayout.RFC_9562);
    }

    /**
     * {@link #version(UUID, UuidLayout)} over 16 raw bytes already in {@code layout}'s order —
     * in {@link UuidLayout#SQL_SERVER}, the bytes SQL Server stores, as
     * {@link #v7ToSqlOrder(byte[])} writes them.
     *
     * @param uuid the 16 bytes, in {@code layout}'s order
     * @param layout the byte order {@code uuid} is in
     * @return their version in that layout
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}, or
     *     {@code uuid} is not exactly 16 bytes
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static int version(byte[] uuid, UuidLayout layout) {
        int code = layoutCode(layout);
        requireSingleUuid(uuid);
        return versionNative(MemorySegment.ofArray(uuid), code);
    }

    /**
     * The variant field of {@code uuid} (RFC 9562 §4.1): {@link UuidVariant#NCS} for
     * {@link #NIL}, {@link UuidVariant#FUTURE} for {@link #MAX}, and
     * {@link UuidVariant#RFC_9562} for anything this library or {@link UUID#randomUUID()}
     * mints. Never {@link UuidVariant#UNSPECIFIED}.
     *
     * <p>RFC 9562 order only, and there is no layout overload. Don't feed it a
     * {@code uniqueidentifier} read-back or a {@link #v7ToSqlOrder(UUID)} result; to validate
     * one of those, use {@link #isRfc(UUID, int, UuidLayout)} with
     * {@link UuidLayout#SQL_SERVER}, which checks the variant where that layout puts it.
     *
     * @param uuid any RFC 9562-ordered UUID
     * @return its variant
     */
    public static UuidVariant variant(UUID uuid) {
        return UuidVariant.of(variantNative(writeUuid(SCRATCH.get().in, uuid)));
    }

    /**
     * {@link #variant(UUID)} over 16 raw RFC 9562-ordered bytes. RFC 9562 order only, as
     * there: for the bytes SQL Server stores, use {@link #isRfc(byte[], int, UuidLayout)} with
     * {@link UuidLayout#SQL_SERVER}.
     *
     * @param uuid the 16 bytes
     * @return their variant
     * @throws IllegalArgumentException if {@code uuid} is not exactly 16 bytes
     */
    public static UuidVariant variant(byte[] uuid) {
        requireSingleUuid(uuid);
        return UuidVariant.of(variantNative(MemorySegment.ofArray(uuid)));
    }

    /**
     * Whether {@code uuid} is an RFC 9562 UUID of version {@code version} — the RFC variant
     * and that version nibble, in one call into the core. The guard to run before trusting a
     * value's version-specific fields, such as a version 7's timestamp. A {@code version}
     * outside 0-15 is simply never matched.
     *
     * <p>Reads {@code uuid} as RFC 9562 order. A SQL-ordered value — from
     * {@link #v7ToSqlOrder(UUID)}, or read back from a {@code uniqueidentifier} column — needs
     * {@link #isRfc(UUID, int, UuidLayout)} with {@link UuidLayout#SQL_SERVER}: this overload
     * reads the wrong bytes there and answers for whatever they happen to hold. There is
     * deliberately no layout-agnostic form: the caller holding the value knows its order, and
     * only the layout it names says which bytes to read.
     *
     * @param uuid any RFC 9562-ordered UUID
     * @param version the version to check for
     * @return {@code true} if {@code uuid} is an RFC 9562 UUID of that version
     */
    public static boolean isRfc(UUID uuid, int version) {
        return isRfcIn(uuid, version, UuidLayout.RFC_9562.code());
    }

    /**
     * {@link #isRfc(UUID, int)} for a {@code uuid} held in {@code layout}'s byte order. In
     * {@link UuidLayout#SQL_SERVER} it asks whether the value was converted from an RFC 9562
     * UUID of {@code version}, so only 7 can be {@code true} (see
     * {@link #version(UUID, UuidLayout)}).
     *
     * @param uuid any UUID, held in {@code layout}'s byte order
     * @param version the version to check for
     * @param layout the byte order {@code uuid} is held in
     * @return {@code true} if {@code uuid} is an RFC 9562 UUID of that version in that layout
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static boolean isRfc(UUID uuid, int version, UuidLayout layout) {
        return isRfcIn(uuid, version, layoutCode(layout));
    }

    private static boolean isRfcIn(UUID uuid, int version, int layout) {
        return isRfcNative(writeUuid(SCRATCH.get().in, uuid), version, layout);
    }

    /**
     * {@link #isRfc(UUID, int)} over 16 raw RFC 9562-ordered bytes. The bytes SQL Server
     * stores for a SQL-ordered value need {@link #isRfc(byte[], int, UuidLayout)} with
     * {@link UuidLayout#SQL_SERVER}: this overload reads the wrong bytes there.
     *
     * @param uuid the 16 bytes
     * @param version the version to check for
     * @return {@code true} if the bytes are an RFC 9562 UUID of that version
     * @throws IllegalArgumentException if {@code uuid} is not exactly 16 bytes
     */
    public static boolean isRfc(byte[] uuid, int version) {
        return isRfc(uuid, version, UuidLayout.RFC_9562);
    }

    /**
     * {@link #isRfc(UUID, int, UuidLayout)} over 16 raw bytes already in {@code layout}'s
     * order.
     *
     * @param uuid the 16 bytes, in {@code layout}'s order
     * @param version the version to check for
     * @param layout the byte order {@code uuid} is in
     * @return {@code true} if the bytes are an RFC 9562 UUID of that version in that layout
     * @throws IllegalArgumentException if {@code layout} is {@link UuidLayout#UNSPECIFIED}, or
     *     {@code uuid} is not exactly 16 bytes
     * @throws NullPointerException if {@code layout} is {@code null}
     */
    public static boolean isRfc(byte[] uuid, int version, UuidLayout layout) {
        int code = layoutCode(layout);
        requireSingleUuid(uuid);
        return isRfcNative(MemorySegment.ofArray(uuid), version, code);
    }

    /**
     * Converts an RFC 9562 version 7 {@code uuid} to SQL Server order: an RFC 9562 version 8
     * (§5.8, the version for custom layouts) whose {@code uniqueidentifier} sort is creation
     * order.
     *
     * <p>{@code System.Data.SqlTypes.SqlGuid} comparison — and therefore T-SQL {@code ORDER BY}
     * on a {@code uniqueidentifier} column — doesn't compare a GUID's 16 bytes left to right;
     * it compares octets {@code 10-15}, then {@code 8-9}, {@code 6-7}, {@code 4-5} and
     * {@code 0-3}. The result holds the version nibble 8 at octet 7's top nibble and the RFC
     * variant at octet 8's top two bits — where SQL Server's text form and a .NET {@code Guid}
     * show them, so both read version 8 — and fills the 122 bits around them, most
     * significant first, with a key: the 48-bit millisecond timestamp, the 26-bit counter,
     * then the 48 trailing random bits as one block. Within a millisecond values sort by
     * counter, which {@link #newV7} and the batches fill in creation order. The layout is
     * computed in the native Rust core and verified there — and against the real
     * {@code System.Data.SqlTypes.SqlGuid} comparator in this project's C# test suite; this
     * binding calls the same native function rather than reimplementing it.
     *
     * <p>A version 7 and a version 8 have the same 122 free bits, so every version 7 converts.
     * {@link #version(UUID, UuidLayout)} with {@link UuidLayout#SQL_SERVER} answers 7 for the
     * result, and {@link #v7FromSqlOrder(UUID)} gives back the exact original.
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
     * @param uuid an RFC 9562-ordered version 7 UUID
     * @return {@code uuid} in SQL Server order
     * @throws IllegalArgumentException if {@code uuid} isn't an RFC 9562 version 7 UUID
     */
    public static UUID v7ToSqlOrder(UUID uuid) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        int rc;
        try {
            rc = (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_SQL_ORDER, seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_sql_order downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw sqlOrderFailure(rc, "uuid_v7_to_sql_order");
        }
        return readUuid(seg, 0);
    }

    /**
     * Inverse of {@link #v7ToSqlOrder(UUID)} — converts a SQL-ordered {@code uuid} back to the
     * RFC 9562 version 7 it came from.
     *
     * @param uuid a SQL-ordered value converted from a version 7
     * @return the RFC 9562 version 7 {@code uuid} was converted from
     * @throws IllegalArgumentException if {@code uuid} isn't a SQL-ordered value converted
     *     from a version 7 — an RFC-ordered value included
     */
    public static UUID v7FromSqlOrder(UUID uuid) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        int rc;
        try {
            rc = (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_RFC_ORDER, seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_rfc_order downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw sqlOrderFailure(rc, "uuid_v7_to_rfc_order");
        }
        return readUuid(seg, 0);
    }

    /**
     * Creates {@code count} time-sortable version 7 UUIDs sharing one timestamp capture and
     * one contiguous block of the monotonic counter — one downcall and one random-bytes
     * fetch instead of {@code count} of each. In strictly increasing order however the batch
     * lands on the counter; see {@link #MAX_V7_BATCH}, which also says why that orders one
     * batch rather than the stream across a counter wrap.
     *
     * @param count how many UUIDs to create
     * @param unixMillis the shared Unix-epoch millisecond timestamp to embed in each
     * @return {@code count} new version 7 UUIDs
     * @throws IllegalArgumentException if {@code unixMillis} is negative or doesn't fit
     *     within 48 bits, or {@code count} is negative or greater than {@link #MAX_V7_BATCH}
     */
    public static UUID[] newV7Batch(int count, long unixMillis) {
        requireBatchCount(count);
        requireV7BatchCount(count);
        if (count == 0) {
            return new UUID[0];
        }
        // One heap array as the destination, crossing pinned, then one UUID per 16 bytes.
        MemorySegment out = MemorySegment.ofArray(new byte[count * 16]);
        int rc;
        try {
            rc = (int) Downcalls.NEW_BATCH.invokeExact(Core.UUID_NEW_V7_BATCH, unixMillis, count, out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v7_batch downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw batchFailure(
                    rc, "uuid_new_v7_batch", count, "unixMillis must be non-negative and fit within 48 bits");
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
    // newV7Batch allocates a fresh UUID[] and a scratch byte[] on every call.
    // These write into storage the caller already owns — the byte[] form pinned and handed to
    // the native side directly, nothing copied — so a hot path can reuse one buffer across
    // batches instead of handing the collector two objects per batch.
    //
    // Unlike the Go and Swift bindings, the UUID[] form still costs a per-element
    // conversion: java.util.UUID is two longs, not 16 RFC-ordered bytes, so every item has
    // to be rebuilt from the native output. That is exactly the C# binding's situation, and
    // it is why the byte[] forms below are the ones that actually remove work
    // rather than just removing an allocation.

    private static void fillBytesNative(
            MemorySegment out, int count, long unixMillis, MemorySegment export, String fn) {
        int rc;
        try {
            rc = (int) Downcalls.NEW_BATCH.invokeExact(export, unixMillis, count, out);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: " + fn + " downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw batchFailure(rc, fn, count, "unixMillis must be non-negative and fit within 48 bits");
        }
    }

    // A batch export's non-zero return code as the exception it means: 2 is the caller's
    // timestamp, 3 a batch too
    // large to address on this platform (count * 16 overflowing the core's usize, a 32-bit
    // host only), 4 a v7 batch past MAX_V7_BATCH. Only 1 is the random source.
    static RuntimeException batchFailure(int rc, String fn, int count, String outOfRangeMessage) {
        return switch (rc) {
            case 2 -> new IllegalArgumentException(outOfRangeMessage);
            case 3 ->
                new IllegalArgumentException("a batch of " + count + " UUIDs is too large to address on this platform");
            case 4 -> v7BatchTooLarge(count);
            default -> new IllegalStateException(fn + " failed with code " + rc + " (random source failure)");
        };
    }

    // A SQL-order export's non-zero return code as the exception it means. The only refusal
    // is 5, a value that isn't
    // the version the conversion takes, and on it the core left the bytes as they were.
    static RuntimeException sqlOrderFailure(int rc, String fn) {
        return rc == 5
                ? new IllegalArgumentException("the value is not a UUID of the version this conversion takes")
                : new IllegalStateException(fn + " failed with unexpected code " + rc);
    }

    // The most UUIDs one batch can carry: its count * 16 bytes of output still have to be an
    // int-sized array.
    static final int MAX_BATCH = Integer.MAX_VALUE / 16;

    // Checked before the core multiplies by 16. Unchecked, a count past MAX_BATCH wraps that
    // product to a small or zero length while the core still writes count * 16 bytes —
    // straight past the end of a pinned Java array — and a negative one
    // reaches the core as a count of billions.
    static void requireBatchCount(int count) {
        if (count < 0 || count > MAX_BATCH) {
            throw new IllegalArgumentException("a batch holds between 0 and " + MAX_BATCH + " UUIDs; got " + count);
        }
    }

    /**
     * The most UUIDs one version 7 batch ({@link #newV7Batch(int, long)},
     * {@link #fillV7(UUID[], long)}, {@link #fillV7(byte[], long)} and their overloads) mints:
     * 67,108,864, the size of the 26-bit counter that orders UUIDs within a millisecond.
     *
     * <p>Every batch up to this size is in strictly increasing order. The counter is one
     * process-wide sequence, so a batch can straddle the point where it wraps back to 0; the
     * UUIDs from there on carry a timestamp one millisecond later than the one supplied rather
     * than sorting before the ones ahead of them. A larger batch would have to reuse counter
     * values within one millisecond, so it is refused with {@link IllegalArgumentException},
     * before anything is allocated or written.
     *
     * <p>The roll-forward orders one batch, not the stream. The next batch or {@code newV7}
     * call in the same real millisecond starts its counter just past the wrap and carries the
     * supplied timestamp, so it sorts before the previous batch's tail, stamped a millisecond
     * later; two single calls either side of the wrap in one millisecond sort in reverse the
     * same way. It happens at most once per {@code MAX_V7_BATCH} UUIDs the process mints.
     */
    public static final int MAX_V7_BATCH = 1 << 26;

    private static IllegalArgumentException v7BatchTooLarge(int count) {
        return new IllegalArgumentException(
                "a version 7 batch holds at most " + MAX_V7_BATCH + " UUIDs (the 26-bit counter space); got " + count);
    }

    // Checked after requireBatchCount, before anything is allocated: the core refuses this
    // too (code 4), but only once a buffer of up to 1 GiB had been allocated to hand it.
    private static void requireV7BatchCount(int count) {
        if (count > MAX_V7_BATCH) {
            throw v7BatchTooLarge(count);
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
     * capture and one contiguous block of the monotonic counter, in strictly increasing order
     * (see {@link #MAX_V7_BATCH}).
     *
     * <p>Writes into an array the caller already owns rather than allocating a new one. Each
     * element is still rebuilt from the native bytes, because {@link UUID} is two longs and
     * not RFC byte order — see {@link #fillV7(byte[], long)} for the form that skips that.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     * @throws IllegalArgumentException if {@code unixMillis} is negative or doesn't fit
     *     within 48 bits, or {@code destination} is longer than {@link #MAX_V7_BATCH}
     */
    public static void fillV7(UUID[] destination, long unixMillis) {
        requireBatchCount(destination.length);
        requireV7BatchCount(destination.length);
        fillUuidArray(destination, unixMillis, Core.UUID_NEW_V7_BATCH, "uuid_new_v7_batch");
    }

    /**
     * Fills {@code destination} with version 7 UUIDs sharing the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV7(UUID[] destination) {
        fillV7(destination, System.currentTimeMillis());
    }

    private static void fillUuidArray(UUID[] destination, long unixMillis, MemorySegment export, String fn) {
        if (destination.length == 0) {
            return;
        }
        MemorySegment out = MemorySegment.ofArray(new byte[destination.length * 16]);
        fillBytesNative(out, destination.length, unixMillis, export, fn);
        for (int i = 0; i < destination.length; i++) {
            destination[i] = readUuid(out, (long) i * 16);
        }
    }

    /**
     * Fills {@code destination} with raw RFC 9562-ordered version 7 UUID bytes, 16 per UUID,
     * in strictly increasing order (see {@link #MAX_V7_BATCH}).
     *
     * <p>This is the conversion-free form: the native core already writes RFC-ordered bytes
     * contiguously, so nothing is rebuilt on the way out. Prefer it when the destination is a
     * wire buffer or a database parameter that wants bytes anyway.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     * @param unixMillis the shared timestamp, in milliseconds since the Unix epoch
     *
     * @throws IllegalArgumentException if {@code destination.length} is not a multiple of
     *     16 or holds more than {@link #MAX_V7_BATCH} UUIDs, or {@code unixMillis} is
     *     negative or doesn't fit within 48 bits
     */
    public static void fillV7(byte[] destination, long unixMillis) {
        requireWholeUuids(destination.length);
        requireV7BatchCount(destination.length / 16);
        fillByteArray(destination, unixMillis, Core.UUID_NEW_V7_BATCH, "uuid_new_v7_batch");
    }

    /**
     * Fills {@code destination} with raw version 7 UUID bytes using the current time.
     *
     * @param destination the array to fill; its length determines how many UUIDs are generated
     */
    public static void fillV7(byte[] destination) {
        fillV7(destination, System.currentTimeMillis());
    }

    private static void fillByteArray(byte[] destination, long unixMillis, MemorySegment export, String fn) {
        requireWholeUuids(destination.length);
        if (destination.length == 0) {
            return;
        }
        // The caller's array is the destination: pinned for the call, written in place.
        fillBytesNative(MemorySegment.ofArray(destination), destination.length / 16, unixMillis, export, fn);
    }

    // ---- Raw-byte SQL-order transforms -----------------------------------------------
    //
    // The same native conversions as the UUID-taking methods above, rewriting a caller's
    // own 16 bytes in place — or, on a refusal, leaving them exactly as they were. Being pure byte-in/byte-out, these
    // are the form a byte-level
    // correctness oracle can be pointed at directly — shared across every binding in this
    // repo rather than re-expressed against each language's own UUID type.

    private static void sqlOrderBytes(byte[] uuid, MemorySegment export, String fn) {
        if (uuid.length != 16) {
            throw new IllegalArgumentException("a UUID is exactly 16 bytes; got " + uuid.length);
        }
        // In place, on the caller's own bytes — no staging copy in either direction.
        // The core writes only on success, so a refused value leaves the caller's bytes as
        // they were.
        MemorySegment seg = MemorySegment.ofArray(uuid);
        int rc;
        try {
            rc = (int) Downcalls.REORDER.invokeExact(export, seg);
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: " + fn + " downcall failed unexpectedly", t);
        }
        if (rc != 0) {
            throw sqlOrderFailure(rc, fn);
        }
    }

    /**
     * Rewrites the 16 RFC 9562-ordered version 7 bytes in {@code uuid} into SQL Server order,
     * in place. See {@link #v7ToSqlOrder(UUID)}.
     *
     * @param uuid the 16 RFC 9562-ordered bytes, rewritten in place; left unchanged if refused
     * @throws IllegalArgumentException if {@code uuid} is not exactly 16 bytes, or isn't an
     *     RFC 9562 version 7
     */
    public static void v7ToSqlOrder(byte[] uuid) {
        sqlOrderBytes(uuid, Core.UUID_V7_TO_SQL_ORDER, "uuid_v7_to_sql_order");
    }

    /**
     * Inverse of {@link #v7ToSqlOrder(byte[])}, in place. See {@link #v7FromSqlOrder(UUID)}.
     *
     * @param uuid the 16 SQL-ordered bytes, rewritten in place into RFC 9562 order; left
     *     unchanged if refused
     * @throws IllegalArgumentException if {@code uuid} is not exactly 16 bytes, or isn't a
     *     SQL-ordered value converted from a version 7
     */
    public static void v7FromSqlOrder(byte[] uuid) {
        sqlOrderBytes(uuid, Core.UUID_V7_TO_RFC_ORDER, "uuid_v7_to_rfc_order");
    }
}
