package io.github.buvinghausen.hyperuuid

/**
 * Maps the running JVM's OS/arch to the RID-style directory (matching the C# wrapper's
 * `runtimes/{RID}/native/` convention) and filename the native library was built for.
 *
 * A `.jar` has no package-manager-level platform selection the way NuGet's RID folders or
 * Python wheel tags do — one jar has to work everywhere, so it bundles every platform's build
 * under `/native/{rid}/{filename}` and this picks the right one at runtime instead.
 */
internal object NativePlatform {
    data class Target(val rid: String, val libraryFileName: String)

    val current: Target by lazy { detect() }

    /** Classpath resource path for this platform's bundled native library. */
    val resourcePath: String get() = "/native/${current.rid}/${current.libraryFileName}"

    private fun detect(): Target {
        val osName = System.getProperty("os.name").lowercase()
        val osArch = System.getProperty("os.arch").lowercase()
        val isArm = osArch.contains("aarch64") || osArch.contains("arm")

        return when {
            osName.contains("win") -> Target(if (isArm) "win-arm64" else "win-x64", "hyperuuid.dll")
            osName.contains("mac") || osName.contains("darwin") ->
                Target(if (isArm) "osx-arm64" else "osx-x64", "libhyperuuid.dylib")
            osName.contains("linux") -> Target(if (isArm) "linux-arm64" else "linux-x64", "libhyperuuid.so")
            else -> error("Unsupported platform for hyperuuid: os.name=$osName os.arch=$osArch")
        }
    }
}
