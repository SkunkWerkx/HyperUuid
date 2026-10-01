/// Maps this build's compile-time OS/arch to the RID-style directory (matching the other
/// bindings' `runtimes/{rid}/native/` / `native/{rid}/` convention) and filename the native
/// library was built for.
///
/// Unlike the Go/Java bindings — which each produce one artifact that must pick a native
/// build at *runtime* (a .jar or a Go binary can end up running on any platform) — a single
/// Swift build product is already single-arch/single-OS, compiled per target triple by the
/// toolchain itself. So this resolves at compile time via `#if os(...) && arch(...)` rather
/// than a `uname`-equivalent runtime check.
///
/// Every branch names its architecture: a target this package has no native build for stops
/// at the `#error` below instead of silently being handed the x64 library and failing in
/// `dlopen` at run time.
enum NativePlatform {
    #if os(Windows) && arch(arm64)
    static let rid = "win-arm64"
    static let libraryFileName = "hyperuuid.dll"
    #elseif os(Windows) && arch(x86_64)
    static let rid = "win-x64"
    static let libraryFileName = "hyperuuid.dll"
    #elseif os(macOS) && arch(arm64)
    static let rid = "osx-arm64"
    static let libraryFileName = "libhyperuuid.dylib"
    #elseif os(macOS) && arch(x86_64)
    static let rid = "osx-x64"
    static let libraryFileName = "libhyperuuid.dylib"
    #elseif os(Linux) && canImport(Musl)
    // The other bindings ship linux-musl-x64/linux-musl-arm64 builds; this one deliberately
    // doesn't. Swift's musl target is the fully static Linux SDK, and a statically linked
    // executable has no dynamic loader to `dlopen` a shared library with — there is nothing
    // a bundled musl `.so` could be loaded by.
    //
    // Deferred, not impossible. The way to cover musl is to link the core statically: a
    // per-triple `.a` in an artifact bundle, declared as a SwiftPM binary static-library
    // target (SE-0482, Swift 6.2+), with this binding taking its function pointers from the
    // linked symbols instead of from `dlsym`. That is a second packaging mechanism and a
    // second code path, and it is not built yet.
    #error("hyperuuid: musl Linux is not supported by the Swift binding yet — Swift's musl target links fully statically and cannot dlopen the native library, and the statically linked build that would cover it (SE-0482, Swift 6.2+) is not built; build against glibc Linux instead")
    #elseif os(Linux) && arch(arm64)
    static let rid = "linux-arm64"
    static let libraryFileName = "libhyperuuid.so"
    #elseif os(Linux) && arch(x86_64)
    static let rid = "linux-x64"
    static let libraryFileName = "libhyperuuid.so"
    #else
    #error("hyperuuid: unsupported platform — the Swift binding bundles native builds for glibc Linux, macOS and Windows on x86_64 and arm64 only")
    #endif

    /// The directory SwiftPM stages this target's resources into, `{package}_{target}` with
    /// the platform's own suffix: a `.bundle` on macOS, a plain `.resources` directory
    /// everywhere else. The generated `Bundle.module` accessor knows the same name, but it
    /// `fatalError`s when the directory is absent, so `DynamicLibrary.locateBundled()` looks
    /// for it by name instead.
    #if os(macOS)
    static let resourceBundleName = "HyperUuid_HyperUuid.bundle"
    #else
    static let resourceBundleName = "HyperUuid_HyperUuid.resources"
    #endif
}
