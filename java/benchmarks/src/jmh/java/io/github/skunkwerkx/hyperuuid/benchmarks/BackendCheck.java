package io.github.skunkwerkx.hyperuuid.benchmarks;

import java.lang.reflect.Modifier;
import java.util.Arrays;

/**
 * Fails a benchmark's setup unless the {@code NativeCore} the jar loaded is the one this JDK
 * should get: FFM on 25 and later, JNI below. A jmh jar that lost its {@code Multi-Release}
 * attribute would otherwise measure the JNI path on 25 and print numbers that look fine. The
 * signals are the native-method count (14 on the JNI class, 0 on the FFM one) and the class
 * file major (65 and 69), the ones the library's own tests use.
 */
final class BackendCheck {
    private BackendCheck() {}

    static void require() {
        boolean ffm = Runtime.version().feature() >= 25;
        Class<?> core;
        try {
            core = Class.forName("io.github.skunkwerkx.hyperuuid.NativeCore");
        } catch (ClassNotFoundException e) {
            throw new IllegalStateException("NativeCore is not loadable", e);
        }
        long natives = Arrays.stream(core.getDeclaredMethods())
                .filter(m -> Modifier.isNative(m.getModifiers()))
                .count();
        int major;
        try (var in = core.getResourceAsStream("NativeCore.class")) {
            if (in == null) {
                throw new IllegalStateException("NativeCore.class is not readable as a resource");
            }
            var data = new java.io.DataInputStream(in);
            data.readInt();
            data.readUnsignedShort();
            major = data.readUnsignedShort();
        } catch (java.io.IOException e) {
            throw new IllegalStateException("NativeCore.class is unreadable", e);
        }
        if (natives != (ffm ? 0 : 14) || major != (ffm ? 69 : 65)) {
            throw new IllegalStateException("expected the " + (ffm ? "ffm" : "jni") + " NativeCore on JDK "
                    + Runtime.version().feature() + ", but it declares " + natives + " natives and is class file major "
                    + major + " (is the jar's Multi-Release attribute missing?)");
        }
    }
}
