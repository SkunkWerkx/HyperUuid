/// Maps this build's compile-time OS/arch to the RID-style directory (matching the other
/// bindings' `runtimes/{rid}/native/` / `native/{rid}/` convention) and filename the native
/// library was built for.
///
/// Unlike the Kotlin/Go bindings — which each produce one artifact that must pick a native
/// build at *runtime* (a .jar or a Go binary can end up running on any platform) — a single
/// Swift build product is already single-arch/single-OS, compiled per target triple by the
/// toolchain itself. So this resolves at compile time via `#if os(...) && arch(...)` rather
/// than a `uname`-equivalent runtime check.
enum NativePlatform {
    #if os(Windows) && arch(arm64)
    static let rid = "win-arm64"
    static let libraryFileName = "hyperuuid.dll"
    #elseif os(Windows)
    static let rid = "win-x64"
    static let libraryFileName = "hyperuuid.dll"
    #elseif os(macOS) && arch(arm64)
    static let rid = "osx-arm64"
    static let libraryFileName = "libhyperuuid.dylib"
    #elseif os(macOS)
    static let rid = "osx-x64"
    static let libraryFileName = "libhyperuuid.dylib"
    #elseif os(Linux) && arch(arm64)
    static let rid = "linux-arm64"
    static let libraryFileName = "libhyperuuid.so"
    #elseif os(Linux)
    static let rid = "linux-x64"
    static let libraryFileName = "libhyperuuid.so"
    #else
    #error("hyperuuid: unsupported platform — no native build for this OS/arch combination")
    #endif
}
