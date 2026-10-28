package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.DataInputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.jar.Attributes;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;
import java.util.jar.JarOutputStream;
import java.util.jar.Manifest;
import java.util.zip.ZipEntry;
import org.junit.jupiter.api.Test;

/**
 * Which path loaded, and the load-time properties of both: each runs on the floor JDK (JNI) and
 * on 25 (FFM), the expected backend and feature coming from the build.
 */
class BackendTest {
    private static String expectBackend() {
        return System.getProperty("hyperuuid.expectBackend");
    }

    private static Path jar() throws Exception {
        return Path.of(UuidGenerator.class
                .getProtectionDomain()
                .getCodeSource()
                .getLocation()
                .toURI());
    }

    private static Path testClasses() throws Exception {
        return Path.of(BackendTest.class
                .getProtectionDomain()
                .getCodeSource()
                .getLocation()
                .toURI());
    }

    private static String java() {
        return ProcessHandle.current().info().command().orElse("java");
    }

    /** Runs a child JVM to completion, output merged; fails the test on a hang. */
    private static String run(int expectedExit, List<String> command) throws Exception {
        var log = Files.createTempFile("hyperuuid-backend", ".log");
        try {
            var process = new ProcessBuilder(command)
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            if (!process.waitFor(120, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("child JVM did not finish in 120 s: " + command);
            }
            var output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(expectedExit, process.exitValue(), output);
            return output;
        } finally {
            Files.deleteIfExists(log);
        }
    }

    @Test
    void theExpectedPathLoaded() throws Exception {
        assertTrue(UuidGenerator.isAvailable());
        assertEquals(expectBackend(), NativeCore.backend());
        assertEquals(
                Integer.parseInt(System.getProperty("hyperuuid.expectFeature")),
                Runtime.version().feature());
        long natives = Arrays.stream(NativeCore.class.getDeclaredMethods())
                .filter(m -> Modifier.isNative(m.getModifiers()))
                .count();
        assertEquals("jni".equals(expectBackend()) ? 14 : 0, natives);
        int major;
        try (InputStream in = NativeCore.class.getResourceAsStream("NativeCore.class")) {
            var data = new DataInputStream(in);
            data.readInt(); // 0xCAFEBABE
            data.readUnsignedShort(); // minor
            major = data.readUnsignedShort();
        }
        assertEquals("jni".equals(expectBackend()) ? 65 : 69, major);
    }

    // The attribute stripped, as a shading plugin does: the JDK then reads only the base, so
    // 25 runs the JNI path too. Correct, slower, and its warning names System::load.
    @Test
    void aStrippedJarRunsJniCorrectlyOn25() throws Exception {
        var dir = Files.createTempDirectory("hyperuuid-stripped");
        var stripped = dir.resolve("stripped.jar");
        try {
            try (var in = new JarFile(jar().toFile());
                    OutputStream file = Files.newOutputStream(stripped);
                    var out = new JarOutputStream(file)) {
                var manifest = new Manifest(in.getManifest());
                manifest.getMainAttributes().remove(new Attributes.Name("Multi-Release"));
                out.putNextEntry(new ZipEntry(JarFile.MANIFEST_NAME));
                manifest.write(out);
                out.closeEntry();
                for (var entries = in.entries(); entries.hasMoreElements(); ) {
                    JarEntry entry = entries.nextElement();
                    if (entry.getName().equals(JarFile.MANIFEST_NAME)) {
                        continue;
                    }
                    out.putNextEntry(new ZipEntry(entry.getName()));
                    if (!entry.isDirectory()) {
                        try (InputStream body = in.getInputStream(entry)) {
                            body.transferTo(out);
                        }
                    }
                    out.closeEntry();
                }
            }
            // No --enable-native-access: on 24+ the warning is what the README promises.
            var output = run(
                    0,
                    List.of(
                            java(),
                            "-cp",
                            stripped + java.io.File.pathSeparator + testClasses(),
                            StrippedMain.class.getName()));
            assertTrue(output.contains("backend: jni"), output);
            assertTrue(output.contains("v5: 2ed6657d-e927-568b-95e1-2665a8aea6a2"), output);
            assertTrue(output.contains("batch: 4097"), output);
            if (Runtime.version().feature() >= 24) {
                assertTrue(output.contains("java.lang.System::load"), output);
            }
        } finally {
            Files.deleteIfExists(stripped);
            Files.deleteIfExists(dir);
        }
    }

    static final class StrippedMain {
        public static void main(String[] args) {
            System.out.println("backend: " + NativeCore.backend());
            System.out.println("v5: " + UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "www.example.com"));
            System.out.println("batch: " + UuidGenerator.newV7Batch(4097).length);
        }
    }

    // JNI refuses one library in two class loaders, so each loader extracts its own copy.
    @Test
    void twoClassLoadersEachLoadTheCore() throws Exception {
        URL url = jar().toUri().toURL();
        try (var a = new URLClassLoader(new URL[] {url}, ClassLoader.getPlatformClassLoader());
                var b = new URLClassLoader(new URL[] {url}, ClassLoader.getPlatformClassLoader())) {
            var versions = new ArrayList<String>();
            for (ClassLoader loader : List.of(a, b)) {
                Class<?> generator = loader.loadClass(UuidGenerator.class.getName());
                assertFalse(generator == UuidGenerator.class);
                Method available = generator.getMethod("isAvailable");
                Method version = generator.getMethod("nativeVersion");
                assertEquals(true, available.invoke(null));
                versions.add((String) version.invoke(null));
            }
            assertEquals(versions.get(0), versions.get(1));
        }
    }

    @Test
    void theModulePathLoadsTheExpectedBackend() throws Exception {
        var output = run(
                0,
                List.of(
                        java(),
                        "--module-path",
                        jar().toString(),
                        "--add-modules",
                        "io.github.skunkwerkx.hyperuuid",
                        "--enable-native-access=io.github.skunkwerkx.hyperuuid",
                        "-cp",
                        testClasses().toString(),
                        "io.github.skunkwerkx.hyperuuid.probe.ModuleMain"));
        assertTrue(output.contains("backend: " + expectBackend()), output);
        assertTrue(output.contains("module: io.github.skunkwerkx.hyperuuid"), output);
        assertFalse(output.contains("WARNING"), output);
    }

    @Test
    void aRacingFirstTouchLoadsOnce() throws Exception {
        var tmp = Files.createTempDirectory("hyperuuid-race");
        try {
            var output = run(
                    0,
                    List.of(
                            java(),
                            "--enable-native-access=ALL-UNNAMED",
                            "-Djava.io.tmpdir=" + tmp,
                            "-cp",
                            System.getProperty("java.class.path"),
                            RaceMain.class.getName()));
            assertTrue(output.contains("all 32 doors succeeded"), output);
            assertTrue(output.contains("extracted: 1"), output);
        } finally {
            try (var left = Files.list(tmp)) {
                for (Path p : (Iterable<Path>) left::iterator) {
                    Files.deleteIfExists(p);
                }
            }
            Files.deleteIfExists(tmp);
        }
    }

    /** 32 threads, one latch, a different door first on each; then counts the extracted libraries. */
    static final class RaceMain {
        public static void main(String[] args) throws Exception {
            var gate = new CountDownLatch(1);
            var failures = new java.util.concurrent.ConcurrentLinkedQueue<Throwable>();
            var uuid = java.util.UUID.fromString("0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b");
            List<Runnable> doors = List.of(
                    UuidGenerator::newV4,
                    () -> UuidGenerator.newV5(UuidGenerator.Namespaces.DNS, "race"),
                    UuidGenerator::newV7,
                    () -> UuidGenerator.newV7(System.currentTimeMillis()),
                    () -> UuidGenerator.newV7Batch(3),
                    () -> UuidGenerator.fillV7(new java.util.UUID[2]),
                    () -> UuidGenerator.fillV7(new byte[32]),
                    UuidGenerator::nativeVersion,
                    UuidGenerator::isAvailable,
                    () -> UuidGenerator.v7UnixMillis(uuid),
                    () -> UuidGenerator.version(uuid),
                    () -> UuidGenerator.variant(uuid),
                    () -> UuidGenerator.v7ToSqlOrder(uuid),
                    () -> UuidGenerator.getTimestamp(uuid),
                    () -> UuidGenerator.isRfc(uuid, 7),
                    () -> UuidGenerator.version(new byte[16]));
            var threads = new ArrayList<Thread>();
            for (int i = 0; i < 32; i++) {
                Runnable door = doors.get(i % doors.size());
                Thread t = new Thread(() -> {
                    try {
                        gate.await();
                        door.run();
                    } catch (Throwable e) {
                        failures.add(e);
                    }
                });
                threads.add(t);
                t.start();
            }
            gate.countDown();
            for (Thread t : threads) {
                t.join();
            }
            if (!failures.isEmpty()) {
                failures.peek().printStackTrace();
                System.exit(1);
            }
            long extracted;
            try (var files = Files.list(Path.of(System.getProperty("java.io.tmpdir")))) {
                extracted = files.filter(p -> p.getFileName().toString().startsWith("hyperuuid"))
                        .count();
            }
            System.out.println("all 32 doors succeeded");
            System.out.println("extracted: " + extracted);
        }
    }
}
