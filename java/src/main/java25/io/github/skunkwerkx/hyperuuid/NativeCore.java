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
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.UUID;

/**
 * The FFM path to the core: the JDK 25 and later half of the multi-release jar. Every call
 * goes straight to the library's C export through a {@link Linker.Option#critical(boolean)
 * critical} downcall, and returns the core's result code raw; {@link UuidGenerator} maps the
 * codes to exceptions. The JDK 21 base tree has a class of the same name and the same
 * members that reaches the core through JNI instead.
 *
 * <p>Nothing is copied on the way across: a caller's own {@code byte[]} is pinned and handed
 * to the native side directly, and the 16-byte in/out scratch the single-UUID doors need
 * lives in one per-thread segment for the life of the thread rather than a confined
 * {@link Arena} opened and torn down on every call.
 */
final class NativeCore {
    private NativeCore() {}

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
     * A holder rather than fields on {@code NativeCore} itself so that nothing loads
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

        // The library must outlive every downcall made through it, so it's loaded into the
        // JDK-provided global arena that lives for the process's lifetime rather than one this
        // class would have to remember to keep a reference to.
        private static SymbolLookup loadLibrary(NativePlatform.Target target) {
            if (target == null) {
                throw new IllegalStateException(NativePlatform.nativeMissing(null, null));
            }
            try (InputStream resource = NativeCore.class.getResourceAsStream(target.resourcePath())) {
                if (resource == null) {
                    throw new IllegalStateException(NativePlatform.nativeMissing(target, target.resourcePath()));
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

    /** Which path reached the core, for the diagnostics; loads nothing. */
    static String backend() {
        return "ffm";
    }

    /**
     * Per-thread scratch for the single-UUID doors: sixteen bytes to hand a UUID in and
     * sixteen to receive one, allocated once per thread instead of a confined Arena opened
     * and closed on every call (a native allocation plus a scope teardown each time, which
     * was most of what those doors cost). Doors never nest, so no call can observe another's
     * scratch mid-flight; nothing is shared between threads, so no door needs locking.
     */
    static final class Scratch {
        private final Arena arena = Arena.ofAuto();
        final MemorySegment in = arena.allocate(16, 8);
        final MemorySegment out = arena.allocate(16, 8);
        // Where the last successful call left its UUID: out, or in for the in-place reorders.
        MemorySegment result = out;
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    /** This thread's scratch. */
    static Scratch scratch() {
        return SCRATCH.get();
    }

    private static UUID readUuid(MemorySegment segment, long offset) {
        return new UUID(
                segment.get(Downcalls.BIG_ENDIAN_LONG, offset), segment.get(Downcalls.BIG_ENDIAN_LONG, offset + 8));
    }

    private static MemorySegment writeUuid(MemorySegment segment, UUID uuid) {
        segment.set(Downcalls.BIG_ENDIAN_LONG, 0, uuid.getMostSignificantBits());
        segment.set(Downcalls.BIG_ENDIAN_LONG, 8, uuid.getLeastSignificantBits());
        return segment;
    }

    /** The UUID the last successful call on {@code s} produced. */
    static UUID result(Scratch s) {
        return readUuid(s.result, 0);
    }

    /** The milliseconds the last successful {@link #getTimestamp} on {@code s} produced. */
    static long resultMillis(Scratch s) {
        return s.out.get(ValueLayout.JAVA_LONG, 0);
    }

    /** The core's packed version, {@code major << 16 | minor << 8 | patch}. */
    static int coreVersion() {
        // Every door evaluates Core's address and Downcalls' handle inside its try, so a core
        // that failed to load arrives here as Core's or Downcalls' initialization error
        // (ExceptionInInitializerError, then NoClassDefFoundError). Those are rethrown as they
        // are, never wrapped: they are what isAvailable() answers false for and what the
        // generating methods document. Only the downcall itself is unexpected.
        try {
            return (int) Downcalls.VERSION.invokeExact(Core.HYPERUUID_VERSION);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: hyperuuid_version downcall failed unexpectedly", t);
        }
    }

    static int newV4(Scratch s) {
        s.result = s.out;
        try {
            return (int) Downcalls.NEW.invokeExact(Core.UUID_NEW_V4, s.out);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v4 downcall failed unexpectedly", t);
        }
    }

    static int newV5(Scratch s, UUID namespace, byte[] name) {
        MemorySegment nsSeg = writeUuid(s.in, namespace);
        // The caller's own array crosses pinned; a zero-length name is the ABI's NULL.
        MemorySegment nameSeg = name.length == 0 ? MemorySegment.NULL : MemorySegment.ofArray(name);
        s.result = s.out;
        try {
            return (int) Downcalls.NEW_NAMED.invokeExact(Core.UUID_NEW_V5, nsSeg, nameSeg, name.length, s.out);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v5 downcall failed unexpectedly", t);
        }
    }

    static int newV7(Scratch s, long unixMillis) {
        s.result = s.out;
        try {
            return (int) Downcalls.NEW_AT.invokeExact(Core.UUID_NEW_V7, unixMillis, s.out);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v7 downcall failed unexpectedly", t);
        }
    }

    /** The version (7 or 0); the milliseconds are in {@link #resultMillis} only when it is 7. */
    static int getTimestamp(Scratch s, UUID uuid, int layout) {
        MemorySegment seg = writeUuid(s.in, uuid);
        try {
            return (int) Downcalls.GET_TIMESTAMP.invokeExact(Core.UUID_GET_TIMESTAMP, seg, layout, s.out);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_get_timestamp downcall failed unexpectedly", t);
        }
    }

    static int v7ToSqlOrder(Scratch s, UUID uuid) {
        MemorySegment seg = writeUuid(s.in, uuid);
        s.result = seg;
        try {
            return (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_SQL_ORDER, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_sql_order downcall failed unexpectedly", t);
        }
    }

    static int v7ToRfcOrder(Scratch s, UUID uuid) {
        MemorySegment seg = writeUuid(s.in, uuid);
        s.result = seg;
        try {
            return (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_RFC_ORDER, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_rfc_order downcall failed unexpectedly", t);
        }
    }

    /** Fills {@code out} (at least {@code count * 16} bytes) with a v7 batch, in place. */
    static int newV7Batch(long unixMillis, int count, byte[] out) {
        MemorySegment seg = MemorySegment.ofArray(out);
        try {
            return (int) Downcalls.NEW_BATCH.invokeExact(Core.UUID_NEW_V7_BATCH, unixMillis, count, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_new_v7_batch downcall failed unexpectedly", t);
        }
    }

    /** Rebuilds {@code out.length} UUIDs from the first {@code out.length * 16} bytes. */
    static void readUuids(byte[] bytes, UUID[] out) {
        MemorySegment seg = MemorySegment.ofArray(bytes);
        for (int i = 0; i < out.length; i++) {
            out[i] = readUuid(seg, (long) i * 16);
        }
    }

    static long v7UnixMillis(UUID uuid) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) Downcalls.UNIX_MILLIS.invokeExact(Core.UUID_V7_UNIX_MILLIS, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_unix_millis downcall failed unexpectedly", t);
        }
    }

    static long v7UnixMillisIn(UUID uuid, int layout) {
        MemorySegment seg = writeUuid(SCRATCH.get().in, uuid);
        try {
            return (long) Downcalls.UNIX_MILLIS_IN.invokeExact(Core.UUID_V7_UNIX_MILLIS_IN, seg, layout);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_unix_millis_in downcall failed unexpectedly", t);
        }
    }

    static int version(UUID uuid, int layout) {
        return versionOf(writeUuid(SCRATCH.get().in, uuid), layout);
    }

    static int version(byte[] uuid, int layout) {
        return versionOf(MemorySegment.ofArray(uuid), layout);
    }

    private static int versionOf(MemorySegment seg, int layout) {
        try {
            return (int) Downcalls.VERSION_IN.invokeExact(Core.UUID_VERSION, seg, layout);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_version downcall failed unexpectedly", t);
        }
    }

    static int variant(UUID uuid) {
        return variantOf(writeUuid(SCRATCH.get().in, uuid));
    }

    static int variant(byte[] uuid) {
        return variantOf(MemorySegment.ofArray(uuid));
    }

    private static int variantOf(MemorySegment seg) {
        try {
            return (int) Downcalls.VARIANT.invokeExact(Core.UUID_VARIANT, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_variant downcall failed unexpectedly", t);
        }
    }

    static int isRfc(UUID uuid, int version, int layout) {
        return isRfcOf(writeUuid(SCRATCH.get().in, uuid), version, layout);
    }

    static int isRfc(byte[] uuid, int version, int layout) {
        return isRfcOf(MemorySegment.ofArray(uuid), version, layout);
    }

    private static int isRfcOf(MemorySegment seg, int version, int layout) {
        try {
            return (int) Downcalls.IS_RFC.invokeExact(Core.UUID_IS_RFC, seg, version, layout);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_is_rfc downcall failed unexpectedly", t);
        }
    }

    /** In place on the caller's own 16 bytes; the core writes only on success. */
    static int v7ToSqlOrder(byte[] uuid) {
        MemorySegment seg = MemorySegment.ofArray(uuid);
        try {
            return (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_SQL_ORDER, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_sql_order downcall failed unexpectedly", t);
        }
    }

    /** In place on the caller's own 16 bytes; the core writes only on success. */
    static int v7ToRfcOrder(byte[] uuid) {
        MemorySegment seg = MemorySegment.ofArray(uuid);
        try {
            return (int) Downcalls.REORDER.invokeExact(Core.UUID_V7_TO_RFC_ORDER, seg);
        } catch (LinkageError unloaded) {
            throw unloaded;
        } catch (Throwable t) {
            throw new AssertionError("hyperuuid: uuid_v7_to_rfc_order downcall failed unexpectedly", t);
        }
    }
}
