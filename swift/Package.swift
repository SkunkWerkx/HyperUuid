// swift-tools-version:6.2
import PackageDescription

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        // The native core as static libraries, one per triple (SE-0482, which is what sets
        // the tools version above): glibc and musl Linux on x86_64 and arm64, and WASI.
        // Where SwiftPM finds a variant for the triple being built, the core is linked into
        // the consumer's executable and nothing has to ship beside it — the only way to
        // reach the static Linux SDK and WebAssembly at all, neither of which can open a
        // shared library.
        .binaryTarget(
            name: "HyperUuidCore",
            path: "HyperUuidCore.artifactbundle"
        ),
        // macOS and Windows load a shared library instead, out of a resource-only target of
        // its own: NativeLibs/{rid}/{lib} under HyperUuidNativeLibs. Resources take no
        // platform condition, so in this target they would be staged beside every Linux
        // executable and carried into every WebAssembly build, loaded by neither; a target
        // dependency does take one, and a target that isn't built stages nothing. The two
        // conditions are disjoint, so each platform gets exactly one way to the core (and
        // the core's keeps SwiftPM from warning, on every macOS or Windows build, that the
        // bundle above has no variant for the triple).
        // NativePlatform.swift picks the library at compile time; DynamicLibrary.swift
        // dlopen/dlsym's (or LoadLibraryW/GetProcAddress's, on Windows) it at run time.
        .target(
            name: "HyperUuidNativeLibs",
            resources: [.copy("NativeLibs")]
        ),
        .target(
            name: "HyperUuid",
            dependencies: [
                .target(name: "HyperUuidCore", condition: .when(platforms: [.linux, .wasi])),
                .target(name: "HyperUuidNativeLibs", condition: .when(platforms: [.macOS, .windows])),
            ]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ]
)
