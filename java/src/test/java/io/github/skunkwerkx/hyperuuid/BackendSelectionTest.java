package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.lang.reflect.InvocationTargetException;
import java.net.URI;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Pins that the property really is what picks the path: the plain {@code test} task sets
 * nothing and must land on FFM (this build always stages the platform's native library first),
 * while {@code testWasm} sets {@code hyperuuid.backend=wasm} and must land on GraalWasm. The
 * rest of the suite runs identically under both; this is the one assertion that would catch
 * the switch silently doing nothing.
 *
 * <p>The suite always has GraalWasm and a loadable library, so the two selections that
 * depend on one of them being missing are run against a second copy of the binding in a
 * class loader of its own (see {@link #bindingAlone}).
 */
class BackendSelectionTest {

    @Test
    void backendFollowsTheProperty() {
        String expected = System.getProperty(UuidGenerator.BACKEND_PROPERTY, "native");
        assertEquals(expected, UuidGenerator.backend());
    }

    @Test
    void selectingWasmWithoutGraalWasmNamesTheTwoArtifacts() throws Exception {
        String previous = setBackend("wasm");
        try (URLClassLoader loader = bindingAlone()) {
            Class<?> generator = Class.forName(UuidGenerator.class.getName(), true, loader);
            Throwable thrown = failureOf(generator, "backend");
            // The message a consumer has to act on, not a bare NoClassDefFoundError and not
            // "could not start the wasm backend".
            IllegalStateException failure = assertInstanceOf(IllegalStateException.class,
                    assertInstanceOf(ExceptionInInitializerError.class, thrown).getCause());
            assertTrue(failure.getMessage().contains("org.graalvm.polyglot:polyglot"), failure.getMessage());
            assertTrue(failure.getMessage().contains("org.graalvm.polyglot:wasm"), failure.getMessage());
            assertInstanceOf(NoClassDefFoundError.class, failure.getCause());
            // And the probe says so without throwing, which is what it is for — while the
            // constants, which never needed the core, still answer.
            assertEquals(false, generator.getMethod("isAvailable").invoke(null));
            assertEquals(UuidGenerator.NIL, generator.getField("NIL").get(null));
        } finally {
            setBackend(previous);
        }
    }

    @Test
    void aBundledLibraryThatWillNotLoadFallsBackToTheWasmModule(@TempDir Path dir) throws Exception {
        NativePlatform.Target target = NativePlatform.current();
        assumeTrue(target != null, "no native build for this platform, so nothing to fail to load");
        Path library = dir.resolve(target.resourcePath().substring(1));
        Files.createDirectories(library.getParent());
        Files.write(library, unloadableLibrary());

        String previous = setBackend(null);
        try (URLClassLoader loader = bindingAlone(dir.toUri().toURL())) {
            Class<?> generator = Class.forName(UuidGenerator.class.getName(), true, loader);
            Throwable thrown = failureOf(generator, "backend");
            // With no GraalWasm behind this copy the fallback cannot start either, so the
            // failure thrown is the native one — and the wasm attempt it made rides along
            // suppressed, saying why wasm was tried at all.
            Throwable nativeFailure = assertInstanceOf(ExceptionInInitializerError.class, thrown).getCause();
            assertEquals(1, nativeFailure.getSuppressed().length, nativeFailure.toString());
            String wasmFailure = nativeFailure.getSuppressed()[0].getMessage();
            assertTrue(wasmFailure.contains("org.graalvm.polyglot:wasm"), wasmFailure);
            assertTrue(wasmFailure.contains("would not load"), wasmFailure);
        } finally {
            setBackend(previous);
        }
    }

    /**
     * The binding's own classes and resources in a class loader with nothing but the JDK
     * behind it — the consumer who added this jar and not GraalWasm. {@code ahead} goes in
     * front of the real resources, so a test can put its own {@code native/{rid}/} there.
     */
    private static URLClassLoader bindingAlone(URL... ahead) throws Exception {
        URL module = UuidGenerator.class.getResource(WasmBackend.RESOURCE_PATH);
        assumeTrue(module != null, "no wasm module staged, so there is no wasm path to select");
        String external = module.toExternalForm();
        URL resources = URI.create(
                external.substring(0, external.length() - WasmBackend.RESOURCE_PATH.length() + 1)).toURL();
        URL classes = UuidGenerator.class.getProtectionDomain().getCodeSource().getLocation();

        URL[] path = new URL[ahead.length + 2];
        System.arraycopy(ahead, 0, path, 0, ahead.length);
        path[ahead.length] = classes;
        path[ahead.length + 1] = resources;
        return new URLClassLoader(path, ClassLoader.getPlatformClassLoader());
    }

    /**
     * A file every dynamic loader refuses up front: an ELF header naming no machine, and one
     * program header saying the stack stays non-executable. Deliberately not arbitrary bytes
     * and not a cut-short copy of the real library. HotSpot reads those two headers before
     * handing a path to {@code dlopen} and, finding none, warns that the library "might have
     * disabled stack guard" — noise that would read as a defect in the real build. And glibc
     * maps a truncated library first and faults on it after, which takes the whole JVM down
     * with SIGBUS instead of failing the load.
     */
    private static byte[] unloadableLibrary() {
        ByteBuffer elf = ByteBuffer.allocate(64 + 56).order(ByteOrder.LITTLE_ENDIAN);
        elf.put(new byte[] {0x7f, 'E', 'L', 'F', 2, 1, 1, 0}).position(16);
        elf.putShort((short) 3)          // e_type: ET_DYN
                .putShort((short) 0)     // e_machine: EM_NONE, which no host matches
                .putInt(1)               // e_version
                .putLong(0)              // e_entry
                .putLong(64)             // e_phoff: the program header follows this one
                .putLong(0)              // e_shoff
                .putInt(0)               // e_flags
                .putShort((short) 64)    // e_ehsize
                .putShort((short) 56)    // e_phentsize
                .putShort((short) 1)     // e_phnum
                .putShort((short) 0)     // e_shentsize
                .putShort((short) 0)     // e_shnum
                .putShort((short) 0);    // e_shstrndx
        elf.putInt(0x6474e551)           // p_type: PT_GNU_STACK
                .putInt(6);              // p_flags: read + write, not execute
        return elf.array();
    }

    /** What a no-argument static method of the isolated copy threw, unwrapped from reflection. */
    private static Throwable failureOf(Class<?> binding, String method) throws Exception {
        try {
            Object result = binding.getMethod(method).invoke(null);
            throw new AssertionError(method + "() returned " + result + " instead of failing");
        } catch (InvocationTargetException e) {
            return e.getCause();
        }
    }

    // The property is process-wide and read once per copy of the binding, at that copy's
    // core load; JUnit runs this class on one thread, and each test puts back what it found.
    private static String setBackend(String value) {
        String previous = System.getProperty(UuidGenerator.BACKEND_PROPERTY);
        if (value == null) {
            System.clearProperty(UuidGenerator.BACKEND_PROPERTY);
        } else {
            System.setProperty(UuidGenerator.BACKEND_PROPERTY, value);
        }
        return previous;
    }
}
