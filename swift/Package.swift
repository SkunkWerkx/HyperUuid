// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        // Bundles every platform's native build under NativeLibs/{rid}/{lib} (see the
        // Kotlin/Go bindings for the same reason) since SwiftPM's real binary-distribution
        // mechanism (a binaryTarget/XCFramework) is Apple-only and can't cover
        // win-x64/win-arm64/linux-x64/linux-arm64. NativePlatform.swift picks the right
        // resource path at compile time; DynamicLibrary.swift dlopen/dlsym's (or
        // LoadLibraryW/GetProcAddress's, on Windows) it at runtime.
        .target(
            name: "HyperUuid",
            resources: [.copy("NativeLibs")]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ]
)
