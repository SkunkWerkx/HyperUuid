// swift-tools-version:6.2
import PackageDescription

let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        // The native core as static libraries, one per triple (SE-0482, which is what sets
        // the tools version above): glibc and musl Linux and macOS on x86_64 and arm64,
        // Windows (MSVC) on x86_64 and arm64, and WASI. SwiftPM picks the variant for the
        // triple being built and links it into the consumer's executable, so nothing ships
        // beside it and nothing is opened at run time. A triple with no variant has no
        // `HyperUuidCore` module, and the build stops there rather than at run time.
        .binaryTarget(
            name: "HyperUuidCore",
            path: "HyperUuidCore.artifactbundle"
        ),
        .target(
            name: "HyperUuid",
            dependencies: ["HyperUuidCore"]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"]
        ),
    ]
)
