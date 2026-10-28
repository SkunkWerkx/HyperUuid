package io.github.skunkwerkx.hyperuuid;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.UUID;

/**
 * The JNI path to the core: the JDK 21 base of the multi-release jar, used on 21 through 24 and
 * wherever the {@code META-INF/versions/25} tree is not read. A shim library
 * ({@code libhyperuuid_jni}) holds the core inside it and registers the natives below when it
 * loads; every call returns the core's result code raw, and {@link UuidGenerator} maps the
 * codes to exceptions. The JDK 25 tree has a class of the same name and the same members that
 * reaches the core through FFM instead.
 *
 * <p>A UUID crosses as its two big-endian longs and comes back through a two-element
 * {@code long[]}, so a single-UUID door allocates nothing. Raw 16-byte arrays are read and
 * written through a big-endian {@code long} view.
 */
final class NativeCore {
    private NativeCore() {}

    // The natives, registered by JNI_OnLoad on this very class. Their names and descriptors
    // are the shim's table, java/jni/src/lib.rs. Reached only through the facade below, which
    // touches Library first.
    private static native int uuidNewV4(long[] out);

    private static native int uuidNewV5(long nsMsb, long nsLsb, byte[] name, long[] out);

    private static native int uuidNewV7(long unixMillis, long[] out);

    private static native long uuidV7UnixMillis(long msb, long lsb);

    private static native long uuidV7UnixMillisIn(long msb, long lsb, int layout);

    private static native int uuidNewV7Batch(long unixMillis, int count, byte[] out);

    private static native int uuidV7ToSqlOrder(long msb, long lsb, long[] out);

    private static native int uuidV7ToRfcOrder(long msb, long lsb, long[] out);

    private static native int uuidVersion(long msb, long lsb, int layout);

    private static native int uuidVariant(long msb, long lsb);

    private static native int uuidIsRfc(long msb, long lsb, int version, int layout);

    private static native int uuidGetTimestamp(long msb, long lsb, int layout, long[] out);

    private static native int hyperuuidVersion();

    private static native int jniNatives();

    /**
     * The shim library, loaded once in this holder's class init. A platform this jar ships no
     * build for, or a library that will not load (a temp directory mounted noexec, say), fails
     * this class's init with the reason, and {@link UuidGenerator#isAvailable()} says so. A
     * library whose natives are not the ones this class declares fails the same way. Nothing
     * loads until a facade member needs the core: every one touches this holder first.
     */
    private static final class Library {
        private Library() {}

        static {
            load(NativePlatform.current());
            int implemented = jniNatives();
            int declared = 0;
            for (Method method : NativeCore.class.getDeclaredMethods()) {
                if (Modifier.isNative(method.getModifiers())) {
                    declared++;
                }
            }
            if (implemented != declared) {
                throw new IllegalStateException(
                        "libhyperuuid_jni implements " + implemented + " natives; this binding declares " + declared);
            }
        }

        /** Forces the load. The call itself is the point; there is nothing to return. */
        static void ensure() {}

        private static void load(NativePlatform.Target target) {
            if (target == null) {
                throw new IllegalStateException(NativePlatform.nativeMissing(null, null));
            }
            String resourcePath = target.jniResourcePath();
            try (InputStream resource = NativeCore.class.getResourceAsStream(resourcePath)) {
                if (resource == null) {
                    throw new IllegalStateException(NativePlatform.nativeMissing(target, resourcePath));
                }
                String libraryFileName = target.jniLibraryFileName();
                String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
                Path tmp = Files.createTempFile("hyperuuid", extension);
                tmp.toFile().deleteOnExit();
                Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
                // JNI_OnLoad registers the natives here; a table that does not match the
                // declarations above surfaces as the NoSuchMethodError it left pending.
                System.load(tmp.toAbsolutePath().toString());
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
    }

    /** Which path reached the core, for the diagnostics; loads nothing. */
    static String backend() {
        return "jni";
    }

    /**
     * Per-thread scratch for the single-UUID doors: the two-longs result a native writes into,
     * allocated once per thread. Doors never nest, so no call can observe another's scratch
     * mid-flight; nothing is shared between threads, so no door needs locking.
     */
    static final class Scratch {
        final long[] out = new long[2];
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    /** This thread's scratch. */
    static Scratch scratch() {
        return SCRATCH.get();
    }

    // Sixteen bytes as two big-endian longs: RFC 9562 order is exactly UUID's msb/lsb
    // decomposition, in either layout.
    private static final VarHandle BE_LONGS = MethodHandles.byteArrayViewVarHandle(long[].class, ByteOrder.BIG_ENDIAN);

    private static long msb(byte[] uuid) {
        return (long) BE_LONGS.get(uuid, 0);
    }

    private static long lsb(byte[] uuid) {
        return (long) BE_LONGS.get(uuid, 8);
    }

    /** The UUID the last successful call on {@code s} produced. */
    static UUID result(Scratch s) {
        return new UUID(s.out[0], s.out[1]);
    }

    /** The milliseconds the last successful {@link #getTimestamp} on {@code s} produced. */
    static long resultMillis(Scratch s) {
        return s.out[0];
    }

    /** The core's packed version, {@code major << 16 | minor << 8 | patch}. */
    static int coreVersion() {
        Library.ensure();
        return hyperuuidVersion();
    }

    static int newV4(Scratch s) {
        Library.ensure();
        return uuidNewV4(s.out);
    }

    static int newV5(Scratch s, UUID namespace, byte[] name) {
        Library.ensure();
        return uuidNewV5(namespace.getMostSignificantBits(), namespace.getLeastSignificantBits(), name, s.out);
    }

    static int newV7(Scratch s, long unixMillis) {
        Library.ensure();
        return uuidNewV7(unixMillis, s.out);
    }

    /** The version (7 or 0); the milliseconds are in {@link #resultMillis} only when it is 7. */
    static int getTimestamp(Scratch s, UUID uuid, int layout) {
        Library.ensure();
        return uuidGetTimestamp(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), layout, s.out);
    }

    static int v7ToSqlOrder(Scratch s, UUID uuid) {
        Library.ensure();
        return uuidV7ToSqlOrder(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), s.out);
    }

    static int v7ToRfcOrder(Scratch s, UUID uuid) {
        Library.ensure();
        return uuidV7ToRfcOrder(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), s.out);
    }

    /** Fills {@code out} (at least {@code count * 16} bytes) with a v7 batch, in place. */
    static int newV7Batch(long unixMillis, int count, byte[] out) {
        Library.ensure();
        return uuidNewV7Batch(unixMillis, count, out);
    }

    /** Rebuilds {@code out.length} UUIDs from the first {@code out.length * 16} bytes. */
    static void readUuids(byte[] bytes, UUID[] out) {
        for (int i = 0; i < out.length; i++) {
            out[i] = new UUID((long) BE_LONGS.get(bytes, i * 16), (long) BE_LONGS.get(bytes, i * 16 + 8));
        }
    }

    static long v7UnixMillis(UUID uuid) {
        Library.ensure();
        return uuidV7UnixMillis(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits());
    }

    static long v7UnixMillisIn(UUID uuid, int layout) {
        Library.ensure();
        return uuidV7UnixMillisIn(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), layout);
    }

    static int version(UUID uuid, int layout) {
        Library.ensure();
        return uuidVersion(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), layout);
    }

    static int version(byte[] uuid, int layout) {
        Library.ensure();
        return uuidVersion(msb(uuid), lsb(uuid), layout);
    }

    static int variant(UUID uuid) {
        Library.ensure();
        return uuidVariant(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits());
    }

    static int variant(byte[] uuid) {
        Library.ensure();
        return uuidVariant(msb(uuid), lsb(uuid));
    }

    static int isRfc(UUID uuid, int version, int layout) {
        Library.ensure();
        return uuidIsRfc(uuid.getMostSignificantBits(), uuid.getLeastSignificantBits(), version, layout);
    }

    static int isRfc(byte[] uuid, int version, int layout) {
        Library.ensure();
        return uuidIsRfc(msb(uuid), lsb(uuid), version, layout);
    }

    /** In place on the caller's own 16 bytes; they are written only on success. */
    static int v7ToSqlOrder(byte[] uuid) {
        Library.ensure();
        long[] out = SCRATCH.get().out;
        int rc = uuidV7ToSqlOrder(msb(uuid), lsb(uuid), out);
        if (rc == 0) {
            BE_LONGS.set(uuid, 0, out[0]);
            BE_LONGS.set(uuid, 8, out[1]);
        }
        return rc;
    }

    /** In place on the caller's own 16 bytes; they are written only on success. */
    static int v7ToRfcOrder(byte[] uuid) {
        Library.ensure();
        long[] out = SCRATCH.get().out;
        int rc = uuidV7ToRfcOrder(msb(uuid), lsb(uuid), out);
        if (rc == 0) {
            BE_LONGS.set(uuid, 0, out[0]);
            BE_LONGS.set(uuid, 8, out[1]);
        }
        return rc;
    }
}
