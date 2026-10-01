package io.github.skunkwerkx.hyperuuid;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.stream.Stream;

/**
 * Maps the running JVM's OS, architecture and — on Linux — C library to the RID-style
 * directory (matching the C# binding's {@code runtimes/{RID}/native/} convention) and
 * filename the native library was built for, or to nothing at all when this jar carries no
 * build for it.
 *
 * <p>A {@code .jar} has no package-manager-level platform selection the way NuGet's RID
 * folders or Python wheel tags do — one jar has to work everywhere, so it bundles every
 * platform's build under {@code /native/{rid}/{filename}} and this picks the right one at
 * runtime instead.
 *
 * <p>Nothing here rounds to the nearest RID. An architecture that is neither x64 nor arm64,
 * or an OS that is none of Linux, macOS and Windows, resolves to no target — which is what
 * sends {@link UuidGenerator} to the bundled wasm module — because the nearest-looking
 * library would only fail to load. Linux resolves to one of two families: glibc
 * ({@code linux-x64}) or musl ({@code linux-musl-x64}, Alpine's), told apart by the loader
 * this very process has mapped.
 */
final class NativePlatform {
    record Target(String rid, String libraryFileName) {
        /** Classpath resource path of this target's bundled native library. */
        String resourcePath() {
            return "/native/" + rid + "/" + libraryFileName;
        }
    }

    // Resolved once, on first access to this class.
    private static final Target CURRENT =
            resolve(System.getProperty("os.name"), System.getProperty("os.arch"), runsOnMusl());

    /** This platform's target, or {@code null} when the jar ships no native build for it. */
    static Target current() {
        return CURRENT;
    }

    /** The OS and architecture as the JVM names them, for a message about a missing build. */
    static String describe() {
        return "os.name=" + System.getProperty("os.name") + " os.arch=" + System.getProperty("os.arch");
    }

    /**
     * The target for an OS name and architecture as {@code os.name}/{@code os.arch} spell
     * them, or {@code null} for a combination no native build exists for. {@code musl} only
     * matters on Linux.
     */
    static Target resolve(String osName, String osArch, boolean musl) {
        String arch = switch (osArch.toLowerCase(Locale.ROOT)) {
            case "amd64", "x86_64", "x64" -> "x64";
            case "aarch64", "arm64" -> "arm64";
            // riscv64, ppc64le, s390x, 32-bit x86 and arm, and whatever comes next.
            default -> null;
        };
        if (arch == null) {
            return null;
        }
        String os = osName.toLowerCase(Locale.ROOT);
        if (os.startsWith("windows")) {
            return new Target("win-" + arch, "hyperuuid.dll");
        }
        if (os.startsWith("mac") || os.startsWith("darwin")) {
            return new Target("osx-" + arch, "libhyperuuid.dylib");
        }
        if (os.startsWith("linux")) {
            return new Target((musl ? "linux-musl-" : "linux-") + arch, "libhyperuuid.so");
        }
        return null;
    }

    // The same rule every binding in this repo applies: the process is a musl one when its
    // own memory map names musl's loader. A map that cannot be read — no /proc, or not
    // Linux at all — means glibc, the common case and the build that has always shipped.
    // Latin-1 because the map is paths, not text: it decodes any byte, so an oddly named
    // mapping cannot turn a readable map into an unreadable one.
    private static boolean runsOnMusl() {
        try (Stream<String> maps = Files.lines(Path.of("/proc/self/maps"), StandardCharsets.ISO_8859_1)) {
            return mentionsMusl(maps);
        } catch (IOException | RuntimeException unreadable) {
            return false;
        }
    }

    /** Whether any line of a {@code /proc/{pid}/maps} listing names musl's loader or libc. */
    static boolean mentionsMusl(Stream<String> maps) {
        return maps.anyMatch(line -> line.contains("ld-musl-") || line.contains("libc.musl-"));
    }

    private NativePlatform() {}
}
