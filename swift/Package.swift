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
        // macOS and Windows load a shared library instead, bundled under
        // NativeLibs/{rid}/{lib} as a resource: the bundle above has no variant for them,
        // and the platform condition keeps SwiftPM from warning about that on every build.
        // NativePlatform.swift picks the resource at compile time; DynamicLibrary.swift
        // dlopen/dlsym's (or LoadLibraryW/GetProcAddress's, on Windows) it at run time.
        .target(
            name: "HyperUuid",
            dependencies: [
                .target(name: "HyperUuidCore", condition: .when(platforms: [.linux, .wasi]))
            ],
            resources: [.copy("NativeLibs")]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ]
)
