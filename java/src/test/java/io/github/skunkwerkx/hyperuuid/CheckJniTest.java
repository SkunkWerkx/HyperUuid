package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.Test;

/**
 * {@code -Xcheck:jni} prints its findings natively, past every Gradle listener, so a suite that
 * merely runs under the flag cannot fail on them. This runs the shim's heaviest paths in a child
 * JVM of its own and reads the output.
 */
class CheckJniTest {
    @Test
    void noCheckJniWarningUnderAllocationPressure() throws Exception {
        var java = ProcessHandle.current().info().command().orElse("java");
        var log = Files.createTempFile("hyperuuid-checkjni", ".log");
        try {
            var process = new ProcessBuilder(
                            java,
                            "--enable-native-access=ALL-UNNAMED",
                            "-Xcheck:jni",
                            "-Xmx3g",
                            "-cp",
                            System.getProperty("java.class.path"),
                            PressureMain.class.getName())
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            if (!process.waitFor(300, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("the -Xcheck:jni child did not finish in 300 s");
            }
            var output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(0, process.exitValue(), output);
            assertTrue(output.contains("checked"), output);
            // HotSpot words its findings three ways: "WARNING in native method" (a call made with
            // an exception pending, say), "Warning: Calling other JNI functions in the scope of
            // Get/ReleasePrimitiveArrayCritical" (measured on 21 and 25), and "FATAL ERROR in
            // native method" (which also fails the exit code).
            assertFalse(output.contains("WARNING"), output);
            assertFalse(output.contains("Warning:"), output);
            assertFalse(output.contains("FATAL ERROR"), output);
        } finally {
            Files.deleteIfExists(log);
        }
    }

    /** Fills the largest batch and hashes a 1 MiB name while another thread churns the heap. */
    static final class PressureMain {
        public static void main(String[] args) throws Exception {
            var stop = new AtomicBoolean();
            var churn = new Thread(() -> {
                byte[] keep = null;
                while (!stop.get()) {
                    keep = new byte[64 * 1024];
                }
                if (keep == null) {
                    throw new IllegalStateException();
                }
            });
            churn.setDaemon(true);
            churn.start();
            try {
                // The critical side of the bound, repeatedly, so a collection lands inside some.
                byte[] small = new byte[4096 * 16];
                for (int i = 0; i < 200; i++) {
                    UuidGenerator.fillV7(small);
                    UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "n".repeat(64 * 1024));
                }
                // The malloc side.
                byte[] batch = new byte[UuidGenerator.MAX_V7_BATCH * 16];
                UuidGenerator.fillV7(batch);
                for (int i = 16; i < batch.length; i += 16) {
                    if (Arrays.compareUnsigned(batch, i - 16, i, batch, i, i + 16) >= 0) {
                        throw new AssertionError("not strictly increasing at " + i / 16);
                    }
                }
                String name = "a".repeat(1 << 20);
                UUID got = UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, name);
                UUID want = referenceV5(UuidGenerator.Namespaces.DNS, name.getBytes(StandardCharsets.UTF_8));
                if (!got.equals(want)) {
                    throw new AssertionError("v5 of 1 MiB: " + got + " != " + want);
                }
            } finally {
                stop.set(true);
            }
            System.out.println("checked");
        }
    }

    /** RFC 9562 section 5.5 from first principles: SHA-1 over the namespace bytes and the name. */
    static UUID referenceV5(UUID namespace, byte[] name) {
        try {
            var sha1 = MessageDigest.getInstance("SHA-1");
            var ns = new byte[16];
            for (int i = 0; i < 8; i++) {
                ns[i] = (byte) (namespace.getMostSignificantBits() >>> (56 - 8 * i));
                ns[8 + i] = (byte) (namespace.getLeastSignificantBits() >>> (56 - 8 * i));
            }
            sha1.update(ns);
            sha1.update(name);
            byte[] h = sha1.digest();
            h[6] = (byte) ((h[6] & 0x0F) | 0x50);
            h[8] = (byte) ((h[8] & 0x3F) | 0x80);
            long msb = 0;
            long lsb = 0;
            for (int i = 0; i < 8; i++) {
                msb = (msb << 8) | (h[i] & 0xFF);
                lsb = (lsb << 8) | (h[8 + i] & 0xFF);
            }
            return new UUID(msb, lsb);
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new AssertionError(e);
        }
    }
}
