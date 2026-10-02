// How this build reaches the native core is decided by the target, at compile time.
//
// Linux (glibc and musl) and WebAssembly link it in: Package.swift declares a binary
// static-library target, `HyperUuidCore`, with one archive per triple, and where SwiftPM
// finds an archive for the triple being built the C module is importable and
// `UuidGenerator` takes its function pointers straight from the linked symbols. Nothing is
// loaded at run time, so nothing has to ship beside the executable — which is also the only
// way to cover the two targets that have no dynamic loader at all, the static Linux SDK and
// WASI.
//
// macOS and Windows load a shared library instead, out of the `HyperUuidNativeLibs` target's
// resources; that is everything below. `canImport`, not an OS check, picks between the two,
// so a target with no archive of its own lands on the `#error` rather than on a loader with
// nothing to load.
#if !canImport(HyperUuidCore)

/// Maps this build's compile-time OS/arch to the RID-style directory (matching the other
/// bindings' `runtimes/{rid}/native/` / `native/{rid}/` convention) and filename of the
/// shared library a macOS or Windows build loads.
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
    #else
    #error("hyperuuid: unsupported platform — the Swift binding links the native core statically on Linux (glibc and musl) and WebAssembly (WASI), and loads a bundled shared library on macOS and Windows, each on x86_64 and arm64 where the platform has both; this target is none of those")
    #endif

    /// The directory SwiftPM stages the `HyperUuidNativeLibs` target's resources into:
    /// `{package}_{target}` plus a suffix that depends on the build system as well as the
    /// platform. Swift Build — SwiftPM's default from Swift 6.4 — writes a `.bundle`
    /// everywhere; the build system before it wrote a `.bundle` on macOS and a plain
    /// `.resources` directory everywhere else, and is still there behind
    /// `--build-system native`. A toolchain produces only one of the two, so on Windows both
    /// names are looked for. The generated `Bundle.module` accessor knows which, but it
    /// `fatalError`s when the directory is absent, so `DynamicLibrary.locateBundled()` looks
    /// for it by name instead.
    #if os(macOS)
    static let resourceBundleNames = ["HyperUuid_HyperUuidNativeLibs.bundle"]
    #else
    static let resourceBundleNames = [
        "HyperUuid_HyperUuidNativeLibs.bundle", "HyperUuid_HyperUuidNativeLibs.resources",
    ]
    #endif
}

#endif
