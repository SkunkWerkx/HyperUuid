package io.github.skunkwerkx.hyperuuid;

/**
 * Maps the running JVM's OS/arch to the RID-style directory (matching the C# wrapper's
 * {@code runtimes/{RID}/native/} convention) and filename the native library was built for.
 *
 * <p>A {@code .jar} has no package-manager-level platform selection the way NuGet's RID
 * folders or Python wheel tags do — one jar has to work everywhere, so it bundles every
 * platform's build under {@code /native/{rid}/{filename}} and this picks the right one at
 * runtime instead.
 */
final class NativePlatform {
    record Target(String rid, String libraryFileName) {}

    // Resolved once on first access to this class, mirroring the Java binding's `by lazy`
    // — a plain static field initializer already only runs on class-init, which happens on
    // first use.
    private static final Target CURRENT = detect();

    static Target current() {
        return CURRENT;
    }

    /** Classpath resource path for this platform's bundled native library. */
    static String resourcePath() {
        return "/native/" + CURRENT.rid() + "/" + CURRENT.libraryFileName();
    }

    private static Target detect() {
        String osName = System.getProperty("os.name").toLowerCase();
        String osArch = System.getProperty("os.arch").toLowerCase();
        boolean isArm = osArch.contains("aarch64") || osArch.contains("arm");

        if (osName.contains("win")) {
            return new Target(isArm ? "win-arm64" : "win-x64", "hyperuuid.dll");
        }
        if (osName.contains("mac") || osName.contains("darwin")) {
            return new Target(isArm ? "osx-arm64" : "osx-x64", "libhyperuuid.dylib");
        }
        if (osName.contains("linux")) {
            return new Target(isArm ? "linux-arm64" : "linux-x64", "libhyperuuid.so");
        }
        throw new IllegalStateException(
                "Unsupported platform for hyperuuid: os.name=" + osName + " os.arch=" + osArch);
    }

    private NativePlatform() {}
}
