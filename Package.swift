// swift-tools-version:5.9
import PackageDescription

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperUuid", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift unchanged (working-directory: swift);
// this one just points its targets' `path:` at the real sources instead of duplicating them.
let package = Package(
    name: "HyperUuid",
    products: [
        .library(name: "HyperUuid", targets: ["HyperUuid"])
    ],
    targets: [
        .target(
            name: "HyperUuid",
            path: "swift/Sources/HyperUuid",
            resources: [.copy("NativeLibs")]
        ),
        .testTarget(
            name: "HyperUuidTests",
            dependencies: ["HyperUuid"],
            path: "swift/Tests/HyperUuidTests"
        ),
    ]
)
