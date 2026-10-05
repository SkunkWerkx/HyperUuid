package io.github.skunkwerkx.hyperuuid;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.stream.Stream;
import org.junit.jupiter.api.Test;

/**
 * Platform resolution as a pure function of what the JVM reports, so every platform's
 * answer is pinned from whichever one the suite happens to run on — including the ones
 * whose answer is "no native build", which is what routes them to the wasm module.
 */
class NativePlatformTest {

    private static String rid(String osName, String osArch, boolean musl) {
        return NativePlatform.resolve(osName, osArch, musl).rid();
    }

    @Test
    void everyShippedBuildResolvesToItsOwnRid() {
        assertEquals("linux-x64", rid("Linux", "amd64", false));
        assertEquals("linux-arm64", rid("Linux", "aarch64", false));
        assertEquals("linux-musl-x64", rid("Linux", "amd64", true));
        assertEquals("linux-musl-arm64", rid("Linux", "aarch64", true));
        assertEquals("osx-x64", rid("Mac OS X", "x86_64", false));
        assertEquals("osx-arm64", rid("Mac OS X", "aarch64", false));
        assertEquals("win-x64", rid("Windows 11", "amd64", false));
        assertEquals("win-arm64", rid("Windows 11", "aarch64", false));
    }

    @Test
    void theLibraryIsNamedTheWayEachOsNamesOne() {
        assertEquals(
                "/native/linux-musl-x64/libhyperuuid.so",
                NativePlatform.resolve("Linux", "amd64", true).resourcePath());
        assertEquals(
                "/native/osx-arm64/libhyperuuid.dylib",
                NativePlatform.resolve("Mac OS X", "aarch64", false).resourcePath());
        assertEquals(
                "/native/win-x64/hyperuuid.dll",
                NativePlatform.resolve("Windows Server 2025", "amd64", false).resourcePath());
    }

    @Test
    void muslOnlyMattersOnLinux() {
        assertEquals("osx-arm64", rid("Mac OS X", "aarch64", true));
        assertEquals("win-x64", rid("Windows 11", "amd64", true));
    }

    @Test
    void anArchitectureWithNoBuildResolvesToNothingRatherThanTheNearestRid() {
        // Each of these used to come back as *-x64 or *-arm64, and then fail at load.
        for (String arch : new String[] {"riscv64", "ppc64le", "s390x", "loongarch64", "x86", "i386", "arm"}) {
            assertNull(NativePlatform.resolve("Linux", arch, false), arch);
        }
    }

    @Test
    void anOsWithNoBuildResolvesToNothing() {
        for (String os : new String[] {"FreeBSD", "AIX", "SunOS", "OpenBSD"}) {
            assertNull(NativePlatform.resolve(os, "amd64", false), os);
        }
    }

    @Test
    void muslIsReadOffTheProcessMap() {
        assertTrue(NativePlatform.mentionsMusl(Stream.of(
                "55d0c8a00000-55d0c8a01000 r--p 00000000 00:2f 1054 /opt/java/openjdk/bin/java",
                "7f2a1c000000-7f2a1c014000 r--p 00000000 00:2f 211 /lib/ld-musl-x86_64.so.1")));
        assertTrue(NativePlatform.mentionsMusl(
                Stream.of("ffff8a000000-ffff8a0a4000 r-xp 00000000 fe:01 77 /lib/libc.musl-aarch64.so.1")));
        assertFalse(NativePlatform.mentionsMusl(Stream.of(
                "7f1c2a400000-7f1c2a428000 r--p 00000000 08:20 4411 /usr/lib/x86_64-linux-gnu/libc.so.6",
                "7f1c2a7c2000-7f1c2a7c3000 r--p 00000000 08:20 4399 /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2")));
        assertFalse(NativePlatform.mentionsMusl(Stream.empty()));
    }
}
